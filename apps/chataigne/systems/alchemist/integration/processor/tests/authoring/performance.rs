use std::time::Instant;

use super::*;

use golden_core::edit::{Edit, NodeTree, UserItemTreeInsertion};
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
    assert_eq!(after_idle.command_listener_reconciliations, prepared.command_listener_reconciliations);
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

#[test]
#[ignore = "opt-in multi-processor full-engine Mapping qualification"]
fn mapping_full_engine_processor_scale_distribution() {
    let processor_count = std::env::var("CHATAIGNE_MAPPING_ENGINE_PROCESSORS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(128);
    let samples = std::env::var("CHATAIGNE_MAPPING_ENGINE_SCALE_SAMPLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100);
    assert!(processor_count >= 2);
    assert!(samples >= 100);

    let (mut engine, mapping_uuid, first_processor) = mapping_engine();
    activate_mapping_processor(&mut engine, first_processor);
    let mut setup_stage = Instant::now();
    let mut setup_snapshot_base = engine.tick_stats();
    let snapshot = engine.process_tree_snapshot();
    let manager = snapshot
        .child_ids(engine.root)
        .into_iter()
        .find(|id| snapshot.node(*id).is_some_and(|node| node.node_type == StateMachineManager::NODE_TYPE))
        .unwrap();
    let state = snapshot
        .child_ids(manager)
        .into_iter()
        .find(|id| snapshot.node(*id).is_some_and(|node| node.node_type == StateMachineState::NODE_TYPE))
        .unwrap();
    let processors = snapshot.find_child_by_decl_id(state, "processors").unwrap();
    let create_type = FormulaSourceRef::project_uuid(mapping_uuid).processor_create_type();
    let mut processor_group = NodeTree::new(crate::app::StateProcessorFolder::new());
    for _ in 1..processor_count {
        let processor = engine.nodes.get(processors).unwrap().create_user_item_tree(&create_type).unwrap();
        processor_group.push_child(processor.as_user_item());
    }
    let processor_group_build_ms = setup_stage.elapsed().as_millis();
    engine.add_user_item_tree(processor_group, Some(processors));
    for _ in 0..4 {
        engine.apply_edits().unwrap();
    }
    let setup_snapshot_now = engine.tick_stats();
    println!("mapping_engine_scale_setup processors={processor_count} stage=processor_group build_ms={processor_group_build_ms} total_ms={} snapshots={} snapshot_ms={} cloned_nodes={}", setup_stage.elapsed().as_millis(), setup_snapshot_now.snapshot_builds - setup_snapshot_base.snapshot_builds, (setup_snapshot_now.snapshot_build_ns - setup_snapshot_base.snapshot_build_ns) / 1_000_000, setup_snapshot_now.snapshot_nodes_cloned - setup_snapshot_base.snapshot_nodes_cloned);
    setup_stage = Instant::now();
    setup_snapshot_base = setup_snapshot_now;
    let source = source_param(&mut engine, "Shared scale source", 1.0);
    let sink = source_param(&mut engine, "Shared scale sink", 0.0);
    let trigger = Parameter::new(
        "Shared scale command sink",
        ParamValue::Trigger(),
        ParameterChangeCheck::None,
    );
    let trigger_sink = trigger.node_data().meta.uuid;
    engine.add_node(trigger.into(), None);
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let command_manager = snapshot
        .child_ids(first_processor)
        .into_iter()
        .find(|id| engine.nodes.get(*id).is_some_and(|node| node.get_type() == OutputsManager::NODE_TYPE))
        .expect("Mapping processor should expose commands");
    let command = create_item(&mut engine, command_manager, GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE);
    let snapshot = engine.process_tree_snapshot();
    let command_target = snapshot.find_child_by_decl_id(command, "target").unwrap();
    let command_uuid = snapshot.node(command).unwrap().uuid;
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: command_target,
        value: ParamValue::Reference(NodeReference::new(trigger_sink)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "shared command should bind its trigger sink: {ack:?}");
    engine.apply_edits().unwrap();
    let setup_snapshot_now = engine.tick_stats();
    println!("mapping_engine_scale_setup processors={processor_count} stage=command elapsed_ms={} snapshots={} snapshot_ms={}", setup_stage.elapsed().as_millis(), setup_snapshot_now.snapshot_builds - setup_snapshot_base.snapshot_builds, (setup_snapshot_now.snapshot_build_ns - setup_snapshot_base.snapshot_build_ns) / 1_000_000);
    setup_stage = Instant::now();
    setup_snapshot_base = setup_snapshot_now;
    let snapshot = engine.process_tree_snapshot();
    let processor_nodes = snapshot
        .child_ids(processors)
        .into_iter()
        .flat_map(|node| {
            if snapshot.node(node).is_some_and(|entry| entry.node_type == StateProcessor::NODE_TYPE) {
                vec![node]
            } else {
                assert_eq!(snapshot.node(node).unwrap().node_type, crate::app::StateProcessorFolder::NODE_TYPE);
                snapshot.child_ids(node)
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(processor_nodes.len(), processor_count);
    let regions = processor_nodes
        .iter()
        .map(|processor| {
            let root = snapshot.find_child_by_decl_id(*processor, PROCESSOR_MANAGED_REGIONS_DECL_ID).unwrap();
            let input = snapshot.find_child_by_decl_id(root, &processor_managed_region_decl_id("inputs")).unwrap();
            let output = snapshot.find_child_by_decl_id(root, &processor_managed_region_decl_id("outputs")).unwrap();
            (*processor == first_processor, input, output)
        })
        .collect::<Vec<_>>();
    let mut item_trees = Vec::with_capacity(processor_count * 2 + 1);
    for (is_first, input, output) in &regions {
        let input_tree = engine
            .nodes
            .get(*input)
            .unwrap()
            .create_user_item_tree(&format!("{ANODE_CREATE_PREFIX}chataigne.input_source"))
            .unwrap();
        let command_output_tree = engine
            .nodes
            .get(*output)
            .unwrap()
            .create_user_item_tree(&format!("{ANODE_CREATE_PREFIX}chataigne.output_target"))
            .unwrap();
        item_trees.push(UserItemTreeInsertion::new(*input, input_tree));
        if *is_first {
            let output_tree = engine
                .nodes
                .get(*output)
                .unwrap()
                .create_user_item_tree(&format!("{ANODE_CREATE_PREFIX}chataigne.output_target"))
                .unwrap();
            item_trees.push(UserItemTreeInsertion::new(*output, output_tree));
        }
        item_trees.push(UserItemTreeInsertion::new(*output, command_output_tree));
    }
    let item_trees_queue_ms = setup_stage.elapsed().as_millis();
    engine.add_user_item_trees(item_trees);
    engine.apply_edits().unwrap();
    let setup_snapshot_now = engine.tick_stats();
    println!("mapping_engine_scale_setup processors={processor_count} stage=item_trees queue_ms={item_trees_queue_ms} total_ms={} snapshots={} snapshot_ms={} cloned_nodes={}", setup_stage.elapsed().as_millis(), setup_snapshot_now.snapshot_builds - setup_snapshot_base.snapshot_builds, (setup_snapshot_now.snapshot_build_ns - setup_snapshot_base.snapshot_build_ns) / 1_000_000, setup_snapshot_now.snapshot_nodes_cloned - setup_snapshot_base.snapshot_nodes_cloned);
    setup_stage = Instant::now();
    let snapshot = engine.process_tree_snapshot();
    for (is_first, input, output) in &regions {
        let input_item = snapshot.child_ids(*input)[0];
        let output_items = snapshot.child_ids(*output);
        let command_output_item = *output_items.last().unwrap();
        let input_config = snapshot.find_child_by_decl_id(input_item, "config").unwrap();
        let command_output_config = snapshot.find_child_by_decl_id(command_output_item, "config").unwrap();
        let input_source = snapshot.find_child_by_decl_id(input_config, "config/source").unwrap();
        let command_output_target = snapshot.find_child_by_decl_id(command_output_config, "config/target").unwrap();
        let command_output_bindings = snapshot.find_child_by_decl_id(command_output_config, "config/bindings").unwrap();
        engine.edits.push(Edit::SetParam {
            node: input_source,
            value: ParamValue::Reference(NodeReference::new(source)),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        if *is_first {
            let output_config = snapshot.find_child_by_decl_id(output_items[0], "config").unwrap();
            let output_target = snapshot.find_child_by_decl_id(output_config, "config/target").unwrap();
            engine.edits.push(Edit::SetParam {
                node: output_target,
                value: ParamValue::Reference(NodeReference::new(sink)),
                behaviour: ParameterEventBehaviour::Coalesce,
            });
        }
        engine.edits.push(Edit::SetParam {
            node: command_output_target,
            value: ParamValue::Reference(NodeReference::new(command_uuid)),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        engine.edits.push(Edit::SetParam {
            node: command_output_bindings,
            value: ParamValue::Str(
                OutputBindingConfig {
                    value: OutputValueSource::Whole,
                    send_policy: OutputSendPolicy::EveryDelivery,
                    ..OutputBindingConfig::default()
                }
                .to_authoring_json()
                .unwrap(),
            ),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
    engine.apply_edits().unwrap();
    run_ticks(&mut engine, 16);
    println!("mapping_engine_scale_setup processors={processor_count} stage=bindings_and_warmup elapsed_ms={}", setup_stage.elapsed().as_millis());
    engine.clear_ui_event_log();
    let snapshot = engine.process_tree_snapshot();
    let source_node = snapshot.node_id_by_uuid(source).unwrap();
    let sink_node = snapshot.node_id_by_uuid(sink).unwrap();
    assert_eq!(snapshot.node(sink_node).unwrap().param_value, Some(ParamValue::Float(1.0)));
    let prepared = runtime_stats(&engine);

    let mut idle = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        engine.run_tick(Duration::from_millis(8)).unwrap();
        idle.push(start.elapsed().as_nanos() as u64);
        engine.clear_ui_event_log();
    }
    let after_idle = runtime_stats(&engine);
    assert_eq!(after_idle.processor_candidate_visits, prepared.processor_candidate_visits);
    assert_eq!(after_idle.command_listener_reconciliations, prepared.command_listener_reconciliations);
    idle.sort_unstable();
    let idle_p95_ns = idle[(samples * 95).div_ceil(100) - 1];
    println!(
        "mapping_engine_scale idle processors={processor_count} samples={samples} p50_ns={} p95_ns={} p99_ns={}",
        idle[(samples * 50).div_ceil(100) - 1],
        idle_p95_ns,
        idle[(samples * 99).div_ceil(100) - 1],
    );

    let mut dense = Vec::with_capacity(samples);
    let mut snapshot_builds = 0usize;
    let mut snapshot_nodes_cloned = 0usize;
    let mut snapshot_build_ns = 0u128;
    let mut dispatch_events = 0usize;
    let mut dispatch_recipients = 0usize;
    let mut events_emitted = 0usize;
    for index in 0..samples {
        let input = if index % 2 == 0 { 2.0 } else { 1.0 };
        let start = Instant::now();
        engine.edits.push(Edit::SetParam {
            node: source_node,
            value: ParamValue::Float(input),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..2 {
            engine.run_tick(Duration::from_millis(8)).unwrap();
            let stats = engine.tick_stats();
            snapshot_builds += stats.snapshot_builds;
            snapshot_nodes_cloned += stats.snapshot_nodes_cloned;
            snapshot_build_ns += stats.snapshot_build_ns;
            dispatch_events += stats.dispatch_events_routed;
            dispatch_recipients += stats.dispatch_recipient_deliveries;
            events_emitted += stats.events_emitted;
        }
        dense.push(start.elapsed().as_nanos() as u64);
        assert_eq!(engine.process_tree_snapshot().node(sink_node).unwrap().param_value, Some(ParamValue::Float(input)));
        engine.clear_ui_event_log();
    }
    let after_dense = runtime_stats(&engine);
    assert_eq!(after_dense.command_listener_reconciliations, after_idle.command_listener_reconciliations);
    let expected = (processor_count * samples) as u64;
    assert_eq!(after_dense.processor_lanes_evaluated - after_idle.processor_lanes_evaluated, expected);
    let command_batches = after_dense.processor_command_batches - after_idle.processor_command_batches;
    let maximum_batches = samples
        * (processor_count.div_ceil(crate::app::module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS) + 1);
    assert!(
        command_batches > 0 && command_batches <= maximum_batches as u64,
        "consecutive processors targeting one command should share bounded batches: {command_batches} > {maximum_batches}"
    );
    assert_eq!(after_dense.processor_batched_executions - after_idle.processor_batched_executions, expected);
    assert_eq!(after_dense.debug_samples_captured, after_idle.debug_samples_captured);
    println!(
        "mapping_engine_scale_breakdown processors={processor_count} input_preparation_ns={} evaluation_ns={} evaluation_calls={} formula_catalog_builds={} runtime_cache_rebuilds={}",
        after_dense.processor_input_preparation_ns - after_idle.processor_input_preparation_ns,
        after_dense.processor_evaluation_ns - after_idle.processor_evaluation_ns,
        after_dense.processor_evaluation_calls - after_idle.processor_evaluation_calls,
        after_dense.formula_catalog_builds - after_idle.formula_catalog_builds,
        after_dense.runtime_cache_rebuilds - after_idle.runtime_cache_rebuilds,
    );
    println!(
        "mapping_engine_scale_engine processors={processor_count} samples={samples} snapshot_builds={snapshot_builds} snapshot_nodes_cloned={snapshot_nodes_cloned} snapshot_build_ns={snapshot_build_ns} dispatch_events={dispatch_events} dispatch_recipients={dispatch_recipients} events_emitted={events_emitted}"
    );
    dense.sort_unstable();
    let dense_p95_ns = dense[(samples * 95).div_ceil(100) - 1];
    println!(
        "mapping_engine_scale shared_source_with_command processors={processor_count} samples={samples} evaluated_lanes={expected} command_batches={command_batches} command_executions={expected} p50_ns={} p95_ns={} p99_ns={}",
        dense[(samples * 50).div_ceil(100) - 1],
        dense_p95_ns,
        dense[(samples * 99).div_ceil(100) - 1],
    );
    if std::env::var_os("CHATAIGNE_MAPPING_ENFORCE_275HX_SCALE_BASELINE").is_some() {
        let (idle_limit_ns, dense_limit_ns) = match processor_count {
            128 => (100_000, 20_000_000),
            256 => (150_000, 45_000_000),
            1_000 => (750_000, 150_000_000),
            _ => panic!("unrecorded 275HX full-engine processor count {processor_count}"),
        };
        assert!(idle_p95_ns <= idle_limit_ns, "idle p95 {idle_p95_ns} ns exceeded {idle_limit_ns} ns");
        assert!(dense_p95_ns <= dense_limit_ns, "dense p95 {dense_p95_ns} ns exceeded {dense_limit_ns} ns");
    }
    if std::env::var_os("CHATAIGNE_MAPPING_ENGINE_MEASURE_RELOAD").is_some() {
        let authored_nodes = engine.nodes.len();
        let mut authored_types = std::collections::BTreeMap::<String, usize>::new();
        let authored_uuids = engine
            .nodes
            .iter()
            .map(|(_, node)| node.node_data().meta.uuid)
            .collect::<std::collections::HashSet<_>>();
        let authored_item_roots = engine
            .nodes
            .iter()
            .filter(|(_, node)| node.node_data().user_role == golden_core::node::UserNodeRole::ItemRoot)
            .map(|(_, node)| node.node_data().meta.uuid)
            .collect::<std::collections::HashSet<_>>();
        for (_, node) in engine.nodes.iter() {
            *authored_types.entry(node.get_type().to_owned()).or_default() += 1;
        }
        let started = Instant::now();
        let saved = golden_core::app::to_sparse_project_json_pretty(&engine)
            .expect("authored Mapping graph should save");
        let save_ms = started.elapsed().as_millis();
        let saved_bytes = saved.len();
        drop(engine);

        let started = Instant::now();
        let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved)
            .expect("authored Mapping graph should reload");
        let decode_ms = started.elapsed().as_millis();
        let started = Instant::now();
        sync_external_formulas(&mut loaded).expect("shipped Formulas should synchronize on project open");
        let formula_sync_ms = started.elapsed().as_millis();
        let started = Instant::now();
        golden_core::app::prepare_engine_for_runtime(&mut loaded)
            .expect("reloaded Mapping graph should prepare for runtime");
        let prepare_ms = started.elapsed().as_millis();
        let loaded_nodes = loaded.nodes.len();
        let mut loaded_types = std::collections::BTreeMap::<String, usize>::new();
        for (_, node) in loaded.nodes.iter() {
            *loaded_types.entry(node.get_type().to_owned()).or_default() += 1;
        }
        for (node_type, authored_count) in &authored_types {
            let loaded_count = loaded_types.get(node_type).copied().unwrap_or_default();
            if *authored_count != loaded_count {
                println!(
                    "mapping_engine_scale_reload_type processors={processor_count} type={node_type} authored={authored_count} loaded={loaded_count}"
                );
            }
        }
        for (node_type, loaded_count) in &loaded_types {
            if !authored_types.contains_key(node_type) {
                println!(
                    "mapping_engine_scale_reload_type processors={processor_count} type={node_type} authored=0 loaded={loaded_count}"
                );
            }
        }
        let loaded_uuids = loaded
            .nodes
            .iter()
            .map(|(_, node)| node.node_data().meta.uuid)
            .collect::<std::collections::HashSet<_>>();
        let missing_uuids = authored_uuids.difference(&loaded_uuids).count();
        let missing_item_roots = authored_item_roots.difference(&loaded_uuids).count();
        println!(
            "mapping_engine_scale_reload processors={processor_count} authored_nodes={authored_nodes} loaded_nodes={loaded_nodes} missing_uuids={missing_uuids} item_roots={} missing_item_roots={missing_item_roots} saved_bytes={saved_bytes} save_ms={save_ms} decode_ms={decode_ms} formula_sync_ms={formula_sync_ms} prepare_ms={prepare_ms}",
            authored_item_roots.len(),
        );
        assert_eq!(missing_item_roots, 0, "reload lost authored user-item identities");
        assert_eq!(
            loaded
                .nodes
                .iter()
                .filter(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
                .count(),
            processor_count,
            "reload changed the Mapping processor count"
        );
        let loaded_snapshot = loaded.process_tree_snapshot();
        assert!(loaded_snapshot.node_id_by_uuid(source).is_some());
        assert!(loaded_snapshot.node_id_by_uuid(command_uuid).is_some());
        let loaded_sink = loaded_snapshot.node_id_by_uuid(sink).expect("shared output sink should reload");
        assert_eq!(loaded_snapshot.node(loaded_sink).unwrap().param_value, Some(ParamValue::Float(1.0)));
        let resaved = golden_core::app::to_sparse_project_json_pretty(&loaded)
            .expect("reloaded Mapping graph should save again");
        let loaded_source = loaded_snapshot.node_id_by_uuid(source).expect("shared source should reload");
        let started = Instant::now();
        run_ticks(&mut loaded, 16);
        let warmup_ms = started.elapsed().as_millis();
        let before_resume = runtime_stats(&loaded);
        let started = Instant::now();
        loaded.edits.push(Edit::SetParam {
            node: loaded_source,
            value: ParamValue::Float(2.0),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        run_ticks(&mut loaded, 2);
        let resume_ms = started.elapsed().as_millis();
        assert_eq!(
            loaded.process_tree_snapshot().node(loaded_sink).unwrap().param_value,
            Some(ParamValue::Float(2.0)),
            "reloaded Mappings should deliver a fresh source value"
        );
        let after_resume = runtime_stats(&loaded);
        assert_eq!(
            after_resume.processor_batched_executions - before_resume.processor_batched_executions,
            processor_count as u64,
            "reloaded Mappings should execute each shared command exactly once"
        );
        println!(
            "mapping_engine_scale_reload_resume processors={processor_count} resaved_bytes={} warmup_ms={warmup_ms} resume_ms={resume_ms} command_executions={}",
            resaved.len(),
            after_resume.processor_batched_executions - before_resume.processor_batched_executions,
        );
    }
}

#[test]
#[ignore = "opt-in long-horizon full-engine Smooth Mapping qualification"]
fn mapping_full_engine_temporal_history_distribution() {
    const TICKS: usize = 100_000;
    const SAMPLE_COUNT: usize = 500;
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let source = source_param(&mut engine, "Temporal engine source", 0.0);
    let sink = source_param(&mut engine, "Temporal engine sink", 0.0);
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
    let smooth_type = engine
        .nodes
        .get(filters)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}smooth_filter")))
        .map(|item| item.node_type)
        .expect("Float source should expose Smooth Filter");
    let smooth = create_item(&mut engine, filters, &smooth_type);
    set_config(&mut engine, smooth, "method", ParamValue::Enum("sma".to_owned()));
    let outputs = region(&engine, processor, "outputs");
    let output = create_item(
        &mut engine,
        outputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.output_target"),
    );
    set_config(
        &mut engine,
        output,
        "target",
        ParamValue::Reference(NodeReference::new(sink)),
    );
    run_ticks(&mut engine, 16);
    let snapshot = engine.process_tree_snapshot();
    let source_node = snapshot.node_id_by_uuid(source).unwrap();
    let sink_node = snapshot.node_id_by_uuid(sink).unwrap();
    let before = runtime_stats(&engine);
    engine.edits.push(Edit::SetParam {
        node: source_node,
        value: ParamValue::Float(10.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    let mut early = Vec::with_capacity(SAMPLE_COUNT);
    let mut late = Vec::with_capacity(SAMPLE_COUNT);
    for tick in 0..TICKS {
        let start = Instant::now();
        engine.run_tick(Duration::from_millis(8)).unwrap();
        let elapsed = start.elapsed().as_nanos() as u64;
        if tick < SAMPLE_COUNT {
            early.push(elapsed);
        }
        if tick >= TICKS - SAMPLE_COUNT {
            late.push(elapsed);
        }
        engine.clear_ui_event_log();
    }
    let after = runtime_stats(&engine);
    assert!(after.processor_lanes_evaluated - before.processor_lanes_evaluated >= TICKS as u64);
    assert_eq!(after.formula_catalog_builds, before.formula_catalog_builds);
    assert_eq!(after.formula_compiles, before.formula_compiles);
    assert_eq!(after.runtime_cache_rebuilds, before.runtime_cache_rebuilds);
    assert_eq!(after.debug_samples_captured, before.debug_samples_captured);
    assert_eq!(
        engine.process_tree_snapshot().node(sink_node).unwrap().param_value,
        Some(ParamValue::Float(10.0)),
    );
    early.sort_unstable();
    late.sort_unstable();
    println!(
        "mapping_engine_temporal_history ticks={TICKS} evaluated_lanes={} early_p50_ns={} early_p95_ns={} early_p99_ns={} late_p50_ns={} late_p95_ns={} late_p99_ns={}",
        after.processor_lanes_evaluated - before.processor_lanes_evaluated,
        early[SAMPLE_COUNT / 2 - 1],
        early[(SAMPLE_COUNT * 95).div_ceil(100) - 1],
        early[(SAMPLE_COUNT * 99).div_ceil(100) - 1],
        late[SAMPLE_COUNT / 2 - 1],
        late[(SAMPLE_COUNT * 95).div_ceil(100) - 1],
        late[(SAMPLE_COUNT * 99).div_ceil(100) - 1],
    );
}
