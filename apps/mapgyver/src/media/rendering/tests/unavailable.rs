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
            assert!(composition(&engine, owner).instances[0].definition.source.is_none());
            let snapshot = crate::matrix::inspect(&engine, matrix.media).unwrap();
            let selected = snapshot.cells.iter().find(|item| item.id == cell).unwrap();
            assert_eq!(selected.media, Some(owned));
            assert!(selected.definition.source.is_none());
            let authored = to_sparse_project_json_pretty(&engine).unwrap();
            let mut restored = from_sparse_project_json::<AppNode>(&authored).unwrap();
            restored.apply_edits().unwrap();
            assert_eq!(crate::matrix::inspect(&restored, matrix.media).unwrap(), snapshot);
            assert!(engine.undo().unwrap());
            assert!(engine.redo().unwrap());
            let parameters = output_parameters(&engine, instance, cell);
            for parameter in parameters {
                assert!(
                    matches!(engine.nodes.get(parameter).unwrap().engine_param_snapshot().unwrap().value, ParamValue::Reference(value) if value.uuid.0.is_nil())
                );
            }
            launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
            let time = time();
            let pending = capture_channels_at(&engine, &[matrix, owner], 64, time).unwrap();
            assert!(pending.is_current());
            if copy {
                for media in [MediaId(instance.0), owned] {
                    edit(
                        &mut engine,
                        MediaEdit::ConfigureReference {
                            media,
                            target: Some(sources[0].media),
                        },
                    );
                }
            } else {
                edit(
                    &mut engine,
                    MediaEdit::ConfigureReference {
                        media: unavailable,
                        target: Some(sources[0].media),
                    },
                );
            }
            assert!(!pending.is_current());
            let before_capture = to_sparse_project_json_pretty(&engine).unwrap();
            let ready = capture_channels_at(&engine, &[matrix, owner], 64, time).unwrap();
            assert!(ready.channels.iter().any(|channel| channel.reference
                == ChannelReference {
                    media: owned,
                    channel: sources[0].channel
                }));
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before_capture);
            let mut definition = composition(&engine, owner).instances[0].definition.clone();
            definition.opacity = 0.75;
            edit(
                &mut engine,
                MediaEdit::Composition {
                    reference: owner,
                    edit: CompositionEdit::ConfigureInstance { instance, definition },
                },
            );
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
            assert_eq!(
                composition(&engine, owner).instances[0]
                    .definition
                    .source
                    .unwrap()
                    .channel,
                peer.channel
            );
            assert!(
                capture_channels_at(&engine, &[matrix], 64, time)
                    .unwrap()
                    .channels
                    .iter()
                    .any(|channel| channel.reference
                        == ChannelReference {
                            media: owned,
                            channel: peer.channel
                        })
            );
            for parameter in parameters {
                assert!(
                    matches!(engine.nodes.get(parameter).unwrap().engine_param_snapshot().unwrap().value, ParamValue::Reference(value) if value.uuid.0.is_nil())
                );
                set_output(&mut engine, parameter, sources[0].channel);
            }
            launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
            assert_eq!(
                composition(&engine, owner).instances[0]
                    .definition
                    .source
                    .unwrap()
                    .channel,
                sources[0].channel
            );
            assert!(
                capture_channels_at(&engine, &[matrix], 64, time)
                    .unwrap()
                    .channels
                    .iter()
                    .any(|channel| channel.reference
                        == ChannelReference {
                            media: owned,
                            channel: sources[0].channel
                        })
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
