use super::*;
use crate::matrix::protocol::*;

#[test]
fn typed_drop_commands_accept_unavailable_reference_entries_with_atomic_history() {
    for kind in [
        MediaCommandKind::DropCompositionMedia,
        MediaCommandKind::DropMatrixMedia,
    ] {
        for copy in [false, true] {
            for missing in [false, true] {
                let mut engine = fixture();
                let producer = create(&mut engine, MediaKind::SolidColor);
                let MediaEditResult::ReferenceCreated { media: source } = apply(
                    &mut engine,
                    MediaEdit::CreateReference {
                        target: missing.then_some(producer.media),
                        label: None,
                    },
                ) else {
                    panic!()
                };
                if missing {
                    apply(&mut engine, MediaEdit::RemoveMedia { media: producer.media });
                }
                let matrix = kind == MediaCommandKind::DropMatrixMedia;
                let owner = create(
                    &mut engine,
                    if matrix {
                        MediaKind::Matrix
                    } else {
                        MediaKind::Composition
                    },
                );
                let command = command(&mut engine, kind);
                set(&mut engine, command, "media", reference(owner.media.0));
                set(&mut engine, command, "channel", reference(owner.channel.0));
                set(&mut engine, command, "entry", reference(source.0));
                set(&mut engine, command, "copy_media", ParamValue::Bool(copy));
                if matrix {
                    apply(
                        &mut engine,
                        MediaEdit::Matrix {
                            media: owner.media,
                            edit: MatrixEdit::AddLayers { parent: None, count: 1 },
                        },
                    );
                    apply(
                        &mut engine,
                        MediaEdit::Matrix {
                            media: owner.media,
                            edit: MatrixEdit::AddColumns { parent: None, count: 1 },
                        },
                    );
                    let snapshot = crate::matrix::inspect(&engine, owner.media).unwrap();
                    let MatrixItemId::Layer { id: layer } = snapshot.items[0].id else {
                        panic!()
                    };
                    set(&mut engine, command, "matrix_layer", reference(layer.0));
                    set(
                        &mut engine,
                        command,
                        "matrix_column",
                        reference(snapshot.columns[0].id.0),
                    );
                }
                let history = engine.undo_len();
                let owned = match apply(&mut engine, MediaEdit::ExecuteCommand { command }) {
                    MediaEditResult::InstancesCreated { instances } => MediaId(instances[0].instance.0),
                    MediaEditResult::MatrixMediaCreated { media, .. } => media,
                    other => panic!("{other:?}"),
                };
                assert_eq!(engine.undo_len(), history + 1);
                assert!(matches!(
                    engine.nodes.get(engine.node_id_by_uuid(owned.0).unwrap()),
                    Some(AppNode::ReferenceMedia(_))
                ));
                assert!(engine.undo().unwrap());
                assert!(engine.node_id_by_uuid(owned.0).is_none());
                assert!(engine.redo().unwrap());
                assert!(engine.node_id_by_uuid(owned.0).is_some());
                crate::media::validate_project(&engine).unwrap();
            }
        }
    }
}
