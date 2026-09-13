use std::{collections::HashMap, path::Path, sync::Arc};

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistFormula, AlchemistGraphDomain, CompileCtx, EvaluationCtx,
    FormulaContextContract, FormulaId, FormulaPropertySchema, FormulaSurface, ManagedItemId,
    ManagedItemInstance, ManagedItemUiState, ManagedRegionDefinition, ManagedRegionId,
    ManagedRegionInstance, ManagedRegionKind, RuntimeInputSnapshot, RuntimeRegistries, SocketId,
    StableRef, SurfaceItemKind, ValueTypeId, primitive_node_registry,
};
use chataigne_state_machine::{
    ChannelSourceSchema, Processor, ProcessorRuntime, ProcessorFormulaUiState, INPUT_SOURCE_FIELD, OUTPUT_TARGET_FIELD,
};
use golden_values::Value as RuntimeValue;

use golden_core::{
    app::load_sparse_project_file,
    engine::EngineTime,
    events::{Event, EventKind},
    node::{Folder, Node, NodeId, NodeUuid},
    parameter::ParamValue,
    ui_sync::UiEditIntent,
    process_ctx::{ExecutionPhase, ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{AlchemistFormulaDefinition, AppEngine, AppNode, FormulaLibrary, StateMachineState};

use super::super::{
    is_condition_valid_result, managed_runtime_input_location, runtime_param_change_requires_snapshot, set_condition_valid_param,
    RuntimeInvalidation, RuntimeProcessor, StateMachineManager,
};
use super::context_scope_test_node;

#[test]
fn authored_formula_value_invalidates_runtime_but_layout_and_status_do_not() {
    let (root, manager_id, library, formula, anode, position, config, value, valid) = (
        NodeId(1), NodeId(2), NodeId(3), NodeId(4), NodeId(5), NodeId(6), NodeId(7), NodeId(8), NodeId(9),
    );
    let formula_uuid = NodeUuid(uuid::Uuid::from_u128(4));
    let mut nodes = HashMap::from([
        (root, context_scope_test_node(root, None, Some(manager_id), None, "root")),
        (manager_id, context_scope_test_node(manager_id, Some(root), None, Some(library), "state_machine_manager")),
        (library, context_scope_test_node(library, Some(root), Some(formula), None, "alchemist_formula_library")),
        (formula, context_scope_test_node(formula, Some(library), Some(anode), None, "alchemist_formula")),
        (anode, context_scope_test_node(anode, Some(formula), Some(position), Some(valid), "alchemist_anode")),
        (position, context_scope_test_node(position, Some(anode), None, Some(config), "vec2")),
        (config, context_scope_test_node(config, Some(anode), Some(value), None, "folder")),
        (value, context_scope_test_node(value, Some(config), None, None, "float")),
        (valid, context_scope_test_node(valid, Some(formula), None, None, "bool")),
    ]);
    nodes.get_mut(&formula).unwrap().uuid = formula_uuid;
    nodes.get_mut(&anode).unwrap().tags.push("alchemist.anode.type:constant".into());
    nodes.get_mut(&position).unwrap().decl_id = "position".into();
    nodes.get_mut(&config).unwrap().decl_id = "config".into();
    nodes.get_mut(&value).unwrap().decl_id = "config/value".into();
    nodes.get_mut(&valid).unwrap().decl_id = "is_valid".into();
    let snapshot = Arc::new(ProcessTreeSnapshot::new(root, nodes));
    let mut manager = StateMachineManager::new();
    manager.node_data_mut().id = manager_id;
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 1, micro: 0, seq: 0 });
    ctx.set_tree_snapshot(Arc::clone(&snapshot));

    manager.on_param_change(&mut ctx, position, ParamValue::Vec2(0.0, 0.0));
    manager.on_param_change(&mut ctx, valid, ParamValue::Bool(false));
    assert!(manager.runtime_cache.structure_dirty.is_empty());

    manager.on_param_change(&mut ctx, value, ParamValue::Float(0.0));
    assert!(manager.runtime_cache.structure_dirty.contains(&formula_uuid));

    manager.runtime_cache.structure_dirty.clear();
    manager.runtime_cache.runtime_snapshot = Some(Arc::clone(&snapshot));
    manager.runtime_cache.topology_dirty = false;
    manager.runtime_cache.formula_catalog_dirty = false;
    manager.runtime_cache.formula_catalog_initialized = true;
    manager.runtime_cache.context_provider_dirty = false;
    manager.runtime_cache.command_dispatch_snapshot_dirty = false;
    let mut no_snapshot = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 2, micro: 0, seq: 0 });
    no_snapshot.events.push_shared(Arc::new(Event {
        time: no_snapshot.time,
        kind: EventKind::ParamChanged {
            param: value,
            old_value: ParamValue::Float(0.0),
            new_value: ParamValue::Float(2.0),
        },
    }));
    assert!(!manager.inbox_requires_tree_snapshot(&no_snapshot.events));
    manager.runtime_cache.structure_dirty.insert(formula_uuid);
    assert!(manager.inbox_requires_tree_snapshot(&no_snapshot.events));
    manager.runtime_cache.structure_dirty.clear();
    let mut type_change = ProcessCtx::new(ExecutionPhase::EngineTick, no_snapshot.time);
    type_change.events.push_shared(Arc::new(Event {
        time: no_snapshot.time,
        kind: EventKind::ParamChanged {
            param: value,
            old_value: ParamValue::Float(0.0),
            new_value: ParamValue::Vec3(1.0, 2.0, 3.0),
        },
    }));
    assert!(manager.inbox_requires_tree_snapshot(&type_change.events));
    manager.on_param_change(&mut no_snapshot, manager_id, ParamValue::Float(0.0));
    assert!(manager.runtime_cache.structure_dirty.is_empty());
    manager.on_param_change(&mut no_snapshot, value, ParamValue::Float(0.0));
    assert!(manager.runtime_cache.structure_dirty.contains(&formula_uuid));
}

