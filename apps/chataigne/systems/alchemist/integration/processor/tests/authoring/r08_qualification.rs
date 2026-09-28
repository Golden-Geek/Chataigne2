use std::time::Instant;

use super::*;

use crate::app::systems_alchemist_processor::{
    processor_frozen_source, set_mapping_compression,
};
use crate::app::systems_state_machine_manager::StateMachineRuntimePerfStats;
use golden_core::engine::TickStats;

#[derive(Debug)]
struct RepresentationSample {
    name: &'static str,
    node_count: usize,
    project_bytes: usize,
    p50_ns: u64,
    p95_ns: u64,
    snapshot_builds: usize,
    snapshot_nodes_cloned: usize,
    evaluation_calls: u64,
    lanes_evaluated: u64,
    command_batches: u64,
    command_executions: u64,
    value_deliveries: usize,
    trigger_deliveries: usize,
    debug_samples: u64,
    formula_compiles: u64,
}

impl RepresentationSample {
    fn report(&self) {
        println!(
            "mapping_r08_representation name={} nodes={} project_bytes={} p50_ns={} p95_ns={} snapshot_builds={} snapshot_nodes_cloned={} evaluation_calls={} lanes={} command_batches={} command_executions={} value_deliveries={} trigger_deliveries={} debug_samples={} formula_compiles={}",
            self.name,
            self.node_count,
            self.project_bytes,
            self.p50_ns,
            self.p95_ns,
            self.snapshot_builds,
            self.snapshot_nodes_cloned,
            self.evaluation_calls,
            self.lanes_evaluated,
            self.command_batches,
            self.command_executions,
            self.value_deliveries,
            self.trigger_deliveries,
            self.debug_samples,
            self.formula_compiles,
        );
    }
}

fn runtime_stats(engine: &AppEngine) -> StateMachineRuntimePerfStats {
    engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats()),
            _ => None,
        })
        .expect("qualification Mapping should have an active StateMachineManager")
}

fn add_tick_stats(total: &mut TickStats, sample: TickStats) {
    total.snapshot_builds += sample.snapshot_builds;
    total.snapshot_nodes_cloned += sample.snapshot_nodes_cloned;
}

