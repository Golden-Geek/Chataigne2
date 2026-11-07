use super::*;
use crate::app::AppNode;
use crate::screens::catalog_runtime::{Source, display_options};
use golden_core::{
    app::{from_sparse_project_json, prepare_engine_for_runtime, to_sparse_project_json_pretty},
    node::{Node, NodeMetaPatch},
    parameter::{ParamValue, ParameterEnumOptions, ParameterEventBehaviour},
};
use golden_video::Extent2D;
use std::{sync::Arc, time::Duration};

fn runtime() -> AppEngine {
    let mut engine = fixture();
    prepare_engine_for_runtime(&mut engine).unwrap();
    engine
}
fn owner(engine: &AppEngine) -> &crate::screens::ScreenManagerNode {
    engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::ScreenManager(manager) => Some(manager),
            _ => None,
        })
        .unwrap()
}
fn screen_node(engine: &AppEngine, screen: ScreenId) -> &crate::screens::ScreenNode {
    match engine.nodes.get(engine.node_id_by_uuid(screen.0).unwrap()).unwrap() {
        AppNode::Screen(screen) => screen,
        _ => panic!("Screen node expected"),
    }
}
fn options(engine: &AppEngine, screen: ScreenId) -> ParameterEnumOptions {
    engine
        .nodes
        .get(screen_node(engine, screen).display_parameter_id())
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints
        .enum_options
}
fn selected(engine: &AppEngine, screen: ScreenId) -> ParamValue {
    engine
        .nodes
        .get(screen_node(engine, screen).display_parameter_id())
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value
}
fn drain(engine: &mut AppEngine) {
    engine.run_tick(Duration::from_millis(1)).unwrap();
}
fn publish(engine: &mut AppEngine, displays: Arc<[DisplayInfo]>) {
    let source = Source::capture(engine).unwrap();
    let update = source.prepare(displays.clone(), &display_options(&displays));
    source.publisher.publish(update).unwrap();
    drain(engine);
}

#[test]
fn disabled_connected_display_row_is_present_after_runtime_allocation_excludes_output() {
    let mut engine = runtime();
    let displays = catalog();
    publish(&mut engine, displays.clone());
    let (screen, _) = create(&mut engine);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                frame_rate: Default::default(),
                enabled: false,
                target: Some(ScreenTarget::Fullscreen {
                    display: displays[0].id.clone(),
                }),
                anti_aliasing: Default::default(),
            },
        },
    );
    publish(&mut engine, displays.clone());
    assert!(owner(&engine).has_runtime_allocation());
    assert!(
        owner(&engine)
            .resolved_display(screen, &screen_node(&engine, screen).display_hint())
            .is_none()
    );
    let ScreenEditResult::Row { snapshot: Some(row) } = apply(&mut engine, ScreenEdit::InspectRow { node: screen.0 })
    else {
        panic!("connected disabled row expected")
    };
    assert!(!row.enabled);
    assert_eq!(row.detail, format!("Fullscreen · {}", displays[0].name));
    assert_eq!(
        screen_node(&engine, screen).definition().target,
        Some(ScreenTarget::Fullscreen {
            display: displays[0].id.clone()
        })
    );
}