#[test]
fn managed_socket_value_edit_identifies_authored_item_and_invalidates_when_runtime_is_absent() {
    let (root, manager_id, processor, regions, region, anode, inputs, socket, value, value_type, position) = (
        NodeId(1), NodeId(2), NodeId(3), NodeId(4), NodeId(5), NodeId(6), NodeId(7), NodeId(8), NodeId(9), NodeId(10), NodeId(11),
    );
    let anode_uuid = NodeUuid(uuid::Uuid::from_u128(6));
    let mut nodes = HashMap::from([
        (root, context_scope_test_node(root, None, Some(manager_id), None, "root")),
        (manager_id, context_scope_test_node(manager_id, Some(root), None, Some(processor), "state_machine_manager")),
        (processor, context_scope_test_node(processor, Some(root), Some(regions), None, "state_processor")),
        (regions, context_scope_test_node(regions, Some(processor), Some(region), None, "folder")),
        (region, context_scope_test_node(region, Some(regions), Some(anode), None, "folder")),
        (anode, context_scope_test_node(anode, Some(region), Some(inputs), None, "alchemist_anode")),
        (inputs, context_scope_test_node(inputs, Some(anode), Some(socket), Some(position), "folder")),
        (socket, context_scope_test_node(socket, Some(inputs), Some(value), None, "alchemist_input_socket")),
        (value, context_scope_test_node(value, Some(socket), None, Some(value_type), "float")),
        (value_type, context_scope_test_node(value_type, Some(socket), None, None, "string")),
        (position, context_scope_test_node(position, Some(anode), None, None, "vec2")),
    ]);
    nodes.get_mut(&regions).unwrap().decl_id = "managed_regions".into();
    nodes.get_mut(&region).unwrap().decl_id = "managed_region/filters".into();
    nodes.get_mut(&anode).unwrap().uuid = anode_uuid;
    nodes.get_mut(&inputs).unwrap().decl_id = "inputs".into();
    nodes.get_mut(&socket).unwrap().decl_id = "inputs/out_max".into();
    nodes.get_mut(&value).unwrap().decl_id = "inputs/out_max/value".into();
    nodes.get_mut(&value).unwrap().param_value = Some(ParamValue::Float(1.0));
    nodes.get_mut(&value_type).unwrap().decl_id = "inputs/out_max/value_type".into();
    nodes.get_mut(&value_type).unwrap().param_value = Some(ParamValue::Str("float".into()));
    nodes.get_mut(&position).unwrap().decl_id = "position".into();
    let snapshot = Arc::new(ProcessTreeSnapshot::new(root, nodes));
    let location = managed_runtime_input_location(&snapshot, value).expect("managed socket location");
    assert_eq!(location.0, processor);
    assert_eq!(location.1.as_uuid(), anode_uuid.0);
    assert_eq!(location.2.as_str(), "out_max");
    assert_eq!(location.3, socket);

    let mut manager = StateMachineManager::new();
    manager.node_data_mut().id = manager_id;
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 1, micro: 0, seq: 0 });
    ctx.set_tree_snapshot(snapshot);
    manager.on_param_change(&mut ctx, position, ParamValue::Vec2(0.0, 0.0));
    assert!(manager.runtime_cache.dirty_processor_overrides.is_empty());
    manager.on_param_change(&mut ctx, value, ParamValue::Float(1.0));
    assert!(manager.runtime_cache.dirty_processor_overrides.contains(&processor));

    let (formula, processor_model, source) = managed_remap_processor(anode_uuid);
    let value_types = chataigne_state_machine::alchemist::value_type_registry();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ProcessorRuntime::new(processor_model.id);
    assert!(runtime.compile(&processor_model, &formula, &compile_ctx));
    runtime.managed_formula.as_mut().unwrap().reconcile_input_source_schema(|_| {
        Some(ChannelSourceSchema { value_type: ValueTypeId::new("float"), metadata: Default::default() })
    }).unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Float(5.0));
    let registries = RuntimeRegistries { value_types: &value_types };
    let before_ctx = EvaluationCtx {
        logical_tick: 1,
        delta_time: std::time::Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let before = runtime.managed_formula.as_mut().unwrap().evaluate(&before_ctx);
    assert!(before.diagnostics.is_empty(), "{:?}", before.diagnostics);
    assert_eq!(before.intents[0].payload, RuntimeValue::Float(0.5));
    manager.runtime_cache.dirty_processor_overrides.clear();
    manager.runtime_cache.processors.insert(processor, RuntimeProcessor {
        processor: processor_model.clone(),
        runtime,
        managed_sources: Vec::new(),
        compile_warning: None,
        formula,
        formula_node: None,
        formula_ui: ProcessorFormulaUiState::project(),
        formula_source_key: "test".to_owned(),
        command_dispatch_plans: Default::default(),
    });
    ctx.events.push_shared(Arc::new(Event {
        time: ctx.time,
        kind: EventKind::ParamChanged {
            param: value,
            old_value: ParamValue::Float(1.0),
            new_value: ParamValue::Float(2.0),
        },
    }));
    manager.on_param_change(&mut ctx, value, ParamValue::Float(1.0));
    assert!(manager.runtime_cache.dirty_processor_overrides.is_empty());
    let live = manager.runtime_cache.processors.get_mut(&processor).unwrap();
    assert_eq!(
        live.processor.formula_instance.managed_regions.regions[&ManagedRegionId::new("filters")].items[0]
            .anode.input_defaults[&SocketId::new("out_max")],
        RuntimeValue::Float(2.0),
    );
    let after_ctx = EvaluationCtx {
        logical_tick: 2,
        delta_time: std::time::Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let after = live.runtime.managed_formula.as_mut().unwrap().evaluate(&after_ctx);
    assert!(after.diagnostics.is_empty(), "{:?}", after.diagnostics);
    assert_eq!(after.intents[0].payload, RuntimeValue::Float(1.0));
}

pub(super) fn managed_remap_processor(item_uuid: NodeUuid) -> (AlchemistFormula, Processor, StableRef) {
    let definition = |id: &str, kind, role| ManagedRegionDefinition {
        id: ManagedRegionId::new(id),
        kind,
        label: id.to_owned(),
        input_socket: None,
        output_socket: None,
        accepted_roles: vec![role],
    };
    let formula = AlchemistFormula {
        id: FormulaId::new("test.managed_edit"),
        version: 1,
        label: "Managed edit".into(),
        description: None,
        tags: Vec::new(),
        graph: AlchemistGraphDomain::new_document(),
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface {
            sections: Vec::new(),
            managed_regions: vec![
                definition("inputs", ManagedRegionKind::InputSet, SurfaceItemKind::Input),
                definition("filters", ManagedRegionKind::FilterPipeline, SurfaceItemKind::Filter),
                definition("outputs", ManagedRegionKind::OutputSet, SurfaceItemKind::Output),
            ],
        },
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    };
    let source = StableRef::new(ValueTypeId::new("float"), "module/fader");
    let mut input = ANodeInstance::new(ANodeTypeId::new("managed_input"), "Fader");
    input.config.set(INPUT_SOURCE_FIELD, RuntimeValue::Ref(source.clone()));
    let mut remap = ANodeInstance::new(ANodeTypeId::new("remap"), "Remap");
    remap.input_defaults.insert(SocketId::new("in_min"), RuntimeValue::Float(0.0));
    remap.input_defaults.insert(SocketId::new("in_max"), RuntimeValue::Float(10.0));
    remap.input_defaults.insert(SocketId::new("out_min"), RuntimeValue::Float(0.0));
    remap.input_defaults.insert(SocketId::new("out_max"), RuntimeValue::Float(1.0));
    let mut output = ANodeInstance::new(ANodeTypeId::new("managed_output"), "Output");
    output.config.set(OUTPUT_TARGET_FIELD, RuntimeValue::Ref(StableRef::new(ValueTypeId::new("float"), "target/output")));
    let item = |anode, id| ManagedItemInstance {
        id,
        anode,
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    };
    let mut processor = Processor::from_formula("Managed edit", &formula);
    for (region, items) in [
        ("inputs", vec![item(input, ManagedItemId::new())]),
        ("filters", vec![item(remap, ManagedItemId::from_uuid(item_uuid.0))]),
        ("outputs", vec![item(output, ManagedItemId::new())]),
    ] {
        processor.formula_instance.managed_regions.regions.insert(
            ManagedRegionId::new(region),
            ManagedRegionInstance { region_id: ManagedRegionId::new(region), items },
        );
    }
    (formula, processor, source)
}

#[test]
fn constant_value_refresh_reuses_unchanged_anodes_in_manager_cache() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.apply_edits().expect("Formula Library should attach");
    let library = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .unwrap();
    engine.add_user_item(AlchemistFormulaDefinition::new().into(), Some(library));
    engine.apply_edits().expect("Formula should attach");
    let snapshot = engine.process_tree_snapshot();
    let formula = snapshot
        .child_ids(library)
        .into_iter()
        .find(|id| snapshot.node(*id).is_some_and(|node| node.node_type == AlchemistFormulaDefinition::NODE_TYPE))
        .unwrap();
    for _ in 0..2 {
        let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent: formula,
            node_type: "alchemist_anode:constant".into(),
            label: None,
            initial_params: Vec::new(),
        });
        assert!(ack.success, "Constant creation should succeed: {ack:?}");
    }
    let snapshot = engine.process_tree_snapshot();
    let formula_uuid = snapshot.node(formula).unwrap().uuid;
    let anodes = snapshot
        .child_ids(formula)
        .into_iter()
        .filter(|node| snapshot.node(*node).is_some_and(|node| node.node_type == "alchemist_anode"))
        .collect::<Vec<_>>();
    assert_eq!(anodes.len(), 2);
    let constant = anodes[0];
    let retained = anodes[1];
    let config = snapshot.find_child_by_decl_id(constant, "config").unwrap();
    let value = snapshot.find_child_by_decl_id(config, "config/value").unwrap();
    let before = snapshot.node(value).and_then(|node| node.param_value.clone()).unwrap();
    let after = match &before {
        ParamValue::Float(value) => ParamValue::Float(value + 1.0),
        other => panic!("Constant value should be a float, got {other:?}"),
    };
    let mut manager = StateMachineManager::new();
    manager.refresh_formula_cache(&snapshot);
    manager.runtime_cache.runtime_snapshot = Some(Arc::clone(&snapshot));
    let cache = manager.runtime_cache.formula_materialization.get(&formula_uuid).unwrap();
    let prior_constant = Arc::clone(cache.cached_instance(constant).unwrap());
    let prior_retained = Arc::clone(cache.cached_instance(retained).unwrap());

    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: value,
        value: after.clone(),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "Constant value edit should succeed: {ack:?}");
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 1, micro: 0, seq: 0 });
    ctx.events.push_shared(Arc::new(Event {
        time: ctx.time,
        kind: EventKind::ParamChanged {
            param: value,
            old_value: before,
            new_value: after.clone(),
        },
    }));
    manager.on_inbox(&mut ctx);
    assert!(manager.runtime_cache.structure_dirty.contains(&formula_uuid));
    assert!(!manager.update_requires_tree_snapshot());
    let mut update_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 2, micro: 0, seq: 0 });
    manager.update(&mut update_ctx);
    assert_eq!(
        manager
            .runtime_cache
            .runtime_snapshot
            .as_deref()
            .and_then(|snapshot| snapshot.node(value))
            .and_then(|node| node.param_value.as_ref()),
        Some(&after),
    );
    let cache = manager.runtime_cache.formula_materialization.get(&formula_uuid).unwrap();
    assert!(!Arc::ptr_eq(&prior_constant, cache.cached_instance(constant).unwrap()));
    assert!(Arc::ptr_eq(&prior_retained, cache.cached_instance(retained).unwrap()));

    manager.apply_runtime_invalidation(RuntimeInvalidation::Formula(formula_uuid));
    assert!(manager.update_requires_tree_snapshot());
    manager.refresh_formula_cache(&engine.process_tree_snapshot());
    let cache = manager.runtime_cache.formula_materialization.get(&formula_uuid).unwrap();
    assert!(!Arc::ptr_eq(&prior_retained, cache.cached_instance(retained).unwrap()));
}

