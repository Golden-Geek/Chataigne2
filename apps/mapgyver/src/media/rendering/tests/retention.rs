use super::transitions::{at, fixture, launch, layers, matrix_edit, prepared, time};
use super::*;
use crate::{app::AppNode, matrix::protocol::*, media::capture_channels_at};
use golden_core::app::to_sparse_project_json_pretty;
use std::sync::Arc;

fn row(engine: &AppEngine, layer: ChannelReference) -> LayerId {
    let ChannelSource::Matrix {
        output: MatrixOutput::Layer { layer },
    } = crate::media::authoring::channel_internal(engine, layer)
        .unwrap()
        .definition
        .source
    else {
        panic!()
    };
    layer
}
pub(super) fn drop_copy(
    engine: &mut AppEngine,
    matrix: ChannelReference,
    layer: ChannelReference,
    column: ColumnId,
    source: ChannelReference,
) -> ChannelReference {
    let layer = row(engine, layer);
    let MediaEditResult::MatrixMediaCreated { media, .. } = matrix_edit(
        engine,
        matrix.media,
        MatrixEdit::DropSource {
            layer,
            column,
            source: source.media.0,
            copy: true,
        },
    ) else {
        panic!()
    };
    crate::matrix::inspect(engine, matrix.media)
        .unwrap()
        .cells
        .iter()
        .find(|cell| cell.definition.column == column && cell.layer == layer)
        .unwrap()
        .definition
        .source
        .filter(|source| source.media == media)
        .unwrap()
}
pub(super) fn registry(engine: &AppEngine) -> Arc<crate::media::retention::RuntimeSources> {
    let id = crate::media::authoring::manager(engine).unwrap();
    let AppNode::MediaManager(manager) = engine.nodes.get(id).unwrap() else {
        panic!()
    };
    manager.sources.clone()
}

#[test]
fn physical_copy_replacement_keeps_admitted_content_through_history_fade_and_final_use() {
    let (mut engine, matrix, sources, layer, columns) = fixture();
    let old = drop_copy(&mut engine, matrix, layer, columns[0], sources[0]);
    let time = time();
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    let before = capture_channels_at(&engine, &[matrix], 32, time).unwrap();
    let definition = before
        .channels
        .iter()
        .find(|channel| channel.reference == old)
        .unwrap()
        .definition
        .clone();
    let replacement = drop_copy(&mut engine, matrix, layer, columns[0], sources[1]);
    assert!(engine.node_id_by_uuid(old.media.0).is_none());
    assert!(!before.is_current());
    assert!(engine.undo().unwrap());
    assert!(engine.node_id_by_uuid(old.media.0).is_some());
    assert!(engine.redo().unwrap());
    assert!(engine.node_id_by_uuid(old.media.0).is_none());
    let unchanged = capture_channels_at(&engine, &[matrix], 32, time).unwrap();
    assert!(unchanged.is_current());
    assert_eq!(
        unchanged
            .channels
            .iter()
            .find(|channel| channel.reference == old)
            .unwrap()
            .definition,
        definition
    );
    assert!(
        !unchanged
            .channels
            .iter()
            .any(|channel| channel.reference == replacement)
    );
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Configured);
    let halfway = capture_channels_at(&engine, &[matrix], 32, time).unwrap();
    let owner = renderer();
    let plan = prepared(&halfway, matrix, &owner);
    assert_eq!(
        layers(&plan, layer)
            .iter()
            .map(|layer| layer.evaluated(at(time, 500)).opacity.get())
            .collect::<Vec<_>>(),
        [0.5, 0.5]
    );
    let cached = Arc::downgrade(registry(&engine).snapshot().get(&old).unwrap());
    let complete = capture_channels_at(&engine, &[matrix], 32, at(time, 1000)).unwrap();
    assert!(!complete.channels.iter().any(|channel| channel.reference == old));
    assert!(complete.channels.iter().any(|channel| channel.reference == replacement));
    assert!(!registry(&engine).snapshot().contains_key(&old));
    assert!(cached.upgrade().is_none());
    matrix_edit(&mut engine, matrix.media, MatrixEdit::ClearPlayback);
    capture_channels_at(&engine, &[matrix], 32, at(time, 1000)).unwrap();
    assert_eq!(registry(&engine).len(), 0);
    close(owner);
}

