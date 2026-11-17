use super::{
    cell_playback::{capture, time},
    *,
};
use golden_core::node::{Node, NodeUuid};

fn toggle(engine: &mut AppEngine, uuid: NodeUuid, enabled: bool) {
    let node = engine.node_id_by_uuid(uuid).unwrap();
    let ack = apply_project_intent(
        engine,
        UiEditIntent::PatchMeta {
            node,
            patch: golden_core::ui_sync::NodeMetaPatch {
                enabled: Some(enabled),
                ..Default::default()
            },
        },
    );
    assert!(ack.success, "{ack:?}");
}

#[test]
fn disabled_cell_and_layer_keep_authored_identity_playback_and_application_demand_while_suppressing_render() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: row,
                column,
                source: Some(source),
            }],
        },
    );
    let MediaEditResult::MatrixCellPrepared { cell, application } = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureCellApplication {
            layer: row,
            column,
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\not-launched.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    ) else {
        panic!()
    };
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    let expected = capture(&engine, matrix.media, row, now).inputs[0].source;
    let playback = serde_json::to_value(super::super::playback::inspect(&engine, matrix.media).unwrap()).unwrap();
    for owner in [cell.0, row.0] {
        toggle(&mut engine, owner, false);
        let state = inspect(&mut engine, matrix.media);
        assert_eq!(state.cells[0].id, cell);
        assert_eq!(state.cells[0].definition.column, column);
        assert_eq!(state.items[0].id, MatrixItemId::Layer { id: row });
        assert!(capture(&engine, matrix.media, row, now).inputs.is_empty());
        assert_eq!(
            serde_json::to_value(super::super::playback::inspect(&engine, matrix.media).unwrap()).unwrap(),
            playback
        );
        let applications = crate::applications::logical::capture_logical_applications(&engine).unwrap();
        assert!(
            applications.demand.iter().any(|entry| {
                entry.application == application
                    && entry
                        .reasons
                        .contains(&crate::applications::demand::DemandReason::Preparation)
            }),
            "owner={owner:?}; demand={:?}; application={:?}",
            applications.demand,
            crate::applications::inspect(&engine, application)
        );
        assert_eq!(
            crate::applications::inspect(&engine, application).unwrap().enablement,
            crate::applications::protocol::ApplicationEnablement::Enabled
        );
        assert!(
            !engine
                .nodes
                .get(engine.node_id_by_uuid(owner).unwrap())
                .unwrap()
                .node_data()
                .meta
                .enabled
        );
        toggle(&mut engine, owner, true);
        assert_eq!(capture(&engine, matrix.media, row, now).inputs[0].source, expected);
    }
}
