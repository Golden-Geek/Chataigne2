use super::*;

#[test]
fn send_custom_message_command_sends_osc_packet_through_module_output() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);

    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    assert!(
        engine
            .nodes
            .get(command_tester_id)
            .and_then(|node| node.node_data().first_child)
            .is_none(),
        "command tester should start empty before commands are user-created"
    );

    let command_id = create_send_custom_message_command(&mut engine, command_tester_id);

    let command_child_labels = child_labels(&engine, command_id);
    assert!(
        !command_child_labels.iter().any(|label| label == "Command"),
        "command children should be flat; children were {command_child_labels:?}"
    );

    let arguments_id = find_path(&engine, command_id, "arguments")
        .unwrap_or_else(|| panic!("arguments folder should exist; command children were {command_child_labels:?}"));
    engine.add_user_item(
        Parameter::new("Int", ParamValue::Int(7), ParameterChangeCheck::ValueChange).into(),
        Some(arguments_id),
    );
    engine.add_user_item(
        Parameter::new("Vec2", ParamValue::Vec2(1.5, 2.5), ParameterChangeCheck::ValueChange).into(),
        Some(arguments_id),
    );
    engine.add_user_item(
        Parameter::new(
            "String",
            ParamValue::Str("hello".to_string()),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        Some(arguments_id),
    );

    let address_param = find_path(&engine, command_id, "address").expect("command address param should exist");
    let trigger_param = find_path(&engine, command_id, "trigger").expect("command trigger param should exist");
    let auto_trigger_param =
        find_path(&engine, command_id, "auto_trigger").expect("command auto-trigger param should exist");
    assert!(
        !engine
            .nodes
            .get(trigger_param)
            .expect("trigger param should exist")
            .node_data()
            .meta
            .presentation
            .show_in_inspector_content,
        "trigger should be hidden from inspector content so the command inspector can render it in the header"
    );
    assert_eq!(
        engine
            .nodes
            .get(auto_trigger_param)
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Bool(false)),
        "command tester auto-trigger should be available and disabled by default"
    );
    assert!(
        !engine
            .nodes
            .get(auto_trigger_param)
            .expect("auto-trigger param should exist")
            .node_data()
            .meta
            .presentation
            .show_in_inspector_content,
        "auto-trigger should be hidden from inspector content so the command inspector can render it in the header"
    );
    assert!(
        find_path(&engine, command_id, "last_result").is_none(),
        "commands should not expose a last result parameter"
    );

    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/custom-message".to_string()),
    );
    engine.apply_edits().expect("command setup edits should apply");

    engine.edits.push(Edit::SetParam {
        node: trigger_param,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("command trigger edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("command trigger should dispatch");
    engine
        .apply_edits()
        .expect("queued command request should apply through the engine");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("queued command request should dispatch to the module");
    engine
        .apply_edits()
        .expect("queued command request side effects should apply through the engine");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should let the transport process the queued command");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("custom OSC command should send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/test/custom-message");
            assert_eq!(
                message.args,
                vec![
                    OscType::Int(7),
                    OscType::Float(1.5),
                    OscType::Float(2.5),
                    OscType::String("hello".to_string()),
                ]
            );
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}

#[test]
fn execute_event_runs_osc_command_through_module_output() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    let command_id = create_send_custom_message_command(&mut engine, command_tester_id);

    let address_param = find_path(&engine, command_id, "address").expect("command address param should exist");
    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/execute-event".to_string()),
    );
    engine.apply_edits().expect("command setup edits should apply");

    // Fire the command the way the state-machine output dispatch does: a single
    // execute event targeting the command (not a trigger-param edit), so the path
    // works per-lane under multiplex.
    let execute_event = golden_core::events::CustomEvent::new(
        crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
        Some(command_id),
        serde_json::to_value(crate::app::module_command::ModuleCommandExecuteEvent {
            command_id,
            param_overrides: Vec::new(),
            invocation_id: None,
            delivery_policy: crate::app::module_command::ModuleCommandDeliveryPolicy::Standard,
        })
        .expect("execute event payload should serialize"),
    );
    engine.edits.push(Edit::EmitCustomEvent { event: execute_event });
    engine.apply_edits().expect("execute event edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("execute event should dispatch to the command");
    engine
        .apply_edits()
        .expect("queued command request should apply through the engine");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("queued command request should dispatch to the module");
    engine
        .apply_edits()
        .expect("queued command request side effects should apply through the engine");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should let the transport process the queued command");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("execute event should cause the command to send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/test/execute-event");
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}

#[test]
fn output_manager_creates_module_linked_osc_command() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);
    engine.add_node(OutputsManager::new().into(), None);
    engine.apply_edits().expect("outputs manager should attach");

    let output_manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == OutputsManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("outputs manager should exist");
    let module_label = engine
        .nodes
        .get(module_id)
        .expect("OSC module should exist")
        .node_data()
        .meta
        .label
        .clone();
    let catalog = engine.catalog_creatable_items(output_manager_id);
    assert!(
        catalog.iter().any(|item| {
            item.node_type == crate::app::systems_alchemist_generic_commands::GenericLogCommand::NODE_TYPE
                && item.menu_path == vec!["Generic".to_string()]
        }),
        "generic output commands should be grouped under Generic; catalog was {catalog:?}"
    );
    let command_item = catalog
        .into_iter()
        .find(|item| {
            item.node_type == crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE
                && item.menu_path.first() == Some(&module_label)
        })
        .expect("outputs manager should expose an OSC command under the OSC module");
    assert_eq!(
        command_item.item_kind,
        crate::app::module_command::MODULE_COMMAND_ITEM_KIND
    );

    let create_ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: output_manager_id,
        node_type: command_item.node_type.clone(),
        label: Some(command_item.label.clone()),
        initial_params: command_item.initial_params.into_iter().map(Into::into).collect(),
    });
    assert!(
        create_ack.success,
        "module-linked output command should be creatable: {create_ack:?}"
    );

    let command_id = find_direct_child_by_type(
        &engine,
        output_manager_id,
        crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
    )
    .expect("outputs manager should contain the created OSC command");
    let target_module = find_path(
        &engine,
        command_id,
        crate::app::module_command::MODULE_COMMAND_TARGET_MODULE_PATH,
    )
    .expect("output-created command should include a target module reference");
    let module_uuid = engine
        .nodes
        .get(module_id)
        .expect("OSC module should exist")
        .node_data()
        .meta
        .uuid;
    assert_eq!(
        engine
            .nodes
            .get(target_module)
            .and_then(|node| node.engine_param_snapshot())
            .and_then(|snapshot| match snapshot.value {
                ParamValue::Reference(reference) => Some(reference.uuid()),
                _ => None,
            }),
        Some(module_uuid)
    );

    let address_param = find_path(&engine, command_id, "address").expect("command address param should exist");
    let trigger_param = find_path(&engine, command_id, "trigger").expect("command trigger param should exist");
    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/output-manager".to_string()),
    );
    engine.apply_edits().expect("command setup edits should apply");

    engine.edits.push(Edit::SetParam {
        node: trigger_param,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("command trigger edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("command trigger should dispatch");
    engine
        .apply_edits()
        .expect("queued command request should apply through the engine");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("queued command request should dispatch to the linked module");
    engine
        .apply_edits()
        .expect("queued command side effects should apply through the engine");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should let the transport process the queued command");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("linked OSC command should send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/test/output-manager");
            assert!(message.args.is_empty());
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}

#[test]
fn auto_trigger_send_custom_message_command_sends_when_command_parameter_changes() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);

    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    let command_id = create_send_custom_message_command(&mut engine, command_tester_id);

    let address_param = find_path(&engine, command_id, "address").expect("command address param should exist");
    let auto_trigger_param =
        find_path(&engine, command_id, "auto_trigger").expect("command auto-trigger param should exist");

    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/before-auto-trigger".to_string()),
    );
    engine.apply_edits().expect("initial command setup edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("initial command setup event should dispatch");
    engine
        .apply_edits()
        .expect("initial command setup event should settle without sending");

    set_param(&mut engine, auto_trigger_param, ParamValue::Bool(true));
    engine.apply_edits().expect("auto-trigger edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("auto-trigger edit should dispatch");
    engine
        .apply_edits()
        .expect("auto-trigger edit should not emit a command request");

    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/auto-trigger".to_string()),
    );
    engine
        .apply_edits()
        .expect("auto-triggered command parameter edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("auto-triggered command parameter edit should dispatch");
    engine
        .apply_edits()
        .expect("queued auto-triggered command request should apply through the engine");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("queued auto-triggered command request should dispatch to the module");
    engine
        .apply_edits()
        .expect("queued auto-triggered command request side effects should apply through the engine");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should let the transport process the auto-triggered command");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("auto-triggered OSC command should send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/test/auto-trigger");
            assert!(message.args.is_empty());
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}

#[test]
fn changing_values_parameter_sends_osc_packet_through_module_output() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);

    let values_id = find_path(&engine, module_id, "values").expect("values folder should exist");

    // Enable auto_feedback to allow sending values
    let auto_feedback_id = find_path(&engine, module_id, "parameters/processing/auto_feedback")
        .expect("auto_feedback parameter should exist");
    set_param(&mut engine, auto_feedback_id, ParamValue::Bool(true));
    engine.apply_edits().expect("auto_feedback change should apply");

    engine.add_node(
        Parameter::new("Foo", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(values_id),
    );
    engine.apply_edits().expect("value node should be created");

    let value_id = find_child_by_key(&engine, values_id, "Foo").expect("value parameter should exist");
    set_param(&mut engine, value_id, ParamValue::Float(3.25));
    engine.apply_edits().expect("value change should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("value change should dispatch");
    engine.apply_edits().expect("outbound value edit queue should settle");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should flush outbound OSC values");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("changing a values parameter should send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/Foo");
            assert_eq!(message.args, vec![OscType::Float(3.25)]);
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}