#[test]
fn physical_cell_clear_column_removal_and_media_removal_need_no_unload() {
    for action in 0..3 {
        let (mut engine, matrix, sources, layer, columns) = fixture();
        let old = drop_copy(&mut engine, matrix, layer, columns[0], sources[0]);
        let time = time();
        launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
        capture_channels_at(&engine, &[matrix], 32, time).unwrap();
        let layer = row(&engine, layer);
        match action {
            0 => {
                matrix_edit(
                    &mut engine,
                    matrix.media,
                    MatrixEdit::SetCells {
                        assignments: vec![CellAssignment {
                            layer,
                            column: columns[0],
                            source: None,
                        }],
                    },
                );
            }
            1 => {
                matrix_edit(
                    &mut engine,
                    matrix.media,
                    MatrixEdit::RemoveColumn { column: columns[0] },
                );
            }
            _ => {
                edit(&mut engine, MediaEdit::RemoveMedia { media: old.media });
            }
        }
        let captured = capture_channels_at(&engine, &[matrix], 32, time).unwrap();
        assert!(captured.is_current());
        assert!(
            captured
                .channels
                .iter()
                .any(|channel| channel.reference == old && channel.definition.is_some())
        );
        assert!(engine.undo().unwrap());
        assert!(engine.redo().unwrap());
        launch(&mut engine, matrix.media, columns[1], LaunchMode::Cut);
        capture_channels_at(&engine, &[matrix], 32, time).unwrap();
        assert!(!registry(&engine).snapshot().contains_key(&old));
    }
}

#[test]
fn uncaptured_launch_does_not_admit_later_authored_replacement() {
    let (mut engine, matrix, sources, layer, columns) = fixture();
    drop_copy(&mut engine, matrix, layer, columns[0], sources[0]);
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    let replacement = drop_copy(&mut engine, matrix, layer, columns[0], sources[1]);
    let captured = capture_channels(&engine, &[matrix], 32).unwrap();
    assert_eq!(captured.channels.len(), 1);
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    assert!(
        capture_channels(&engine, &[matrix], 32)
            .unwrap()
            .channels
            .iter()
            .any(|channel| channel.reference == replacement)
    );
}

#[test]
fn retired_composition_keeps_owned_children_but_external_target_deletion_stays_missing() {
    let (mut engine, matrix, sources, layer, columns) = fixture();
    let composition = create(&mut engine, MediaKind::Composition);
    edit(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: crate::media::composition::protocol::CompositionEdit::AddInstances {
                sources: vec![sources[0]],
            },
        },
    );
    let owned = drop_copy(&mut engine, matrix, layer, columns[0], composition);
    let time = time();
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    let before = capture_channels_at(&engine, &[matrix], 32, time).unwrap();
    let child = before
        .channels
        .iter()
        .find(|channel| channel.reference == owned)
        .unwrap()
        .instances
        .as_ref()
        .unwrap()[0]
        .source
        .unwrap();
    edit(&mut engine, MediaEdit::RemoveMedia { media: owned.media });
    let after = capture_channels_at(&engine, &[matrix], 32, time).unwrap();
    assert!(
        after
            .channels
            .iter()
            .any(|channel| channel.reference == child && channel.definition.is_some())
    );
    let owner = renderer();
    assert_eq!(
        prepared(&after, matrix, &owner).submission.plan.passes.len(),
        prepared(&before, matrix, &owner).submission.plan.passes.len()
    );
    edit(
        &mut engine,
        MediaEdit::RemoveMedia {
            media: sources[0].media,
        },
    );
    let missing = capture_channels_at(&engine, &[matrix], 32, time).unwrap();
    assert!(
        missing
            .channels
            .iter()
            .any(|channel| channel.reference == sources[0] && channel.definition.is_none())
    );
    assert!(
        !to_sparse_project_json_pretty(&engine)
            .unwrap()
            .contains(&owned.media.0.0.to_string())
    );
    close(owner);
}

