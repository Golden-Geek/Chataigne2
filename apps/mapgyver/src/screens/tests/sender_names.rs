use super::*;
use crate::app::AppNode;
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty, validate_engine_project_candidate},
    node::{Node, NodeId, NodeMetaPatch},
    parameter::ParamValue,
};

fn screen_definition(engine: &AppEngine, screen: ScreenId) -> ScreenDefinition {
    super::super::inspect(engine)
        .unwrap()
        .screens
        .into_iter()
        .find(|item| item.id == screen)
        .unwrap()
        .definition
}

fn sender_field(engine: &AppEngine, screen: ScreenId) -> NodeId {
    let node = engine.node_id_by_uuid(screen.0).unwrap();
    parameters::parameters(engine, node)["parameters/sender"]
}

fn mode(engine: &mut AppEngine, screen: ScreenId, enabled: bool) {
    let node = sender_field(engine, screen);
    let ack = apply_project_intent(
        engine,
        UiEditIntent::PatchMeta {
            node,
            patch: NodeMetaPatch {
                enabled: Some(enabled),
                ..Default::default()
            }
            .into(),
        },
    );
    assert!(ack.success, "{ack:?}");
}

fn output_name(engine: &AppEngine, screen: ScreenId) -> String {
    let capture = super::super::rendering::capture_outputs(engine, 128, 16, &[]).unwrap();
    let output = capture.outputs.iter().find(|output| output.screen == screen).unwrap();
    let super::super::rendering::ResolvedTarget::Spout { sender, .. } = &output.target else {
        panic!()
    };
    sender.clone()
}

fn configure_auto(engine: &mut AppEngine, screen: ScreenId) {
    apply(
        engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::AutomaticSpout {
                    extent: golden_video::Extent2D::new(96, 64).unwrap(),
                }),
                ..Default::default()
            },
        },
    );
}

#[test]
fn automatic_sender_tracks_names_while_custom_metadata_preserves_dormant_text_and_history() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    let field = sender_field(&engine, screen);
    let meta = &engine.nodes.get(field).unwrap().node_data().meta;
    assert!(!meta.enabled);
    assert!(meta.can_be_disabled);
    configure_auto(&mut engine, screen);
    assert_eq!(output_name(&engine, screen), "MapGyver - Screen");
    assert!(parameters::set(&mut engine, field, ParamValue::Str("My custom output".into())).success);
    assert_eq!(output_name(&engine, screen), "MapGyver - Screen");
    let capture = super::super::rendering::capture_outputs(&engine, 128, 16, &[]).unwrap();
    apply(
        &mut engine,
        ScreenEdit::RenameScreen {
            screen,
            label: "Stage".into(),
        },
    );
    assert!(capture.guards.iter().any(|guard| !guard.is_current()));
    assert_eq!(output_name(&engine, screen), "MapGyver - Stage");
    let history = engine.undo_len();
    mode(&mut engine, screen, true);
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(output_name(&engine, screen), "My custom output");
    apply(
        &mut engine,
        ScreenEdit::RenameScreen {
            screen,
            label: "Other stage".into(),
        },
    );
    assert_eq!(output_name(&engine, screen), "My custom output");
    mode(&mut engine, screen, false);
    assert_eq!(output_name(&engine, screen), "MapGyver - Other stage");
    assert!(engine.undo().unwrap());
    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .unwrap();
    assert_eq!(output_name(&engine, screen), "My custom output");
    assert!(engine.redo().unwrap());
    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .unwrap();
    assert_eq!(output_name(&engine, screen), "MapGyver - Other stage");
    mode(&mut engine, screen, true);
    assert_eq!(output_name(&engine, screen), "My custom output");
}

#[test]
fn automatic_configuration_roundtrips_without_freezing_names_or_overwriting_custom_values() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    configure_auto(&mut engine, screen);
    let field = sender_field(&engine, screen);
    assert!(parameters::set(&mut engine, field, ParamValue::Str("Dormant".into())).success);
    let mut definition = screen_definition(&engine, screen);
    definition.frame_rate = ScreenFrameRate::Explicit { fps: 30.0 };
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: definition.clone(),
        },
    );
    assert_eq!(screen_definition(&engine, screen), definition);
    assert!(!engine.nodes.get(field).unwrap().node_data().meta.enabled);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored: AppEngine = from_sparse_project_json(&json).unwrap();
    restored.apply_edits().unwrap();
    validate_engine_project_candidate(&restored).unwrap();
    assert_eq!(screen_definition(&restored, screen), definition);
    assert_eq!(output_name(&restored, screen), "MapGyver - Screen");
    mode(&mut restored, screen, true);
    assert_eq!(output_name(&restored, screen), "Dormant");
    let id = restored.node_id_by_uuid(screen.0).unwrap();
    assert!(matches!(restored.nodes.get(id), Some(AppNode::Screen(_))));
}

#[test]
fn automatic_and_custom_outputs_share_one_exact_collision_domain() {
    let mut engine = fixture();
    let (first, _) = create(&mut engine);
    let (second, _) = create(&mut engine);
    configure_auto(&mut engine, first);
    configure_auto(&mut engine, second);
    assert_ne!(output_name(&engine, first), output_name(&engine, second));
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let ack = invoke(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen: second,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Spout {
                    sender: "MapGyver - Screen".into(),
                    extent: golden_video::Extent2D::new(96, 64).unwrap(),
                }),
                ..Default::default()
            },
        },
    );
    assert!(!ack.success);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn configuring_a_new_mode_does_not_validate_the_dormant_endpoint_as_its_destination() {
    for automatic in [false, true] {
        let mut engine = fixture();
        let (first, _) = create(&mut engine);
        let (second, _) = create(&mut engine);
        let extent = golden_video::Extent2D::new(96, 64).unwrap();
        apply(
            &mut engine,
            ScreenEdit::ConfigureScreen {
                screen: first,
                definition: ScreenDefinition {
                    target: Some(ScreenTarget::Spout {
                        sender: if automatic { "Screen" } else { "MapGyver - Screen 2" }.into(),
                        extent,
                    }),
                    ..Default::default()
                },
            },
        );
        mode(&mut engine, second, automatic);
        let before = screen_definition(&engine, second);
        let history = engine.undo_len();
        apply(
            &mut engine,
            ScreenEdit::ConfigureScreen {
                screen: second,
                definition: ScreenDefinition {
                    target: Some(if automatic {
                        ScreenTarget::AutomaticSpout { extent }
                    } else {
                        ScreenTarget::Spout {
                            sender: "Fresh custom".into(),
                            extent,
                        }
                    }),
                    ..Default::default()
                },
            },
        );
        assert_eq!(
            output_name(&engine, second),
            if automatic {
                "MapGyver - Screen 2"
            } else {
                "Fresh custom"
            }
        );
        assert_eq!(engine.undo_len(), history + 1);
        assert!(engine.undo().unwrap());
        assert_eq!(screen_definition(&engine, second), before);
    }
}
