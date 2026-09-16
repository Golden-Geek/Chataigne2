use std::{sync::Arc, time::Duration};

use chataigne_alchemist::{
    AxisSet, CompileCtx, ContextAxisId, ContextKey, DebugCaptureMode, EvaluationCtx,
    ManagedItemId, RuntimeEvent, RuntimeInputSnapshot, RuntimeRegistries, SocketId, StableRef,
    TriggerValue, ValueComponent, ValueTypeId,
};
use chataigne_state_machine::{
    alchemist::{shared_node_registry, shared_value_type_registry},
    ManagedFormulaRuntime, RuntimeInputBinding,
};
use golden_core::{
    app::ProjectFileSpec,
    edit::Edit,
    application::ProductionRuntime,
    node::{Folder, Node, NodeId, NodeReference, NodeUuid},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    ui_sync::{UiEditIntent, UiNodeDataDto, UiProjectFileSpec, UiSubscriptionScope},
};
use golden_values::Value as RuntimeValue;

use crate::app::{
    AlchemistFormulaDefinition, AppEngine, AppNode, FormulaLibrary, OutputsManager, StateMachineManager,
    StateMachineState, StateProcessor, StateProcessorManager,
};
use crate::app::systems_alchemist_formula::{
    anode_from_snapshot, create_anode_user_item_tree, formula_from_snapshot,
    runtime_value_to_param, ANODE_CREATE_PREFIX,
};
use crate::app::systems_alchemist_generic_commands::{
    GENERIC_INVOKE_COMMAND_NODE_TYPE, GENERIC_LOG_COMMAND_NODE_TYPE,
    GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE,
};
use crate::app::systems_alchemist_managed_nodes::{
    mapping_output_binding_config,
};
use crate::app::systems_alchemist_processor::{
    managed_regions_from_snapshot, managed_source_schema,
    migrate_mapping_output_adapters, processor_managed_region_decl_id,
    FormulaSourceRef, MAPPING_CONCRETE_OUTPUTS_V1_TAG,
    PROCESSOR_MANAGED_REGIONS_DECL_ID,
};
use chataigne_state_machine::{
    CommandArgumentValues, OutputArgumentBinding, OutputBindingConfig, OutputSendPolicy, OutputValueSource,
};

use super::super::sync_external_formulas;

mod script_controls;
mod performance;
mod factory;
mod filter_creation;
mod corrective_baseline;
mod r05_product;

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
    let available = snapshot.child_ids(processor).into_iter().filter_map(|child| snapshot.node(child)).map(|node| (node.decl_id.clone(), node.node_type.clone())).collect::<Vec<_>>();
    snapshot
        .find_child_by_decl_id(processor, &processor_managed_region_decl_id(id))
        .unwrap_or_else(|| panic!("missing {id}: {available:?}"))
}

fn create_item(engine: &mut AppEngine, parent: NodeId, node_type: &str) -> NodeId {
    let node_type = if node_type
        == format!("{ANODE_CREATE_PREFIX}chataigne.output_target")
    {
        GENERIC_INVOKE_COMMAND_NODE_TYPE
    } else {
        node_type
    };
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
    if snapshot.node(item).is_some_and(|node| {
        node.node_type == GENERIC_INVOKE_COMMAND_NODE_TYPE
            || node.node_type == GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE
    }) && field == "target"
    {
        drop(snapshot);
        set_direct_child_param(engine, item, "target", value);
        return;
    }
    if snapshot.node(item).is_some_and(|node| {
        node.node_type == GENERIC_INVOKE_COMMAND_NODE_TYPE
            || node.node_type == GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE
    }) && field == "bindings"
    {
        let ParamValue::Str(document) = value else {
            panic!("Mapping bindings must be an authored JSON document");
        };
        let mut config = OutputBindingConfig::from_authoring_json(&document).unwrap();
        if snapshot
            .node(item)
            .is_some_and(|node| node.node_type == GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE)
            && config.arguments.is_empty()
        {
            let value = snapshot.find_child_by_decl_id(item, "value").unwrap();
            config.arguments.push(OutputArgumentBinding {
                parameter: StableRef::new(
                    ValueTypeId::new("float"),
                    snapshot.node(value).unwrap().uuid.0.to_string(),
                ),
                source: config.value.clone(),
            });
        }
        drop(snapshot);
        set_mapping_bindings(engine, item, &config);
        let snapshot = engine.process_tree_snapshot();
        assert!(
            mapping_output_binding_config(&snapshot, item).is_ok(),
            "typed Mapping binding should materialize: {:?}",
            mapping_output_binding_config(&snapshot, item)
        );
        return;
    }
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

fn set_mapping_bindings(
    engine: &mut AppEngine,
    command: NodeId,
    config: &OutputBindingConfig,
) {
    let snapshot = engine.process_tree_snapshot();
    let bindings = snapshot
        .find_child_by_decl_id(command, "mapping_bindings")
        .unwrap();
    let existing_arguments = snapshot
        .child_ids(bindings)
        .into_iter()
        .filter(|child| {
            snapshot.node(*child).is_some_and(|node| {
                node.node_type == "mapping_command_argument_binding"
            })
        })
        .collect::<Vec<_>>();
    drop(snapshot);
    set_authored_value_source(engine, bindings, &config.value);
    set_direct_child_param(
        engine,
        bindings,
        "send_policy",
        ParamValue::Enum(
            match config.send_policy {
                OutputSendPolicy::EveryDelivery => "every_delivery",
                OutputSendPolicy::OnChange => "on_change",
            }
            .to_owned(),
        ),
    );
    for argument in existing_arguments {
        engine.edits.push(Edit::RemoveNode { node: argument });
    }
    engine.apply_edits().unwrap();
    for argument in &config.arguments {
        let binding = create_item(
            engine,
            bindings,
            "mapping_command_argument_binding",
        );
        let uuid = argument.parameter.stable_id.parse::<uuid::Uuid>().unwrap();
        set_direct_child_param(
            engine,
            binding,
            "parameter",
            ParamValue::Reference(NodeReference::new(NodeUuid(uuid))),
        );
        set_authored_value_source(engine, binding, &argument.source);
    }
}

fn set_authored_value_source(
    engine: &mut AppEngine,
    owner: NodeId,
    source: &OutputValueSource,
) {
    let (kind, element, component, constant) = match source {
        OutputValueSource::Whole => ("whole", "", "x", ParamValue::Bool(false)),
        OutputValueSource::Element(element) => {
            ("element", element.as_str(), "x", ParamValue::Bool(false))
        }
        OutputValueSource::Component { element, component } => (
            "component",
            element.as_ref().map_or("", |element| element.as_str()),
            match component {
                ValueComponent::X => "x",
                ValueComponent::Y => "y",
                ValueComponent::Z => "z",
                ValueComponent::R => "r",
                ValueComponent::G => "g",
                ValueComponent::B => "b",
                ValueComponent::A => "a",
            },
            ParamValue::Bool(false),
        ),
        OutputValueSource::Constant(value) => (
            "constant",
            "",
            "x",
            runtime_value_to_param(value).unwrap(),
        ),
    };
    set_direct_child_param(
        engine,
        owner,
        "value_source",
        ParamValue::Enum(kind.to_owned()),
    );
    set_direct_child_param(
        engine,
        owner,
        "value_element",
        ParamValue::Str(element.to_owned()),
    );
    set_direct_child_param(
        engine,
        owner,
        "value_component",
        ParamValue::Enum(component.to_owned()),
    );
    let (constant_type, constant_decl) = match constant {
        ParamValue::Bool(_) => ("bool", "value_constant_bool"),
        ParamValue::Int(_) => ("int", "value_constant_int"),
        ParamValue::Float(_) => ("float", "value_constant_float"),
        ParamValue::Str(_) => ("string", "value_constant_string"),
        ParamValue::Vec2(_, _) => ("vec2", "value_constant_vec2"),
        ParamValue::Vec3(_, _, _) => ("vec3", "value_constant_vec3"),
        ParamValue::Color(_, _, _, _) => ("color", "value_constant_color"),
        _ => panic!("unsupported standard Mapping constant"),
    };
    set_direct_child_param(
        engine,
        owner,
        "value_constant_type",
        ParamValue::Enum(constant_type.to_owned()),
    );
    set_direct_child_param(engine, owner, constant_decl, constant);
}

fn set_direct_child_param(
    engine: &mut AppEngine,
    item: NodeId,
    decl_id: &str,
    value: ParamValue,
) {
    let parameter = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(item, decl_id)
        .unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: parameter,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "command field should accept the edit: {ack:?}");
    engine.apply_edits().unwrap();
}

