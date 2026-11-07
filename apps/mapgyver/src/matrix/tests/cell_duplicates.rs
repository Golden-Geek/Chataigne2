use super::*;
use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};

pub(super) fn scene(engine: &mut AppEngine) -> (MediaId, LayerId, Vec<ColumnId>) {
    let matrix = create(engine, MediaKind::Matrix);
    let row = layer(layers(engine, matrix.media, None, 1)[0]);
    let columns = columns(engine, matrix.media, 4);
    (matrix.media, row, columns)
}
pub(super) fn owned(engine: &mut AppEngine, media: MediaId, layer: LayerId, column: ColumnId) -> CellId {
    let MediaEditResult::MatrixMediaCreated { cell, .. } = edit(
        engine,
        media,
        MatrixEdit::CreateCellMedia {
            layer,
            column,
            media_kind: MediaKind::SolidColor,
        },
    ) else {
        panic!()
    };
    cell
}
pub(super) fn duplicate(
    engine: &mut AppEngine,
    media: MediaId,
    source: CellId,
    layer: LayerId,
    column: ColumnId,
    mode: CellTransferAction,
) -> CellId {
    let MediaEditResult::MatrixCellPrepared { cell, .. } = edit(
        engine,
        media,
        MatrixEdit::TransferCell {
            cell: source,
            layer,
            column,
            action: mode,
        },
    ) else {
        panic!()
    };
    cell
}
fn field(engine: &AppEngine, cell: CellId, name: &str) -> NodeId {
    let root = engine.node_id_by_uuid(cell.0).unwrap();
    field_on(engine, root, name)
}
fn field_on(engine: &AppEngine, root: NodeId, name: &str) -> NodeId {
    let mut next = engine.nodes.get(root).unwrap().node_data().first_child;
    while let Some(id) = next {
        let node = engine.nodes.get(id).unwrap();
        next = node.node_data().next_sibling;
        if !crate::parameters::is_parameter_folder(node) {
            continue;
        }
        let mut next = node.node_data().first_child;
        while let Some(id) = next {
            let node = engine.nodes.get(id).unwrap();
            if node.node_data().meta.decl_id.0 == format!("parameters/{name}") {
                return id;
            }
            next = node.node_data().next_sibling;
        }
    }
    panic!("missing Cell Parameter {name}")
}
pub(super) fn application_at(
    engine: &AppEngine,
    media: MediaId,
    row: LayerId,
    column: ColumnId,
) -> Option<crate::applications::protocol::ApplicationId> {
    let document = super::super::document::Document::load(engine, media).unwrap();
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(document.root) else {
        panic!()
    };
    super::super::application_axis::for_document(engine, &document, root.matrix_playback().unwrap())
        .unwrap()
        .cell_apps
        .get(&(row, column))
        .copied()
}

