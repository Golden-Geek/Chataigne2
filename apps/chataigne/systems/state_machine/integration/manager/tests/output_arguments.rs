use super::*;

#[test]
fn command_arguments_are_typed_local_and_cached_only_after_acceptance() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(Folder::new("Command").into(), None);
    engine.add_node(
        Parameter::new("Outside", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let find = |label: &str, engine: &crate::app::AppEngine| {
        engine
            .nodes
            .iter()
            .find(|(_, node)| node.node_data().meta.label == label)
            .map(|(id, _)| id)
            .unwrap()
    };
    let command = find("Command", &engine);
    let outside = find("Outside", &engine);
    engine.add_node(
        Parameter::new("Gain", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(command),
    );
    engine.add_node(
        Parameter::new("Position", ParamValue::Vec3(0.0, 0.0, 0.0), ParameterChangeCheck::ValueChange).into(),
        Some(command),
    );
    engine.apply_edits().unwrap();
    let gain = find("Gain", &engine);
    let position = find("Position", &engine);
    let snapshot = engine.process_tree_snapshot();
    let reference = |node| StableRef::new(ValueTypeId::new("test"), snapshot.node(node).unwrap().uuid.0.to_string());
    let payload = |first_param| {
        chataigne_state_machine::CommandArgumentValues {
            value: RuntimeValue::Unit,
            arguments: vec![
                chataigne_state_machine::ResolvedCommandArgument {
                    parameter: reference(first_param),
                    value: RuntimeValue::Float(0.75),
                },
                chataigne_state_machine::ResolvedCommandArgument {
                    parameter: reference(position),
                    value: RuntimeValue::Vec3([1.0, 2.0, 3.0]),
                },
            ],
            send_policy: chataigne_state_machine::OutputSendPolicy::OnChange,
        }
        .into_runtime_value()
        .unwrap()
    };
    let intent = |first_param| RuntimeIntent {
        kind: chataigne_state_machine::COMMAND_INTENT_KIND.into(),
        source_node: None,
        source_socket: None,
        target: Some(reference(command)),
        payload: payload(first_param),
        logical_tick: 1,
    };
    let plan = RuntimeCommandDispatchPlan {
        actions: vec![RuntimeCommandDispatchAction::Command {
            node: command,
            contextual_params: Vec::new(),
            batchable: false,
        }],
        manager_with_children: false,
        truncated_actions: 0,
    };
    let provider = SnapshotProcessorContextProvider::default();
    let mut live = HashMap::new();
    let mut pending = PendingRuntimeCommandBatch::default();
    let mut cache = OutputSendCache::default();
    let mut budget = RuntimeCommandTickBudget::default();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime { tick: 1, micro: 0, seq: 0 },
    );
    let invalid = intent(outside);
    let mut invalid_type = intent(gain);
    invalid_type.payload = chataigne_state_machine::CommandArgumentValues {
        value: RuntimeValue::Unit,
        arguments: vec![chataigne_state_machine::ResolvedCommandArgument {
            parameter: reference(gain),
            value: RuntimeValue::Vec3([1.0, 2.0, 3.0]),
        }],
        send_policy: chataigne_state_machine::OutputSendPolicy::OnChange,
    }
    .into_runtime_value()
    .unwrap();
    let valid = intent(gain);
    for (candidate, expected_events) in [(&invalid, 0), (&invalid_type, 0), (&valid, 1), (&valid, 1)] {
        dispatch_command_intent(
            &mut ctx,
            snapshot.as_ref(),
            RuntimeCommandDispatch {
                processor_node: command,
                processor_id: ProcessorId::new(),
                context_key: None,
                context_provider: &provider,
                live_param_values: &mut live,
                invocation_id: crate::app::module_command::ModuleCommandInvocationId::new(command, 1),
                intent: candidate,
                plan: &plan,
                pending_batch: &mut pending,
                send_cache: &mut cache,
            },
            &mut budget,
        );
        assert_eq!(ctx.edits.pending.len(), expected_events);
    }
    let (rejected, error) = pending.take_emission_issue();
    assert_eq!(rejected, 2);
    assert!(error.unwrap().contains("outside target command"));
    let Edit::EmitCustomEvent { event } = &ctx.edits.pending[0].edit else {
        panic!("expected command execution event");
    };
    let execution = event
        .payload_as::<crate::app::module_command::ModuleCommandExecuteEvent>()
        .unwrap();
    assert_eq!(execution.command_id, command);
    assert_eq!(execution.param_overrides.len(), 2);
    assert!(execution.param_overrides.iter().any(|item| item.param_id == gain && item.value == ParamValue::Float(0.75)));
    assert!(execution.param_overrides.iter().any(|item| item.param_id == position && item.value == ParamValue::Vec3(1.0, 2.0, 3.0)));
}

#[test]
fn change_aware_command_keeps_repeated_trigger_arguments() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(Folder::new("Command").into(), None);
    engine.apply_edits().unwrap();
    let command = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Command")
        .map(|(id, _)| id)
        .unwrap();
    engine.add_node(
        Parameter::new("Fire", ParamValue::Trigger(), ParameterChangeCheck::ValueChange).into(),
        Some(command),
    );
    engine.apply_edits().unwrap();
    let fire = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Fire")
        .map(|(id, _)| id)
        .unwrap();
    let snapshot = engine.process_tree_snapshot();
    let reference = |node| StableRef::new(ValueTypeId::new("test"), snapshot.node(node).unwrap().uuid.0.to_string());
    let intent = RuntimeIntent {
        kind: chataigne_state_machine::COMMAND_INTENT_KIND.into(),
        source_node: None,
        source_socket: None,
        target: Some(reference(command)),
        payload: chataigne_state_machine::CommandArgumentValues {
            value: RuntimeValue::Unit,
            arguments: vec![chataigne_state_machine::ResolvedCommandArgument {
                parameter: reference(fire),
                value: RuntimeValue::Trigger(TriggerValue::fired(1, 1)),
            }],
            send_policy: chataigne_state_machine::OutputSendPolicy::OnChange,
        }
        .into_runtime_value()
        .unwrap(),
        logical_tick: 1,
    };
    let plan = RuntimeCommandDispatchPlan {
        actions: vec![RuntimeCommandDispatchAction::Command {
            node: command,
            contextual_params: Vec::new(),
            batchable: false,
        }],
        manager_with_children: false,
        truncated_actions: 0,
    };
    let provider = SnapshotProcessorContextProvider::default();
    let mut live = HashMap::new();
    let mut pending = PendingRuntimeCommandBatch::default();
    let mut cache = OutputSendCache::default();
    let mut budget = RuntimeCommandTickBudget::default();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime { tick: 1, micro: 0, seq: 0 },
    );
    for expected in [1, 2] {
        dispatch_command_intent(
            &mut ctx,
            snapshot.as_ref(),
            RuntimeCommandDispatch {
                processor_node: command,
                processor_id: ProcessorId::new(),
                context_key: None,
                context_provider: &provider,
                live_param_values: &mut live,
                invocation_id: crate::app::module_command::ModuleCommandInvocationId::new(command, 1),
                intent: &intent,
                plan: &plan,
                pending_batch: &mut pending,
                send_cache: &mut cache,
            },
            &mut budget,
        );
        assert_eq!(ctx.edits.pending.len(), expected);
    }
    assert_eq!(pending.take_emission_issue().0, 0);
}