fn create_parameter_output(
    engine: &mut AppEngine,
    parent: NodeId,
    target: NodeUuid,
) -> NodeId {
    let output = create_item(engine, parent, GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE);
    set_config(
        engine,
        output,
        "target",
        ParamValue::Reference(NodeReference::new(target)),
    );
    set_config(
        engine,
        output,
        "bindings",
        ParamValue::Str(OutputBindingConfig::default().to_authoring_json().unwrap()),
    );
    output
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

fn transitional_command_manager(engine: &mut AppEngine) -> NodeId {
    let manager = OutputsManager::new();
    let uuid = manager.node_data().meta.uuid;
    engine.add_node(manager.into(), None);
    engine.apply_edits().unwrap();
    engine
        .process_tree_snapshot()
        .node_id_by_uuid(uuid)
        .expect("transitional OutputTarget command should remain addressable")
}

#[test]
fn action_and_mapping_catalogs_create_the_same_concrete_command() {
    let (mut engine, _, processor) = mapping_engine();
    let mapping_outputs = region(&engine, processor, "outputs");
    let action_outputs = transitional_command_manager(&mut engine);

    for container in [mapping_outputs, action_outputs] {
        assert!(
            engine
                .nodes
                .get(container)
                .unwrap()
                .user_creatable_items()
                .iter()
                .any(|item| item.node_type == GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE),
            "both output contexts should advertise Set Parameter"
        );
    }

    let mapping_command = create_item(
        &mut engine,
        mapping_outputs,
        GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE,
    );
    let action_command = create_item(
        &mut engine,
        action_outputs,
        GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE,
    );
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(
        snapshot.node(mapping_command).unwrap().node_type,
        snapshot.node(action_command).unwrap().node_type
    );
    assert!(
        snapshot
            .find_child_by_decl_id(mapping_command, "mapping_bindings")
            .is_some(),
        "Mapping should add its binding controls around the concrete command"
    );
    assert!(
        snapshot
            .find_child_by_decl_id(action_command, "mapping_bindings")
            .is_none(),
        "Action should use the concrete command directly"
    );
}

#[test]
fn mapping_command_bindings_are_ordinary_addressable_controls() {
    let (mut engine, _, processor) = mapping_engine();
    let outputs = region(&engine, processor, "outputs");
    let command = create_item(
        &mut engine,
        outputs,
        GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE,
    );
    let snapshot = engine.process_tree_snapshot();
    let bindings = snapshot
        .find_child_by_decl_id(command, "mapping_bindings")
        .expect("Mapping command should expose its binding folder");
    let binding_fields = [
        "value_source",
        "value_element",
        "value_component",
        "value_constant_type",
        "value_constant_bool",
        "value_constant_int",
        "value_constant_float",
        "value_constant_string",
        "value_constant_vec2",
        "value_constant_vec3",
        "value_constant_color",
        "send_policy",
        "unresolved_legacy_bindings",
    ];
    for field in binding_fields {
        let node = snapshot
            .find_child_by_decl_id(bindings, field)
            .unwrap_or_else(|| panic!("Mapping binding should expose {field}"));
        assert!(
            snapshot.node(node).unwrap().is_parameter(),
            "Mapping binding {field} should be an ordinary parameter"
        );
    }
    drop(snapshot);

    let argument = create_item(
        &mut engine,
        bindings,
        "mapping_command_argument_binding",
    );
    let snapshot = engine.process_tree_snapshot();
    for field in [
        "parameter",
        "value_source",
        "value_element",
        "value_component",
        "value_constant_type",
        "value_constant_bool",
        "value_constant_int",
        "value_constant_float",
        "value_constant_string",
        "value_constant_vec2",
        "value_constant_vec3",
        "value_constant_color",
    ] {
        let node = snapshot
            .find_child_by_decl_id(argument, field)
            .unwrap_or_else(|| panic!("Argument binding should expose {field}"));
        assert!(snapshot.node(node).unwrap().is_parameter());
    }
    let value_source = snapshot
        .find_child_by_decl_id(bindings, "value_source")
        .unwrap();
    drop(snapshot);

    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(
            ProjectFileSpec::new("Mapping", "mapping"),
            None,
        ),
    );
    let result = runtime.apply_ui_transaction(
        UiEditIntent::SetParam {
            node: value_source,
            value: ParamValue::Enum("constant".to_owned()),
            behaviour: ParameterEventBehaviour::Coalesce,
        },
        Some("mapping-binding-control-audit"),
    );
    assert!(
        result.acknowledgement.success,
        "ordinary binding edit should apply through ProductionRuntime: {:?}",
        result.acknowledgement
    );
    let snapshot = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    let edited = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == value_source)
        .unwrap();
    let UiNodeDataDto::Parameter { param } = &edited.data else {
        panic!("binding control should remain a normal parameter");
    };
    assert_eq!(param.value, ParamValue::Enum("constant".to_owned()));
}