#[test]
fn copy_and_reference_preserve_exact_parameters_with_independent_or_linked_ownership_and_history() {
    let mut engine = fixture();
    let (media, row, columns) = scene(&mut engine);
    let (other, other_row, other_columns) = scene(&mut engine);
    let source = owned(&mut engine, media, row, columns[0]);
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCell {
            layer: row,
            column: columns[0],
            settings: CellSettings {
                position: [37.5, -19.25],
                size: Some(golden_video::Extent2D::new(41, 23).unwrap()),
                fit_mode: MediaFitMode::Fill,
                opacity: 0.25,
                enabled: false,
            },
        },
    );
    let opacity = field(&engine, source, "opacity");
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: opacity,
                value: ParamValue::Float(0.123456789012345),
                behaviour: ParameterEventBehaviour::Append
            }
        )
        .success
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCellApplication {
            layer: row,
            column: columns[0],
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\missing\\producer.exe".into()),
                arguments: vec!["original".into()],
                ..Default::default()
            },
            pre: 1,
            post: 2,
        },
    );
    let original = inspect(&mut engine, media).cells[0].clone();
    for (index, (target, target_row, column, mode)) in [
        (media, row, columns[1], CellTransferAction::Copy),
        (media, row, columns[2], CellTransferAction::Reference),
        (other, other_row, other_columns[0], CellTransferAction::Copy),
        (other, other_row, other_columns[1], CellTransferAction::Reference),
    ]
    .into_iter()
    .enumerate()
    {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        let cell = duplicate(&mut engine, target, source, target_row, column, mode);
        assert_ne!(cell, source);
        let snapshot = inspect(&mut engine, target);
        let created = snapshot.cells.iter().find(|value| value.id == cell).unwrap();
        let new_opacity = field(&engine, cell, "opacity");
        assert_ne!(new_opacity, opacity);
        assert_eq!(
            engine.nodes.get(new_opacity).unwrap().engine_param_snapshot(),
            engine.nodes.get(opacity).unwrap().engine_param_snapshot()
        );
        assert_eq!(created.definition.settings, original.definition.settings);
        if mode == CellTransferAction::Copy {
            assert_ne!(created.media, original.media);
            assert_eq!(created.definition.application.0, cell.0);
            assert!(
                crate::applications::control_node(&engine, created.definition.application)
                    .unwrap()
                    .configured()
            );
            assert_ne!(
                created.definition.source.unwrap().channel,
                original.definition.source.unwrap().channel
            );
        } else {
            let id = engine.node_id_by_uuid(created.media.unwrap().0).unwrap();
            let Some(AppNode::ReferenceMedia(reference)) = engine.nodes.get(id) else {
                panic!()
            };
            assert_eq!(reference.target(), original.media);
            assert_eq!(created.definition.application, original.definition.application);
            assert!(
                !crate::applications::control_node(&engine, crate::applications::protocol::ApplicationId(cell.0))
                    .unwrap()
                    .configured()
            );
        }
        assert_eq!(engine.undo_len(), history + 1, "case{index}");
        assert_eq!(
            application_at(&engine, target, target_row, column),
            Some(created.definition.application)
        );
        let after = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_eq!(application_at(&engine, target, target_row, column), None);
        assert!(apply_project_intent(&mut engine, UiEditIntent::Redo).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
        assert_eq!(
            application_at(&engine, target, target_row, column),
            Some(created.definition.application)
        );
        let mut loaded = from_sparse_project_json::<AppNode>(&after).unwrap();
        loaded.apply_edits().unwrap();
        assert_eq!(inspect(&mut loaded, target), snapshot);
    }
}

#[test]
fn duplicate_rejects_occupied_same_address_foreign_addresses_and_feedback_without_partial_edits() {
    let mut engine = fixture();
    let (media, row, columns) = scene(&mut engine);
    let (other, other_row, _) = scene(&mut engine);
    let source = owned(&mut engine, media, row, columns[0]);
    edit(
        &mut engine,
        media,
        MatrixEdit::EnsureCell {
            layer: row,
            column: columns[1],
        },
    );
    for mode in [CellTransferAction::Copy, CellTransferAction::Reference] {
        for (layer, column) in [(row, columns[0]), (row, columns[1]), (other_row, columns[2])] {
            let before = to_sparse_project_json_pretty(&engine).unwrap();
            let history = engine.undo_len();
            assert!(
                !invoke(
                    &mut engine,
                    MediaEdit::Matrix {
                        media,
                        edit: MatrixEdit::TransferCell {
                            cell: source,
                            layer,
                            column,
                            action: mode
                        }
                    }
                )
                .success
            );
            assert_eq!(engine.undo_len(), history);
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        }
    }
    let target_output = inspect(&mut engine, other).items[0].channel;
    edit(
        &mut engine,
        media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: row,
                column: columns[0],
                source: Some(target_output),
            }],
        },
    );
    for mode in [CellTransferAction::Copy, CellTransferAction::Reference] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: other,
                    edit: MatrixEdit::TransferCell {
                        cell: source,
                        layer: other_row,
                        column: columns[2],
                        action: mode
                    }
                }
            )
            .success
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}