#[test]
fn disabled_and_missing_targets_reject_without_events() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let mut disabled = Folder::new("Disabled");
    disabled.node_data_mut().meta.enabled = false;
    engine.add_node(disabled.into(), None);
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let disabled = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Disabled")
        .map(|(id, _)| id)
        .unwrap();
    let targets = [
        StableRef::new(ValueTypeId::new("test"), snapshot.node(disabled).unwrap().uuid.0.to_string()),
        StableRef::new(ValueTypeId::new("test"), "missing-target"),
    ];
    let provider = SnapshotProcessorContextProvider::default();
    let mut plans = RuntimeCommandDispatchPlanCache::default();
    let mut live = HashMap::new();
    let mut pending = PendingRuntimeCommandBatch::default();
    let mut cache = OutputSendCache::default();
    let mut budget = RuntimeCommandTickBudget::default();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime { tick: 1, micro: 0, seq: 0 },
    );
    for target in targets {
        let intent = RuntimeIntent {
            kind: chataigne_state_machine::COMMAND_INTENT_KIND.into(),
            source_node: None,
            source_socket: None,
            target: Some(target.clone()),
            payload: RuntimeValue::Float(1.0),
            logical_tick: 1,
        };
        let plan = plans.plan_for(snapshot.as_ref(), snapshot.root(), &target);
        assert!(plan.actions.is_empty());
        dispatch_command_intent(
            &mut ctx,
            snapshot.as_ref(),
            RuntimeCommandDispatch {
                processor_node: snapshot.root(),
                processor_id: ProcessorId::new(),
                context_key: None,
                context_provider: &provider,
                live_param_values: &mut live,
                invocation_id: crate::app::module_command::ModuleCommandInvocationId::new(snapshot.root(), 1),
                intent: &intent,
                plan,
                pending_batch: &mut pending,
                send_cache: &mut cache,
            },
            &mut budget,
        );
    }
    assert!(ctx.edits.pending.is_empty());
    let (rejected, error) = pending.take_emission_issue();
    assert_eq!(rejected, 2);
    assert!(error.unwrap().contains("unavailable"));
}
