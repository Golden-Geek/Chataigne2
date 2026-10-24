use super::*;
use crate::app::module_modules_protocol_dmx::tests::service::fixture_protocol;
use std::net::UdpSocket;

fn packet(socket: &UdpSocket) -> (u8, Vec<u8>, u8) {
    let mut bytes = [0; 2048];
    let length = socket.recv(&mut bytes).unwrap();
    let sacn::packet::Payload::Data(data) = sacn::Packet::parse(&bytes[..length]).unwrap().payload else {
        panic!("expected sACN data");
    };
    assert!(!data.stream_terminated);
    (data.priority, data.values.to_vec(), data.sequence_number.get())
}

fn priority(engine: &mut AppEngine, id: NodeId, value: i32) {
    let param = base(engine, id).priority.id();
    engine.edits.push(Edit::SetParam {
        node: param,
        value: ParamValue::Int(value),
        behaviour: ParameterEventBehaviour::Append,
    });
}

fn execute(engine: &mut AppEngine, command: NodeId) {
    let execution = ModuleCommandExecuteEvent {
        command_id: command,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![],
    };
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(execution).unwrap(),
        ),
    });
}

#[test]
fn priority_controls_properties_scripts_and_commands_retain_occurrence_order() {
    let socket = receiver();
    let (mut engine, id) = fixture_protocol(&socket, DmxProtocol::Sacn);
    let command = command(&mut engine, id);
    let slots = child(&engine, command, "slots");
    set(&mut engine, slots, ParamValue::Bytes(vec![44, 45]));
    let output = base(&mut engine, id).output_frame.id();
    execute(&mut engine, command);
    engine.edits.push(Edit::SetParam {
        node: output,
        value: ParamValue::Bytes(vec![11, 12]),
        behaviour: ParameterEventBehaviour::Append,
    });
    priority(&mut engine, id, 0);
    script(&mut engine, id, "sendFrame", vec![ParamValue::Bytes(vec![22, 23])]);
    priority(&mut engine, id, 200);
    engine.edits.push(Edit::SetParam {
        node: output,
        value: ParamValue::Bytes(vec![33, 34]),
        behaviour: ParameterEventBehaviour::Append,
    });
    execute(&mut engine, command);
    priority(&mut engine, id, 50);
    engine.apply_edits().unwrap();
    settle(&mut engine, id);
    let mut last = None;
    for expected in [
        (100, vec![44, 45]),
        (100, vec![11, 12]),
        (0, vec![11, 12]),
        (0, vec![22, 23]),
        (200, vec![22, 23]),
        (200, vec![33, 34]),
        (200, vec![44, 45]),
        (50, vec![44, 45]),
    ] {
        let (priority, values, sequence) = packet(&socket);
        assert_eq!((priority, values), expected);
        if let Some(previous) = last {
            assert_eq!(sequence, previous + 1);
        }
        last = Some(sequence);
    }
    assert_eq!(base(&mut engine, id).output_frame.get_ref().as_slice(), &[44, 45]);
    base(&mut engine, id).stop_transport();
}

#[test]
fn static_priority_control_uses_confirmed_channels_and_does_not_reopen_the_socket() {
    let socket = receiver();
    let (mut engine, id) = fixture_protocol(&socket, DmxProtocol::Sacn);
    script(&mut engine, id, "sendFrame", vec![ParamValue::Bytes(vec![9, 8])]);
    engine.apply_edits().unwrap();
    settle(&mut engine, id);
    assert_eq!(packet(&socket).0, 100);
    let param = base(&mut engine, id).priority.id();
    // Only channel reflection is an internal echo. Module-attributed priority
    // changes must still drive the native stream.
    engine.edits.push_from(
        id,
        Edit::SetParam {
            node: param,
            value: ParamValue::Int(180),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    engine.apply_edits().unwrap();
    settle(&mut engine, id);
    // Ignore any pre-suppression repeat emitted before the control was captured.
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (priority, values, _) = packet(&socket);
        assert_eq!(values, vec![9, 8]);
        if priority == 180 {
            break;
        }
        assert_eq!(priority, 100);
        assert!(Instant::now() < deadline, "priority control did not reach the wire");
    }
    assert!(!base(&mut engine, id).transport_dirty);
    let (priority, values, _) = packet(&socket);
    assert_eq!(
        priority, 180,
        "the native source retains the new metadata for its next refresh"
    );
    assert_eq!(values, vec![9, 8]);
    assert!(base(&mut engine, id).transport.as_ref().unwrap().is_ready());
    assert_eq!(base(&mut engine, id).output_frame.get_ref().as_slice(), &[9, 8]);
    base(&mut engine, id).stop_transport();
}

#[test]
fn priority_without_a_confirmed_look_stays_silent_and_does_not_consume_initialization() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let socket = receiver();
        let (mut engine, id) = fixture_protocol(&socket, protocol);
        priority(&mut engine, id, 0);
        engine.apply_edits().unwrap();
        settle(&mut engine, id);
        assert!(base(&mut engine, id).output_slots.is_empty());
        assert!(base(&mut engine, id).output_frame.get_ref().as_slice().is_empty());
        socket.set_read_timeout(Some(Duration::from_millis(30))).unwrap();
        assert!(
            socket.recv(&mut [0; 2048]).is_err(),
            "priority alone cannot create a look"
        );
        script(&mut engine, id, "sendFrame", vec![ParamValue::Bytes(vec![7, 6])]);
        engine.apply_edits().unwrap();
        settle(&mut engine, id);
        socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        if protocol == DmxProtocol::Sacn {
            let (priority, values, _) = packet(&socket);
            assert_eq!(priority, 0);
            assert_eq!(values, vec![7, 6]);
        } else {
            assert_eq!(receive(&socket), vec![7, 6]);
        }
        base(&mut engine, id).stop_transport();
    }
}