#[test]
fn reference_application_dependencies_follow_definition_edits_without_rebuilding_on_launch() {
    let mut engine = fixture();
    let (source_media, source_row, source_columns) = scene(&mut engine);
    let (media, row, columns) = scene(&mut engine);
    let source = owned(&mut engine, source_media, source_row, source_columns[0]);
    let reference = duplicate(
        &mut engine,
        media,
        source,
        row,
        columns[0],
        CellTransferAction::Reference,
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchCell {
            layer: row,
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let time = super::cell_playback::time();
    let initial = super::cell_playback::capture(&engine, media, row, time);
    assert!(initial.applications.references.is_empty());
    let root = crate::matrix::playback::root(&engine, media).unwrap();
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get(root) else {
        panic!()
    };
    let control = node.matrix_playback().unwrap().clone();
    let axis = control.application_axis().unwrap();
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchCell {
            layer: row,
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    assert!(std::sync::Arc::ptr_eq(&axis, &control.application_axis().unwrap()));
    edit(
        &mut engine,
        source_media,
        MatrixEdit::ConfigureCellApplication {
            layer: source_row,
            column: source_columns[0],
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\missing\\producer.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    );
    assert!(!axis.is_current());
    assert!(control.application_axis().is_none());
    let updated = super::cell_playback::capture(&engine, media, row, time);
    assert_eq!(
        updated.applications.references.as_ref(),
        &[crate::applications::protocol::ApplicationId(source.0)]
    );
    let reference_node = engine.node_id_by_uuid(reference.0).unwrap();
    let Some(AppNode::MatrixCell(reference_owner)) = engine.nodes.get(reference_node) else {
        panic!()
    };
    let app_file = field_on(&engine, reference_owner.application_node().unwrap(), "app_file");
    for (value, owner) in [("C:\\missing\\local.exe", reference), ("", source)] {
        assert!(
            apply_project_intent(
                &mut engine,
                UiEditIntent::SetParam {
                    node: app_file,
                    value: ParamValue::File(value.into()),
                    behaviour: ParameterEventBehaviour::Append
                }
            )
            .success
        );
        assert_eq!(
            inspect(&mut engine, media)
                .cells
                .iter()
                .find(|cell| cell.id == reference)
                .unwrap()
                .definition
                .application
                .0,
            owner.0
        );
        assert_eq!(application_at(&engine, media, row, columns[0]).unwrap().0, owner.0);
    }
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCellApplication {
            layer: row,
            column: columns[0],
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\missing\\independent.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    );
    let snapshot = inspect(&mut engine, media);
    assert_eq!(
        snapshot
            .cells
            .iter()
            .find(|cell| cell.id == reference)
            .unwrap()
            .definition
            .application
            .0,
        reference.0
    );
    assert!(
        crate::applications::control_node(&engine, crate::applications::protocol::ApplicationId(reference.0))
            .unwrap()
            .configured()
    );
}

#[test]
fn reference_chain_limit_rejects_the_new_link_atomically_before_importing_a_cell() {
    let mut engine = fixture();
    let (media, row, mut columns) = scene(&mut engine);
    columns.extend(super::columns(&mut engine, media, 30));
    let mut source = owned(&mut engine, media, row, columns[0]);
    for column in &columns[1..32] {
        source = duplicate(&mut engine, media, source, row, *column, CellTransferAction::Reference);
        inspect(&mut engine, media);
    }
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::TransferCell {
                cell: source,
                layer: row,
                column: columns[32],
                action: CellTransferAction::Reference,
            },
        },
    );
    assert!(
        !ack.success,
        "a new link may not exceed the existing32-Cell owner bound: {ack:?}"
    );
    assert!(
        ack.error_message
            .as_deref()
            .is_some_and(|message| message.contains("reference chain exceeds 32"))
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(inspect(&mut engine, media).cells.len(), 32);
}
