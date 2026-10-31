use super::*;
use crate::matrix::protocol::*;
use golden_video::plan::{BlendMode, LayerSource, RenderOperation};

fn matrix_edit(engine: &mut AppEngine, media: MediaId, operation: MatrixEdit) -> MediaEditResult {
    edit(engine, MediaEdit::Matrix { media, edit: operation })
}
fn inspect(engine: &mut AppEngine, media: MediaId) -> MatrixSnapshot {
    let MediaEditResult::Matrix { snapshot } = matrix_edit(engine, media, MatrixEdit::Inspect) else {
        panic!()
    };
    snapshot
}
fn prepare(engine: &AppEngine, roots: &[ChannelReference]) -> super::super::plan::Prepared {
    let owner = renderer();
    let routes = roots
        .iter()
        .enumerate()
        .map(|(i, channel)| ChannelRoute {
            output: OutputId::new(i as u64 + 1).unwrap(),
            channel: *channel,
        })
        .collect::<Vec<_>>();
    let result = super::super::plan::build(
        &capture_channels(engine, roots, 16).unwrap(),
        super::super::plan::Targets {
            routes: &routes,
            preview: None,
            screens: &[],
        },
        super::super::plan::Previous {
            channels: &Default::default(),
            screens: &Default::default(),
        },
        &mut 3,
        config().plan_limits,
        &owner.source_observations(),
        None,
    )
    .unwrap();
    close(owner);
    result
}

#[test]
fn selected_column_shares_layer_group_final_passes_and_group_opacity_is_post_composite() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, source, 4);
    let matrix = create(&mut engine, MediaKind::Matrix);
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::Configure {
            definition: MatrixDefinition {
                canvas: Extent2D::new(4, 2).unwrap(),
                ..Default::default()
            },
        },
    );
    let MediaEditResult::MatrixCreated { items, .. } = matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let group = items[0];
    let MatrixItemId::Group { id: group_id } = group else {
        panic!()
    };
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddLayers {
            parent: Some(group_id),
            count: 2,
        },
    );
    matrix_edit(&mut engine, matrix.media, MatrixEdit::AddColumns { count: 2 });
    let authored = inspect(&mut engine, matrix.media);
    let layers = authored
        .items
        .iter()
        .filter_map(|item| {
            if let MatrixItemId::Layer { id } = item.id {
                Some((id, item.channel))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let group_channel = authored.items.iter().find(|item| item.id == group).unwrap().channel;
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureItem {
            item: group,
            settings: CompositeSettings {
                opacity: 0.5,
                blend: BlendMode::Add,
                enabled: true,
            },
        },
    );
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: layers
                .iter()
                .map(|(layer, _)| CellAssignment {
                    layer: *layer,
                    column: authored.columns[0].id,
                    source: Some(source),
                })
                .collect(),
        },
    );
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: authored.columns[0].id,
            mode: LaunchMode::Cut,
        },
    );
    let roots = [matrix, group_channel, layers[0].1];
    let capture = capture_channels(&engine, &roots, 16).unwrap();
    assert_eq!(capture.channels.len(), 5);
    let root_id = engine.node_id_by_uuid(matrix.channel.0).unwrap();
    let crate::app::AppNode::MediaChannel(root) = engine.nodes.get(root_id).unwrap() else {
        panic!()
    };
    let index = root.matrix_playback().unwrap().cached().unwrap();
    let prepared = prepare(&engine, &roots);
    assert_eq!(prepared.submission.plan.passes.len(), 6); // five shared channels + group scratch
    let owner = renderer();
    let mut limits = config().plan_limits;
    limits.max_passes = 5;
    let bounded = super::super::plan::build(
        &capture,
        super::super::plan::Targets {
            routes: &[
                ChannelRoute {
                    output: OutputId::new(1).unwrap(),
                    channel: matrix,
                },
                ChannelRoute {
                    output: OutputId::new(2).unwrap(),
                    channel: source,
                },
            ],
            preview: None,
            screens: &[],
        },
        super::super::plan::Previous {
            channels: &Default::default(),
            screens: &Default::default(),
        },
        &mut 3,
        limits,
        &owner.source_observations(),
        None,
    )
    .unwrap();
    assert!(bounded.submission.plan.passes.len() <= 5);
    assert_eq!(
        bounded
            .channels
            .iter()
            .find(|channel| channel.reference == group_channel)
            .unwrap()
            .resolution
            .availability,
        crate::media::resolution::ChannelAvailability::Unavailable
    );
    assert_eq!(
        bounded
            .channels
            .iter()
            .find(|channel| channel.reference == source)
            .unwrap()
            .resolution
            .availability,
        crate::media::resolution::ChannelAvailability::Ready
    );
    close(owner);
    let scratch = prepared.entries[&group_channel].scratch.unwrap();
    let group_pass = prepared
        .submission
        .plan
        .passes
        .iter()
        .find(|pass| pass.output == prepared.entries[&group_channel].target)
        .unwrap();
    let RenderOperation::Composite { layers: group_layers } = &group_pass.operation else {
        panic!()
    };
    assert_eq!(group_layers.len(), 1);
    assert_eq!(group_layers[0].source, LayerSource::Texture(scratch));
    assert_eq!(group_layers[0].opacity, golden_video::Opacity::new(0.5).unwrap());
    let scratch_pass = prepared
        .submission
        .plan
        .passes
        .iter()
        .find(|pass| pass.output == scratch)
        .unwrap();
    let RenderOperation::Composite { layers: scratch_layers } = &scratch_pass.operation else {
        panic!()
    };
    assert_eq!(scratch_layers.len(), 2);
    assert!(
        scratch_layers
            .iter()
            .all(|layer| layer.opacity == golden_video::Opacity::OPAQUE)
    );
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: authored.columns[1].id,
            mode: LaunchMode::Cut,
        },
    );
    assert!(!capture.is_current());
    assert_eq!(capture_channels(&engine, &[matrix], 16).unwrap().channels.len(), 1);
    let crate::app::AppNode::MediaChannel(root) = engine.nodes.get(root_id).unwrap() else {
        panic!()
    };
    assert!(std::sync::Arc::ptr_eq(
        &index,
        &root.matrix_playback().unwrap().cached().unwrap()
    ));
    let empty = prepare(&engine, &[matrix]);
    let RenderOperation::Composite { layers } = &empty.submission.plan.passes[0].operation else {
        panic!()
    };
    assert!(layers.is_empty());
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: authored.columns[0].id,
            mode: LaunchMode::Cut,
        },
    );
    let before = capture_channels(&engine, &[matrix], 16).unwrap();
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureItem {
            item: group,
            settings: CompositeSettings {
                enabled: false,
                ..Default::default()
            },
        },
    );
    assert!(!before.is_current());
    assert_eq!(capture_channels(&engine, &[matrix], 16).unwrap().channels.len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(capture_channels(&engine, &[matrix], 16).unwrap().channels.len(), 5);
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RemoveColumn {
            column: authored.columns[0].id,
        },
    );
    assert_eq!(capture_channels(&engine, &[matrix], 16).unwrap().channels.len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(capture_channels(&engine, &[matrix], 16).unwrap().channels.len(), 5);
}
