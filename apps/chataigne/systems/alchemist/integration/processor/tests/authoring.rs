use std::{sync::Arc, time::Duration};

use chataigne_alchemist::{
    CompileCtx, EvaluationCtx, RuntimeInputSnapshot, RuntimeRegistries, StableRef, ValueTypeId,
};
use chataigne_state_machine::{
    alchemist::{shared_node_registry, shared_value_type_registry},
    ManagedFormulaRuntime,
};
use golden_core::{
    app::ProjectFileSpec,
    application::ProductionRuntime,
    node::{Folder, Node, NodeId, NodeReference, NodeUuid},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    ui_sync::{UiEditIntent, UiNodeDataDto, UiProjectFileSpec, UiSubscriptionScope},
};
use golden_values::Value as RuntimeValue;

use crate::app::{
    AlchemistFormulaDefinition, AppEngine, AppNode, FormulaLibrary, OutputsManager, StateProcessor,
    StateProcessorManager,
};
use crate::app::systems_alchemist_formula::{
    anode_from_snapshot, formula_from_snapshot, ANODE_CREATE_PREFIX,
};
use crate::app::systems_alchemist_generic_commands::GENERIC_LOG_COMMAND_NODE_TYPE;
use crate::app::systems_alchemist_processor::{
    managed_regions_from_snapshot, managed_source_schema, processor_managed_region_decl_id,
    FormulaSourceRef, PROCESSOR_MANAGED_REGIONS_DECL_ID,
};
use chataigne_state_machine::{
    CommandArgumentValues, OutputArgumentBinding, OutputBindingConfig, OutputSendPolicy, OutputValueSource,
};

use super::super::sync_external_formulas;

fn mapping_engine() -> (AppEngine, NodeUuid, NodeId) {
    let root: AppNode = Folder::new("Mapping authoring").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.apply_edits().unwrap();
    sync_external_formulas(&mut engine).unwrap();
    engine.add_node(StateProcessorManager::new().into(), None);
    for _ in 0..4 {
        engine.apply_edits().unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    let mapping = engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
                && node.node_data().meta.label == "Mapping"
        })
        .map(|(id, node)| (id, node.node_data().meta.uuid))
        .expect("one Mapping builtin should load");
    let manager = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .unwrap();
    assert_eq!(snapshot.node_id_by_uuid(mapping.1), Some(mapping.0));
    let create_type = FormulaSourceRef::project_uuid(mapping.1).processor_create_type();
    assert!(
        engine.nodes.get(manager).unwrap().user_creatable_items().iter().any(|item| item.node_type == create_type),
        "processor palette should include Mapping: {:?}",
        engine.nodes.get(manager).unwrap().user_creatable_items()
    );
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: manager,
        node_type: create_type,
        label: None,
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Mapping processor should be creatable: {ack:?}");
    for _ in 0..3 {
        engine.apply_edits().unwrap();
    }
    let processor = engine
        .process_tree_snapshot()
        .child_ids(manager)
        .into_iter()
        .find(|id| engine.nodes.get(*id).is_some_and(|node| node.get_type() == StateProcessor::NODE_TYPE))
        .unwrap();
    (engine, mapping.1, processor)
}

fn region(engine: &AppEngine, processor: NodeId, id: &str) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    let regions = snapshot
        .find_child_by_decl_id(processor, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .unwrap();
    let available = snapshot.child_ids(regions).into_iter().filter_map(|child| snapshot.node(child)).map(|node| (node.decl_id.clone(), node.node_type.clone())).collect::<Vec<_>>();
    snapshot
        .find_child_by_decl_id(regions, &processor_managed_region_decl_id(id))
        .unwrap_or_else(|| panic!("missing {id}: {available:?}"))
}

fn create_item(engine: &mut AppEngine, parent: NodeId, node_type: &str) -> NodeId {
    let before = engine.process_tree_snapshot().child_ids(parent);
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent,
        node_type: node_type.to_owned(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(ack.success, "item {node_type} should be creatable: {ack:?}");
    engine.apply_edits().unwrap();
    engine
        .process_tree_snapshot()
        .child_ids(parent)
        .into_iter()
        .find(|id| !before.contains(id))
        .unwrap()
}

fn set_config(engine: &mut AppEngine, item: NodeId, field: &str, value: ParamValue) {
    let snapshot = engine.process_tree_snapshot();
    let config = snapshot.find_child_by_decl_id(item, "config").unwrap();
    let field = snapshot
        .find_child_by_decl_id(config, &format!("config/{field}"))
        .unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: field,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "config field should accept the edit: {ack:?}");
    engine.apply_edits().unwrap();
}

