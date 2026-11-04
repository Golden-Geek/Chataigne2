use super::matrix::{inspect, matrix_edit};
use super::*;
use crate::{app::AppNode, matrix::protocol::*};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};

fn discovered(engine: &mut AppEngine, media: MediaId) -> Vec<ChannelReference> {
    let MediaEditResult::Channels { page } = edit(
        engine,
        MediaEdit::InspectChannels {
            query: MediaChannelsQuery {
                media,
                start: 0,
                count: 128,
            },
        },
    ) else {
        panic!()
    };
    page.channels.into_iter().map(|channel| channel.reference).collect()
}

#[test]
fn opt_in_discovery_keeps_ids_and_reference_choices_through_history_and_projects() {
    let mut engine = engine();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let layer = layers(&mut engine, matrix.media, None, 1)[0];
    let snapshot = inspect(&mut engine, matrix.media);
    let item = snapshot.items.iter().find(|item| item.id == layer).unwrap().clone();
    assert!(!item.settings.expose_channel);
    let MediaEditResult::ReferenceCreated { media: alias } = edit(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(matrix.media),
            label: None,
        },
    ) else {
        panic!()
    };
    assert_eq!(discovered(&mut engine, matrix.media), [matrix]);
    assert_eq!(discovered(&mut engine, alias).len(), 1);
    let source = crate::media::MediaSource {
        media: matrix.media,
        channel: Some(item.channel.channel),
    };
    assert_eq!(
        source.output(&engine).unwrap(),
        Some(item.channel),
        "authored output identity is retained"
    );
    assert_eq!(source.capture(&engine, &mut vec![]).unwrap(), None);
    let hidden = crate::media::capture_channels(&engine, &[item.channel], 16).unwrap();
    assert_eq!(hidden.channels.len(), 1);
    assert!(hidden.channels[0].definition.is_none());
    let history = engine.undo_len();
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureItem {
            item: layer,
            settings: CompositeSettings {
                expose_channel: true,
                ..item.settings.clone()
            },
        },
    );
    assert!(
        !hidden.is_current(),
        "visibility changes invalidate a hidden-only demand"
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(discovered(&mut engine, matrix.media).contains(&item.channel));
    assert_eq!(source.output(&engine).unwrap(), Some(item.channel));
    let alias_channel = ChannelReference {
        media: alias,
        channel: item.channel.channel,
    };
    assert!(discovered(&mut engine, alias).contains(&alias_channel));
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&saved).unwrap();
    restored.apply_edits().unwrap();
    assert_eq!(inspect(&mut restored, matrix.media), inspect(&mut engine, matrix.media));
    assert!(engine.undo().unwrap());
    assert_eq!(discovered(&mut engine, matrix.media), [matrix]);
    assert_eq!(source.capture(&engine, &mut vec![]).unwrap(), None);
    assert!(
        engine.node_id_by_uuid(item.channel.channel.0).is_some(),
        "hidden exports keep their authored identity"
    );
    assert!(engine.redo().unwrap());
    assert!(discovered(&mut engine, alias).contains(&alias_channel));
}

#[test]
fn groups_and_layers_remain_internal_composite_owners_when_outputs_are_hidden() {
    let mut engine = engine();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let source = create(&mut engine, MediaKind::SolidColor);
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
    let MatrixItemId::Group { id } = group else { panic!() };
    let layer = layers(&mut engine, matrix.media, Some(id), 1)[0];
    let column = columns(&mut engine, matrix.media, 1)[0];
    let MatrixItemId::Layer { id: layer_id } = layer else {
        panic!()
    };
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer_id,
                column,
                source: Some(source),
            }],
        },
    );
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    let snapshot = inspect(&mut engine, matrix.media);
    let layer_output = snapshot.items.iter().find(|item| item.id == layer).unwrap().channel;
    let captured = crate::media::capture_channels(&engine, &[matrix, layer_output], 32).unwrap();
    assert!(
        captured.is_current(),
        "committed launch settlement precedes visibility guards"
    );
    for item in snapshot.items {
        assert!(!item.settings.expose_channel);
        let internal = captured
            .channels
            .iter()
            .find(|channel| channel.reference == item.channel)
            .unwrap();
        assert!(!internal.public_output);
        assert!(
            internal.matrix.is_some(),
            "Final keeps actual hidden Group/Layer dependencies"
        );
    }
    assert_eq!(discovered(&mut engine, matrix.media), [matrix]);
    assert!(captured.channels.iter().any(|channel| channel.reference == source));
}

fn layers(engine: &mut AppEngine, media: MediaId, parent: Option<GroupId>, count: u16) -> Vec<MatrixItemId> {
    let MediaEditResult::MatrixCreated { items, .. } =
        matrix_edit(engine, media, MatrixEdit::AddLayers { parent, count })
    else {
        panic!()
    };
    items
}
fn columns(engine: &mut AppEngine, media: MediaId, count: u16) -> Vec<ColumnId> {
    let MediaEditResult::MatrixCreated { columns, .. } = matrix_edit(engine, media, MatrixEdit::AddColumns { count })
    else {
        panic!()
    };
    columns
}
