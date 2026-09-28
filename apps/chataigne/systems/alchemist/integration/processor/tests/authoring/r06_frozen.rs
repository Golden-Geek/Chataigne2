use super::*;

use golden_core::{
    edit::Edit,
    engine::EngineTime,
    process_ctx::{ExecutionPhase, ProcessCtx},
};

use super::super::super::frozen::{
    prepare_frozen_mapping_source, FrozenMappingSource,
    FROZEN_MAPPING_SOURCE_VERSION,
};

#[test]
fn frozen_source_rebuilds_mapping_and_executes_shared_command_without_workflow_node_ids() {
    let (mut engine, mapping_uuid, processor) = mapping_engine();
    let source_uuid = source_param(&mut engine, "Frozen source", 0.75);
    let sink_uuid = source_param(&mut engine, "Frozen sink", 0.0);
    let trigger_sink = Parameter::new(
        "Frozen trigger sink",
        ParamValue::Trigger(),
        ParameterChangeCheck::None,
    );
    let trigger_sink_uuid = trigger_sink.node_data().meta.uuid;
    engine.add_node(trigger_sink.into(), None);
    engine.apply_edits().unwrap();
    let inputs_manager = region(&engine, processor, "inputs");
    let input = create_item(
        &mut engine,
        inputs_manager,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(
        &mut engine,
        input,
        "source",
        ParamValue::Reference(NodeReference::new(source_uuid)),
    );
    let outputs_manager = region(&engine, processor, "outputs");
    let command = create_parameter_output(&mut engine, outputs_manager, sink_uuid);
    let command_uuid = engine.process_tree_snapshot().node(command).unwrap().uuid;
    let trigger_command = create_item(
        &mut engine,
        outputs_manager,
        crate::app::systems_alchemist_generic_commands::GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE,
    );
    set_direct_child_param(
        &mut engine,
        trigger_command,
        "target",
        ParamValue::Reference(NodeReference::new(trigger_sink_uuid)),
    );
    let trigger_command_uuid = engine
        .process_tree_snapshot()
        .node(trigger_command)
        .unwrap()
        .uuid;

    let frozen = prepare_frozen_mapping_source(&engine, processor)
        .expect("supported Mapping should prepare immutable source");
    assert_eq!(frozen.version, FROZEN_MAPPING_SOURCE_VERSION);
    assert_eq!(frozen.formula_source_uuid, mapping_uuid);
    assert_eq!(frozen.authored_regions.len(), 3);
    assert_eq!(frozen.commands.len(), 2);
    assert!(frozen.commands.iter().any(|command| command.uuid == command_uuid));
    assert!(
        frozen
            .commands
            .iter()
            .any(|command| command.uuid == trigger_command_uuid)
    );

    let encoded = serde_json::to_string(&frozen).expect("frozen source should serialize");
    assert!(
        !encoded.contains("cached_id"),
        "frozen source must not persist process-local NodeIds"
    );
    let decoded: FrozenMappingSource =
        serde_json::from_str(&encoded).expect("frozen source should deserialize");
    assert_eq!(decoded.version, frozen.version);
    assert_eq!(decoded.formula_source_uuid, frozen.formula_source_uuid);
    assert_eq!(decoded.formula.id, frozen.formula.id);
    assert_eq!(decoded.formula_instance.formula_ref, frozen.formula_instance.formula_ref);
    assert_eq!(decoded.authored_regions.len(), frozen.authored_regions.len());
    assert_eq!(decoded.commands, frozen.commands);

    let snapshot = engine.process_tree_snapshot();
    let formula_node = snapshot.node_id_by_uuid(mapping_uuid).unwrap();
    let formula = formula_from_snapshot(&snapshot, formula_node).unwrap();
    let mut live_instance = formula.instantiate();
    live_instance.managed_regions =
        managed_regions_from_snapshot(&snapshot, processor, &formula).unwrap();
    let compile = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&formula.properties),
    };
    let mut live = ManagedFormulaRuntime::compile(&formula, &live_instance, &compile)
        .unwrap()
        .unwrap();
    let mut compressed = decoded.compile_runtime().unwrap().unwrap();
    live.reconcile_input_source_schema(|source| managed_source_schema(&snapshot, source))
        .unwrap();
    compressed
        .reconcile_input_source_schema(|source| managed_source_schema(&snapshot, source))
        .unwrap();

    let input_instance = anode_from_snapshot(&snapshot, input).unwrap();
    let RuntimeValue::Ref(source_ref) = input_instance.config.get("source").unwrap() else {
        panic!("Mapping input should retain a stable source reference");
    };
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source_ref.clone(), RuntimeValue::Float(0.75));
    let registries = RuntimeRegistries {
        value_types: shared_value_type_registry(),
    };
    let evaluation = EvaluationCtx {
        logical_tick: 7,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let live_output = live.evaluate(&evaluation);
    let compressed_output = compressed.evaluate(&evaluation);
    assert_eq!(compressed_output.intents, live_output.intents);
    assert_eq!(compressed_output.diagnostics, live_output.diagnostics);
    assert_eq!(compressed_output.debug_samples, live_output.debug_samples);
    let command_stable_id = command_uuid.0.to_string();
    let trigger_command_stable_id = trigger_command_uuid.0.to_string();
    let intent = compressed_output
        .intents
        .iter()
        .find(|intent| {
            intent
                .target
                .as_ref()
                .is_some_and(|target| target.stable_id.as_ref() == command_stable_id)
        })
        .expect("frozen runtime should emit the concrete command intent");
    let trigger_intent = compressed_output
        .intents
        .iter()
        .find(|intent| {
            intent
                .target
                .as_ref()
                .is_some_and(|target| target.stable_id.as_ref() == trigger_command_stable_id)
        })
        .expect("frozen runtime should emit the trigger command intent");

    let prepared = decoded.command_for_intent(intent).unwrap();
    let mut process = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 7,
            micro: 0,
            seq: 0,
        },
    );
    process.set_tree_snapshot(snapshot.clone());
    prepared
        .execute_intent(&mut process, snapshot.as_ref(), intent)
        .expect("frozen command should use the shared generic executor");
    decoded
        .command_for_intent(trigger_intent)
        .unwrap()
        .execute_intent(&mut process, snapshot.as_ref(), trigger_intent)
        .expect("frozen trigger should use the shared generic executor");
    let sink = snapshot.node_id_by_uuid(sink_uuid).unwrap();
    let trigger_sink = snapshot.node_id_by_uuid(trigger_sink_uuid).unwrap();
    assert!(matches!(
        &process.edits.pending[0].edit,
        Edit::SetParam {
            node,
            value: ParamValue::Float(value),
            ..
        } if *node == sink && (*value - 0.75).abs() < f64::EPSILON
    ));
    assert!(matches!(
        &process.edits.pending[1].edit,
        Edit::SetParam {
            node,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Append,
        } if *node == trigger_sink
    ));
}

