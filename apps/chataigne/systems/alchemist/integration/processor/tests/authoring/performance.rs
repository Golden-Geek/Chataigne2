use std::time::Instant;

use super::*;

use golden_core::edit::Edit;
use chataigne_state_machine::protocol::{FormulaPreviewDemandDto, FormulaPreviewModeDto};

use crate::app::systems_alchemist_generic_commands::GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE;
use crate::app::systems_state_machine_manager::StateMachineRuntimePerfStats;

fn runtime_stats(engine: &AppEngine) -> StateMachineRuntimePerfStats {
    engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats()),
            _ => None,
        })
        .expect("active StateMachineManager should exist")
}

fn report(label: &str, samples: &mut [u64]) {
    samples.sort_unstable();
    let percentile = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1];
    let p95 = percentile(95);
    println!(
        "mapping_engine_latency {label} samples={} p50_ns={} p95_ns={} p99_ns={}",
        samples.len(),
        percentile(50),
        p95,
        percentile(99)
    );
    if std::env::var_os("CHATAIGNE_MAPPING_ENFORCE_275HX_ENGINE_BASELINE").is_some() {
        let limit = match label {
            "idle" => 2_000,
            "source_change_with_command" => 1_500_000,
            "runtime_setting_with_command" => 1_500_000,
            "structural_filter_reorder_with_command" => 5_000_000,
            "source_change_with_command_and_preview" => 1_500_000,
            _ => panic!("unrecorded Mapping engine workload {label}"),
        };
        assert!(p95 <= limit, "{label} p95 {p95} ns exceeded recorded 275HX guard {limit} ns");
    }
}

fn run_ticks(engine: &mut AppEngine, count: usize) {
    for _ in 0..count {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
}

fn request_processor_preview(engine: &mut AppEngine, processor_id: &str, enabled: bool) {
    let demand = FormulaPreviewDemandDto {
        subscription_id: "mapping-engine-performance".to_owned(),
        mode: enabled.then(|| FormulaPreviewModeDto::ProcessorDefaultLane {
            processor_id: processor_id.to_owned(),
        }),
    };
    let manager = engine
        .nodes
        .iter()
        .find_map(|(node_id, node)| matches!(node, AppNode::StateMachineManager(_)).then_some(node_id))
        .expect("active StateMachineManager should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::SendNodeEvent {
        node: manager,
        topic: "chataigne.state_machine.runtime_preview_demand".to_owned(),
        payload: serde_json::to_value(demand).unwrap(),
    });
    assert!(ack.success, "preview demand should reach manager: {ack:?}");
}