#[test]
fn legacy_mapping_output_migrates_to_explicit_invoke_without_moving_target() {
    let (mut engine, _, processor) = mapping_engine();
    let outputs = region(&engine, processor, "outputs");
    let action_outputs = transitional_command_manager(&mut engine);
    let command = create_item(&mut engine, action_outputs, GENERIC_LOG_COMMAND_NODE_TYPE);
    let snapshot = engine.process_tree_snapshot();
    let command_uuid = snapshot.node(command).unwrap().uuid;
    let message = snapshot.find_child_by_decl_id(command, "message").unwrap();
    let message_uuid = snapshot.node(message).unwrap().uuid;
    drop(snapshot);

    let legacy_type = format!("{ANODE_CREATE_PREFIX}chataigne.output_target");
    let tree = create_anode_user_item_tree(&legacy_type).unwrap();
    let legacy_uuid = tree.node.node_data().meta.uuid;
    engine.edits.push(Edit::AddNodeTree {
        parent: outputs,
        prev_sibling: None,
        tree,
    });
    engine.apply_edits().unwrap();
    let legacy = engine
        .process_tree_snapshot()
        .node_id_by_uuid(legacy_uuid)
        .unwrap();
    set_config(
        &mut engine,
        legacy,
        "target",
        ParamValue::Reference(NodeReference::new(command_uuid)),
    );
    let expected = OutputBindingConfig {
        value: OutputValueSource::Whole,
        arguments: vec![OutputArgumentBinding {
            parameter: StableRef::new(
                ValueTypeId::new("string"),
                message_uuid.0.to_string(),
            ),
            source: OutputValueSource::Constant(RuntimeValue::String(Arc::from(
                "migrated",
            ))),
        }],
        send_policy: OutputSendPolicy::OnChange,
    };
    set_config(
        &mut engine,
        legacy,
        "bindings",
        ParamValue::Str(expected.to_authoring_json().unwrap()),
    );

    migrate_mapping_output_adapters(&mut engine).unwrap();
    engine.run_tick(Duration::ZERO).unwrap();
    engine.run_tick(Duration::ZERO).unwrap();
    let snapshot = engine.process_tree_snapshot();
    let migrated = snapshot.node_id_by_uuid(legacy_uuid).unwrap();
    assert_eq!(
        snapshot.node(migrated).unwrap().node_type,
        GENERIC_INVOKE_COMMAND_NODE_TYPE
    );
    assert_eq!(snapshot.node(command).unwrap().parent, Some(action_outputs));
    assert_eq!(mapping_output_binding_config(&snapshot, migrated).unwrap(), expected);
    assert!(snapshot
        .node(snapshot.root())
        .unwrap()
        .tags
        .iter()
        .any(|tag| tag == MAPPING_CONCRETE_OUTPUTS_V1_TAG));
    assert!(!snapshot.child_ids(outputs).into_iter().any(|child| {
        snapshot
            .node(child)
            .is_some_and(|node| node.node_type == "alchemist_anode")
    }));

    let project = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&project).unwrap();
    let loaded_snapshot = loaded.process_tree_snapshot();
    let loaded_output = loaded_snapshot.node_id_by_uuid(legacy_uuid).unwrap();
    assert_eq!(
        mapping_output_binding_config(&loaded_snapshot, loaded_output).unwrap(),
        expected
    );
    assert!(loaded_snapshot.node_id_by_uuid(command_uuid).is_some());
}

#[test]
fn legacy_mapping_output_retains_unrepresentable_bindings_for_resolution() {
    let (mut engine, _, processor) = mapping_engine();
    let outputs = region(&engine, processor, "outputs");
    let action_outputs = transitional_command_manager(&mut engine);
    let command = create_item(&mut engine, action_outputs, GENERIC_LOG_COMMAND_NODE_TYPE);
    let command_uuid = engine.process_tree_snapshot().node(command).unwrap().uuid;

    let legacy_type = format!("{ANODE_CREATE_PREFIX}chataigne.output_target");
    let tree = create_anode_user_item_tree(&legacy_type).unwrap();
    let legacy_uuid = tree.node.node_data().meta.uuid;
    engine.edits.push(Edit::AddNodeTree {
        parent: outputs,
        prev_sibling: None,
        tree,
    });
    engine.apply_edits().unwrap();
    let legacy = engine
        .process_tree_snapshot()
        .node_id_by_uuid(legacy_uuid)
        .unwrap();
    set_config(
        &mut engine,
        legacy,
        "target",
        ParamValue::Reference(NodeReference::new(command_uuid)),
    );
    let bindings = OutputBindingConfig {
        value: OutputValueSource::Constant(RuntimeValue::Unit),
        ..OutputBindingConfig::default()
    };
    let authored = bindings.to_authoring_json().unwrap();
    set_config(
        &mut engine,
        legacy,
        "bindings",
        ParamValue::Str(authored.clone()),
    );

    migrate_mapping_output_adapters(&mut engine).unwrap();
    let snapshot = engine.process_tree_snapshot();
    let migrated = snapshot.node_id_by_uuid(legacy_uuid).unwrap();
    let binding_manager = snapshot
        .find_child_by_decl_id(migrated, "mapping_bindings")
        .unwrap();
    let retained = snapshot
        .find_child_by_decl_id(binding_manager, "unresolved_legacy_bindings")
        .unwrap();
    assert_eq!(
        snapshot.node(retained).unwrap().param_value,
        Some(ParamValue::Str(authored))
    );
    assert_eq!(
        mapping_output_binding_config(&snapshot, migrated).unwrap(),
        OutputBindingConfig::default()
    );
    assert!(snapshot
        .node(migrated)
        .unwrap()
        .presentation
        .warnings
        .iter()
        .any(|warning| warning.message.contains("original binding document is retained")));
}

