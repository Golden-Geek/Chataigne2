use super::super::{DmxFrame, DmxModuleBase};
use super::*;
use golden_core::{
    edit::Edit,
    node::{NodeId, NodeMetaPatch},
    parameter::ParameterEventBehaviour,
    process_ctx::{ExecutionPhase, ProcessCtx},
    script::ScriptMethodCall,
};
use std::{
    net::{Ipv4Addr, UdpSocket},
    time::{Duration, Instant},
};

fn base(engine: &mut crate::app::AppEngine, id: NodeId) -> &mut DmxModuleBase {
    match engine.nodes.get_mut(id).unwrap() {
        AppNode::ArtNetModule(node) => &mut node.base,
        AppNode::SacnModule(node) => &mut node.base,
        _ => unreachable!(),
    }
}
fn turn(engine: &mut crate::app::AppEngine) {
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    engine.apply_edits().unwrap();
}
fn set(engine: &mut crate::app::AppEngine, id: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node: id,
        value,
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
}
fn fixture(destination: &UdpSocket) -> (crate::app::AppEngine, NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let mut node = ArtNetModule::create();
    node.node_data_mut().meta.enabled = false;
    engine.add_node(node.into(), None);
    for _ in 0..8 {
        turn(&mut engine);
    }
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let destination_id = base(&mut engine, id).destination.id();
    set(
        &mut engine,
        destination_id,
        ParamValue::Str(destination.local_addr().unwrap().to_string()),
    );
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        turn(&mut engine);
        if base(&mut engine, id)
            .transport
            .as_ref()
            .is_some_and(super::super::DmxTransportHandle::is_ready)
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    for _ in 0..10 {
        turn(&mut engine);
        std::thread::sleep(Duration::from_millis(1));
    }
    (engine, id)
}
fn receiver() -> UdpSocket {
    let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    receiver.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    receiver
}
fn receive(receiver: &UdpSocket) -> Vec<u8> {
    let mut bytes = [0; 1024];
    let length = receiver.recv(&mut bytes).unwrap();
    let artnet_protocol::ArtCommand::Output(frame) =
        artnet_protocol::ArtCommand::from_buffer(&bytes[..length]).unwrap()
    else {
        panic!("expected ArtDMX")
    };
    frame.data.as_ref().to_vec()
}

#[test]
fn full_universe_commands_are_admitted_and_large_callbacks_bypass_command_validation() {
    use crate::app::module_command::{ModuleCommandRequestEvent, MODULE_COMMAND_REQUEST_TOPIC};
    use golden_core::events::CustomEvent;
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    let request = ModuleCommandRequestEvent {
        module_id: id,
        module_type: "artnet_module".into(),
        module_label: "Art-Net".into(),
        command_id: id,
        command_type: crate::app::module_modules_protocol_dmx_commands::DMX_SEND_FRAME_COMMAND_NODE_TYPE.into(),
        command_label: "Send DMX Frame".into(),
        payload: serde_json::to_value(DmxCommandRequest::SendFrame { slots: vec![5; 512] }).unwrap(),
    };
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    base(&mut engine, id).on_custom_event_inner(
        &mut ctx,
        CustomEvent::new(
            MODULE_COMMAND_REQUEST_TOPIC,
            Some(id),
            serde_json::to_value(request).unwrap(),
        ),
    );
    assert_eq!(receive(&receiver), vec![5; 512]);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let callback = crate::app::module::script_api::script_callback_event(
        id,
        "dmxFrameReceived",
        vec![serde_json::json!(1), serde_json::json!(vec![5; 512])],
    );
    base(&mut engine, id).on_custom_event_inner(&mut ctx, callback);
    assert!(ctx.edits.pending.is_empty());
    base(&mut engine, id).stop_transport();
}

#[test]
fn connected_idle_service_is_passive_and_script_output_reports_a_native_outcome() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    for _ in 0..100 {
        turn(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_rebuilds, 0);
    }
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("setChannel", vec![ParamValue::Int(1), ParamValue::Int(42)]).unwrap(),
    });
    engine.apply_edits().unwrap();
    assert_eq!(receive(&receiver)[0], 42);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !base(&mut engine, id).pending_sends.is_empty() {
        turn(&mut engine);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    base(&mut engine, id).stop_transport();
}

#[test]
fn same_value_internal_echo_does_not_swallow_the_next_authored_frame_change() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    for _ in 0..2 {
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: id,
            call: ScriptMethodCall::prepare("sendFrame", vec![ParamValue::Str("[7]".into())]).unwrap(),
        });
        engine.apply_edits().unwrap();
        assert_eq!(receive(&receiver)[0], 7);
        turn(&mut engine);
    }
    let output = base(&mut engine, id).output_frame.id();
    set(&mut engine, output, ParamValue::Str("[9]".into()));
    turn(&mut engine);
    assert_eq!(receive(&receiver)[0], 9);
    base(&mut engine, id).stop_transport();
}

#[test]
fn refused_command_has_no_persisted_frame_side_effect_and_disable_retires_immediately() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(base(&mut engine, id).transport.is_none());
    let time = engine.time;
    let node = base(&mut engine, id);
    let before = node.output_slots.clone();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    assert!(node.apply_command(&mut ctx, DmxCommandRequest::Blackout).is_err());
    assert_eq!(node.output_slots, before);
    assert!(ctx.edits.pending.is_empty());
}

#[test]
fn endpoint_text_and_script_json_are_bounded_before_parsing_or_copying() {
    let mut node = DmxModuleBase::create(DmxProtocol::ArtNet);
    assert!(super::super::parse_slots_json(&" ".repeat(8193)).is_err());
    assert!(script_send_frame(&[ParamValue::Str("0".repeat(1024 * 1024))]).is_err());
    let engine = crate::app::AppEngine::new(Folder::new("clock").into());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(node
        .apply_command(&mut ctx, DmxCommandRequest::SendFrame { slots: vec![1; 513] })
        .is_err());
    assert!(ctx.edits.pending.is_empty());
    assert!(DmxFrame::with_metadata(1, 0, 100, vec![1; 512]).is_ok());
}