#[test]
fn fullscreen_allocation_handoff_is_pending_until_current_metadata_confirms_target_or_absence() {
    let mut engine = runtime();
    let displays = catalog();
    publish(&mut engine, displays.clone());
    let (screen, _) = create(&mut engine);
    publish(&mut engine, displays.clone());
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                frame_rate: Default::default(),
                target: Some(ScreenTarget::Fullscreen {
                    display: "screen-b".into(),
                }),
                enabled: true,
                anti_aliasing: Default::default(),
            },
        },
    );
    let capture = super::super::rendering::capture_outputs(&engine, 128, 7, &displays).unwrap();
    assert!(
        capture.allocation_pending,
        "pending reassignment is not confirmed output removal"
    );
    let guard = owner(&engine).guard();
    publish(&mut engine, displays.clone());
    assert!(
        !guard.is_current(),
        "allocation completion must wake retained host preparation"
    );
    let ready = super::super::rendering::capture_outputs(&engine, 128, 7, &displays).unwrap();
    assert!(!ready.allocation_pending);
    assert_eq!(ready.outputs.len(), 1);
    // A dirty allocation can resolve to the identical cohort (disable/enable before delivery).
    for enabled in [false, true] {
        apply(
            &mut engine,
            ScreenEdit::ConfigureScreen {
                screen,
                definition: ScreenDefinition {
                    frame_rate: Default::default(),
                    target: Some(ScreenTarget::Fullscreen {
                        display: "screen-b".into(),
                    }),
                    enabled,
                    anti_aliasing: Default::default(),
                },
            },
        );
    }
    let guard = owner(&engine).guard();
    assert!(
        super::super::rendering::capture_outputs(&engine, 128, 7, &displays)
            .unwrap()
            .allocation_pending
    );
    publish(&mut engine, displays.clone());
    assert!(
        !guard.is_current(),
        "identical allocation still completes the pending handoff"
    );
    let missing: Arc<[DisplayInfo]> = Arc::from([]);
    assert!(
        super::super::rendering::capture_outputs(&engine, 128, 7, &missing)
            .unwrap()
            .allocation_pending
    );
    publish(&mut engine, missing.clone());
    let absent = super::super::rendering::capture_outputs(&engine, 128, 7, &missing).unwrap();
    assert!(
        !absent.allocation_pending,
        "confirmed absence must retire instead of retaining wrong display"
    );
    assert!(absent.outputs.is_empty());
}
fn display(id: &str, index: u32, primary: bool, edid: u8) -> DisplayInfo {
    DisplayInfo {
        refresh_rate: None,
        id: id.into(),
        name: format!("Monitor {index}"),
        edid: Some(format!("{edid:02x}").repeat(128)),
        index,
        primary,
        bounds: PhysicalRectangle {
            left: (index as i32 - 1) * 1920,
            top: 0,
            extent: Extent2D::new(1920, 1080).unwrap(),
        },
    }
}
fn catalog() -> Arc<[DisplayInfo]> {
    Arc::from([
        display("primary", 1, true, 1),
        display("screen-a", 2, false, 2),
        display("screen-b", 3, false, 3),
    ])
}