fn activate_mapping_processor(engine: &mut AppEngine, processor: NodeId) {
    engine.add_node(StateMachineManager::new().into(), None);
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let manager = snapshot
        .child_ids(engine.root)
        .into_iter()
        .find(|id| snapshot.node(*id).is_some_and(|node| node.node_type == StateMachineManager::NODE_TYPE))
        .unwrap();
    engine.add_user_item(StateMachineState::new().into(), Some(manager));
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let state = snapshot
        .child_ids(manager)
        .into_iter()
        .find(|id| snapshot.node(*id).is_some_and(|node| node.node_type == StateMachineState::NODE_TYPE))
        .unwrap();
    let state_processors = snapshot.find_child_by_decl_id(state, "processors").unwrap();
    let move_ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: processor,
        new_parent: state_processors,
        new_prev_sibling: None,
    });
    assert!(move_ack.success, "processor should move into an active State: {move_ack:?}");
    engine.apply_edits().unwrap();
}

fn manager_candidate_visits(engine: &AppEngine) -> u64 {
    engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => Some(manager.runtime_perf_stats().processor_candidate_visits),
            _ => None,
        })
        .expect("active StateMachineManager should exist")
}

#[test]
fn mapping_idle_and_sparse_ticks_visit_only_dirty_processors() {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
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
    let state_processors = snapshot.find_child_by_decl_id(state, "processors").unwrap();
    engine.add_user_item(StateProcessor::new().into(), Some(state_processors));
    engine.apply_edits().unwrap();

    let source = source_param(&mut engine, "Sparse source", 1.0);
    let inputs = region(&engine, processor, "inputs");
    let input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(&mut engine, input, "source", ParamValue::Reference(NodeReference::new(source)));
    for _ in 0..8 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let settled = manager_candidate_visits(&engine);
    engine.run_tick(Duration::from_millis(8)).unwrap();
    assert_eq!(manager_candidate_visits(&engine), settled, "idle tick should visit no processors");

    let source_node = engine.process_tree_snapshot().node_id_by_uuid(source).unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: source_node,
        value: ParamValue::Float(2.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "source edit should apply: {ack:?}");
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(8)).unwrap();
    assert_eq!(manager_candidate_visits(&engine) - settled, 1);
}

#[test]
fn mapping_self_target_is_applied_by_the_queued_engine_path() {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let source = source_param(&mut engine, "Feedback", 1.0);
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
    let outputs = region(&engine, processor, "outputs");
    let output = create_parameter_output(&mut engine, outputs, source);
    let bindings = OutputBindingConfig {
        value: OutputValueSource::Constant(RuntimeValue::Float(2.0)),
        send_policy: OutputSendPolicy::OnChange,
        ..OutputBindingConfig::default()
    };
    set_config(
        &mut engine,
        output,
        "bindings",
        ParamValue::Str(bindings.to_authoring_json().unwrap()),
    );

    for _ in 0..12 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    let source_node = snapshot.node_id_by_uuid(source).unwrap();
    assert_eq!(snapshot.node(source_node).unwrap().param_value, Some(ParamValue::Float(2.0)));
    let event_count = engine.ui_event_log().len();
    for _ in 0..12 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_eq!(engine.ui_event_log().len(), event_count);
}

#[test]
fn mapping_output_changes_its_filter_coefficient_on_the_next_engine_tick() {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let source = source_param(&mut engine, "Input", 2.0);
    let sink = source_param(&mut engine, "Result", 0.0);
    let inputs = region(&engine, processor, "inputs");
    let input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(&mut engine, input, "source", ParamValue::Reference(NodeReference::new(source)));

    let filters = region(&engine, processor, "filters");
    let remap_type = engine
        .nodes
        .get(filters)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}remap@managed/0")))
        .map(|item| item.node_type)
        .expect("a Float input should expose Remap");
    let remap = create_item(&mut engine, filters, &remap_type);
    set_socket_default(&mut engine, remap, "in_max", ParamValue::Float(5.0));
    let snapshot = engine.process_tree_snapshot();
    let remap_inputs = snapshot.find_child_by_decl_id(remap, "inputs").unwrap();
    let maximum_socket = snapshot.find_child_by_decl_id(remap_inputs, "inputs/in_max").unwrap();
    let maximum = snapshot
        .find_child_by_decl_id(maximum_socket, "inputs/in_max/value")
        .unwrap();
    let maximum_uuid = snapshot.node(maximum).unwrap().uuid;

    let outputs = region(&engine, processor, "outputs");
    let feedback = create_parameter_output(&mut engine, outputs, maximum_uuid);
    let feedback_binding = OutputBindingConfig {
        value: OutputValueSource::Constant(RuntimeValue::Float(4.0)),
        send_policy: OutputSendPolicy::OnChange,
        ..OutputBindingConfig::default()
    };
    set_config(
        &mut engine,
        feedback,
        "bindings",
        ParamValue::Str(feedback_binding.to_authoring_json().unwrap()),
    );
    create_parameter_output(&mut engine, outputs, sink);

    for _ in 0..12 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.node(maximum).unwrap().param_value, Some(ParamValue::Float(4.0)));
    let sink_node = snapshot.node_id_by_uuid(sink).unwrap();
    assert_eq!(snapshot.node(sink_node).unwrap().param_value, Some(ParamValue::Float(0.5)));
}

