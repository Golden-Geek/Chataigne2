use golden_core::{
    app::ProjectNode,
    edit::Edit,
    engine::EngineTime,
    events::{CustomEvent, CustomEventRetention},
    node::{Node, NodeId, NodeMeta},
    process_ctx::{ExecutionPhase, ProcessCtx},
};

use super::{
    emit_command_execute_batch, emit_command_execute_with_invocation, ModuleCommandDeliveryPolicy,
    ModuleCommandExecuteBatchEmission, ModuleCommandExecuteBatchEvent, ModuleCommandExecuteEvent,
    ModuleCommandInvocationId, ModuleCommandParamOverride, ModuleCommandTester,
    MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS, MODULE_COMMAND_EXECUTE_BATCH_TOPIC, MODULE_COMMAND_ITEM_KIND,
};

#[test]
fn internal_command_execute_events_are_transient_and_keep_invocation_identity() {
    let command = NodeId(20);
    let invocation_id = ModuleCommandInvocationId::new(NodeId(10), 7);
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    emit_command_execute_with_invocation(
        &mut ctx,
        command,
        Vec::new(),
        Some(invocation_id),
        ModuleCommandDeliveryPolicy::Standard,
    )
    .expect("command execute should serialize");

    let Edit::EmitCustomEvent { event } = &ctx
        .edits
        .pending
        .last()
        .expect("command execute should enqueue one custom event")
        .edit
    else {
        panic!("command execute should enqueue a custom event");
    };
    assert_eq!(event.retention, CustomEventRetention::Transient);
    let decoded = super::command_execute_request(event, command)
        .expect("transient event should retain its typed command payload");
    assert_eq!(decoded.invocation_id, Some(invocation_id));
}

#[test]
fn command_execute_batches_are_transient_ordered_and_target_checked() {
    let command = NodeId(20);
    let invocation_emitter = NodeId(10);
    let param = NodeId(30);
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    let executions = [("first", 7), ("second", 8), ("third", 9)]
        .into_iter()
        .map(|(value, stream)| ModuleCommandExecuteEvent {
            command_id: command,
            param_overrides: vec![ModuleCommandParamOverride {
                param_id: param,
                value: golden_core::parameter::ParamValue::Str(value.to_owned()),
            }],
            invocation_id: Some(ModuleCommandInvocationId::new(invocation_emitter, stream)),
            delivery_policy: ModuleCommandDeliveryPolicy::ChangeAwareLogAdmitted,
        })
        .collect::<Vec<_>>();

    let emission = emit_command_execute_batch(&mut ctx, command, executions.clone())
        .expect("command execute batch should serialize");
    assert_eq!(
        emission,
        ModuleCommandExecuteBatchEmission {
            event_count: 1,
            execution_count: executions.len(),
            rejected_execution_count: 0,
        }
    );

    let Edit::EmitCustomEvent { event } = &ctx
        .edits
        .pending
        .last()
        .expect("command execute batch should enqueue one custom event")
        .edit
    else {
        panic!("command execute batch should enqueue a custom event");
    };
    assert_eq!(ctx.edits.pending.len(), 1);
    assert_eq!(event.retention, CustomEventRetention::Transient);
    assert_eq!(super::command_execute_batch_requests(event, command), Some(executions));

    let wrong_target = ModuleCommandExecuteEvent {
        command_id: NodeId(99),
        param_overrides: Vec::new(),
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
    };
    assert!(
        emit_command_execute_batch(&mut ctx, command, vec![wrong_target]).is_err(),
        "routing and per-entry command ids must not diverge"
    );
    assert_eq!(ctx.edits.pending.len(), 1, "invalid batches must not emit partial work");
}