fn set_socket_default(engine: &mut AppEngine, item: NodeId, socket: &str, value: ParamValue) {
    let snapshot = engine.process_tree_snapshot();
    let inputs = snapshot.find_child_by_decl_id(item, "inputs").unwrap();
    let socket_node = snapshot.find_child_by_decl_id(inputs, &format!("inputs/{socket}")).unwrap();
    let parameter = snapshot.find_child_by_decl_id(socket_node, &format!("inputs/{socket}/value")).unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: parameter,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "socket default should accept the edit: {ack:?}");
}

fn source_param(engine: &mut AppEngine, label: &str, value: f64) -> NodeUuid {
    let parameter = Parameter::new(label, ParamValue::Float(value), ParameterChangeCheck::ValueChange);
    let uuid = parameter.node_data().meta.uuid;
    engine.add_node(parameter.into(), None);
    engine.apply_edits().unwrap();
    uuid
}

#[test]
fn mapping_can_be_authored_through_backend_intents_and_reloaded() {
    let (mut engine, mapping_uuid, processor) = mapping_engine();
    let inputs = region(&engine, processor, "inputs");
    let filters = region(&engine, processor, "filters");
    let outputs = region(&engine, processor, "outputs");
    let input_type = format!("{ANODE_CREATE_PREFIX}chataigne.input_source");
    let output_type = format!("{ANODE_CREATE_PREFIX}chataigne.output_target");
    let first_source = source_param(&mut engine, "X", 2.0);
    let second_source = source_param(&mut engine, "Y", 3.0);
    let first = create_item(&mut engine, inputs, &input_type);
    let second = create_item(&mut engine, inputs, &input_type);
    set_config(&mut engine, first, "source", ParamValue::Reference(NodeReference::new(first_source)));
    set_config(&mut engine, second, "source", ParamValue::Reference(NodeReference::new(second_source)));

    let remap_type = engine.nodes.get(filters).unwrap().user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}remap@managed/0")))
        .map(|item| item.node_type)
        .expect("two Float sources should expose elementwise Remap");
    let remap = create_item(&mut engine, filters, &remap_type);
    set_socket_default(&mut engine, remap, "in_max", ParamValue::Float(5.0));
    let remap_instance = anode_from_snapshot(&engine.process_tree_snapshot(), remap).unwrap();
    assert_eq!(remap_instance.input_defaults.get(&chataigne_alchemist::SocketId::new("in_max")), Some(&RuntimeValue::Float(5.0)));

    let sum_type = engine.nodes.get(filters).unwrap().user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}sum")))
        .map(|item| item.node_type)
        .expect("two Float sources should expose Sum");
    let before_events = engine.ui_event_log().len();
    let before_undo = engine.undo_len();
    let sum = create_item(&mut engine, filters, &sum_type);
    assert_eq!(engine.undo_len(), before_undo + 1, "one configured filter tree should be one history step");
    assert!(engine.ui_event_log().len().saturating_sub(before_events) < 256, "one filter tree should emit a bounded event batch");
    let sum_instance = anode_from_snapshot(&engine.process_tree_snapshot(), sum).unwrap();
    assert_eq!(sum_instance.input_defaults.len(), 2);

    let command_manager = engine.process_tree_snapshot().child_ids(processor)
        .into_iter()
        .find(|id| engine.nodes.get(*id).is_some_and(|node| node.get_type() == OutputsManager::NODE_TYPE))
        .expect("Mapping processor should expose its command manager");
    let command = create_item(&mut engine, command_manager, GENERIC_LOG_COMMAND_NODE_TYPE);
    let command_uuid = engine.nodes.get(command).unwrap().node_data().meta.uuid;
    let message = engine.process_tree_snapshot().find_child_by_decl_id(command, "message").unwrap();
    let message_uuid = engine.nodes.get(message).unwrap().node_data().meta.uuid;
    let output = create_item(&mut engine, outputs, &output_type);
    set_config(&mut engine, output, "target", ParamValue::Reference(NodeReference::new(command_uuid)));
    let bindings = OutputBindingConfig {
        value: OutputValueSource::Whole,
        arguments: vec![OutputArgumentBinding {
            parameter: StableRef::new(ValueTypeId::new("string"), message_uuid.0.to_string()),
            source: OutputValueSource::Constant(RuntimeValue::String(Arc::from("mapped"))),
        }],
        send_policy: OutputSendPolicy::OnChange,
    };
    set_config(&mut engine, output, "bindings", ParamValue::Str(bindings.to_authoring_json().unwrap()));

    let snapshot = engine.process_tree_snapshot();
    let mapping = snapshot.node_id_by_uuid(mapping_uuid).unwrap();
    let formula = formula_from_snapshot(&snapshot, mapping).unwrap();
    assert_eq!(formula.surface.managed_regions.len(), 3);
    assert_eq!(formula.surface.managed_regions[1].filter_value_mode, chataigne_alchemist::ManagedFilterValueMode::Tuple);
    let mut instance = formula.instantiate();
    instance.managed_regions = managed_regions_from_snapshot(&snapshot, processor, &formula).unwrap();
    let ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&formula.properties),
    };
    let mut compiled = ManagedFormulaRuntime::compile(&formula, &instance, &ctx).unwrap().unwrap();
    compiled.reconcile_input_source_schema(|source| managed_source_schema(&snapshot, source)).unwrap();
    assert_eq!(compiled.input_layout().unwrap().channels().len(), 2);
    let mut inputs = RuntimeInputSnapshot::default();
    for (item, value) in [(first, 2.0), (second, 3.0)] {
        let source = anode_from_snapshot(&snapshot, item).unwrap();
        let Some(RuntimeValue::Ref(reference)) = source.config.get("source") else { panic!("source should be a reference") };
        inputs.insert(reference.clone(), RuntimeValue::Float(value));
    }
    let registries = RuntimeRegistries { value_types: shared_value_type_registry() };
    let output_frame = compiled.evaluate(&EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });
    assert!(output_frame.diagnostics.is_empty(), "{:?}", output_frame.diagnostics);
    let command_id = command_uuid.0.to_string();
    let command_intent = output_frame.intents.iter().find(|intent| intent.target.as_ref().is_some_and(|target| target.stable_id.as_ref() == command_id)).unwrap();
    let arguments = CommandArgumentValues::from_runtime_value(&command_intent.payload).unwrap().unwrap();
    assert_eq!(arguments.value, RuntimeValue::Float(1.0));
    assert_eq!(arguments.arguments[0].value, RuntimeValue::String(Arc::from("mapped")));
    set_socket_default(&mut engine, remap, "in_max", ParamValue::Float(4.0));
    let revised_snapshot = engine.process_tree_snapshot();
    let mut revised_instance = formula.instantiate();
    revised_instance.managed_regions = managed_regions_from_snapshot(&revised_snapshot, processor, &formula).unwrap();
    let mut revised = ManagedFormulaRuntime::compile(&formula, &revised_instance, &ctx).unwrap().unwrap();
    revised.reconcile_input_source_schema(|source| managed_source_schema(&revised_snapshot, source)).unwrap();
    let revised_output = revised.evaluate(&EvaluationCtx {
        logical_tick: 2,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });
    assert!(revised_output.diagnostics.is_empty(), "{:?}", revised_output.diagnostics);
    let revised_command = revised_output.intents.iter().find(|intent| intent.target.as_ref().is_some_and(|target| target.stable_id.as_ref() == command_id)).unwrap();
    let revised_arguments = CommandArgumentValues::from_runtime_value(&revised_command.payload).unwrap().unwrap();
    assert_eq!(revised_arguments.value, RuntimeValue::Float(1.25));
    let authored_output = anode_from_snapshot(&snapshot, output).unwrap();
    assert!(matches!(authored_output.config.get("bindings"), Some(RuntimeValue::String(_))));

    let input_uuids = [first, second].map(|id| snapshot.node(id).unwrap().uuid);
    let remap_uuid = snapshot.node(remap).unwrap().uuid;
    let sum_uuid = snapshot.node(sum).unwrap().uuid;
    let output_uuid = snapshot.node(output).unwrap().uuid;
    let project = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&project).unwrap();
    sync_external_formulas(&mut loaded).unwrap();
    let loaded_snapshot = loaded.process_tree_snapshot();
    for uuid in input_uuids.into_iter().chain([remap_uuid, sum_uuid, output_uuid]) {
        assert!(loaded_snapshot.node_id_by_uuid(uuid).is_some(), "authored item {uuid:?} should survive reload");
    }
    let loaded_output = loaded_snapshot.node_id_by_uuid(output_uuid).unwrap();
    let loaded_instance = anode_from_snapshot(&loaded_snapshot, loaded_output).unwrap();
    assert_eq!(loaded_instance.config.get("bindings"), authored_output.config.get("bindings"));
}