#[test]
fn formula_children_do_not_reconcile_state_network_topology() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
        .join("test_simple_load.noisette");
    let engine = load_sparse_project_file::<AppNode, _>(fixture).expect("product fixture should load");
    let snapshot = engine.process_tree_snapshot();
    let formula = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == "alchemist_formula")
        .map(|(id, _)| id)
        .expect("fixture should contain a formula");
    let formula_child = snapshot
        .child_ids(formula)
        .into_iter()
        .next()
        .expect("formula should have a child");
    let state = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateMachineState::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("fixture should contain a state");
    let state_parent = snapshot.node(state).and_then(|node| node.parent).expect("state should have a parent");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateMachineManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("fixture should contain a manager");
    let mut manager = StateMachineManager::new();
    manager.node_data_mut().id = manager_id;
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    ctx.set_tree_snapshot(snapshot);

    assert!(!manager.child_change_affects_state_topology(&ctx, formula, formula_child));
    assert!(manager.child_change_affects_state_topology(&ctx, state_parent, state));
}

#[test]
fn generated_condition_validity_does_not_dirty_processor_overrides() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
        .join("test_simple_load.noisette");
    let engine = load_sparse_project_file::<AppNode, _>(fixture).expect("product fixture should load");
    let snapshot = engine.process_tree_snapshot();
    let condition = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == "sm_condition_manager")
        .map(|(id, _)| id)
        .expect("product fixture should contain a condition manager");
    let valid = snapshot
        .find_child_by_decl_id(condition, "valid")
        .expect("condition manager should expose validity");
    assert!(is_condition_valid_result(snapshot.as_ref(), valid));
    assert!(!is_condition_valid_result(snapshot.as_ref(), condition));

    let mut manager = StateMachineManager::new();
    manager.runtime_cache.runtime_snapshot = Some(Arc::clone(&snapshot));
    manager.runtime_cache.topology_dirty = false;
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    ctx.events.push_shared(Arc::new(Event {
        time: ctx.time,
        kind: EventKind::ParamChanged {
            param: valid,
            old_value: ParamValue::Bool(false),
            new_value: ParamValue::Bool(true),
        },
    }));
    assert!(!manager.inbox_requires_tree_snapshot(&ctx.events));
    manager.on_inbox(&mut ctx);
    assert!(manager.runtime_cache.dirty_processor_overrides.is_empty());
    assert_eq!(manager.runtime_cache.condition_valid_param_values.get(&valid), Some(&true));
    assert!(!manager.update_requires_tree_snapshot());

    manager.runtime_cache.context_provider_params.insert(valid);
    assert!(manager.inbox_requires_tree_snapshot(&ctx.events));
    assert!(runtime_param_change_requires_snapshot(false, false, false, false));
    assert!(!runtime_param_change_requires_snapshot(false, false, false, true));
    assert!(manager.inbox_requires_tree_snapshot(&event_for_unknown_param(ctx.time)));
}