#[test]
fn command_execute_batches_are_chunked_without_reordering() {
    let command = NodeId(20);
    let emitter = NodeId(10);
    let execution_count = MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS * 2 + 17;
    let executions = (0..execution_count)
        .map(|stream| ModuleCommandExecuteEvent {
            command_id: command,
            param_overrides: Vec::new(),
            invocation_id: Some(ModuleCommandInvocationId::new(emitter, stream as u64)),
            delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        })
        .collect::<Vec<_>>();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    let emission = emit_command_execute_batch(&mut ctx, command, executions)
        .expect("large command execute batch should serialize in bounded chunks");

    assert_eq!(
        emission,
        ModuleCommandExecuteBatchEmission {
            event_count: 3,
            execution_count,
            rejected_execution_count: 0,
        }
    );
    assert_eq!(ctx.edits.pending.len(), 3);
    let mut decoded_streams = Vec::with_capacity(execution_count);
    let mut chunk_lengths = Vec::new();
    for pending in &ctx.edits.pending {
        let Edit::EmitCustomEvent { event } = &pending.edit else {
            panic!("command execute chunk should enqueue a custom event");
        };
        assert_eq!(event.retention, CustomEventRetention::Transient);
        let chunk = super::command_execute_batch_requests(event, command)
            .expect("each transient event should retain its typed command payload");
        assert!(
            chunk.len() <= MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS,
            "serialized command chunks must remain bounded"
        );
        chunk_lengths.push(chunk.len());
        decoded_streams.extend(chunk.into_iter().map(|execution| {
            execution
                .invocation_id
                .expect("test execution should retain its invocation")
                .stream
        }));
    }

    assert_eq!(
        chunk_lengths,
        vec![
            MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS,
            MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS,
            17,
        ]
    );
    assert_eq!(
        decoded_streams,
        (0..execution_count as u64).collect::<Vec<_>>(),
        "sequential chunks must preserve exact global execution order"
    );

    let mut invalid = (0..=MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS)
        .map(|stream| ModuleCommandExecuteEvent {
            command_id: command,
            param_overrides: Vec::new(),
            invocation_id: Some(ModuleCommandInvocationId::new(emitter, stream as u64)),
            delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        })
        .collect::<Vec<_>>();
    invalid
        .last_mut()
        .expect("invalid batch should contain a tail execution")
        .command_id = NodeId(99);
    let mut invalid_ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    assert!(
        emit_command_execute_batch(&mut invalid_ctx, command, invalid).is_err(),
        "target validation must inspect entries beyond the first chunk"
    );
    assert!(
        invalid_ctx.edits.pending.is_empty(),
        "target validation must complete before any chunk is emitted"
    );
}

#[test]
fn command_execute_batch_parser_rejects_oversized_inbound_payloads() {
    let command = NodeId(20);
    let execution = ModuleCommandExecuteEvent {
        command_id: command,
        param_overrides: Vec::new(),
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
    };
    let payload = ModuleCommandExecuteBatchEvent {
        command_id: command,
        executions: vec![execution; MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS + 1],
    };
    let event = CustomEvent::from_transient_payload(MODULE_COMMAND_EXECUTE_BATCH_TOPIC, Some(command), &payload)
        .expect("malicious oversized payload should still serialize");

    assert!(
        super::command_execute_batch_requests(&event, command).is_none(),
        "the parser boundary must not admit more work than one declared batch chunk"
    );
    assert!(!super::is_command_execute_batch_request(&event, command));
}

#[test]
fn command_execute_batches_isolate_non_finite_overrides_without_dropping_neighbors() {
    let command = NodeId(20);
    let emitter = NodeId(10);
    let param = NodeId(30);
    let execution = |stream: u64, value: f64| ModuleCommandExecuteEvent {
        command_id: command,
        param_overrides: vec![ModuleCommandParamOverride {
            param_id: param,
            value: golden_core::parameter::ParamValue::Float(value),
        }],
        invocation_id: Some(ModuleCommandInvocationId::new(emitter, stream)),
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
    };
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    let emission = emit_command_execute_batch(
        &mut ctx,
        command,
        vec![
            execution(1, 1.0),
            execution(2, f64::NAN),
            execution(3, 3.0),
            execution(4, f64::INFINITY),
            execution(5, 5.0),
        ],
    )
    .expect("valid neighbors should still be emitted");

    assert_eq!(
        emission,
        ModuleCommandExecuteBatchEmission {
            event_count: 1,
            execution_count: 3,
            rejected_execution_count: 2,
        }
    );
    let decoded_streams = ctx
        .edits
        .pending
        .iter()
        .flat_map(|pending| {
            let Edit::EmitCustomEvent { event } = &pending.edit else {
                panic!("command execute chunk should enqueue a custom event");
            };
            super::command_execute_batch_requests(event, command)
                .expect("every emitted chunk must round-trip")
                .into_iter()
        })
        .map(|execution| {
            execution
                .invocation_id
                .expect("test execution should retain its invocation")
                .stream
        })
        .collect::<Vec<_>>();
    assert_eq!(decoded_streams, vec![1, 3, 5]);

    let pending_before = ctx.edits.pending.len();
    assert!(
        emit_command_execute_with_invocation(
            &mut ctx,
            command,
            vec![ModuleCommandParamOverride {
                param_id: param,
                value: golden_core::parameter::ParamValue::Vec2(1.0, f64::NEG_INFINITY),
            }],
            Some(ModuleCommandInvocationId::new(emitter, 6)),
            ModuleCommandDeliveryPolicy::Standard,
        )
        .is_err(),
        "single executions must reject the same malformed JSON boundary"
    );
    assert_eq!(
        ctx.edits.pending.len(),
        pending_before,
        "a rejected single execution must not enqueue a malformed event"
    );
}