#[test]
fn mapping_can_be_authored_through_backend_intents_and_reloaded() {
    let (mut engine, mapping_uuid, processor) = mapping_engine();
    let inputs = region(&engine, processor, "inputs");
    let filters = region(&engine, processor, "filters");
    let outputs = region(&engine, processor, "outputs");
    let input_type = format!("{ANODE_CREATE_PREFIX}chataigne.input_source");
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

    let output = create_item(&mut engine, outputs, GENERIC_LOG_COMMAND_NODE_TYPE);
    let command_uuid = engine.nodes.get(output).unwrap().node_data().meta.uuid;
    let message = engine.process_tree_snapshot().find_child_by_decl_id(output, "message").unwrap();
    let message_uuid = engine.nodes.get(message).unwrap().node_data().meta.uuid;
    let bindings = OutputBindingConfig {
        value: OutputValueSource::Whole,
        arguments: vec![OutputArgumentBinding {
            parameter: StableRef::new(ValueTypeId::new("string"), message_uuid.0.to_string()),
            source: OutputValueSource::Constant(RuntimeValue::String(Arc::from("mapped"))),
        }],
        send_policy: OutputSendPolicy::OnChange,
    };
    set_mapping_bindings(&mut engine, output, &bindings);

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
    let output_id = snapshot.node(output).unwrap().uuid.0.to_string();
    let command_intent = output_frame.intents.iter().find(|intent| intent.target.as_ref().is_some_and(|target| target.stable_id.as_ref() == output_id)).unwrap();
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
    let revised_command = revised_output.intents.iter().find(|intent| intent.target.as_ref().is_some_and(|target| target.stable_id.as_ref() == output_id)).unwrap();
    let revised_arguments = CommandArgumentValues::from_runtime_value(&revised_command.payload).unwrap().unwrap();
    assert_eq!(revised_arguments.value, RuntimeValue::Float(1.25));

    let processor_uuid = revised_snapshot.node(processor).unwrap().uuid;
    let convert = revised_snapshot.find_child_by_decl_id(processor, "convert_to_formula").unwrap();
    let before_conversion_history = engine.undo_len();
    let client_edit_id = "mapping-conversion-test".to_owned();
    assert!(engine.apply_ui_intent(UiEditIntent::BeginEdit {
        client_edit_id: client_edit_id.clone(),
        label: Some("Convert Mapping to Formula".to_owned()),
    }).success);
    let conversion = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: convert,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(conversion.success, "conversion intent should apply: {conversion:?}");
    assert!(engine.apply_ui_intent(UiEditIntent::EndEdit { client_edit_id }).success);
    engine.apply_edits().unwrap();
    assert_eq!(engine.undo_len(), before_conversion_history + 1, "conversion should be one history step");
    let converted_snapshot = engine.process_tree_snapshot();
    let converted_processor = converted_snapshot.node_id_by_uuid(processor_uuid).unwrap();
    let formula_ref = converted_snapshot.find_child_by_decl_id(converted_processor, "formula").unwrap();
    let Some(ParamValue::Reference(reference)) = converted_snapshot.node(formula_ref).unwrap().param_value.as_ref() else { panic!("processor should have a Formula reference") };
    let converted_uuid = reference.uuid();
    assert_ne!(converted_uuid, mapping_uuid);
    let converted_node = converted_snapshot.node_id_by_uuid(converted_uuid).unwrap();
    assert_eq!(converted_snapshot.node(converted_node).unwrap().presentation.icon, snapshot.node(mapping).unwrap().presentation.icon);
    assert_eq!(
        converted_snapshot.node_id_by_uuid(command_uuid),
        Some(output),
        "conversion should retain the concrete owned command node"
    );
    assert_eq!(converted_snapshot.child_ids(converted_processor).into_iter().filter(|id| converted_snapshot.node(*id).is_some_and(|node| node.node_type == OutputsManager::NODE_TYPE)).count(), 0);
    assert!(!converted_snapshot.node(converted_node).unwrap().tags.iter().any(|tag| tag.contains("external.builtin")));
    assert_eq!(converted_snapshot.child_ids(region(&engine, converted_processor, "inputs")), vec![first, second]);
    let converted_formula = formula_from_snapshot(&converted_snapshot, converted_node).unwrap();
    assert_eq!(converted_formula.graph.nodes().count(), formula.graph.nodes().count());
    assert_eq!(converted_formula.graph.edges().count(), formula.graph.edges().count());
    let mut converted_instance = converted_formula.instantiate();
    converted_instance.managed_regions = managed_regions_from_snapshot(&converted_snapshot, converted_processor, &converted_formula).unwrap();
    let converted_ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&converted_formula.properties),
    };
    let mut converted = ManagedFormulaRuntime::compile(&converted_formula, &converted_instance, &converted_ctx).unwrap().unwrap();
    converted.reconcile_input_source_schema(|source| managed_source_schema(&converted_snapshot, source)).unwrap();
    let converted_output = converted.evaluate(&EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });
    assert!(converted_output.diagnostics.is_empty(), "{:?}", converted_output.diagnostics);
    assert_eq!(converted_output.intents.len(), 1, "conversion must not duplicate command dispatch");
    let converted_arguments = CommandArgumentValues::from_runtime_value(&converted_output.intents[0].payload).unwrap().unwrap();
    assert_eq!(converted_arguments.value, RuntimeValue::Float(1.25));
    assert_eq!(converted_arguments.arguments[0].value, RuntimeValue::String(Arc::from("mapped")));
    assert_eq!(converted_arguments.send_policy, OutputSendPolicy::OnChange);

    let mut baseline = ManagedFormulaRuntime::compile(&formula, &revised_instance, &ctx).unwrap().unwrap();
    baseline.reconcile_input_source_schema(|source| managed_source_schema(&revised_snapshot, source)).unwrap();
    let mut converted_fresh = ManagedFormulaRuntime::compile(&converted_formula, &converted_instance, &converted_ctx).unwrap().unwrap();
    converted_fresh.reconcile_input_source_schema(|source| managed_source_schema(&converted_snapshot, source)).unwrap();
    let source_refs = [first, second].map(|item| {
        let item = anode_from_snapshot(&revised_snapshot, item).unwrap();
        let Some(RuntimeValue::Ref(reference)) = item.config.get("source") else { panic!("source should be bound") };
        reference.clone()
    });
    let axes: AxisSet = [ContextAxisId::new("device")].into_iter().collect();
    let context_a = ContextKey::single("device", "a");
    let context_b = ContextKey::single("device", "b");
    let mut context_inputs = RuntimeInputSnapshot::default();
    for (context, values) in [(&context_a, [2.0, 3.0]), (&context_b, [4.0, 3.0])] {
        for (source, value) in source_refs.iter().zip(values) {
            context_inputs.insert_context(source.clone(), &axes, context.clone(), RuntimeValue::Float(value));
        }
    }
    let event = RuntimeEvent { topic: Arc::from("mapping-conversion-test"), value: RuntimeValue::Bool(true) };
    for (tick, context) in [(1, &context_a), (2, &context_b), (3, &context_a), (4, &context_b)] {
        let frame = EvaluationCtx {
            logical_tick: tick,
            delta_time: Duration::from_millis(16),
            events: std::slice::from_ref(&event),
            inputs: &context_inputs,
            registries: &registries,
        };
        let before = baseline.evaluate_with_context_frame(&frame, context, None, DebugCaptureMode::Off);
        let after = converted_fresh.evaluate_with_context_frame(&frame, context, None, DebugCaptureMode::Off);
        assert_eq!(before.diagnostics.iter().map(|diagnostic| &diagnostic.message).collect::<Vec<_>>(), after.diagnostics.iter().map(|diagnostic| &diagnostic.message).collect::<Vec<_>>());
        assert_eq!(before.intents.len(), 1);
        assert_eq!(after.intents.len(), 1);
        assert_eq!(before.intents.iter().map(|intent| (&intent.kind, &intent.target, &intent.payload, intent.logical_tick)).collect::<Vec<_>>(), after.intents.iter().map(|intent| (&intent.kind, &intent.target, &intent.payload, intent.logical_tick)).collect::<Vec<_>>());
    }
    assert!(engine.apply_ui_intent(UiEditIntent::Undo).success);
    let undone = engine.process_tree_snapshot();
    assert!(undone.node_id_by_uuid(converted_uuid).is_none());
    let undo_formula_ref = undone.find_child_by_decl_id(undone.node_id_by_uuid(processor_uuid).unwrap(), "formula").unwrap();
    assert_eq!(undone.node(undo_formula_ref).unwrap().param_value, Some(ParamValue::Reference(NodeReference::new(mapping_uuid))));
    assert!(engine.apply_ui_intent(UiEditIntent::Redo).success);
    let redone = engine.process_tree_snapshot();
    assert!(redone.node_id_by_uuid(converted_uuid).is_some());
    assert_eq!(formula_from_snapshot(&redone, redone.node_id_by_uuid(converted_uuid).unwrap()).unwrap().surface.managed_regions.len(), 3, "redo should retain managed-region metadata");
    let redo_formula_ref = redone.find_child_by_decl_id(redone.node_id_by_uuid(processor_uuid).unwrap(), "formula").unwrap();
    assert_eq!(redone.node(redo_formula_ref).unwrap().param_value, Some(ParamValue::Reference(NodeReference::new(converted_uuid))));
    assert_eq!(redone.child_ids(region(&engine, redone.node_id_by_uuid(processor_uuid).unwrap(), "inputs")), vec![first, second]);

    let authored_bindings = mapping_output_binding_config(&snapshot, output).unwrap();
    assert_eq!(authored_bindings, bindings);

    let region_uuids = ["inputs", "filters", "outputs"].map(|name| {
        snapshot.node(region(&engine, processor, name)).unwrap().uuid
    });
    let input_uuids = [first, second].map(|id| snapshot.node(id).unwrap().uuid);
    let remap_uuid = snapshot.node(remap).unwrap().uuid;
    let sum_uuid = snapshot.node(sum).unwrap().uuid;
    let output_uuid = snapshot.node(output).unwrap().uuid;
    let project = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&project).unwrap();
    sync_external_formulas(&mut loaded).unwrap();
    let loaded_snapshot = loaded.process_tree_snapshot();
    let loaded_processor = loaded_snapshot.node_id_by_uuid(processor_uuid).unwrap();
    let loaded_formula_ref = loaded_snapshot.find_child_by_decl_id(loaded_processor, "formula").unwrap();
    assert_eq!(loaded_snapshot.node(loaded_formula_ref).unwrap().param_value, Some(ParamValue::Reference(NodeReference::new(converted_uuid))));
    let loaded_custom = loaded_snapshot.node_id_by_uuid(converted_uuid).unwrap();
    assert_eq!(loaded_snapshot.node(loaded_custom).unwrap().presentation.icon, snapshot.node(mapping).unwrap().presentation.icon);
    let loaded_formula = formula_from_snapshot(&loaded_snapshot, loaded_custom)
        .expect("converted Formula should reload as an editable project Formula");
    assert_eq!(loaded_formula.surface.managed_regions.len(), 3);
    assert_eq!(loaded_formula.graph.nodes().count(), formula.graph.nodes().count());
    assert_eq!(loaded_formula.graph.edges().count(), formula.graph.edges().count());
    for ((name, expected), region_uuid) in [("inputs", vec![first, second]), ("filters", vec![remap, sum]), ("outputs", vec![output])]
        .into_iter()
        .zip(region_uuids)
    {
        let region_node = region(&loaded, loaded_processor, name);
        assert_eq!(loaded_snapshot.node(region_node).unwrap().uuid, region_uuid);
        let reloaded = loaded_snapshot.child_ids(region_node);
        assert_eq!(reloaded.len(), expected.len(), "{name} should keep its ordered items");
        for (actual, original) in reloaded.into_iter().zip(expected) {
            assert_eq!(loaded_snapshot.node(actual).unwrap().uuid, snapshot.node(original).unwrap().uuid);
        }
    }
    for uuid in input_uuids.into_iter().chain([remap_uuid, sum_uuid, output_uuid]) {
        assert!(loaded_snapshot.node_id_by_uuid(uuid).is_some(), "authored item {uuid:?} should survive reload");
    }
    let loaded_output = loaded_snapshot.node_id_by_uuid(output_uuid).unwrap();
    let loaded_bindings = mapping_output_binding_config(&loaded_snapshot, loaded_output).unwrap();
    assert_eq!(loaded_bindings, authored_bindings);
    assert!(loaded_snapshot.node_id_by_uuid(command_uuid).is_some(), "output command identity should survive");
    let mut loaded_instance = loaded_formula.instantiate();
    loaded_instance.managed_regions = managed_regions_from_snapshot(&loaded_snapshot, loaded_processor, &loaded_formula).unwrap();
    let loaded_ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&loaded_formula.properties),
    };
    let mut loaded_runtime = ManagedFormulaRuntime::compile(&loaded_formula, &loaded_instance, &loaded_ctx).unwrap().unwrap();
    loaded_runtime.reconcile_input_source_schema(|source| managed_source_schema(&loaded_snapshot, source)).unwrap();
    let loaded_result = loaded_runtime.evaluate(&EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });
    assert!(loaded_result.diagnostics.is_empty(), "{:?}", loaded_result.diagnostics);
    assert_eq!(loaded_result.intents.len(), 1);
    let loaded_arguments = CommandArgumentValues::from_runtime_value(&loaded_result.intents[0].payload).unwrap().unwrap();
    assert_eq!(loaded_arguments.value, RuntimeValue::Float(1.25));
    let loaded_remap = loaded.process_tree_snapshot().node_id_by_uuid(remap_uuid).unwrap();
    set_socket_default(&mut loaded, loaded_remap, "in_max", ParamValue::Float(10.0));
    let edited_snapshot = loaded.process_tree_snapshot();
    let mut edited_instance = loaded_formula.instantiate();
    edited_instance.managed_regions =
        managed_regions_from_snapshot(&edited_snapshot, loaded_processor, &loaded_formula).unwrap();
    let mut edited_runtime =
        ManagedFormulaRuntime::compile(&loaded_formula, &edited_instance, &loaded_ctx)
            .unwrap()
            .unwrap();
    edited_runtime
        .reconcile_input_source_schema(|source| managed_source_schema(&edited_snapshot, source))
        .unwrap();
    let edited_result = edited_runtime.evaluate(&EvaluationCtx {
        logical_tick: 2,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    });
    assert!(edited_result.diagnostics.is_empty(), "{:?}", edited_result.diagnostics);
    assert_eq!(edited_result.intents.len(), 1);
    let edited_arguments =
        CommandArgumentValues::from_runtime_value(&edited_result.intents[0].payload)
            .unwrap()
            .unwrap();
    assert_eq!(edited_arguments.value, RuntimeValue::Float(0.5));
    assert_eq!(edited_arguments.arguments[0].value, RuntimeValue::String(Arc::from("mapped")));
    let added = create_item(&mut loaded, loaded_custom, &format!("{ANODE_CREATE_PREFIX}constant"));
    assert!(loaded.process_tree_snapshot().node(added).is_some());
    assert_eq!(formula_from_snapshot(&loaded.process_tree_snapshot(), loaded_custom).unwrap().graph.nodes().count(), formula.graph.nodes().count() + 1);
    let formula_count = loaded.nodes.iter().filter(|(_, node)| node.get_type() == AlchemistFormulaDefinition::NODE_TYPE).count();
    let convert = loaded_snapshot.find_child_by_decl_id(loaded_processor, "convert_to_formula").unwrap();
    assert!(loaded.apply_ui_intent(UiEditIntent::SetParam {
        node: convert,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    }).success);
    loaded.apply_edits().unwrap();
    assert_eq!(loaded.nodes.iter().filter(|(_, node)| node.get_type() == AlchemistFormulaDefinition::NODE_TYPE).count(), formula_count);
    assert_eq!(loaded.process_tree_snapshot().node(loaded_formula_ref).unwrap().param_value, Some(ParamValue::Reference(NodeReference::new(converted_uuid))));
}

