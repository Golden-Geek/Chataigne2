use super::*;
use golden_core::node::{Node, NodeId, NodeReference};
use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
use mapgyver::screens::protocol::{SCREEN_NAMESPACE, SCREEN_OPERATION, ScreenEdit, ScreenEditResult};

fn surface_parameter(engine: &AppEngine, surface: golden_core::node::NodeUuid, declaration: &str) -> NodeId {
    let root = engine.node_id_by_uuid(surface).unwrap();
    let mut next = engine.nodes.get(root).unwrap().node_data().first_child;
    while let Some(id) = next {
        let node = engine.nodes.get(id).unwrap();
        next = node.node_data().next_sibling;
        if node.node_data().meta.decl_id.0 != "parameters" {
            continue;
        }
        let mut child = node.node_data().first_child;
        while let Some(id) = child {
            let node = engine.nodes.get(id).unwrap();
            if node.node_data().meta.decl_id.0 == declaration {
                return id;
            }
            child = node.node_data().next_sibling;
        }
    }
    panic!("missing Surface Parameter {declaration}")
}
fn set(engine: &mut AppEngine, parameter: NodeId, uuid: golden_core::node::NodeUuid) {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node: parameter,
            value: ParamValue::Reference(NodeReference::new(uuid)),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
}

#[test]
#[ignore = "requires explicit hardware D3D11 and native Spout output availability"]
fn native_surface_media_parameter_default_and_explicit_channel_route_on_both_gpus() {
    let adapters = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|a| !a.software)
        .collect::<Vec<_>>();
    assert_eq!(adapters.len(), 2);
    for adapter in adapters {
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::InvokeAppOperation {
                namespace: SCREEN_NAMESPACE.into(),
                operation: SCREEN_OPERATION.into(),
                payload: serde_json::to_value(ScreenEdit::CreateScreen { label: None }).unwrap(),
            },
        );
        assert!(ack.success, "{ack:?}");
        let ScreenEditResult::ScreenCreated { screen, surface } =
            serde_json::from_value(ack.result.unwrap().value).unwrap()
        else {
            panic!()
        };
        let red = color(&mut engine, [1., 0., 0., 1.]);
        let MediaEditResult::Created { reference: blue } = edit(
            &mut engine,
            MediaEdit::AddChannel {
                media: red.media,
                label: None,
            },
        ) else {
            panic!()
        };
        edit(
            &mut engine,
            MediaEdit::ConfigureChannel {
                reference: blue,
                source: ChannelSource::SolidColor { rgba: [0., 0., 1., 1.] },
            },
        );
        let green = color(&mut engine, [0., 1., 0., 1.]);
        let target = surface_parameter(&engine, surface.surface.0, "parameters/source_media");
        let channel = surface_parameter(&engine, surface.surface.0, "parameters/source_channel");
        let mut fixture = Fixture::new(adapter.id);
        for (index, expected) in [[1., 0., 0., 1.], [0., 0., 1., 1.], [0., 1., 0., 1.], [0., 0., 1., 1.]]
            .into_iter()
            .enumerate()
        {
            match index {
                0 => set(&mut engine, target, red.media.0),
                1 => set(&mut engine, channel, blue.channel.0),
                2 => set(&mut engine, target, green.media.0),
                3 => {
                    assert!(engine.undo().unwrap());
                    engine
                        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EndOfTickStabilization)
                        .unwrap();
                }
                _ => unreachable!(),
            }
            fixture.publish_screen(&engine, screen);
            fixture.pixels(adapter.id, &image(expected), &format!("surface-target-{index}"));
        }
    }
}