#[test]
fn mapping_input_intents_preserve_order_and_history() {
    let (mut engine, _, processor) = mapping_engine();
    let inputs = region(&engine, processor, "inputs");
    let input_type = format!("{ANODE_CREATE_PREFIX}chataigne.input_source");
    let first = create_item(&mut engine, inputs, &input_type);
    let second = create_item(&mut engine, inputs, &input_type);
    let snapshot = engine.process_tree_snapshot();
    let first_uuid = snapshot.node(first).unwrap().uuid;
    let second_uuid = snapshot.node(second).unwrap().uuid;

    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: second,
        new_parent: inputs,
        new_prev_sibling: None,
    });
    assert!(ack.success, "reorder should apply: {ack:?}");
    assert_eq!(engine.process_tree_snapshot().child_ids(inputs), vec![second, first]);
    assert!(engine.apply_ui_intent(UiEditIntent::Undo).success);
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.child_ids(inputs).iter().map(|id| snapshot.node(*id).unwrap().uuid).collect::<Vec<_>>(), vec![first_uuid, second_uuid]);
    assert!(engine.apply_ui_intent(UiEditIntent::Redo).success);
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.child_ids(inputs).iter().map(|id| snapshot.node(*id).unwrap().uuid).collect::<Vec<_>>(), vec![second_uuid, first_uuid]);

    let first = snapshot.node_id_by_uuid(first_uuid).unwrap();
    assert!(engine.apply_ui_intent(UiEditIntent::RemoveNode { node: first }).success);
    assert!(engine.process_tree_snapshot().node_id_by_uuid(first_uuid).is_none());
    assert!(engine.apply_ui_intent(UiEditIntent::Undo).success);
    assert!(engine.process_tree_snapshot().node_id_by_uuid(first_uuid).is_some());
    assert!(engine.apply_ui_intent(UiEditIntent::Redo).success);
    assert!(engine.process_tree_snapshot().node_id_by_uuid(first_uuid).is_none());
}

