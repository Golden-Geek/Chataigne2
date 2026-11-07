use super::transitions::{fixture, launch, matrix_edit, time};
use super::*;
use crate::{app::AppNode, matrix::protocol::*, media::capture_channels_at, media::composition::protocol::*};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::{Node, NodeReference},
    parameter::{ParamValue, ParameterEventBehaviour},
};

fn reference(engine: &mut AppEngine, missing: bool) -> MediaId {
    let target = missing.then(|| create(engine, MediaKind::SolidColor));
    let MediaEditResult::ReferenceCreated { media } = edit(
        engine,
        MediaEdit::CreateReference {
            target: target.map(|target| target.media),
            label: None,
        },
    ) else {
        panic!()
    };
    if let Some(target) = target {
        edit(engine, MediaEdit::RemoveMedia { media: target.media });
    }
    media
}
fn drop_composition(engine: &mut AppEngine, composition: ChannelReference, source: MediaId, copy: bool) -> InstanceId {
    let MediaEditResult::InstancesCreated { instances } = edit(
        engine,
        MediaEdit::Composition {
            reference: composition,
            edit: CompositionEdit::DropSource {
                source: source.0,
                instance: None,
                copy,
                position: Point { x: 12.0, y: 34.0 },
            },
        },
    ) else {
        panic!()
    };
    instances[0].instance
}
fn drop_matrix(engine: &mut AppEngine, matrix: ChannelReference, source: MediaId, copy: bool) -> (CellId, MediaId) {
    let snapshot = crate::matrix::inspect(engine, matrix.media).unwrap();
    let MatrixItemId::Layer { id: layer } = snapshot.items[0].id else {
        panic!()
    };
    let MediaEditResult::MatrixMediaCreated { cell, media } = matrix_edit(
        engine,
        matrix.media,
        MatrixEdit::DropSource {
            layer,
            column: snapshot.columns[0].id,
            source: source.0,
            copy,
        },
    ) else {
        panic!()
    };
    (cell, media)
}
fn composition(engine: &AppEngine, owner: ChannelReference) -> CompositionSnapshot {
    crate::media::composition::inspect(engine, owner).unwrap()
}
fn output_parameters(engine: &AppEngine, instance: InstanceId, cell: CellId) -> [golden_core::node::NodeId; 2] {
    let placement =
        crate::media::composition::children::placement(engine, engine.node_id_by_uuid(instance.0).unwrap()).unwrap();
    let AppNode::MatrixCell(cell) = engine.nodes.get(engine.node_id_by_uuid(cell.0).unwrap()).unwrap() else {
        panic!()
    };
    [placement.output_parameter(), cell.output_parameter()]
}
fn set_output(engine: &mut AppEngine, parameter: golden_core::node::NodeId, channel: ChannelId) {
    let result = apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node: parameter,
            value: ParamValue::Reference(NodeReference::new(channel.0)),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(result.success, "{result:?}");
}