#[test]
fn display_metadata_latest_mailbox_preserves_authored_state_history_and_shared_domains() {
    let mut engine = runtime();
    let initial = catalog();
    publish(&mut engine, initial.clone());
    let (first, _) = create(&mut engine);
    let (second, _) = create(&mut engine);
    apply(
        &mut engine,
        ScreenEdit::RenameScreen {
            screen: first,
            label: "Temporary".into(),
        },
    );
    assert!(engine.undo().unwrap());
    drain(&mut engine);
    let hints = [
        screen_node(&engine, first).display_hint(),
        screen_node(&engine, second).display_hint(),
    ];
    let values = [selected(&engine, first), selected(&engine, second)];
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = (engine.undo_len(), engine.redo_len());
    let source = Source::capture(&engine).unwrap();
    for number in 0..8 {
        let next: Arc<[DisplayInfo]> = Arc::from([display(&format!("intermediate-{number}"), 2, false, 2)]);
        source
            .publisher
            .publish(source.prepare(next.clone(), &display_options(&next)))
            .unwrap();
    }
    let latest: Arc<[DisplayInfo]> = Arc::from([initial[0].clone(), display("recovered-a", 2, false, 2)]);
    source
        .publisher
        .publish(source.prepare(latest.clone(), &display_options(&latest)))
        .unwrap();
    assert_eq!(
        owner(&engine).displays(),
        initial,
        "worker publication waits for the engine wake boundary"
    );
    drain(&mut engine);
    assert_eq!(
        owner(&engine).displays(),
        latest,
        "only the newest bounded mailbox value is applied"
    );
    let first_options = options(&engine, first);
    let second_options = options(&engine, second);
    assert!(
        first_options.same_storage(&second_options),
        "all Screens share one detected/saved identity domain"
    );
    for identity in ["primary", "recovered-a", "screen-a", "screen-b"] {
        assert!(
            first_options.contains_variant(identity),
            "missing live or retained saved token: {identity}"
        );
    }
    assert!(!first_options.contains_variant("intermediate-7"));
    assert_eq!(screen_node(&engine, first).display_hint(), hints[0]);
    assert_eq!(screen_node(&engine, second).display_hint(), hints[1]);
    assert_eq!([selected(&engine, first), selected(&engine, second)], values);
    assert_eq!((engine.undo_len(), engine.redo_len()), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert!(
        engine.redo().unwrap(),
        "native metadata cannot consume or truncate authored redo"
    );
}

#[test]
fn display_metadata_same_saved_project_reload_fences_old_publisher_and_accepts_fresh_catalog() {
    let mut engine = runtime();
    let displays = catalog();
    publish(&mut engine, displays.clone());
    let (screen, _) = create(&mut engine);
    publish(&mut engine, displays.clone());
    let hint = screen_node(&engine, screen).display_hint();
    let leaf = screen_node(&engine, screen).display_parameter_id();
    let leaf_uuid = engine.nodes.get(leaf).unwrap().node_data().meta.uuid;
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let old = Source::capture(&engine).unwrap();
    let late = old.prepare(displays.clone(), &display_options(&displays));
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    prepare_engine_for_runtime(&mut loaded).unwrap();
    drop(engine);
    assert!(old.publisher.is_closed());
    assert!(
        old.publisher.publish(late).is_err(),
        "same UUIDs do not revive an old engine destination"
    );
    let loaded_leaf = screen_node(&loaded, screen).display_parameter_id();
    assert_eq!(loaded.nodes.get(loaded_leaf).unwrap().node_data().meta.uuid, leaf_uuid);
    publish(&mut loaded, displays.clone());
    assert_eq!(owner(&loaded).displays(), displays);
    assert_eq!(screen_node(&loaded, screen).display_hint(), hint);
    assert_eq!(selected(&loaded, screen), ParamValue::Enum(hint.identity.clone()));
    assert!(options(&loaded, screen).contains_variant(&hint.identity));
    assert_eq!(loaded.undo_len(), 0);
}

#[test]
fn display_metadata_stale_prepared_hint_cannot_replace_new_selection_or_activate_old_mapping() {
    let mut engine = runtime();
    let displays = catalog();
    publish(&mut engine, displays.clone());
    let (screen, _) = create(&mut engine);
    publish(&mut engine, displays.clone());
    let old = Source::capture(&engine).unwrap();
    let stale_catalog: Arc<[DisplayInfo]> = Arc::from([display("stale-recovery", 2, false, 2)]);
    let prepared = old.prepare(stale_catalog.clone(), &display_options(&stale_catalog));
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                frame_rate: Default::default(),
                target: Some(ScreenTarget::Fullscreen {
                    display: "screen-b".into(),
                }),
                enabled: true,
                anti_aliasing: Default::default(),
            },
        },
    );
    let hint = screen_node(&engine, screen).display_hint();
    let value = selected(&engine, screen);
    let domain = options(&engine, screen);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = (engine.undo_len(), engine.redo_len());
    assert!(!old.guard.is_current());
    old.publisher.publish(prepared).unwrap();
    drain(&mut engine);
    assert_eq!(screen_node(&engine, screen).display_hint(), hint);
    assert_eq!(selected(&engine, screen), value);
    assert!(
        options(&engine, screen).same_storage(&domain),
        "old expected-value guard cannot overwrite the new domain"
    );
    assert!(
        owner(&engine).resolved_display(screen, &hint).is_none(),
        "old intended hint never activates against a new selection"
    );
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert_eq!((engine.undo_len(), engine.redo_len()), history);
    publish(&mut engine, displays.clone());
    assert_eq!(owner(&engine).resolved_display(screen, &hint).unwrap().id, "screen-b");
    assert_eq!(screen_node(&engine, screen).display_hint(), hint);
}

#[test]
fn display_metadata_disabled_reservations_do_not_steal_active_recovery_targets() {
    let mut engine = runtime();
    let displays = catalog();
    publish(&mut engine, displays.clone());
    let (disabled, _) = create(&mut engine);
    let (active, _) = create(&mut engine);
    let id = engine.node_id_by_uuid(disabled.0).unwrap();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: id,
                patch: NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                }
                .into(),
            }
        )
        .success
    );
    let hint = screen_node(&engine, active).display_hint();
    publish(&mut engine, Arc::from([displays[1].clone()]));
    assert_eq!(owner(&engine).resolved_display(active, &hint).unwrap().id, "screen-a");
    assert!(!screen_node(&engine, disabled).definition().enabled);
}

