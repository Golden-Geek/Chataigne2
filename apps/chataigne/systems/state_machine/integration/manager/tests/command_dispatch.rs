use super::*;

#[test]
fn output_target_param_write_skips_unchanged_non_trigger_values() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(
        Parameter::new("Target", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("target parameter should attach");
    let target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Target")
        .map(|(id, _)| id)
        .expect("target parameter should exist");
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    ctx.set_tree_snapshot(snapshot.clone());
    let mut live_param_values = HashMap::new();

    assert!(!set_output_target_param(
        &mut ctx,
        snapshot.as_ref(),
        &mut live_param_values,
        target,
        ParamValue::Float(1.0)
    ));
    assert!(ctx.edits.pending.is_empty());
    assert!(set_output_target_param(
        &mut ctx,
        snapshot.as_ref(),
        &mut live_param_values,
        target,
        ParamValue::Trigger()
    ));
    assert_eq!(ctx.edits.pending.len(), 1);
}

#[test]
fn command_dispatch_plan_cache_reuses_resolved_targets() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(
        Parameter::new("Target", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("target parameter should attach");
    let target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Target")
        .map(|(id, _)| id)
        .expect("target parameter should exist");
    let snapshot = engine.process_tree_snapshot();
    let target_ref = StableRef::new(
        ValueTypeId::new("test"),
        snapshot
            .node(target)
            .expect("target parameter should be in the snapshot")
            .uuid
            .0
            .to_string(),
    );
    let mut cache = RuntimeCommandDispatchPlanCache::default();
    let first = {
        let plan = cache.plan_for(snapshot.as_ref(), target, &target_ref);
        assert!(matches!(
            plan.actions.as_slice(),
            [RuntimeCommandDispatchAction::Param(node)] if *node == target
        ));
        plan as *const _
    };
    let second = cache.plan_for(snapshot.as_ref(), target, &target_ref) as *const _;

    assert_eq!(first, second, "the same stable target should reuse its resolved plan");
    assert_eq!(cache.plans.len(), 1);
}

#[test]
fn external_command_plan_dependency_survives_mutation_and_target_removal() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(Folder::new("Processor Scope").into(), None);
    engine.add_node(Folder::new("External Module").into(), None);
    engine.apply_edits().expect("top-level scopes should attach");
    let processor_scope = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Processor Scope")
        .map(|(id, _)| id)
        .expect("processor scope should exist");
    let external_module = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "External Module")
        .map(|(id, _)| id)
        .expect("external module should exist");
    engine.add_node(
        Parameter::new("Target", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        Some(external_module),
    );
    engine.add_node(
        Parameter::new("Unrelated", ParamValue::Float(2.0), ParameterChangeCheck::ValueChange).into(),
        Some(external_module),
    );
    engine.apply_edits().expect("external parameters should attach");
    let target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Target")
        .map(|(id, _)| id)
        .expect("external target should exist");
    let unrelated = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Unrelated")
        .map(|(id, _)| id)
        .expect("unrelated sibling should exist");
    let previous = engine.process_tree_snapshot();
    let target_ref = StableRef::new(
        ValueTypeId::new("test"),
        previous
            .node(target)
            .expect("external target should be in the snapshot")
            .uuid
            .0
            .to_string(),
    );
    let mut cache = RuntimeCommandDispatchPlanCache::default();

    cache.plan_for(previous.as_ref(), processor_scope, &target_ref);

    let dependency = cache
        .dependencies
        .get(&target_ref)
        .expect("an external direct target should register a dependency");
    assert_eq!(dependency.root, target);
    assert_eq!(dependency.parent, Some(external_module));
    assert!(
        cache.observes_change(Some(previous.as_ref()), unrelated),
        "the depth-one parent listener observes sibling parameter events"
    );
    assert!(
        !cache.depends_on_change(Some(previous.as_ref()), None, unrelated),
        "an unrelated sibling must not invalidate the target plan"
    );
    assert!(
        !runtime_param_change_requires_snapshot(false, false, true),
        "an event known to come only from a command parent listener must stay snapshot-free"
    );
    if cache.depends_on_change(Some(previous.as_ref()), None, unrelated) {
        cache.invalidate_plans();
    }
    assert_eq!(
        cache.plans.len(),
        1,
        "an unrelated sibling parameter must leave the cached target plan intact"
    );
    let cached_plan = cache
        .plans
        .get(&target_ref)
        .expect("the target plan should still be cached") as *const _;
    for _ in 0..128 {
        let requires_snapshot = runtime_param_change_requires_snapshot(
            false,
            false,
            cache.observes_change(Some(previous.as_ref()), target),
        );
        assert!(
            !requires_snapshot,
            "value-only writes to the resolved target must stay snapshot-free"
        );
        if requires_snapshot {
            cache.invalidate_plans();
        }
        assert_eq!(
            cache
                .plans
                .get(&target_ref)
                .expect("value-only writes must not rebuild the target plan") as *const _,
            cached_plan
        );
    }
    assert!(
        cache.depends_on_change(Some(previous.as_ref()), None, target),
        "target mutations must invalidate the cached plan"
    );
    cache.invalidate_plans();
    assert!(cache.plans.is_empty());
    assert!(
        cache.dependencies.contains_key(&target_ref),
        "the dependency anchor must survive invalidation so target recreation stays observable"
    );

    engine.edits.push(Edit::RemoveNode { node: target });
    engine.apply_edits().expect("external target should be removable");
    let current = engine.process_tree_snapshot();

    assert!(current.node(target).is_none());
    assert!(
        cache.depends_on_change(Some(current.as_ref()), Some(previous.as_ref()), target),
        "target removal must resolve through the previous runtime snapshot"
    );
}

#[test]
fn external_target_dispatch_uses_live_value_without_rebuilding_snapshot() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(
        Parameter::new("Target", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("target parameter should attach");
    let target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Target")
        .map(|(id, _)| id)
        .expect("target parameter should exist");
    let snapshot = engine.process_tree_snapshot();
    let target_ref = StableRef::new(
        ValueTypeId::new("test"),
        snapshot
            .node(target)
            .expect("target parameter should be in the snapshot")
            .uuid
            .0
            .to_string(),
    );
    let intent = RuntimeIntent {
        kind: chataigne_state_machine::COMMAND_INTENT_KIND.into(),
        source_node: None,
        source_socket: None,
        target: Some(target_ref),
        payload: RuntimeValue::Float(1.0),
        logical_tick: 1,
    };
    let plan = RuntimeCommandDispatchPlan {
        actions: vec![RuntimeCommandDispatchAction::Param(target)],
        manager_with_children: false,
        truncated_actions: 0,
    };
    let mut live_param_values = HashMap::from([(target, ParamValue::Float(2.0))]);
    let provider = SnapshotProcessorContextProvider::default();
    let mut pending_batch = PendingRuntimeCommandBatch::default();
    let mut command_budget = RuntimeCommandTickBudget::default();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    dispatch_command_intent(
        &mut ctx,
        snapshot.as_ref(),
        RuntimeCommandDispatch {
            processor_node: target,
            processor_id: ProcessorId::new(),
            context_key: None,
            context_provider: &provider,
            live_param_values: &mut live_param_values,
            invocation_id: crate::app::module_command::ModuleCommandInvocationId::new(target, 1),
            intent: &intent,
            plan: &plan,
            pending_batch: &mut pending_batch,
        },
        &mut command_budget,
    );

    assert_eq!(ctx.edits.pending.len(), 1);
    assert!(matches!(
        ctx.edits.pending.last().map(|request| &request.edit),
        Some(Edit::SetParam {
            node,
            value: ParamValue::Float(value),
            ..
        }) if *node == target && *value == 1.0
    ));
    assert_eq!(live_param_values.get(&target), Some(&ParamValue::Float(1.0)));
}

#[test]
fn external_target_dispatch_rejects_non_finite_payload_without_poisoning_state() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(
        Parameter::new("Target", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("target parameter should attach");
    let target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Target")
        .map(|(id, _)| id)
        .expect("target parameter should exist");
    let snapshot = engine.process_tree_snapshot();
    let target_ref = StableRef::new(
        ValueTypeId::new("test"),
        snapshot.node(target).unwrap().uuid.0.to_string(),
    );
    let intent = RuntimeIntent {
        kind: chataigne_state_machine::COMMAND_INTENT_KIND.into(),
        source_node: None,
        source_socket: None,
        target: Some(target_ref),
        payload: RuntimeValue::Float(f64::NAN),
        logical_tick: 1,
    };
    let plan = RuntimeCommandDispatchPlan {
        actions: vec![RuntimeCommandDispatchAction::Param(target)],
        manager_with_children: false,
        truncated_actions: 0,
    };
    let mut live_param_values = HashMap::from([(target, ParamValue::Float(1.0))]);
    let provider = SnapshotProcessorContextProvider::default();
    let mut pending_batch = PendingRuntimeCommandBatch::default();
    let mut command_budget = RuntimeCommandTickBudget::default();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    dispatch_command_intent(
        &mut ctx,
        snapshot.as_ref(),
        RuntimeCommandDispatch {
            processor_node: target,
            processor_id: ProcessorId::new(),
            context_key: None,
            context_provider: &provider,
            live_param_values: &mut live_param_values,
            invocation_id: crate::app::module_command::ModuleCommandInvocationId::new(target, 1),
            intent: &intent,
            plan: &plan,
            pending_batch: &mut pending_batch,
        },
        &mut command_budget,
    );

    assert!(ctx.edits.pending.is_empty());
    assert_eq!(live_param_values.get(&target), Some(&ParamValue::Float(1.0)));
    let (rejected, error) = pending_batch.take_emission_issue();
    assert_eq!(rejected, 1);
    assert!(error.is_some(), "the rejected lane must remain observable");
}

#[test]
fn command_dispatch_caps_large_action_fanout_and_preserves_the_stable_prefix() {
    const OVERFLOW: usize = 17;
    let root: crate::app::AppNode = Folder::new("root").into();
    let engine = crate::app::AppEngine::new(root);
    let snapshot = engine.process_tree_snapshot();
    let processor_node = snapshot.root();
    let actions = (0..MAX_RUNTIME_COMMAND_ACTIONS_PER_TICK + OVERFLOW)
        .map(|index| RuntimeCommandDispatchAction::Command {
            node: NodeId(10_000 + index as u64),
            contextual_params: Vec::new(),
            batchable: false,
        })
        .collect();
    let plan = RuntimeCommandDispatchPlan {
        actions,
        manager_with_children: false,
        truncated_actions: 0,
    };
    let intent = RuntimeIntent {
        kind: chataigne_state_machine::COMMAND_INTENT_KIND.into(),
        source_node: None,
        source_socket: None,
        target: Some(StableRef::new(ValueTypeId::new("test"), "budget-target")),
        payload: RuntimeValue::Unit,
        logical_tick: 1,
    };
    let provider = SnapshotProcessorContextProvider::default();
    let mut live_param_values = HashMap::new();
    let mut pending_batch = PendingRuntimeCommandBatch::default();
    let mut command_budget = RuntimeCommandTickBudget::default();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    dispatch_command_intent(
        &mut ctx,
        snapshot.as_ref(),
        RuntimeCommandDispatch {
            processor_node,
            processor_id: ProcessorId::new(),
            context_key: None,
            context_provider: &provider,
            live_param_values: &mut live_param_values,
            invocation_id: crate::app::module_command::ModuleCommandInvocationId::new(processor_node, 1),
            intent: &intent,
            plan: &plan,
            pending_batch: &mut pending_batch,
        },
        &mut command_budget,
    );

    assert_eq!(ctx.edits.pending.len(), MAX_RUNTIME_COMMAND_ACTIONS_PER_TICK);
    for (index, request) in ctx.edits.pending.iter().enumerate() {
        let Edit::EmitCustomEvent { event } = &request.edit else {
            panic!("command dispatch should emit only transient command events");
        };
        assert_eq!(event.origin, Some(NodeId(10_000 + index as u64)));
    }
    assert_eq!(command_budget.remaining_actions(), 0);
    assert_eq!(command_budget.rejections().actions, OVERFLOW as u64);
    assert_eq!(command_budget.rejections().unresolved_intents, 0);

    command_budget.reject_unresolved_intent();
    assert_eq!(
        command_budget.rejections().unresolved_intents,
        1,
        "later intents must be counted rather than silently disappearing"
    );
}

#[test]
fn external_target_value_event_updates_overlay_without_invalidating_plan() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(Folder::new("Processor Scope").into(), None);
    engine.add_node(
        Parameter::new("Target", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("test nodes should attach");
    let processor_node = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Processor Scope")
        .map(|(id, _)| id)
        .expect("processor scope should exist");
    let target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Target")
        .map(|(id, _)| id)
        .expect("target parameter should exist");
    let snapshot = engine.process_tree_snapshot();
    let target_ref = StableRef::new(
        ValueTypeId::new("test"),
        snapshot.node(target).unwrap().uuid.0.to_string(),
    );
    let formula = continuous_formula();
    let processor = Processor::from_formula("Processor", &formula);
    let mut plans = RuntimeCommandDispatchPlanCache::default();
    plans.plan_for(snapshot.as_ref(), processor_node, &target_ref);
    let mut manager = StateMachineManager::new();
    manager.runtime_cache.topology_dirty = false;
    manager.runtime_cache.runtime_snapshot = Some(Arc::clone(&snapshot));
    manager.runtime_cache.processors.insert(
        processor_node,
        RuntimeProcessor {
            runtime: ProcessorRuntime::new(processor.id),
            processor,
            compile_warning: None,
            formula,
            formula_node: None,
            formula_ui: chataigne_state_machine::ProcessorFormulaUiState::project(),
            formula_source_key: "test".to_owned(),
            command_dispatch_plans: plans,
        },
    );
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    ctx.events.push_shared(Arc::new(golden_core::events::Event {
        time: ctx.time,
        kind: golden_core::events::EventKind::ParamChanged {
            param: target,
            old_value: ParamValue::Float(1.0),
            new_value: ParamValue::Float(2.0),
        },
    }));

    assert!(
        !manager.inbox_requires_tree_snapshot(&ctx.events),
        "value-only writes to a resolved external target must not clone the tree"
    );
    manager.on_inbox(&mut ctx);

    assert_eq!(
        manager.runtime_cache.command_listener_values.get(&target),
        Some(&ParamValue::Float(2.0))
    );
    assert_eq!(
        manager.runtime_cache.processors[&processor_node]
            .command_dispatch_plans
            .plans
            .len(),
        1,
        "value-only target changes must preserve the resolved dispatch plan"
    );
    assert!(!manager.runtime_cache.command_dispatch_snapshot_dirty);
}

#[test]
fn pending_command_batch_preserves_target_run_boundaries() {
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    let first_command = NodeId(41);
    let second_command = NodeId(42);
    let execution = |command_id| crate::app::module_command::ModuleCommandExecuteEvent {
        command_id,
        param_overrides: Vec::new(),
        invocation_id: None,
        delivery_policy: crate::app::module_command::ModuleCommandDeliveryPolicy::Standard,
    };
    let mut pending = PendingRuntimeCommandBatch::default();

    pending.push(&mut ctx, first_command, execution(first_command));
    pending.push(&mut ctx, first_command, execution(first_command));
    assert!(ctx.edits.pending.is_empty(), "one target run should remain buffered");

    pending.push(&mut ctx, second_command, execution(second_command));
    assert_eq!(
        ctx.edits.pending.len(),
        1,
        "switching targets should flush the preceding run"
    );
    pending.flush(&mut ctx);
    assert_eq!(ctx.edits.pending.len(), 2);
    assert_eq!(
        pending.take_emission_counts(),
        (2, 3),
        "emission counters must include target-switch and final flushes"
    );
}

#[test]
fn pending_command_batch_auto_flushes_at_wire_chunk_limit() {
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    let command = NodeId(41);
    let mut pending = PendingRuntimeCommandBatch::default();

    for stream in 0..crate::app::module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS {
        pending.push(
            &mut ctx,
            command,
            crate::app::module_command::ModuleCommandExecuteEvent {
                command_id: command,
                param_overrides: Vec::new(),
                invocation_id: Some(crate::app::module_command::ModuleCommandInvocationId::new(
                    command,
                    stream as u64,
                )),
                delivery_policy: crate::app::module_command::ModuleCommandDeliveryPolicy::Standard,
            },
        );
    }

    assert!(pending.command.is_none());
    assert!(pending.executions.is_empty());
    assert_eq!(
        pending.take_emission_counts(),
        (
            1,
            crate::app::module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS as u64,
        )
    );
}

#[test]
fn output_param_overrides_resolve_context_links_per_lane() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);

    engine.add_node(Folder::new("Command").into(), None);
    engine.apply_edits().expect("command container should attach");
    let command_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Command")
        .map(|(id, _)| id)
        .expect("command container should exist");
    let axis = ContextAxisId::new("lane");

    let mut message = Parameter::new(
        "Message",
        ParamValue::Str("original".to_owned()),
        ParameterChangeCheck::ValueChange,
    );
    message.node_data_mut().meta.decl_id = DeclId("message".to_owned());
    message
        .engine_set_param_control_state(ParameterControlState::new(
            ParameterControlMode::ContextLink,
            ParameterControlSpec::ContextLink {
                symbol: "msg".to_owned(),
                projection: None,
            },
        ))
        .expect("context link should be valid for string parameter");
    engine.add_node(message.into(), Some(command_id));
    let mut lane_number = Parameter::new(
        "Lane Number",
        ParamValue::Float(-1.0),
        ParameterChangeCheck::ValueChange,
    );
    lane_number.node_data_mut().meta.decl_id = DeclId("lane_number".to_owned());
    lane_number
        .engine_set_param_control_state(ParameterControlState::new(
            ParameterControlMode::ContextLink,
            ParameterControlSpec::ContextLink {
                symbol: golden_core::contexts::multiplex_index_context_link_symbol("lane", false),
                projection: None,
            },
        ))
        .expect("multiplex index link should be valid for a float parameter");
    engine.add_node(lane_number.into(), Some(command_id));
    let mut template = Parameter::new(
        "Template",
        ParamValue::Str("Lane {list:msg}".to_owned()),
        ParameterChangeCheck::ValueChange,
    );
    template.node_data_mut().meta.decl_id = DeclId("template".to_owned());
    template
        .engine_set_param_control_state(ParameterControlState::new(
            ParameterControlMode::TemplateText,
            ParameterControlSpec::TemplateText {
                template: "Lane {list:msg}".to_owned(),
            },
        ))
        .expect("template text should be valid for a string parameter");
    engine.add_node(template.into(), Some(command_id));
    engine.apply_edits().expect("output parameters should attach");

    let snapshot = engine.process_tree_snapshot();
    let message_id = snapshot
        .find_child_by_decl_id(command_id, "message")
        .expect("message parameter should exist");
    let lane_number_id = snapshot
        .find_child_by_decl_id(command_id, "lane_number")
        .expect("lane number parameter should exist");
    let template_id = snapshot
        .find_child_by_decl_id(command_id, "template")
        .expect("template parameter should exist");
    let processor_id = ProcessorId::new();
    let left_item = ContextItemId::new("left");
    let right_item = ContextItemId::new("right");
    let provider = context_provider(
        processor_id,
        context_runtime(
            vec![context_axis(
                axis.clone(),
                "Lane",
                vec![left_item.clone(), right_item.clone()],
            )],
            vec![context_list(
                axis.clone(),
                "msg",
                "messages",
                [
                    (left_item, RuntimeValue::String("left lane".into())),
                    (right_item, RuntimeValue::String("right lane".into())),
                ],
            )],
        ),
    );

    let left_key = ContextKey::single("lane", "left");
    let left_resolver = LaneParamResolver {
        processor_id,
        context_key: &left_key,
        context_provider: &provider,
    };
    let left_overrides = resolved_output_param_overrides(&snapshot, command_id, Some(&left_resolver));
    assert_eq!(left_overrides.len(), 3);
    assert_eq!(
        &left_overrides
            .iter()
            .find(|override_value| override_value.param_id == message_id)
            .expect("message override should exist")
            .value,
        &ParamValue::Str("left lane".to_owned())
    );
    assert_eq!(
        &left_overrides
            .iter()
            .find(|override_value| override_value.param_id == lane_number_id)
            .expect("lane number override should exist")
            .value,
        &ParamValue::Float(1.0)
    );
    assert_eq!(
        &left_overrides
            .iter()
            .find(|override_value| override_value.param_id == template_id)
            .expect("template override should exist")
            .value,
        &ParamValue::Str("Lane left lane".to_owned())
    );

    let right_key = ContextKey::single("lane", "right");
    let right_resolver = LaneParamResolver {
        processor_id,
        context_key: &right_key,
        context_provider: &provider,
    };
    let right_overrides = resolved_output_param_overrides(&snapshot, command_id, Some(&right_resolver));
    assert_eq!(right_overrides.len(), 3);
    assert_eq!(
        &right_overrides
            .iter()
            .find(|override_value| override_value.param_id == message_id)
            .expect("message override should exist")
            .value,
        &ParamValue::Str("right lane".to_owned())
    );
    assert_eq!(
        &right_overrides
            .iter()
            .find(|override_value| override_value.param_id == lane_number_id)
            .expect("lane number override should exist")
            .value,
        &ParamValue::Float(2.0)
    );

    let mut parameter_previews = Vec::new();
    collect_processor_lane_parameter_inspection(&snapshot, command_id, Some(&right_resolver), &mut parameter_previews);
    let preview_by_node = parameter_previews
        .into_iter()
        .map(|preview| (preview.node_id, preview.value))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        preview_by_node.get(&snapshot.node(message_id).unwrap().uuid.0.to_string()),
        Some(&"right lane".to_owned())
    );
    assert_eq!(
        preview_by_node.get(&snapshot.node(lane_number_id).unwrap().uuid.0.to_string()),
        Some(&"2.000".to_owned())
    );
    assert_eq!(
        preview_by_node.get(&snapshot.node(template_id).unwrap().uuid.0.to_string()),
        Some(&"Lane right lane".to_owned())
    );
}