fn run_ticks(engine: &mut AppEngine, count: usize) {
    for _ in 0..count {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
}

fn sample_representation(
    engine: &mut AppEngine,
    name: &'static str,
    source: NodeId,
    value_sink: NodeId,
    trigger_sink: NodeId,
    samples: usize,
) -> RepresentationSample {
    run_ticks(engine, 4);
    engine.clear_ui_event_log();
    let before = runtime_stats(engine);
    let mut latencies = Vec::with_capacity(samples);
    let mut tick_total = TickStats::default();
    let mut value_deliveries = 0;
    let mut trigger_deliveries = 0;

    for index in 0..samples {
        let value = if index % 2 == 0 { 0.25 } else { 0.75 };
        let started = Instant::now();
        engine.edits.push(Edit::SetParam {
            node: source,
            value: ParamValue::Float(value),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..4 {
            engine.run_tick(Duration::from_millis(8)).unwrap();
            add_tick_stats(&mut tick_total, engine.tick_stats());
        }
        latencies.push(started.elapsed().as_nanos() as u64);
        assert_eq!(
            engine.process_tree_snapshot().node(value_sink).unwrap().param_value,
            Some(ParamValue::Float(value)),
        );
        value_deliveries += engine
            .ui_event_log()
            .iter()
            .filter(|event| {
                matches!(event.kind, golden_core::events::EventKind::ParamChanged { param, .. } if param == value_sink)
            })
            .count();
        trigger_deliveries += engine
            .ui_event_log()
            .iter()
            .filter(|event| {
                matches!(event.kind, golden_core::events::EventKind::ParamChanged { param, .. } if param == trigger_sink)
            })
            .count();
        engine.clear_ui_event_log();
    }

    latencies.sort_unstable();
    let after = runtime_stats(engine);
    let percentile = |percent: usize| latencies[(samples * percent).div_ceil(100) - 1];
    let project_bytes = golden_core::app::to_sparse_project_json_pretty(engine)
        .unwrap()
        .len();
    RepresentationSample {
        name,
        node_count: engine.nodes.len(),
        project_bytes,
        p50_ns: percentile(50),
        p95_ns: percentile(95),
        snapshot_builds: tick_total.snapshot_builds,
        snapshot_nodes_cloned: tick_total.snapshot_nodes_cloned,
        evaluation_calls: after.processor_evaluation_calls - before.processor_evaluation_calls,
        lanes_evaluated: after.processor_lanes_evaluated - before.processor_lanes_evaluated,
        command_batches: after.processor_command_batches - before.processor_command_batches,
        command_executions: after.processor_batched_executions - before.processor_batched_executions,
        value_deliveries,
        trigger_deliveries,
        debug_samples: after.debug_samples_captured - before.debug_samples_captured,
        formula_compiles: after.formula_compiles - before.formula_compiles,
    }
}

#[test]
#[ignore = "opt-in R08 expanded/compressed product qualification"]
fn expanded_and_compressed_mapping_report_the_same_real_workload() {
    let samples = std::env::var("CHATAIGNE_MAPPING_R08_SAMPLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100);
    assert!(samples >= 50, "qualification requires at least 50 samples");

    let construction_started = Instant::now();
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let source_uuid = source_param(&mut engine, "R08 source", 0.0);
    let value_sink_uuid = source_param(&mut engine, "R08 value sink", 0.0);
    let trigger_sink = Parameter::new(
        "R08 trigger sink",
        ParamValue::Trigger(),
        ParameterChangeCheck::None,
    );
    let trigger_sink_uuid = trigger_sink.node_data().meta.uuid;
    engine.add_node(trigger_sink.into(), None);
    engine.apply_edits().unwrap();

    let inputs = region(&engine, processor, "inputs");
    let input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(
        &mut engine,
        input,
        "source",
        ParamValue::Reference(NodeReference::new(source_uuid)),
    );
    let outputs = region(&engine, processor, "outputs");
    let value_output = create_parameter_output(&mut engine, outputs, value_sink_uuid);
    let value_bindings = OutputBindingConfig {
        send_policy: OutputSendPolicy::OnChange,
        ..OutputBindingConfig::default()
    };
    set_config(
        &mut engine,
        value_output,
        "bindings",
        ParamValue::Str(value_bindings.to_authoring_json().unwrap()),
    );
    let trigger_output = create_item(
        &mut engine,
        outputs,
        crate::app::systems_alchemist_generic_commands::GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE,
    );
    set_direct_child_param(
        &mut engine,
        trigger_output,
        "target",
        ParamValue::Reference(NodeReference::new(trigger_sink_uuid)),
    );
    let construction_ms = construction_started.elapsed().as_millis();

    run_ticks(&mut engine, 12);
    let snapshot = engine.process_tree_snapshot();
    let source = snapshot.node_id_by_uuid(source_uuid).unwrap();
    let value_sink = snapshot.node_id_by_uuid(value_sink_uuid).unwrap();
    let trigger_sink = snapshot.node_id_by_uuid(trigger_sink_uuid).unwrap();
    drop(snapshot);

    let expanded = sample_representation(
        &mut engine,
        "expanded",
        source,
        value_sink,
        trigger_sink,
        samples,
    );
    let transition_started = Instant::now();
    set_mapping_compression(&mut engine, processor, true).unwrap();
    let compress_ms = transition_started.elapsed().as_millis();
    let frozen_bytes = processor_frozen_source(&engine.process_tree_snapshot(), processor)
        .unwrap()
        .map(|source| serde_json::to_vec(&source).unwrap().len())
        .unwrap();
    let compressed = sample_representation(
        &mut engine,
        "compressed",
        source,
        value_sink,
        trigger_sink,
        samples,
    );

    assert!(compressed.node_count < expanded.node_count);
    for result in [&expanded, &compressed] {
        result.report();
        assert_eq!(result.evaluation_calls, samples as u64);
        assert_eq!(result.lanes_evaluated, samples as u64);
        assert_eq!(result.value_deliveries, samples);
        assert_eq!(result.trigger_deliveries, samples);
        assert_eq!(result.debug_samples, 0);
        assert_eq!(result.formula_compiles, 0);
    }
    // These counters describe transport batches emitted toward live command
    // nodes. Frozen commands execute their shared prepared implementation
    // directly, so exact dispatch equivalence is asserted from sink events.
    assert_eq!(expanded.command_executions, samples as u64);
    assert_eq!(expanded.command_batches, samples as u64);
    assert_eq!(compressed.command_executions, 0);
    assert_eq!(compressed.command_batches, 0);

    let expansion_started = Instant::now();
    set_mapping_compression(&mut engine, processor, false).unwrap();
    let expand_ms = expansion_started.elapsed().as_millis();
    assert_eq!(engine.nodes.len(), expanded.node_count);
    println!(
        "mapping_r08_summary revision={} samples={} construction_ms={} compress_ms={} expand_ms={} frozen_bytes={}",
        env!("CARGO_PKG_VERSION"),
        samples,
        construction_ms,
        compress_ms,
        expand_ms,
        frozen_bytes,
    );
}

#[test]
fn compressed_archive_rejects_script_and_headless_writes() {
    let (mut engine, _, processor) = mapping_engine();
    set_mapping_compression(&mut engine, processor, true).unwrap();
    let frozen = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(
            processor,
            crate::app::systems_alchemist_processor::PROCESSOR_FROZEN_SOURCE_DECL_ID,
        )
        .unwrap();
    let before = engine
        .process_tree_snapshot()
        .node(frozen)
        .unwrap()
        .param_value
        .clone();

    engine.edits.push(Edit::SetNodeScriptProperty {
        node: frozen,
        property: "value".to_owned(),
        value: ParamValue::Str("script replacement".to_owned()),
    });
    let script_error = engine
        .apply_edits()
        .expect_err("script property writes must respect the compressed archive lock");
    assert!(matches!(
        script_error,
        golden_core::engine::EngineEditError::ScriptPropertyRejected { message, .. }
            if message == "parameter is read-only"
    ));
    assert_eq!(engine.process_tree_snapshot().node(frozen).unwrap().param_value, before);

    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(
            ProjectFileSpec::new("R08 compressed lock", "r08-compressed-lock"),
            None,
        ),
    );
    let headless = runtime.apply_ui_transaction(
        UiEditIntent::SetParam {
            node: frozen,
            value: ParamValue::Str("headless replacement".to_owned()),
            behaviour: ParameterEventBehaviour::Coalesce,
        },
        Some("r08-headless-client"),
    );
    assert!(!headless.acknowledgement.success);
    assert_eq!(
        headless.acknowledgement.error_code.as_deref(),
        Some("param_constraint_violation")
    );
    let published = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    let archive = published
        .nodes
        .iter()
        .find(|node| node.node_id == frozen)
        .unwrap();
    let UiNodeDataDto::Parameter { param } = &archive.data else {
        panic!("frozen source should remain a parameter");
    };
    assert_eq!(param.value, before.unwrap());
}

#[test]
fn compressed_mapping_diagnoses_external_source_type_changes_and_recovers() {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let source_uuid = source_param(&mut engine, "R08 typed source", 1.0);
    let sink_uuid = source_param(&mut engine, "R08 typed sink", 0.0);
    let inputs = region(&engine, processor, "inputs");
    let input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(
        &mut engine,
        input,
        "source",
        ParamValue::Reference(NodeReference::new(source_uuid)),
    );
    let outputs = region(&engine, processor, "outputs");
    create_parameter_output(&mut engine, outputs, sink_uuid);
    run_ticks(&mut engine, 8);
    set_mapping_compression(&mut engine, processor, true).unwrap();
    run_ticks(&mut engine, 4);

    let snapshot = engine.process_tree_snapshot();
    let source = snapshot.node_id_by_uuid(source_uuid).unwrap();
    let sink = snapshot.node_id_by_uuid(sink_uuid).unwrap();
    assert_eq!(snapshot.node(sink).unwrap().param_value, Some(ParamValue::Float(1.0)));
    drop(snapshot);

    let mut incompatible = Parameter::new(
        "R08 typed source",
        ParamValue::Bool(true),
        ParameterChangeCheck::ValueChange,
    );
    incompatible.node_data_mut().meta.uuid = source_uuid;
    engine.edits.push(Edit::ReplaceNode {
        node: source,
        new_node: Box::new(AppNode::from(incompatible)),
    });
    engine.apply_edits().unwrap();
    run_ticks(&mut engine, 4);
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(
        snapshot.node(sink).unwrap().param_value,
        Some(ParamValue::Float(1.0)),
        "an incompatible external type must not dispatch a fabricated value"
    );
    drop(snapshot);

    let mut compatible = Parameter::new(
        "R08 typed source",
        ParamValue::Float(2.0),
        ParameterChangeCheck::ValueChange,
    );
    compatible.node_data_mut().meta.uuid = source_uuid;
    engine.edits.push(Edit::ReplaceNode {
        node: source,
        new_node: Box::new(AppNode::from(compatible)),
    });
    engine.apply_edits().unwrap();
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: source,
                value: ParamValue::Float(3.0),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .success
    );
    run_ticks(&mut engine, 6);
    let recovered = engine.process_tree_snapshot();
    assert_eq!(recovered.node(sink).unwrap().param_value, Some(ParamValue::Float(3.0)));
}