#[test]
fn mapping_conversion_starts_temporal_filters_with_fresh_state() {
    let (mut engine, mapping_uuid, processor) = mapping_engine();
    let source_uuid = source_param(&mut engine, "Signal", 0.0);
    let inputs_region = region(&engine, processor, "inputs");
    let input = create_item(&mut engine, inputs_region, &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"));
    set_config(&mut engine, input, "source", ParamValue::Reference(NodeReference::new(source_uuid)));
    let filters = region(&engine, processor, "filters");
    let gate_type = engine.nodes.get(filters).unwrap().user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}condition_gate")))
        .map(|item| item.node_type)
        .expect("Float source should expose Condition Gate");
    let gate = create_item(&mut engine, filters, &gate_type);
    set_socket_default(&mut engine, gate, "condition", ParamValue::Bool(true));
    let smooth_type = engine.nodes.get(filters).unwrap().user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}smooth_filter")))
        .map(|item| item.node_type)
        .expect("Float source should expose Smooth Filter");
    let smooth = create_item(&mut engine, filters, &smooth_type);
    set_config(&mut engine, smooth, "method", ParamValue::Enum("sma".to_owned()));

    let command_manager = transitional_command_manager(&mut engine);
    let command = create_item(&mut engine, command_manager, GENERIC_LOG_COMMAND_NODE_TYPE);
    let command_uuid = engine.nodes.get(command).unwrap().node_data().meta.uuid;
    let outputs_region = region(&engine, processor, "outputs");
    let output = create_item(&mut engine, outputs_region, &format!("{ANODE_CREATE_PREFIX}chataigne.output_target"));
    set_config(&mut engine, output, "target", ParamValue::Reference(NodeReference::new(command_uuid)));
    set_config(&mut engine, output, "bindings", ParamValue::Str(OutputBindingConfig::default().to_authoring_json().unwrap()));

    let snapshot = engine.process_tree_snapshot();
    let mapping = snapshot.node_id_by_uuid(mapping_uuid).unwrap();
    let formula = formula_from_snapshot(&snapshot, mapping).unwrap();
    let mut instance = formula.instantiate();
    instance.managed_regions = managed_regions_from_snapshot(&snapshot, processor, &formula).unwrap();
    let ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&formula.properties),
    };
    let mut warmed = ManagedFormulaRuntime::compile(&formula, &instance, &ctx).unwrap().unwrap();
    warmed.reconcile_input_source_schema(|source| managed_source_schema(&snapshot, source)).unwrap();
    let source = anode_from_snapshot(&snapshot, input).unwrap();
    let Some(RuntimeValue::Ref(source)) = source.config.get("source") else { panic!("source should be bound") };
    let registries = RuntimeRegistries { value_types: shared_value_type_registry() };
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source.clone(), RuntimeValue::Float(0.0));
    let initial = warmed.evaluate(&EvaluationCtx { logical_tick: 1, delta_time: Duration::from_millis(16), events: &[], inputs: &inputs, registries: &registries });
    assert!(initial.diagnostics.is_empty(), "{:?}", initial.diagnostics);
    inputs.insert(source.clone(), RuntimeValue::Float(10.0));
    let warm = warmed.evaluate(&EvaluationCtx { logical_tick: 2, delta_time: Duration::from_millis(16), events: &[], inputs: &inputs, registries: &registries });
    let warm_value = warm.intents[0].payload.clone();
    assert_eq!(warm_value, RuntimeValue::Float(5.0));

    let convert = snapshot.find_child_by_decl_id(processor, "convert_to_formula").unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: convert,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "conversion should apply: {ack:?}");
    engine.apply_edits().unwrap();
    let converted_snapshot = engine.process_tree_snapshot();
    let formula_ref = converted_snapshot.find_child_by_decl_id(processor, "formula").unwrap();
    let Some(ParamValue::Reference(reference)) = converted_snapshot.node(formula_ref).unwrap().param_value.as_ref() else { panic!("Formula reference is missing") };
    let converted_formula = formula_from_snapshot(&converted_snapshot, converted_snapshot.node_id_by_uuid(reference.uuid()).unwrap()).unwrap();
    let mut converted_instance = converted_formula.instantiate();
    converted_instance.managed_regions = managed_regions_from_snapshot(&converted_snapshot, processor, &converted_formula).unwrap();
    let converted_ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&converted_formula.properties),
    };
    let mut converted = ManagedFormulaRuntime::compile(&converted_formula, &converted_instance, &converted_ctx).unwrap().unwrap();
    converted.reconcile_input_source_schema(|source| managed_source_schema(&converted_snapshot, source)).unwrap();
    let mut baseline_fresh = ManagedFormulaRuntime::compile(&formula, &instance, &ctx).unwrap().unwrap();
    baseline_fresh.reconcile_input_source_schema(|source| managed_source_schema(&snapshot, source)).unwrap();
    let gate_id = ManagedItemId::from_uuid(snapshot.node(gate).unwrap().uuid.0);
    for runtime in [&mut converted, &mut baseline_fresh] {
        runtime.update_filter_input(gate_id, &SocketId::new("condition"), RuntimeInputBinding::Constant(RuntimeValue::Bool(false))).unwrap();
        let blocked = runtime.evaluate(&EvaluationCtx { logical_tick: 3, delta_time: Duration::from_millis(16), events: &[], inputs: &inputs, registries: &registries });
        assert!(blocked.diagnostics.is_empty(), "{:?}", blocked.diagnostics);
        assert!(blocked.intents.is_empty(), "closed gate must suppress command delivery");
        runtime.update_filter_input(gate_id, &SocketId::new("condition"), RuntimeInputBinding::Constant(RuntimeValue::Bool(true))).unwrap();
    }
    let frame = EvaluationCtx { logical_tick: 4, delta_time: Duration::from_millis(16), events: &[], inputs: &inputs, registries: &registries };
    let after = converted.evaluate(&frame);
    let fresh = baseline_fresh.evaluate(&frame);
    assert_eq!(after.intents.len(), 1);
    assert_eq!(fresh.intents.len(), 1);
    let after_value = after.intents[0].payload.clone();
    let fresh_value = fresh.intents[0].payload.clone();
    assert_eq!(after_value, fresh_value);
    assert_eq!(after_value, RuntimeValue::Float(10.0), "conversion should reset SMA history");
}

