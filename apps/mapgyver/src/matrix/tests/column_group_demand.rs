use super::*;
use crate::applications::{demand::DemandReason, logical::capture_logical_applications, protocol::*};
use std::collections::HashSet;

fn group(engine: &mut AppEngine, media: MediaId, parent: Option<ColumnGroupId>) -> ColumnGroupId {
    let MediaEditResult::MatrixColumnGroupCreated { group } =
        edit(engine, media, MatrixEdit::AddColumnGroup { parent, label: None })
    else {
        panic!()
    };
    group
}
fn nested_columns(engine: &mut AppEngine, media: MediaId, group: ColumnGroupId, count: u16) -> Vec<ColumnId> {
    let MediaEditResult::MatrixCreated { columns, .. } = edit(
        engine,
        media,
        MatrixEdit::AddColumns {
            parent: Some(group),
            count,
        },
    ) else {
        panic!()
    };
    columns
}
fn configure(engine: &mut AppEngine, group: ColumnGroupId) -> ApplicationId {
    let application = ApplicationId(group.0);
    apply(
        engine,
        MediaEdit::Applications {
            edit: ApplicationEdit::Configure {
                application,
                definition: ApplicationDefinition {
                    executable: Some("C:\\missing\\never-launched.exe".into()),
                    ..Default::default()
                },
            },
        },
    );
    application
}
fn launch(engine: &mut AppEngine, media: MediaId, column: ColumnId) {
    edit(
        engine,
        media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
}
fn demand(engine: &AppEngine) -> HashSet<ApplicationId> {
    capture_logical_applications(engine)
        .unwrap()
        .demand
        .iter()
        .map(|d| d.application)
        .collect()
}

#[test]
fn nested_group_demand_is_continuous_across_children_and_independent_cell_triggers_stay_independent() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let outer = group(&mut engine, media, None);
    let inner = group(&mut engine, media, Some(outer));
    let children = nested_columns(&mut engine, media, inner, 2);
    let outside = columns(&mut engine, media, 1)[0];
    let a = configure(&mut engine, outer);
    let b = configure(&mut engine, inner);
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCell {
            layer: row,
            column: children[0],
            settings: CellSettings::default(),
        },
    );
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for _ in 0..20 {
        for column in &children {
            launch(&mut engine, media, *column);
            assert_eq!(demand(&engine), [a, b].into_iter().collect());
        }
    }
    launch(&mut engine, media, outside);
    assert!(demand(&engine).is_empty());
    edit(&mut engine, media, MatrixEdit::ClearPlayback);
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchCell {
            layer: row,
            column: children[0],
            mode: LaunchMode::Cut,
        },
    );
    assert!(
        demand(&engine).is_empty(),
        "individual Cells do not activate Column/group controls"
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
}

#[test]
fn regrouping_invalidates_logical_ranges_and_existing_capture_guards() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let group = group(&mut engine, media, None);
    let child = nested_columns(&mut engine, media, group, 1)[0];
    let outside = columns(&mut engine, media, 1)[0];
    let app = configure(&mut engine, group);
    launch(&mut engine, media, child);
    let capture = capture_logical_applications(&engine).unwrap();
    assert!(capture.is_current());
    assert_eq!(demand(&engine), [app].into_iter().collect());
    edit(
        &mut engine,
        media,
        MatrixEdit::MoveColumnItem {
            item: ColumnItemId::Column { id: child },
            parent: None,
            after: Some(ColumnItemId::Column { id: outside }),
        },
    );
    assert!(!capture.is_current());
    assert!(demand(&engine).is_empty());
    assert!(engine.undo().unwrap());
    assert_eq!(demand(&engine), [app].into_iter().collect());
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
    launch(&mut loaded, media, child);
    assert_eq!(demand(&loaded), [app].into_iter().collect());
}

#[test]
fn large_group_uses_one_range_and_no_per_child_bindings() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let group = group(&mut engine, media, None);
    let children = nested_columns(&mut engine, media, group, 1024);
    let app = configure(&mut engine, group);
    launch(&mut engine, media, children[1023]);
    assert_eq!(demand(&engine), [app].into_iter().collect());
    let root = super::super::playback::root(&engine, media).unwrap();
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(root) else {
        panic!()
    };
    let axis = root.matrix_playback().unwrap().application_axis().unwrap();
    assert_eq!(axis.spans.len(), 1);
    assert!(axis.columns.iter().all(|column| column.bindings.is_empty()));
    assert!(crate::matrix::inspect(&engine, media).unwrap().cells.is_empty());
}

#[test]
fn typed_group_pre_post_edits_invalidate_cached_demand_and_keep_flat_distances() {
    use golden_core::{
        node::Node,
        parameter::{ParamValue, ParameterEventBehaviour},
    };
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let before = columns(&mut engine, media, 2);
    let group = group(&mut engine, media, None);
    let children = nested_columns(&mut engine, media, group, 2);
    let after = columns(&mut engine, media, 2);
    let app = configure(&mut engine, group);
    launch(&mut engine, media, before[1]);
    let initial = capture_logical_applications(&engine).unwrap();
    assert!(initial.demand.is_empty());
    let [(pre, _), (post, _)] = crate::applications::control_node(&engine, app)
        .unwrap()
        .distance_values(1, 1);
    for node in [pre, post] {
        assert!(
            apply_project_intent(
                &mut engine,
                UiEditIntent::SetParam {
                    node,
                    value: ParamValue::Int(1),
                    behaviour: ParameterEventBehaviour::Append,
                }
            )
            .success
        );
    }
    assert!(!initial.is_current());
    for (column, expected) in [
        (before[0], None),
        (before[1], Some(DemandReason::Neighbor)),
        (children[0], Some(DemandReason::Preparation)),
        (children[1], Some(DemandReason::Preparation)),
        (after[0], Some(DemandReason::Neighbor)),
        (after[1], None),
    ] {
        launch(&mut engine, media, column);
        let capture = capture_logical_applications(&engine).unwrap();
        if let Some(reason) = expected {
            assert_eq!(capture.demand[0].reasons, [reason].into_iter().collect());
        } else {
            assert!(capture.demand.is_empty());
        }
    }
    let root = super::super::playback::root(&engine, media).unwrap();
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(root) else {
        panic!()
    };
    assert!(
        root.matrix_playback().unwrap().cached().is_none(),
        "logical group capture never builds a render index"
    );
    assert!(engine.nodes.get(pre).unwrap().node_data().parent.is_some());
}