#[test]
fn unavailable_reference_drops_recover_live_without_hidden_authoring_and_keep_explicit_output_choices() {
    for missing in [false, true] {
        for copy in [false, true] {
            let (mut engine, matrix, sources, _, columns) = fixture();
            let unavailable = reference(&mut engine, missing);
            let owner = create(&mut engine, MediaKind::Composition);
            let history = engine.undo_len();
            let instance = drop_composition(&mut engine, owner, unavailable, copy);
            let (cell, owned) = drop_matrix(&mut engine, matrix, unavailable, copy);
            assert_eq!(engine.undo_len(), history + 2);
            let composition_output = crate::media::resolve_source_node(&engine, instance.0).unwrap();
            let cell_output = crate::media::resolve_source_node(&engine, owned.0).unwrap();
            assert_eq!(
                composition(&engine, owner).instances[0].definition.source,
                Some(composition_output)
            );
            let snapshot = crate::matrix::inspect(&engine, matrix.media).unwrap();
            let selected = snapshot.cells.iter().find(|item| item.id == cell).unwrap();
            assert_eq!(selected.media, Some(owned));
            assert_eq!(selected.definition.source, Some(cell_output));
            let authored = to_sparse_project_json_pretty(&engine).unwrap();
            let mut restored = from_sparse_project_json::<AppNode>(&authored).unwrap();
            restored.apply_edits().unwrap();
            assert_eq!(crate::matrix::inspect(&restored, matrix.media).unwrap(), snapshot);
            assert!(engine.undo().unwrap());
            assert!(engine.redo().unwrap());
            let parameters = output_parameters(&engine, instance, cell);
            let selections = parameters.map(|parameter| {
                engine
                    .nodes
                    .get(parameter)
                    .unwrap()
                    .engine_param_snapshot()
                    .unwrap()
                    .value
            });
            launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
            let time = time();
            let pending = capture_channels_at(&engine, &[matrix, owner], 64, time).unwrap();
            assert!(pending.is_current());
            let targets = if copy {
                vec![MediaId(instance.0), owned]
            } else {
                vec![unavailable]
            };
            for media in &targets {
                edit(
                    &mut engine,
                    MediaEdit::ConfigureReference {
                        media: *media,
                        target: Some(sources[0].media),
                    },
                );
            }
            assert!(!pending.is_current());
            let before_capture = to_sparse_project_json_pretty(&engine).unwrap();
            let ready = capture_channels_at(&engine, &[matrix, owner], 64, time).unwrap();
            assert_eq!(
                ready
                    .channels
                    .iter()
                    .filter(|channel| channel.reference == sources[0])
                    .count(),
                1
            );
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before_capture);
            let MediaEditResult::Created { reference: peer } = edit(
                &mut engine,
                MediaEdit::AddChannel {
                    media: sources[0].media,
                    label: None,
                },
            ) else {
                panic!()
            };
            edit(
                &mut engine,
                MediaEdit::ReorderChannel {
                    reference: peer,
                    after: None,
                },
            );
            let pinned = capture_channels_at(&engine, &[matrix, owner], 64, time).unwrap();
            assert!(pinned.channels.iter().any(|channel| channel.reference == sources[0]));
            assert!(!pinned.channels.iter().any(|channel| channel.reference == peer));
            for media in &targets {
                let AppNode::ReferenceMedia(reference) =
                    engine.nodes.get(engine.node_id_by_uuid(media.0).unwrap()).unwrap()
                else {
                    panic!()
                };
                let parameter = reference.channel_parameter();
                set_output(&mut engine, parameter, peer.channel);
            }
            let changed = capture_channels_at(&engine, &[matrix, owner], 64, time).unwrap();
            assert_eq!(
                changed
                    .channels
                    .iter()
                    .filter(|channel| channel.reference == peer)
                    .count(),
                1
            );
            assert_eq!(
                composition(&engine, owner).instances[0].definition.source,
                Some(composition_output)
            );
            assert_eq!(
                crate::matrix::inspect(&engine, matrix.media)
                    .unwrap()
                    .cells
                    .iter()
                    .find(|item| item.id == cell)
                    .unwrap()
                    .definition
                    .source,
                Some(cell_output)
            );
            for (parameter, original) in parameters.into_iter().zip(selections) {
                assert_eq!(
                    engine
                        .nodes
                        .get(parameter)
                        .unwrap()
                        .engine_param_snapshot()
                        .unwrap()
                        .value,
                    original,
                    "single-output placement selection must not acquire hidden writes"
                );
            }
            edit(&mut engine, MediaEdit::RemoveChannel { reference: peer });
            let missing = capture_channels_at(&engine, &[matrix, owner], 64, time).unwrap();
            assert!(
                missing
                    .channels
                    .iter()
                    .any(|channel| channel.reference == peer && channel.definition.is_none()),
                "an explicit deleted channel must not fall back to the first source"
            );
            assert!(engine.undo().unwrap());
            let recovered = capture_channels_at(&engine, &[matrix, owner], 64, time).unwrap();
            assert!(
                recovered
                    .channels
                    .iter()
                    .any(|channel| channel.reference == peer && channel.definition.is_some())
            );
            let saved = to_sparse_project_json_pretty(&engine).unwrap();
            let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
            loaded.apply_edits().unwrap();
            assert_eq!(
                crate::media::resolve_source_node(&loaded, instance.0).unwrap(),
                composition_output
            );
            assert_eq!(
                crate::media::resolve_source_node(&loaded, owned.0).unwrap(),
                cell_output
            );
        }
    }
}

#[test]
fn unavailable_auto_outputs_cannot_hide_reference_composition_or_matrix_cycles() {
    for matrix_target in [false, true] {
        let (mut engine, matrix, _, _, _) = fixture();
        let unavailable = reference(&mut engine, false);
        let owner = create(&mut engine, MediaKind::Composition);
        if matrix_target {
            drop_matrix(&mut engine, matrix, unavailable, false);
        } else {
            drop_composition(&mut engine, owner, unavailable, false);
        }
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        let operation = MediaEdit::ConfigureReference {
            media: unavailable,
            target: Some(if matrix_target { matrix.media } else { owner.media }),
        };
        let result = apply_project_intent(
            &mut engine,
            UiEditIntent::InvokeAppOperation {
                namespace: MEDIA_NAMESPACE.into(),
                operation: MEDIA_OPERATION.into(),
                payload: serde_json::to_value(operation).unwrap(),
            },
        );
        assert!(!result.success, "{result:?}");
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}