#[test]
fn generated_condition_validity_uses_live_value_when_snapshot_is_stale() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("samples")
        .join("test_simple_load.noisette");
    let engine = load_sparse_project_file::<AppNode, _>(fixture).expect("product fixture should load");
    let snapshot = engine.process_tree_snapshot();
    let condition = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == "sm_condition_manager")
        .map(|(id, _)| id)
        .expect("product fixture should contain a condition manager");
    let valid = snapshot
        .find_child_by_decl_id(condition, "valid")
        .expect("condition manager should expose validity");
    assert_eq!(snapshot.node(valid).and_then(|node| node.param_value.as_ref()).and_then(ParamValue::as_bool), Some(false));
    let time = EngineTime {
        tick: 1,
        micro: 0,
        seq: 0,
    };
    let mut values = HashMap::new();
    let mut true_tick = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    set_condition_valid_param(&mut true_tick, snapshot.as_ref(), condition, true, &mut values);
    assert_eq!(true_tick.edits.pending.len(), 1);
    assert_eq!(values.get(&valid), Some(&true));

    let mut false_tick = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    set_condition_valid_param(&mut false_tick, snapshot.as_ref(), condition, false, &mut values);
    assert_eq!(false_tick.edits.pending.len(), 1, "the stale false snapshot must not hide a true-to-false edit");
    assert_eq!(values.get(&valid), Some(&false));

    let mut settled_tick = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    set_condition_valid_param(&mut settled_tick, snapshot.as_ref(), condition, false, &mut values);
    assert!(settled_tick.edits.pending.is_empty());
}

fn event_for_unknown_param(time: EngineTime) -> golden_core::events::EventFrame {
    let mut events = golden_core::events::EventFrame::default();
    events.push_shared(Arc::new(Event {
        time,
        kind: EventKind::ParamChanged {
            param: NodeId(u64::MAX),
            old_value: ParamValue::Bool(false),
            new_value: ParamValue::Bool(true),
        },
    }));
    events
}