#[test]
fn module_command_tester_uses_advertised_command_catalog() {
    let tester =
        ModuleCommandTester::create(crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES);
    let items = tester.user_creatable_items();
    let item_types = items.iter().map(|item| item.node_type.as_str()).collect::<Vec<_>>();

    assert_eq!(
        items.len(),
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES.len()
    );
    assert!(items.iter().all(|item| item.item_kind == MODULE_COMMAND_ITEM_KIND));
    assert!(items.iter().all(|item| !item.select_when_created));
    assert!(items.iter().all(|item| {
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES.contains(&item.node_type.as_str())
    }));
    assert_eq!(
        item_types,
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES,
        "module command tester should preserve the advertised command order"
    );
    assert!(!items
        .iter()
        .any(|item| item.node_type == crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE));
    assert!(tester.user_container_accepts_item(
        crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
        MODULE_COMMAND_ITEM_KIND,
    ));
    assert!(!tester.user_container_accepts_item(
        crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
        MODULE_COMMAND_ITEM_KIND,
    ));

    let created = tester
        .create_user_item(crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE)
        .expect("advertised command should be creatable");
    assert_eq!(
        created.get_type(),
        crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
    );
    assert!(tester.create_user_item("folder").is_none());
}

#[test]
fn module_command_tester_uses_owned_command_catalog() {
    let tester =
        ModuleCommandTester::create_owned(vec![crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE.to_string()]);

    let item_types = tester
        .user_creatable_items()
        .into_iter()
        .map(|item| item.node_type)
        .collect::<Vec<_>>();
    assert_eq!(
        item_types,
        vec![crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE.to_string()]
    );
    assert!(!tester.user_container_accepts_item(
        crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
        MODULE_COMMAND_ITEM_KIND,
    ));
}

#[test]
fn empty_module_command_catalog_remains_empty() {
    let tester = ModuleCommandTester::create_owned(Vec::new());

    assert!(tester.user_creatable_items().is_empty());
    assert!(!tester.user_container_accepts_item(
        crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
        MODULE_COMMAND_ITEM_KIND,
    ));
}

#[test]
fn module_command_tester_decodes_from_project_node_type() {
    let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
        "module_command_tester",
        &serde_json::Value::Null,
        &NodeMeta::new("Command Tester".to_string()),
    )
    .expect("module command tester should decode from project files");

    assert_eq!(node.get_type(), "module_command_tester");
}

#[test]
fn decoded_module_command_tester_restores_persisted_command_catalog() {
    let tester =
        ModuleCommandTester::create(crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES);
    let data = tester
        .project_encode_data()
        .expect("module command tester should encode its advertised catalog");

    let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
        "module_command_tester",
        &data,
        &NodeMeta::new("Command Tester".to_string()),
    )
    .expect("module command tester should decode from project files");

    let items = node.user_creatable_items();
    let item_types = items.iter().map(|item| item.node_type.as_str()).collect::<Vec<_>>();

    assert_eq!(
        item_types,
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES,
        "decoded testers should restore their persisted advertised command order"
    );
    assert!(!items
        .iter()
        .any(|item| item.node_type == crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE));
    assert!(
        node.user_container_accepts_item(
            crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
            MODULE_COMMAND_ITEM_KIND,
        ),
        "decoded testers should accept persisted advertised command items"
    );
    assert!(
        !node.user_container_accepts_item(
            crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
            MODULE_COMMAND_ITEM_KIND,
        ),
        "decoded testers should reject commands outside their persisted advertised catalog"
    );
}

#[test]
fn decoded_legacy_module_command_tester_accepts_declared_module_commands_without_catalog_data() {
    let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
        "module_command_tester",
        &serde_json::Value::Null,
        &NodeMeta::new("Command Tester".to_string()),
    )
    .expect("module command tester should decode from project files");

    let items = node.user_creatable_items();
    assert!(
        items
            .iter()
            .any(|item| item.node_type == crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE),
        "legacy decoded testers should accept saved OSC command items without persisted catalog data"
    );
    assert!(
        items.iter().any(|item| {
            item.node_type == crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE
        }),
        "legacy decoded testers should accept saved streaming command items without persisted catalog data"
    );
    assert!(
        node.user_container_accepts_item(
            crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
            MODULE_COMMAND_ITEM_KIND,
        ),
        "legacy decoded testers should accept saved OSC command items without persisted catalog data"
    );
}