#[test]
fn display_metadata_enabling_exact_owner_never_duplicates_a_retained_fallback_before_reallocation() {
    let mut engine = runtime();
    let displays = catalog();
    publish(&mut engine, displays.clone());
    let (exact_owner, _) = create(&mut engine);
    let (fallback, _) = create(&mut engine);
    let id = engine.node_id_by_uuid(exact_owner.0).unwrap();
    let enable = |engine: &mut AppEngine, enabled| {
        assert!(
            apply_project_intent(
                engine,
                UiEditIntent::PatchMeta {
                    node: id,
                    patch: NodeMetaPatch {
                        enabled: Some(enabled),
                        ..Default::default()
                    }
                    .into(),
                },
            )
            .success
        );
    };
    enable(&mut engine, false);
    let connected: Arc<[DisplayInfo]> = Arc::from([displays[1].clone()]);
    publish(&mut engine, connected.clone());
    let targets = |engine: &AppEngine| {
        crate::screens::rendering::capture_outputs(engine, 128, 8, &connected)
            .unwrap()
            .outputs
            .into_iter()
            .filter_map(|output| match output.target {
                crate::screens::rendering::ResolvedTarget::Fullscreen { display, .. } => Some((output.screen, display)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(targets(&engine), vec![(fallback, "screen-a".into())]);
    let old = Source::capture(&engine).unwrap();
    let stale_catalog: Arc<[DisplayInfo]> = Arc::from([display("stale-must-not-apply", 2, false, 2)]);
    let stale = old.prepare(stale_catalog.clone(), &display_options(&stale_catalog));
    enable(&mut engine, true);
    let pending = targets(&engine);
    assert_eq!(
        pending.len(),
        1,
        "a healthy output remains while cohort recovery is pending"
    );
    assert_eq!(
        pending
            .iter()
            .map(|(_, display)| display)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        pending.len(),
        "an independent exact path cannot duplicate an old cohort's physical allocation",
    );
    assert!(!old.guard.is_current());
    old.publisher.publish(stale).unwrap();
    drain(&mut engine);
    assert_eq!(
        owner(&engine).displays(),
        connected,
        "a stale cohort cannot alter native metadata"
    );
    assert_eq!(targets(&engine).len(), 1);
    publish(&mut engine, connected.clone());
    assert_eq!(targets(&engine), vec![(exact_owner, "screen-a".into())]);
}

#[test]
fn display_metadata_reselecting_rebound_path_refreshes_hints_and_fences_prepared_allocation() {
    let mut engine = runtime();
    let initial = catalog();
    publish(&mut engine, initial.clone());
    let (screen, _) = create(&mut engine);
    publish(&mut engine, initial.clone());
    let old_hint = screen_node(&engine, screen).display_hint();
    let value = selected(&engine, screen);
    let mut rebound = initial.to_vec();
    rebound[1].edid = Some("44".repeat(128));
    let expected_hint = DisplayIdentityHint::from(&rebound[1]);
    let rebound: Arc<[DisplayInfo]> = rebound.into();
    publish(&mut engine, rebound.clone());
    assert_eq!(screen_node(&engine, screen).display_hint(), old_hint);

    let old = Source::capture(&engine).unwrap();
    let screen_guard = screen_node(&engine, screen).guard();
    let prepared = old.prepare(initial.clone(), &display_options(&initial));
    let parameter = screen_node(&engine, screen).display_parameter_id();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: parameter,
            value: value.clone(),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    drain(&mut engine);
    assert_eq!(selected(&engine, screen), value);
    assert_eq!(screen_node(&engine, screen).display_hint(), expected_hint);
    assert!(
        !screen_guard.is_current(),
        "hidden hint changes invalidate the Screen output"
    );
    assert!(
        !old.guard.is_current(),
        "hidden hint changes invalidate the whole allocation"
    );

    old.publisher.publish(prepared).unwrap();
    drain(&mut engine);
    assert_eq!(
        owner(&engine).displays(),
        rebound,
        "stale metadata cannot undo the rebind"
    );
    assert_eq!(screen_node(&engine, screen).display_hint(), expected_hint);
    assert!(owner(&engine).resolved_display(screen, &expected_hint).is_none());

    publish(&mut engine, rebound.clone());
    assert_eq!(
        owner(&engine).resolved_display(screen, &expected_hint),
        Some(&rebound[1])
    );
    let capture = crate::screens::rendering::capture_outputs(&engine, 128, 8, &rebound).unwrap();
    assert_eq!(capture.outputs.len(), 1);
    assert_eq!(capture.outputs[0].screen, screen);
    assert!(matches!(
        &capture.outputs[0].target,
        crate::screens::rendering::ResolvedTarget::Fullscreen { display, .. } if display == "screen-a"
    ));
}