#[test]
fn frozen_preflight_reports_unsupported_commands_and_known_inbound_references() {
    let (mut unsupported, _, unsupported_processor) = mapping_engine();
    let outputs = region(&unsupported, unsupported_processor, "outputs");
    create_item(&mut unsupported, outputs, GENERIC_LOG_COMMAND_NODE_TYPE);
    let blockers = prepare_frozen_mapping_source(&unsupported, unsupported_processor)
        .expect_err("live-only command should block compression preparation");
    assert!(
        blockers.iter().any(|blocker| blocker.code == "unsupported_command"),
        "unexpected blockers: {blockers:?}"
    );

    let (mut scheduled, _, scheduled_processor) = mapping_engine();
    let outputs = region(&scheduled, scheduled_processor, "outputs");
    create_item(&mut scheduled, outputs, "sm_output_group");
    let blockers = prepare_frozen_mapping_source(&scheduled, scheduled_processor)
        .expect_err("node-bound group scheduling should block a representation transition");
    assert!(
        blockers
            .iter()
            .any(|blocker| blocker.code == "unsupported_command"),
        "unexpected scheduling blockers: {blockers:?}"
    );

    let (mut referenced, _, referenced_processor) = mapping_engine();
    let sink_uuid = source_param(&mut referenced, "Referenced sink", 0.0);
    let outputs = region(&referenced, referenced_processor, "outputs");
    let command = create_parameter_output(&mut referenced, outputs, sink_uuid);
    let value_uuid = referenced
        .process_tree_snapshot()
        .find_child_by_decl_id(command, "value")
        .and_then(|value| referenced.process_tree_snapshot().node(value).map(|node| node.uuid))
        .unwrap();
    referenced.add_node(
        Parameter::new(
            "External inbound reference",
            ParamValue::Reference(NodeReference::new(value_uuid)),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        None,
    );
    referenced.apply_edits().unwrap();
    let blockers = prepare_frozen_mapping_source(&referenced, referenced_processor)
        .expect_err("known inbound reference should block compression preparation");
    assert!(blockers.iter().any(|blocker| {
        blocker.code == "inbound_reference" && blocker.detail.contains(&value_uuid.0.to_string())
    }));
}