#[test]
fn mapping_input_duplicate_owns_a_distinct_authored_identity() {
    let (mut engine, _, processor) = mapping_engine();
    let inputs = region(&engine, processor, "inputs");
    let source_uuid = source_param(&mut engine, "Source", 4.0);
    let original = create_item(&mut engine, inputs, &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"));
    set_config(&mut engine, original, "source", ParamValue::Reference(NodeReference::new(source_uuid)));
    let original_uuid = engine.process_tree_snapshot().node(original).unwrap().uuid;
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(ProjectFileSpec::new("Mapping", "mapping"), None),
    );
    let result = runtime.apply_ui_transaction(UiEditIntent::DuplicateNode {
        source: original,
        new_parent: inputs,
        new_prev_sibling: Some(original),
        initial_params: Vec::new(),
    }, Some("mapping-authoring-test"));
    assert!(result.acknowledgement.success, "input duplication should apply: {:?}", result.acknowledgement);
    let snapshot = runtime.read_model().snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    let input_region = snapshot.nodes.iter().find(|node| node.node_id == inputs).unwrap();
    assert_eq!(input_region.children.len(), 2);
    let identities = input_region.children.iter().map(|id| snapshot.nodes.iter().find(|node| node.node_id == *id).unwrap().uuid).collect::<Vec<_>>();
    assert_eq!(identities[0], original_uuid);
    assert_ne!(identities[1], original_uuid);
    assert_ne!(identities[0], identities[1]);
    for item in &input_region.children {
        let item = snapshot.nodes.iter().find(|node| node.node_id == *item).unwrap();
        let config = snapshot.nodes.iter().find(|node| item.children.contains(&node.node_id) && node.decl_id.0 == "config").unwrap();
        let source = snapshot.nodes.iter().find(|node| config.children.contains(&node.node_id) && node.decl_id.0 == "config/source").unwrap();
        let UiNodeDataDto::Parameter { param } = &source.data else { panic!("source should be a parameter") };
        assert_eq!(param.value, ParamValue::Reference(NodeReference::new(source_uuid)));
    }
}
