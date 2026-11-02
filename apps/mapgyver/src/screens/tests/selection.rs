use super::*;
use crate::{app::AppNode, screens::catalog_runtime};
use golden_core::{
    app::{from_sparse_project_json, prepare_engine_for_runtime, to_sparse_project_json_pretty},
    node::{NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};
use std::{sync::Arc, time::Duration};

fn runtime() -> AppEngine {
    let mut engine = fixture();
    prepare_engine_for_runtime(&mut engine).unwrap();
    engine
}

fn display(id: &str, index: u32) -> DisplayInfo {
    DisplayInfo {
        id: id.into(),
        name: format!("Panel {index}"),
        edid: Some(format!("{index:02x}").repeat(128)),
        index,
        primary: index == 1,
        bounds: PhysicalRectangle {
            left: ((index - 1) * 1920) as i32,
            top: 0,
            extent: golden_video::Extent2D::new(1920, 1080).unwrap(),
        },
    }
}

fn publish(engine: &mut AppEngine, displays: Vec<DisplayInfo>) {
    let source = catalog_runtime::Source::capture(engine).unwrap();
    let options = catalog_runtime::display_options(&displays);
    let update = source.prepare(Arc::from(displays), &options);
    source.publisher.publish(update).unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
}

fn screen(engine: &AppEngine, id: ScreenId) -> &super::super::ScreenNode {
    let Some(AppNode::Screen(node)) = engine.nodes.get(engine.node_id_by_uuid(id.0).unwrap()) else {
        panic!()
    };
    node
}

fn set(engine: &mut AppEngine, node: NodeId, value: ParamValue) -> UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    )
}

fn dispatch(engine: &mut AppEngine) {
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
}

#[test]
fn selecting_an_occupied_display_atomically_swaps_saved_hints_and_history_restores_them_exactly() {
    let mut engine = runtime();
    let original_catalog = vec![display("primary", 1), display("two", 2), display("three", 3)];
    publish(&mut engine, original_catalog.clone());
    let (first, first_surface) = create(&mut engine);
    let (second, second_surface) = create(&mut engine);
    publish(&mut engine, original_catalog);
    let first_hint = screen(&engine, first).display_hint();
    let second_hint = screen(&engine, second).display_hint();
    let first_body = definition(&engine, first_surface);
    let second_body = definition(&engine, second_surface);
    let history = engine.undo_len();
    let parameter = screen(&engine, first).display_parameter_id();
    let ack = set(&mut engine, parameter, ParamValue::Enum("three".into()));
    assert!(ack.success, "{ack:?}");
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(screen(&engine, first).display_hint(), second_hint);
    assert_eq!(screen(&engine, second).display_hint(), first_hint);
    assert_eq!(definition(&engine, first_surface), first_body);
    assert_eq!(definition(&engine, second_surface), second_body);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(screen(&loaded, second).display_hint(), first_hint);

    let mut changed = display("two", 2);
    changed.name = "Changed after selection".into();
    publish(&mut engine, vec![display("primary", 1), changed, display("three", 3)]);
    assert_eq!(
        engine.undo_len(),
        history + 1,
        "catalog updates are not authored history"
    );
    assert!(engine.undo().unwrap());
    dispatch(&mut engine);
    assert_eq!(screen(&engine, first).display_hint(), first_hint);
    assert_eq!(screen(&engine, second).display_hint(), second_hint);
    assert!(engine.redo().unwrap());
    dispatch(&mut engine);
    assert_eq!(screen(&engine, first).display_hint(), second_hint);
    assert_eq!(screen(&engine, second).display_hint(), first_hint);
}

#[test]
fn shared_texture_requester_replaces_the_occupied_output_without_changing_its_surface() {
    let mut engine = runtime();
    let catalog = vec![display("primary", 1), display("two", 2)];
    publish(&mut engine, catalog.clone());
    let (occupied, surface) = create(&mut engine);
    let (requester, _) = create(&mut engine);
    let (reserved_sender, _) = create(&mut engine);
    publish(&mut engine, catalog);
    let occupied_node = engine.node_id_by_uuid(occupied.0).unwrap();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: occupied_node,
                patch: NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                }
                .into(),
            }
        )
        .success
    );
    let body = definition(&engine, surface);
    let hint = screen(&engine, occupied).display_hint();
    let history = engine.undo_len();
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen: requester,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Fullscreen { display: "two".into() }),
                enabled: true,
            },
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(
        matches!(screen(&engine, occupied).definition().target, Some(ScreenTarget::Spout { sender, .. }) if sender == "MapGyver Screen")
    );
    assert!(
        matches!(screen(&engine, reserved_sender).definition().target, Some(ScreenTarget::Spout { sender, .. }) if sender == "MapGyver Screen 2")
    );
    assert_eq!(screen(&engine, requester).display_hint(), hint);
    assert_eq!(definition(&engine, surface), body);
    assert!(
        !screen(&engine, occupied).definition().enabled,
        "reassignment preserves metadata enablement"
    );
    assert!(
        golden_core::logger::records().iter().any(|record| {
            record.origin == Some(occupied_node)
                && record
                    .message
                    .contains("fullscreen assignment replaced with Shared Texture")
        }),
        "only committed output changes are logged"
    );
    assert!(engine.undo().unwrap());
    dispatch(&mut engine);
    assert!(
        matches!(screen(&engine, occupied).definition().target, Some(ScreenTarget::Fullscreen { display }) if display == "two")
    );
    assert!(
        matches!(screen(&engine, requester).definition().target, Some(ScreenTarget::Spout { sender, .. }) if sender == "MapGyver Screen")
    );
}

#[test]
fn conflicting_explicit_peer_assignments_reject_the_whole_proposal() {
    let mut engine = runtime();
    let catalog = vec![display("primary", 1), display("two", 2), display("three", 3)];
    publish(&mut engine, catalog.clone());
    let (first, _) = create(&mut engine);
    let (second, _) = create(&mut engine);
    publish(&mut engine, catalog);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let first_parameter = screen(&engine, first).display_parameter_id();
    let second_parameter = screen(&engine, second).display_parameter_id();
    engine.edits.push(golden_core::edit::Edit::SetParamsAtomic {
        values: vec![
            (first_parameter, ParamValue::Enum("three".into())),
            (second_parameter, ParamValue::Enum("three".into())),
        ],
        behaviour: ParameterEventBehaviour::Append,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn a_recovered_physical_display_is_occupied_even_when_the_saved_path_is_offline() {
    let mut engine = runtime();
    publish(&mut engine, vec![display("primary", 1), display("old", 2)]);
    let (occupied, _) = create(&mut engine);
    let (requester, _) = create(&mut engine);
    let hint = screen(&engine, occupied).display_hint();
    publish(&mut engine, vec![display("primary", 1), display("replacement-path", 2)]);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen: requester,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Fullscreen {
                    display: "replacement-path".into(),
                }),
                enabled: true,
            },
        },
    );
    assert!(matches!(
        screen(&engine, occupied).definition().target,
        Some(ScreenTarget::Spout { .. })
    ));
    assert_eq!(
        screen(&engine, occupied).display_hint(),
        hint,
        "recovery never rewrites authored evidence"
    );
    assert_eq!(
        screen(&engine, requester).authored_display_identity(),
        "replacement-path"
    );
}
