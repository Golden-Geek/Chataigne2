use super::*;
use crate::audio::{ndi_route, voice};
use crate::screens::rendering::{ResolvedTarget, capture_outputs};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty, validate_engine_project_candidate};
use golden_core::{node::Node, parameter::ParamValue};
use golden_video::Extent2D;

#[test]
fn ndi_screen_target_resolves_typed_size_rate_and_protocol_local_names_through_reload() {
    let mut engine = fixture();
    let (first, _) = create(&mut engine);
    let (second, _) = create(&mut engine);
    let extent = Extent2D::new(320, 240).unwrap();
    let configure_ndi = |screen, sender: Option<&str>| ScreenEdit::ConfigureScreen {
        screen,
        definition: ScreenDefinition {
            target: Some(ScreenTarget::Ndi {
                sender: sender.map(str::to_owned),
                extent,
            }),
            frame_rate: ScreenFrameRate::Explicit { fps: 30.0 },
            ..Default::default()
        },
    };
    apply(&mut engine, configure_ndi(first, None));
    let get = |engine: &AppEngine| {
        capture_outputs(engine, 128, 16, &[])
            .unwrap()
            .outputs
            .into_iter()
            .find(|output| output.screen == first)
            .unwrap()
    };
    let initial = get(&engine);
    assert_eq!(initial.frame_rate, golden_time::RationalRate::new(30, 1).unwrap());
    assert_eq!(
        initial.target,
        ResolvedTarget::Ndi {
            sender: "MapGyver - Screen".into(),
            extent
        }
    );
    apply(
        &mut engine,
        ScreenEdit::RenameScreen {
            screen: first,
            label: "Stage / A".into(),
        },
    );
    assert_eq!(
        get(&engine).target,
        ResolvedTarget::Ndi {
            sender: "MapGyver - Stage   A".into(),
            extent
        }
    );
    assert!(!invoke(&mut engine, configure_ndi(first, Some("invalid:sender"))).success);
    apply(&mut engine, configure_ndi(first, Some("Local shared name")));
    assert!(!invoke(&mut engine, configure_ndi(second, Some("Local shared name"))).success);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen: second,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Spout {
                    sender: "Local shared name".into(),
                    extent,
                }),
                ..Default::default()
            },
        },
    );
    assert_eq!(
        get(&engine).target,
        ResolvedTarget::Ndi {
            sender: "Local shared name".into(),
            extent
        }
    );

    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut reloaded = from_sparse_project_json(&json).unwrap();
    reloaded.apply_edits().unwrap();
    validate_engine_project_candidate(&reloaded).unwrap();
    assert_eq!(get(&reloaded).target, get(&engine).target);
    assert_eq!(get(&reloaded).frame_rate, get(&engine).frame_rate);
}

#[test]
fn screen_audio_routes_remain_typed_through_target_changes_undo_and_reload() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    let root = engine.node_id_by_uuid(screen.0).unwrap();
    let add_route = |engine: &mut AppEngine| {
        apply_project_intent(
            engine,
            UiEditIntent::CreateUserItem {
                parent: root,
                node_type: ndi_route::TYPE.into(),
                label: None,
                initial_params: vec![],
            },
        )
    };
    assert!(!add_route(&mut engine).success);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Ndi {
                    sender: None,
                    extent: Extent2D::new(320, 240).unwrap(),
                }),
                ..Default::default()
            },
        },
    );
    let route = add_route(&mut engine).created_node.expect("created Screen audio route");
    let destination = |engine: &AppEngine| {
        let id = engine.node_id_by_uuid(route.uuid).unwrap();
        engine
            .process_tree_snapshot()
            .child_ids(id)
            .into_iter()
            .find(|id| engine.nodes.get(*id).unwrap().node_data().meta.decl_id.0 == "destination")
            .unwrap()
    };
    let left = |engine: &AppEngine| {
        let id = engine.node_id_by_uuid(screen.0).unwrap();
        parameters::parameters(engine, id)["parameters/ndi_audio/ndi_left"]
    };
    let right_bus = ParamValue::Enum(voice::RIGHT.into());
    let left_bus = ParamValue::Enum(voice::LEFT.into());
    let left_source = left(&engine);
    let route_source = destination(&engine);
    assert!(parameters::set(&mut engine, left_source, right_bus.clone()).success);
    assert!(parameters::set(&mut engine, route_source, left_bus.clone()).success);
    assert!(engine.undo().unwrap());
    assert!(engine.redo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition::default(),
        },
    );
    assert!(!add_route(&mut engine).success);

    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut reloaded = from_sparse_project_json(&json).unwrap();
    reloaded.apply_edits().unwrap();
    validate_engine_project_candidate(&reloaded).unwrap();
    let values = [left(&reloaded), destination(&reloaded)]
        .map(|id| reloaded.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value);
    assert_eq!(values, [right_bus, left_bus]);
}