#[test]
fn mapping_conversion_preserves_repeated_trigger_delivery() {
    let (mut engine, mapping_uuid, processor) = mapping_engine();
    let trigger = Parameter::new("Pulse", ParamValue::Trigger(), ParameterChangeCheck::ValueChange);
    let trigger_uuid = trigger.node_data().meta.uuid;
    engine.add_node(trigger.into(), None);
    engine.apply_edits().unwrap();
    let inputs_region = region(&engine, processor, "inputs");
    let input = create_item(&mut engine, inputs_region, &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"));
    set_config(&mut engine, input, "source", ParamValue::Reference(NodeReference::new(trigger_uuid)));
    let command_manager = transitional_command_manager(&mut engine);
    let command = create_item(&mut engine, command_manager, GENERIC_LOG_COMMAND_NODE_TYPE);
    let command_uuid = engine.nodes.get(command).unwrap().node_data().meta.uuid;
    let outputs_region = region(&engine, processor, "outputs");
    let output = create_item(&mut engine, outputs_region, &format!("{ANODE_CREATE_PREFIX}chataigne.output_target"));
    set_config(&mut engine, output, "target", ParamValue::Reference(NodeReference::new(command_uuid)));
    set_config(&mut engine, output, "bindings", ParamValue::Str(OutputBindingConfig::default().to_authoring_json().unwrap()));

    let snapshot = engine.process_tree_snapshot();
    let formula = formula_from_snapshot(&snapshot, snapshot.node_id_by_uuid(mapping_uuid).unwrap()).unwrap();
    let mut instance = formula.instantiate();
    instance.managed_regions = managed_regions_from_snapshot(&snapshot, processor, &formula).unwrap();
    let compile_ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&formula.properties),
    };
    let mut baseline = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx).unwrap().unwrap();
    baseline.reconcile_input_source_schema(|source| managed_source_schema(&snapshot, source)).unwrap();

    let convert = snapshot.find_child_by_decl_id(processor, "convert_to_formula").unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: convert,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "conversion should apply: {ack:?}");
    engine.apply_edits().unwrap();
    let converted_snapshot = engine.process_tree_snapshot();
    let formula_ref = converted_snapshot.find_child_by_decl_id(processor, "formula").unwrap();
    let Some(ParamValue::Reference(reference)) = converted_snapshot.node(formula_ref).unwrap().param_value.as_ref() else { panic!("Formula reference is missing") };
    let converted_formula = formula_from_snapshot(&converted_snapshot, converted_snapshot.node_id_by_uuid(reference.uuid()).unwrap()).unwrap();
    let mut converted_instance = converted_formula.instantiate();
    converted_instance.managed_regions = managed_regions_from_snapshot(&converted_snapshot, processor, &converted_formula).unwrap();
    let converted_ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&converted_formula.properties),
    };
    let mut converted = ManagedFormulaRuntime::compile(&converted_formula, &converted_instance, &converted_ctx).unwrap().unwrap();
    converted.reconcile_input_source_schema(|source| managed_source_schema(&converted_snapshot, source)).unwrap();

    let source = anode_from_snapshot(&snapshot, input).unwrap();
    let Some(RuntimeValue::Ref(source)) = source.config.get("source") else { panic!("trigger source should be bound") };
    let registries = RuntimeRegistries { value_types: shared_value_type_registry() };
    for edge in 1..=2 {
        let pulse = RuntimeValue::Trigger(TriggerValue::fired(edge, edge));
        let mut inputs = RuntimeInputSnapshot::default();
        inputs.insert(source.clone(), pulse.clone());
        let event = RuntimeEvent { topic: Arc::from("pulse"), value: pulse.clone() };
        let frame = EvaluationCtx { logical_tick: edge, delta_time: Duration::ZERO, events: &[event], inputs: &inputs, registries: &registries };
        let before = baseline.evaluate(&frame);
        let after = converted.evaluate(&frame);
        assert!(before.diagnostics.is_empty(), "{:?}", before.diagnostics);
        assert!(after.diagnostics.is_empty(), "{:?}", after.diagnostics);
        assert_eq!(before.intents.len(), 1, "every pulse should deliver once");
        assert_eq!(after.intents.len(), 1, "conversion should preserve pulse multiplicity");
        assert_eq!(before.intents[0].target, after.intents[0].target);
        assert_eq!(before.intents[0].payload, after.intents[0].payload);
    }
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