#[test]
#[ignore = "opt-in full-engine Mapping latency qualification"]
fn mapping_full_engine_latency_distribution() {
    let samples = std::env::var("CHATAIGNE_MAPPING_ENGINE_SAMPLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(200);
    assert!(samples >= 100, "full-engine distribution needs at least 100 samples");

    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let processor_id = engine.process_tree_snapshot().node(processor).unwrap().uuid.0.to_string();
    let source = source_param(&mut engine, "Engine source", 2.0);
    let sink = source_param(&mut engine, "Engine value sink", 0.0);
    let trigger = Parameter::new(
        "Engine command sink",
        ParamValue::Trigger(),
        ParameterChangeCheck::None,
    );
    let trigger_sink = trigger.node_data().meta.uuid;
    engine.add_node(trigger.into(), None);
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
        ParamValue::Reference(NodeReference::new(source)),
    );
    let filters = region(&engine, processor, "filters");
    let remap_type = engine
        .nodes
        .get(filters)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}remap@managed/0")))
        .map(|item| item.node_type)
        .expect("a Float source should expose Remap");
    let remap = create_item(&mut engine, filters, &remap_type);
    set_socket_default(&mut engine, remap, "in_max", ParamValue::Float(5.0));
    let snapshot = engine.process_tree_snapshot();
    let remap_inputs = snapshot.find_child_by_decl_id(remap, "inputs").unwrap();
    let maximum_socket = snapshot.find_child_by_decl_id(remap_inputs, "inputs/in_max").unwrap();
    let maximum = snapshot
        .find_child_by_decl_id(maximum_socket, "inputs/in_max/value")
        .unwrap();

    let command_manager = snapshot
        .child_ids(processor)
        .into_iter()
        .find(|id| engine.nodes.get(*id).is_some_and(|node| node.get_type() == OutputsManager::NODE_TYPE))
        .expect("Mapping processor should expose commands");
    let command = create_item(&mut engine, command_manager, GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE);
    let snapshot = engine.process_tree_snapshot();
    let target = snapshot.find_child_by_decl_id(command, "target").unwrap();
    let command_uuid = snapshot.node(command).unwrap().uuid;
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: target,
        value: ParamValue::Reference(NodeReference::new(trigger_sink)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "command target should accept trigger sink: {ack:?}");
    engine.apply_edits().unwrap();
    let outputs = region(&engine, processor, "outputs");
    let value_output = create_item(
        &mut engine,
        outputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.output_target"),
    );
    set_config(
        &mut engine,
        value_output,
        "target",
        ParamValue::Reference(NodeReference::new(sink)),
    );
    let command_output = create_item(
        &mut engine,
        outputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.output_target"),
    );
    set_config(
        &mut engine,
        command_output,
        "target",
        ParamValue::Reference(NodeReference::new(command_uuid)),
    );
    let binding = OutputBindingConfig {
        value: OutputValueSource::Whole,
        send_policy: OutputSendPolicy::EveryDelivery,
        ..OutputBindingConfig::default()
    };
    set_config(
        &mut engine,
        command_output,
        "bindings",
        ParamValue::Str(binding.to_authoring_json().unwrap()),
    );

    run_ticks(&mut engine, 16);
    let snapshot = engine.process_tree_snapshot();
    let source_node = snapshot.node_id_by_uuid(source).unwrap();
    let sink_node = snapshot.node_id_by_uuid(sink).unwrap();
    assert_eq!(snapshot.node(sink_node).unwrap().param_value, Some(ParamValue::Float(0.4)));
    let prepared = runtime_stats(&engine);
    assert!(prepared.formula_compiles > 0);

    let mut idle = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        engine.run_tick(Duration::from_millis(8)).unwrap();
        idle.push(start.elapsed().as_nanos() as u64);
        engine.clear_ui_event_log();
    }
    let after_idle = runtime_stats(&engine);
    assert_eq!(after_idle.processor_candidate_visits, prepared.processor_candidate_visits);
    assert_eq!(after_idle.formula_compiles, prepared.formula_compiles);
    assert_eq!(after_idle.debug_samples_captured, prepared.debug_samples_captured);
    report("idle", &mut idle);

    let mut source_change = Vec::with_capacity(samples);
    for index in 0..samples {
        let input = if index % 2 == 0 { 3.0 } else { 2.0 };
        let start = Instant::now();
        engine.edits.push(Edit::SetParam {
            node: source_node,
            value: ParamValue::Float(input),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        run_ticks(&mut engine, 2);
        source_change.push(start.elapsed().as_nanos() as u64);
        assert_eq!(
            engine.process_tree_snapshot().node(sink_node).unwrap().param_value,
            Some(ParamValue::Float(input / 5.0))
        );
        engine.clear_ui_event_log();
    }
    let after_source = runtime_stats(&engine);
    assert_eq!(after_source.formula_compiles, prepared.formula_compiles);
    assert!(after_source.processor_candidate_visits > after_idle.processor_candidate_visits);
    assert!(after_source.processor_command_batches > after_idle.processor_command_batches);
    assert!(after_source.processor_batched_executions > after_idle.processor_batched_executions);
    println!(
        "mapping_engine_volume source_changes={} evaluated_lanes={} command_batches={} command_executions={} previews={}",
        samples,
        after_source.processor_lanes_evaluated - after_idle.processor_lanes_evaluated,
        after_source.processor_command_batches - after_idle.processor_command_batches,
        after_source.processor_batched_executions - after_idle.processor_batched_executions,
        after_source.debug_samples_captured - after_idle.debug_samples_captured
    );
    report("source_change_with_command", &mut source_change);

    let mut setting_change = Vec::with_capacity(samples);
    for index in 0..samples {
        let maximum_value = if index % 2 == 0 { 4.0 } else { 5.0 };
        let start = Instant::now();
        engine.edits.push(Edit::SetParam {
            node: maximum,
            value: ParamValue::Float(maximum_value),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        run_ticks(&mut engine, 2);
        setting_change.push(start.elapsed().as_nanos() as u64);
        assert_eq!(
            engine.process_tree_snapshot().node(sink_node).unwrap().param_value,
            Some(ParamValue::Float(2.0 / maximum_value))
        );
        engine.clear_ui_event_log();
    }
    let after_setting = runtime_stats(&engine);
    assert_eq!(after_setting.formula_compiles, prepared.formula_compiles);
    assert_eq!(after_setting.debug_samples_captured, prepared.debug_samples_captured);
    println!(
        "mapping_engine_volume setting_changes={} evaluated_lanes={} command_batches={} command_executions={} previews={}",
        samples,
        after_setting.processor_lanes_evaluated - after_source.processor_lanes_evaluated,
        after_setting.processor_command_batches - after_source.processor_command_batches,
        after_setting.processor_batched_executions - after_source.processor_batched_executions,
        after_setting.debug_samples_captured - after_source.debug_samples_captured
    );
    report("runtime_setting_with_command", &mut setting_change);

    let clamp_type = engine
        .nodes
        .get(filters)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}clamp@managed/0")))
        .map(|item| item.node_type)
        .expect("a Float chain should expose Clamp");
    let clamp = create_item(&mut engine, filters, &clamp_type);
    set_socket_default(&mut engine, clamp, "minimum", ParamValue::Float(0.0));
    set_socket_default(&mut engine, clamp, "maximum", ParamValue::Float(1.0));
    run_ticks(&mut engine, 8);
    let before_structure = runtime_stats(&engine);
    let mut structural_reorder = Vec::with_capacity(samples);
    for index in 0..samples {
        let clamp_first = index % 2 == 0;
        let start = Instant::now();
        let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
            node: clamp,
            new_parent: filters,
            new_prev_sibling: (!clamp_first).then_some(remap),
        });
        assert!(ack.success, "filter reorder should apply: {ack:?}");
        engine.apply_edits().unwrap();
        assert_eq!(
            engine.process_tree_snapshot().child_ids(filters),
            if clamp_first {
                vec![clamp, remap]
            } else {
                vec![remap, clamp]
            }
        );
        run_ticks(&mut engine, 2);
        structural_reorder.push(start.elapsed().as_nanos() as u64);
        assert_eq!(
            engine.process_tree_snapshot().node(sink_node).unwrap().param_value,
            Some(ParamValue::Float(if clamp_first { 0.2 } else { 0.4 }))
        );
        engine.clear_ui_event_log();
    }
    let after_structure = runtime_stats(&engine);
    assert_eq!(
        after_structure.runtime_cache_rebuilds,
        before_structure.runtime_cache_rebuilds,
        "filter reorders should rematerialize only their processor"
    );
    assert_eq!(
        after_structure.formula_compiles,
        before_structure.formula_compiles,
        "filter reorders should reuse the unchanged shared Formula plan"
    );
    assert_eq!(
        after_structure.debug_samples_captured,
        before_structure.debug_samples_captured
    );
    println!(
        "mapping_engine_volume structural_reorders={} runtime_cache_rebuilds={} formula_compiles={} evaluated_lanes={} command_batches={} command_executions={} previews={}",
        samples,
        after_structure.runtime_cache_rebuilds - before_structure.runtime_cache_rebuilds,
        after_structure.formula_compiles - before_structure.formula_compiles,
        after_structure.processor_lanes_evaluated - before_structure.processor_lanes_evaluated,
        after_structure.processor_command_batches - before_structure.processor_command_batches,
        after_structure.processor_batched_executions - before_structure.processor_batched_executions,
        after_structure.debug_samples_captured - before_structure.debug_samples_captured
    );
    report("structural_filter_reorder_with_command", &mut structural_reorder);

    request_processor_preview(&mut engine, &processor_id, true);
    run_ticks(&mut engine, 2);
    let before_preview = runtime_stats(&engine);
    let mut preview = Vec::with_capacity(samples);
    for index in 0..samples {
        if index > 0 && index % 100 == 0 {
            request_processor_preview(&mut engine, &processor_id, true);
            run_ticks(&mut engine, 1);
        }
        let input = if index % 2 == 0 { 3.0 } else { 2.0 };
        let start = Instant::now();
        engine.edits.push(Edit::SetParam {
            node: source_node,
            value: ParamValue::Float(input),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        run_ticks(&mut engine, 2);
        preview.push(start.elapsed().as_nanos() as u64);
        assert_eq!(
            engine.process_tree_snapshot().node(sink_node).unwrap().param_value,
            Some(ParamValue::Float(input / 5.0))
        );
        engine.clear_ui_event_log();
    }
    let after_preview = runtime_stats(&engine);
    assert!(
        after_preview.debug_samples_captured - before_preview.debug_samples_captured >= samples as u64,
        "requested preview should capture at least one sample per source change"
    );
    println!(
        "mapping_engine_volume preview_source_changes={} evaluated_lanes={} command_batches={} command_executions={} previews={}",
        samples,
        after_preview.processor_lanes_evaluated - before_preview.processor_lanes_evaluated,
        after_preview.processor_command_batches - before_preview.processor_command_batches,
        after_preview.processor_batched_executions - before_preview.processor_batched_executions,
        after_preview.debug_samples_captured - before_preview.debug_samples_captured
    );
    report("source_change_with_command_and_preview", &mut preview);

    request_processor_preview(&mut engine, &processor_id, false);
    run_ticks(&mut engine, 2);
    let after_release = runtime_stats(&engine);
    engine.edits.push(Edit::SetParam {
        node: source_node,
        value: ParamValue::Float(3.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    run_ticks(&mut engine, 2);
    assert_eq!(
        runtime_stats(&engine).debug_samples_captured,
        after_release.debug_samples_captured,
        "releasing preview demand should stop debug capture"
    );
}