#[test]
fn retired_nested_matrix_finishes_its_envelope_without_authored_nodes_or_control_cycles() {
    let (mut engine, matrix, sources, layer, columns) = fixture();
    let template = create(&mut engine, MediaKind::Matrix);
    matrix_edit(
        &mut engine,
        template.media,
        MatrixEdit::AddLayers { parent: None, count: 1 },
    );
    matrix_edit(&mut engine, template.media, MatrixEdit::AddColumns { count: 2 });
    let snapshot = crate::matrix::inspect(&engine, template.media).unwrap();
    let nested_row = row(&engine, snapshot.items[0].channel);
    matrix_edit(
        &mut engine,
        template.media,
        MatrixEdit::SetCells {
            assignments: snapshot
                .columns
                .iter()
                .zip(&sources)
                .map(|(column, source)| CellAssignment {
                    layer: nested_row,
                    column: column.id,
                    source: Some(*source),
                })
                .collect(),
        },
    );
    matrix_edit(
        &mut engine,
        template.media,
        MatrixEdit::ConfigureLayer {
            layer: nested_row,
            settings: LayerSettings {
                duration_ms: 1000,
                ..snapshot.items[0].layer.unwrap()
            },
        },
    );
    let nested = drop_copy(&mut engine, matrix, layer, columns[0], template);
    let nested_snapshot = crate::matrix::inspect(&engine, nested.media).unwrap();
    let nested_layer = nested_snapshot.items[0].channel;
    let old_alias = nested_snapshot
        .cells
        .iter()
        .find(|cell| cell.definition.column == nested_snapshot.columns[0].id)
        .unwrap()
        .definition
        .source
        .unwrap();
    let time = time();
    launch(
        &mut engine,
        nested.media,
        nested_snapshot.columns[0].id,
        LaunchMode::Cut,
    );
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    capture_channels_at(&engine, &[matrix], 64, time).unwrap();
    launch(
        &mut engine,
        nested.media,
        nested_snapshot.columns[1].id,
        LaunchMode::Configured,
    );
    capture_channels_at(&engine, &[matrix], 64, time).unwrap();
    edit(&mut engine, MediaEdit::RemoveMedia { media: nested.media });
    let halfway = capture_channels_at(&engine, &[matrix], 64, at(time, 500)).unwrap();
    assert!(halfway.is_current());
    assert!(
        !halfway
            .channels
            .iter()
            .find(|channel| channel.reference == nested_layer)
            .expect("retained nested Layer")
            .public_output,
        "retired internal exports retain their hidden policy"
    );
    let hidden = capture_channels_at(&engine, &[nested_layer], 64, at(time, 500)).unwrap();
    assert!(
        hidden.channels.iter().all(|channel| channel.definition.is_none()),
        "a hidden retired producer cannot become a new public demand"
    );
    assert!(
        halfway
            .channels
            .iter()
            .find(|channel| channel.reference == nested)
            .unwrap()
            .matrix
            .as_ref()
            .unwrap()
            .control
            .is_none()
    );
    let owner = renderer();
    let plan = prepared(&halfway, matrix, &owner);
    assert_eq!(
        layers(&plan, nested_layer)
            .iter()
            .map(|layer| layer.evaluated(at(time, 500)).opacity.get())
            .collect::<Vec<_>>(),
        [0.5, 0.5]
    );
    let complete = capture_channels_at(&engine, &[matrix], 64, at(time, 1500)).unwrap();
    assert!(!complete.channels.iter().any(|channel| channel.reference == old_alias));
    assert!(!registry(&engine).snapshot().contains_key(&old_alias));
    matrix_edit(&mut engine, matrix.media, MatrixEdit::ClearPlayback);
    capture_channels_at(&engine, &[matrix], 64, at(time, 1500)).unwrap();
    assert_eq!(registry(&engine).len(), 0);
    close(owner);
}

#[test]
fn retired_graph_capacity_rejection_preserves_the_previously_admitted_graph() {
    let (mut engine, matrix, sources, layer, columns) = fixture();
    let old = drop_copy(&mut engine, matrix, layer, columns[0], sources[0]);
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    let capture = capture_channels(&engine, &[matrix], 32).unwrap();
    let sources = registry(&engine);
    let previous = sources.snapshot();
    let template = capture
        .channels
        .iter()
        .find(|channel| channel.reference == old)
        .unwrap();
    let mut pending = Vec::new();
    let mut key = old;
    for _ in 0..4097 {
        let mut channel = template.clone();
        channel.reference = key;
        key = ChannelReference {
            media: MediaId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
            channel: ChannelId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
        };
        channel.definition.as_mut().unwrap().source = ChannelSource::Reference { target: Some(key) };
        pending.push(channel);
    }
    assert!(sources.update(&pending).unwrap_err().contains("4096"));
    let mut oversized = template.clone();
    oversized.definition.as_mut().unwrap().source = ChannelSource::Composition {
        canvas: Extent2D::new(4, 2).unwrap(),
    };
    let mut instance =
        crate::media::composition::protocol::InstanceDefinition::for_canvas(Extent2D::new(4, 2).unwrap());
    instance.source = Some(key);
    oversized.instances = Some(vec![instance; 4097]);
    assert!(sources.update(&[oversized]).unwrap_err().contains("elements"));
    let after = sources.snapshot();
    assert_eq!(after.len(), previous.len());
    for (key, value) in previous {
        assert!(Arc::ptr_eq(after.get(&key).unwrap(), &value));
    }
}

#[test]
fn removed_owned_composition_cannot_hide_feedback_through_its_external_producer() {
    let (mut engine, matrix, _, layer, columns) = fixture();
    let external = create(&mut engine, MediaKind::Composition);
    let template = create(&mut engine, MediaKind::Composition);
    edit(
        &mut engine,
        MediaEdit::Composition {
            reference: template,
            edit: crate::media::composition::protocol::CompositionEdit::AddInstances {
                sources: vec![external],
            },
        },
    );
    let owned = drop_copy(&mut engine, matrix, layer, columns[0], template);
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    capture_channels(&engine, &[matrix], 32).unwrap();
    edit(&mut engine, MediaEdit::RemoveMedia { media: owned.media });
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let result = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::Composition {
                reference: external,
                edit: crate::media::composition::protocol::CompositionEdit::AddInstances { sources: vec![matrix] },
            })
            .unwrap(),
        },
    );
    assert!(!result.success, "{result:?}");
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}
