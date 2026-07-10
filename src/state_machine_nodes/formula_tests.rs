use golden_core::{
    app::ProjectNode,
    color::Color,
    edit::{Edit, EditOrigin},
    node::{
        DeclId, Folder, Node, NodeId, NodeMetaPatch, NodeReference,
    },
    parameter::{ParamValue, Parameter, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    ui_sync::{
        UiCreateUserItemInitialParam, UiDuplicateDependentInitialParamValue,
        UiDuplicateDependentUserItem, UiDuplicateDependentUserItemInitialParam,
        UiDuplicateNodeSpec, UiEditIntent,
    },
};
use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, CompileCtx, RuntimeValue, StableRef, TriggerValue,
    ValueTypeId, compile_graph,
};

use super::{
    ANODE_CREATE_PREFIX, ANODE_ITEM_KIND, ANODE_TYPE_TAG_PREFIX,
    AlchemistANode, AlchemistConnection, AlchemistFormulaDefinition,
    AlchemistFormulaFolder, AlchemistProperty, AlchemistPropertyManager,
    AlchemistPropertiesManager, FORMULA_EXTERNAL_FILE_CREATE_TYPE,
    FORMULA_EXTERNAL_FILE_DECL_ID, FORMULA_EXTERNAL_FILE_TAG,
    FORMULA_FOLDER_ITEM_KIND, FORMULA_FOLDER_NODE_TYPE, FORMULA_ITEM_KIND,
    FORMULA_MANAGED_REGIONS_JSON_DECL_ID, FormulaLibrary, PROPERTIES_DECL_ID,
    PROPERTY_CREATE_PREFIX, PROPERTY_MANAGER_CREATE_PREFIX, formula_from_snapshot,
    param_to_runtime_value,
};
use crate::app::{AppEngine, AppNode};

#[test]
fn trigger_parameter_snapshot_materializes_idle() {
    let value = param_to_runtime_value(&ParamValue::Trigger(), &ValueTypeId::new("trigger"))
        .expect("trigger parameter should convert to a runtime trigger");

    assert_eq!(value, RuntimeValue::Trigger(TriggerValue::default()));
}

#[test]
fn boolean_trigger_parameter_materializes_fired() {
    let value = param_to_runtime_value(&ParamValue::Bool(true), &ValueTypeId::new("trigger"))
        .expect("boolean trigger parameter should convert to a runtime trigger");

    assert_eq!(
        value,
        RuntimeValue::Trigger(TriggerValue {
            fired: true,
            ..TriggerValue::default()
        })
    );
}

#[test]
fn formula_library_accepts_real_formula_nodes() {
    let library = FormulaLibrary::new();
    assert!(library.user_container_accepts_item(
        AlchemistFormulaDefinition::NODE_TYPE,
        FORMULA_ITEM_KIND
    ));
}

#[test]
fn formula_library_and_folders_accept_formulas_and_formula_folders() {
    let library = FormulaLibrary::new();
    let folder = AlchemistFormulaFolder::new();

    for container in [&library as &dyn Node, &folder as &dyn Node] {
        assert!(container.user_container_accepts_item(
            AlchemistFormulaDefinition::NODE_TYPE,
            FORMULA_ITEM_KIND
        ));
        assert!(container.user_container_accepts_item(
            FORMULA_FOLDER_NODE_TYPE,
            FORMULA_FOLDER_ITEM_KIND
        ));
        let items = container.user_creatable_items();
        assert!(items.iter().any(|item| {
            item.node_type == AlchemistFormulaDefinition::NODE_TYPE
                && item.item_kind == FORMULA_ITEM_KIND
        }));
        assert!(items.iter().any(|item| {
            item.node_type == FORMULA_EXTERNAL_FILE_CREATE_TYPE
                && item.item_kind == FORMULA_ITEM_KIND
                && item.label == "External Formula"
        }));
        assert!(items.iter().any(|item| {
            item.node_type == FORMULA_FOLDER_NODE_TYPE
                && item.item_kind == FORMULA_FOLDER_ITEM_KIND
        }));
    }
}

#[test]
fn external_formula_creation_exposes_file_parameter_and_loads_formula_file() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine
        .apply_edits()
        .expect("Formula Library should attach");
    let library = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Formula Library should exist");

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: library,
        node_type: FORMULA_EXTERNAL_FILE_CREATE_TYPE.to_owned(),
        label: Some("Action".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "External Formula creation should succeed: {ack:?}");
    let formula = direct_children(&engine, library)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
            })
        })
        .expect("External Formula should exist");
    assert!(engine
        .nodes
        .get(formula)
        .expect("External Formula should exist")
        .node_data()
        .meta
        .tags
        .iter()
        .any(|tag| tag == FORMULA_EXTERNAL_FILE_TAG));

    let file_param = find_child_by_decl(&engine, formula, FORMULA_EXTERNAL_FILE_DECL_ID)
        .expect("External Formula should expose a Formula File parameter");
    let file_parameter = engine
        .nodes
        .get(file_param)
        .and_then(|node| node.as_any().downcast_ref::<Parameter>())
        .expect("Formula File should be a parameter");
    assert!(!file_parameter.read_only);
    assert!(matches!(file_parameter.value, ParamValue::File(ref path) if path.is_empty()));

    let action_fixture = std::env::current_dir()
        .expect("current dir should resolve")
        .join("builtin_formulas")
        .join("Action.json");
    let action_path = std::env::temp_dir().join(format!(
        "chataigne2-action-formula-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos()
    ));
    std::fs::copy(&action_fixture, &action_path)
        .expect("Action formula fixture should copy to an isolated test file");
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: file_param,
        value: ParamValue::File(action_path.to_string_lossy().into_owned()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(
        ack.success,
        "Setting the external formula file should succeed: {ack:?}"
    );
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("External Formula should receive the file change");
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("External formula file load should settle");
    }

    assert!(
        find_child_by_decl(&engine, formula, FORMULA_EXTERNAL_FILE_DECL_ID).is_some(),
        "External Formula should keep its file parameter after reload"
    );
    let regions_param = find_child_by_decl(&engine, formula, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
        .expect("External Action formula should expose managed regions metadata");
    let raw_regions = match engine
        .nodes
        .get(regions_param)
        .and_then(|node| node.as_any().downcast_ref::<Parameter>())
        .map(|parameter| &parameter.value)
    {
        Some(ParamValue::Str(value)) => value,
        other => panic!("Managed regions metadata should be a string, got {other:?}"),
    };
    assert!(
        raw_regions.trim().is_empty(),
        "graph formulas expose their managers directly and must not synthesize sidecar regions"
    );
    assert_eq!(
        parameter_value(&engine, formula, "is_valid"),
        ParamValue::Bool(true),
        "External formula load should settle validation without a later edit"
    );
    std::fs::remove_file(action_path).expect("isolated Action formula should be removable");
}

#[test]
fn formula_exposes_anode_catalog_as_real_user_items() {
    let formula = AlchemistFormulaDefinition::new();
    let items = formula.user_creatable_items();

    assert!(items.iter().any(|item| {
        item.node_type == format!("{ANODE_CREATE_PREFIX}constant")
            && item.item_kind == ANODE_ITEM_KIND
    }));
    assert!(items.iter().any(|item| {
        item.node_type == AlchemistConnection::NODE_TYPE
    }));
}

#[test]
fn formula_properties_offer_manager_roles_and_graph_getters() {
    let properties = AlchemistPropertiesManager::new();
    let creatable = properties.user_creatable_items();

    for role in ["condition", "filter", "input", "output"] {
        let node_type = format!("{PROPERTY_MANAGER_CREATE_PREFIX}{role}");
        assert!(
            creatable.iter().any(|item| item.node_type == node_type),
            "{role} manager property should be creatable"
        );
    }

    for node_type in [
        chataigne_state_machine::alchemist::CONDITIONS_MANAGER_TYPE,
        chataigne_state_machine::alchemist::FILTERS_MANAGER_TYPE,
        chataigne_state_machine::alchemist::INPUTS_MANAGER_TYPE,
        chataigne_state_machine::alchemist::OUTPUTS_MANAGER_TYPE,
    ] {
        let mut graph = AlchemistGraph::new();
        let mut node = ANodeInstance::new(ANodeTypeId::new(node_type), "Manager");
        node.config.set(
            chataigne_state_machine::alchemist::MANAGER_PROPERTY_FIELD,
            RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), "manager")),
        );
        graph.add_node(node).unwrap();
        let value_types = chataigne_state_machine::alchemist::value_type_registry();
        let nodes = chataigne_state_machine::alchemist::node_registry();
        let result = compile_graph(
            &graph,
            &CompileCtx {
                value_types: &value_types,
                nodes: &nodes,
                properties: None,
            },
        );
        assert!(
            !result.has_errors(),
            "{node_type} should compile as a graph-usable manager getter: {:?}",
            result.diagnostics
        );
        assert!(
            result
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.code.starts_with("chataigne_manager_bridge")),
            "{node_type} should not use legacy manager bridge diagnostics"
        );
    }
}

#[test]
fn formula_manager_property_roles_survive_project_reload() {
    fn strip_manager_role_persistence(record: &mut serde_json::Value) {
        let Some(object) = record.as_object_mut() else {
            return;
        };
        let is_manager = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            == Some(AlchemistPropertyManager::NODE_TYPE);

        if is_manager {
            if let Some(tags) = object
                .get_mut("meta")
                .and_then(serde_json::Value::as_object_mut)
                .and_then(|meta| meta.get_mut("tags"))
                .and_then(serde_json::Value::as_array_mut)
            {
                tags.retain(|tag| {
                    !tag.as_str()
                        .is_some_and(|tag| tag.starts_with("alchemist.manager.role:"))
                });
            }

            if let Some(children) = object
                .get_mut("children")
                .and_then(serde_json::Value::as_array_mut)
            {
                for child in children.iter_mut() {
                    let is_role = child
                        .get("meta")
                        .and_then(|meta| meta.get("decl_id"))
                        .and_then(serde_json::Value::as_str)
                        == Some("role");
                    if is_role {
                        if let Some(role) = child.as_object_mut() {
                            role.remove("data");
                        }
                    }
                }
            }
        }

        if let Some(children) = object
            .get_mut("children")
            .and_then(serde_json::Value::as_array_mut)
        {
            for child in children {
                strip_manager_role_persistence(child);
            }
        }
    }

    fn loaded_formula_id(engine: &AppEngine) -> NodeId {
        engine
            .nodes
            .iter()
            .find(|(_, node)| {
                node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
            })
            .map(|(id, _)| id)
            .expect("Formula should reload")
    }

    fn assert_manager_roles(
        engine: &AppEngine,
        formula: NodeId,
        specs: &[(&str, &str)],
    ) {
        let properties = find_child_by_decl(engine, formula, PROPERTIES_DECL_ID)
            .expect("Properties manager should reload");

        for (role, label) in specs {
            let manager = direct_children(engine, properties)
                .into_iter()
                .find(|node| {
                    engine.nodes.get(*node).is_some_and(|node| {
                        node.get_type() == AlchemistPropertyManager::NODE_TYPE
                            && node.node_data().meta.label == *label
                    })
                })
                .unwrap_or_else(|| {
                    panic!("{label} manager property should reload")
                });
            assert_eq!(
                parameter_value(engine, manager, "role"),
                ParamValue::Enum((*role).into()),
                "{label} manager property role should survive reload"
            );
        }

        let materialized =
            formula_from_snapshot(&engine.process_tree_snapshot(), formula)
                .expect("reloaded Formula surface should materialize");
        for (role, label) in specs {
            assert!(
                materialized
                    .surface
                    .sections
                    .iter()
                    .any(|section| section.id.as_str() == *role && section.label == *label),
                "{label} manager surface section should keep id {role}"
            );
        }
    }

    let (mut engine, formula) = engine_with_formula();
    let properties = find_child_by_decl(&engine, formula, PROPERTIES_DECL_ID)
        .expect("Formula should own a Properties manager");
    let specs = [
        ("condition", "Conditions"),
        ("filter", "Filters"),
        ("input", "Inputs"),
        ("output", "Outputs"),
    ];

    for (role, _) in specs {
        let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent: properties,
            node_type: format!("{PROPERTY_MANAGER_CREATE_PREFIX}{role}"),
            label: None,
            initial_params: Vec::new(),
        });
        assert!(
            ack.success,
            "{role} manager property creation should succeed: {ack:?}"
        );
    }

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("Project should encode");
    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json)
        .expect("Project should decode");
    assert_manager_roles(&loaded, loaded_formula_id(&loaded), &specs);

    let mut legacy_project: serde_json::Value =
        serde_json::from_str(&json).expect("Project JSON should parse");
    strip_manager_role_persistence(
        legacy_project
            .get_mut("root")
            .expect("Project JSON should have a root record"),
    );
    let legacy_json = serde_json::to_string(&legacy_project)
        .expect("legacy-shaped Project JSON should encode");
    let recovered =
        golden_core::app::from_sparse_project_json::<AppNode>(&legacy_json)
            .expect("legacy-shaped Project should decode");
    assert_manager_roles(&recovered, loaded_formula_id(&recovered), &specs);
}

#[test]
fn materialized_anode_preserves_enabled_state() {
    let (mut engine, formula) = engine_with_formula();
    let previous_children = direct_children(&engine, formula).len();
    create_anode(&mut engine, formula, "constant", 0.0, 0.0);
    let constant = direct_children(&engine, formula)
        .into_iter()
        .skip(previous_children)
        .find(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == AlchemistANode::NODE_TYPE)
        })
        .expect("Constant ANode should exist");

    assert!(
        engine
            .nodes
            .get(constant)
            .is_some_and(|node| node.node_data().meta.can_be_disabled),
        "ANodes should expose the enable switch"
    );
    let ack = engine.apply_ui_intent(UiEditIntent::PatchMeta {
        node: constant,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..NodeMetaPatch::default()
        },
    });
    assert!(ack.success, "ANode disable should succeed: {ack:?}");

    let materialized = formula_from_snapshot(&engine.process_tree_snapshot(), formula)
        .expect("Formula should materialize");

    assert!(materialized
        .graph
        .nodes
        .values()
        .find(|node| node.type_id.as_str() == "constant")
        .is_some_and(|node| !node.enabled));
}

#[test]
fn library_created_formula_has_no_opaque_document_parameter() {
    let (engine, formula) = engine_with_formula();

    assert!(
        find_descendant_by_decl(&engine, formula, "formula_json").is_none(),
        "Formula source of truth must be its visible node subtree"
    );
    assert!(find_child_by_decl(&engine, formula, "is_valid").is_some());
    assert!(find_child_by_decl(&engine, formula, "diagnostics_json").is_some());
}

#[test]
fn formula_properties_use_one_catalog_and_bind_read_only_getters() {
    let (mut engine, formula) = engine_with_formula();
    let properties = find_child_by_decl(&engine, formula, PROPERTIES_DECL_ID)
        .expect("Formula should own a Properties manager");
    assert_eq!(
        engine
            .nodes
            .get(properties)
            .expect("Properties manager should exist")
            .get_type(),
        AlchemistPropertiesManager::NODE_TYPE
    );
    assert!(
        direct_children(&engine, properties).is_empty(),
        "Formula categories must be opt-in rather than eagerly materialized"
    );
    let parameter_items = engine
        .nodes
        .get(properties)
        .expect("Properties manager should exist")
        .user_creatable_items();
    for property_type in [
        "trigger",
        "int",
        "float",
        "str",
        "file",
        "enum",
        "bool",
        "css_value",
        "vec2",
        "vec3",
        "color",
        "reference",
    ] {
        assert!(parameter_items.iter().any(|item| {
            item.node_type
                == format!("{PROPERTY_CREATE_PREFIX}{property_type}")
        }));
    }
    for role in ["condition", "input", "output"] {
        assert!(parameter_items.iter().any(|item| {
            item.node_type
                == format!("{PROPERTY_MANAGER_CREATE_PREFIX}{role}")
        }));
    }

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: format!("{PROPERTY_CREATE_PREFIX}float"),
        label: Some("Amount".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Property creation should succeed: {ack:?}");
    let property = direct_children(&engine, properties)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistProperty::NODE_TYPE
            })
        })
        .expect("Amount property should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: format!("{PROPERTY_MANAGER_CREATE_PREFIX}condition"),
        label: Some("Conditions".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Conditions manager creation should succeed: {ack:?}");
    assert!(direct_children(&engine, properties).into_iter().any(|node| {
        engine.nodes.get(node).is_some_and(|node| {
            node.get_type() == AlchemistPropertyManager::NODE_TYPE
        })
    }));
    let value = find_child_by_decl(&engine, property, "value")
        .expect("Property value should be a real parameter");
    assert!(
        find_child_by_decl(&engine, property, "value_type").is_none(),
        "Property value type is internal and must not be a child parameter"
    );
    let property_color = engine
        .nodes
        .get(property)
        .expect("Property should exist")
        .node_data()
        .meta
        .presentation
        .default_color;
    let value_color = engine
        .nodes
        .get(value)
        .expect("Property value should exist")
        .node_data()
        .meta
        .presentation
        .default_color;
    assert!(property_color.is_some());
    assert_eq!(property_color, value_color);
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: value,
        value: ParamValue::Float(3.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "Property value edit should succeed: {ack:?}");

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula,
        node_type: format!("{ANODE_CREATE_PREFIX}property"),
        label: Some("Amount".into()),
        initial_params: vec![
            UiCreateUserItemInitialParam {
                decl_id: DeclId("config/property_id".into()),
                value: reference_to_node(&engine, property),
            },
        ],
    });
    assert!(ack.success, "Property getter creation should succeed: {ack:?}");
    let getter_node = direct_children(&engine, formula)
        .into_iter()
        .find(|candidate| {
            engine.nodes.get(*candidate).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
                    && anode_type(&engine, *candidate).as_deref()
                        == Some("property")
            })
        })
        .expect("Property getter should be a real ANode");
    let getter_meta = &engine
        .nodes
        .get(getter_node)
        .expect("Property getter should exist")
        .node_data()
        .meta;
    assert!(
        getter_meta.user_permissions.can_edit_name,
        "Property getters should follow normal ANode naming permissions"
    );
    assert!(
        getter_meta.user_permissions.can_edit_color,
        "Property getters should follow normal ANode color permissions"
    );
    let getter_config = find_child_by_decl(&engine, getter_node, "config")
        .expect("Property getter Config should exist");
    let property_id = find_child_by_decl(&engine, getter_config, "config/property_id")
        .expect("Property getter should expose a property reference parameter");
    let property_id_parameter = engine
        .nodes
        .get(property_id)
        .and_then(|node| node.as_any().downcast_ref::<Parameter>())
        .expect("Property getter binding should be a Parameter node");
    assert!(
        !property_id_parameter.read_only,
        "Property getter binding should be user-editable"
    );
    assert!(
        property_id_parameter
            .node_data()
            .meta
            .presentation
            .show_in_inspector_content,
        "Property getter binding should be visible in the inspector"
    );
    assert_eq!(
        parameter_value(&engine, getter_config, "config/property_id"),
        reference_to_node(&engine, property)
    );
    let mut property_presentation = engine
        .nodes
        .get(property)
        .expect("Property should exist")
        .node_data()
        .meta
        .presentation
        .clone();
    property_presentation.color = Some(Color::new(0.12, 0.34, 0.56, 1.0));
    let ack = engine.apply_ui_intent(UiEditIntent::PatchMeta {
        node: property,
        patch: NodeMetaPatch {
            label: Some("Intensity".into()),
            presentation: Some(property_presentation.clone()),
            ..NodeMetaPatch::default()
        },
    });
    assert!(ack.success, "Property rename should succeed: {ack:?}");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Formula should receive the Property rename");
    engine
        .apply_edits()
        .expect("Property getter label synchronization should stabilize");
    assert_eq!(
        engine
            .nodes
            .get(getter_node)
            .expect("Property getter should exist")
            .node_data()
            .meta
            .label,
        "Intensity"
    );
    assert_eq!(
        engine
            .nodes
            .get(getter_node)
            .expect("Property getter should exist")
            .node_data()
            .meta
            .presentation
            .default_color,
        property_presentation.color
    );

    let materialized =
        formula_from_snapshot(&engine.process_tree_snapshot(), formula)
            .expect("Formula properties should project to the surface");
    let item = materialized
        .surface
        .sections
        .iter()
        .find(|section| section.label == "Parameters")
        .and_then(|section| section.items.first())
        .expect("Amount should be exposed to Processors");
    assert_eq!(item.label, "Intensity");
    assert!(item.path.is_empty());
    assert_eq!(item.bindings.len(), 1);
    assert!(materialized
        .surface
        .sections
        .iter()
        .any(|section| section.label == "Conditions"));
    let getter = materialized
        .graph
        .nodes
        .values()
        .find(|node| node.type_id.as_str() == "property")
        .expect("Property getter should materialize");
    assert_eq!(
        getter.config.get("value"),
        Some(&golden_alchemist::RuntimeValue::Float(3.5))
    );
}

#[test]
fn formula_property_types_survive_project_reload() {
    let (mut engine, formula) = engine_with_formula();
    let properties = find_child_by_decl(&engine, formula, PROPERTIES_DECL_ID)
        .expect("Formula should own a Properties manager");
    let specs = [
        ("bool", "Enabled", ParamValue::Bool(false)),
        ("int", "Count", ParamValue::Int(0)),
        ("vec2", "Position", ParamValue::Vec2(0.0, 0.0)),
        ("vec3", "Direction", ParamValue::Vec3(0.0, 0.0, 0.0)),
        ("color", "Tint", ParamValue::Color(0.0, 0.0, 0.0, 1.0)),
        ("str", "Name", ParamValue::Str(String::new())),
    ];

    for (property_type, label, _) in &specs {
        let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent: properties,
            node_type: format!("{PROPERTY_CREATE_PREFIX}{property_type}"),
            label: Some((*label).into()),
            initial_params: Vec::new(),
        });
        assert!(
            ack.success,
            "{property_type} property creation should succeed: {ack:?}"
        );
    }

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("Project should encode");
    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json)
        .expect("Project should decode");
    let loaded_formula = loaded
        .nodes
        .iter()
        .find(|(_, node)| {
            node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
        })
        .map(|(id, _)| id)
        .expect("Formula should reload");
    let loaded_properties =
        find_child_by_decl(&loaded, loaded_formula, PROPERTIES_DECL_ID)
            .expect("Properties manager should reload");

    for (property_type, label, expected_value) in specs {
        let property = direct_children(&loaded, loaded_properties)
            .into_iter()
            .find(|node| {
                loaded.nodes.get(*node).is_some_and(|node| {
                    node.get_type() == AlchemistProperty::NODE_TYPE
                        && node.node_data().meta.label == label
                })
            })
            .unwrap_or_else(|| panic!("{label} property should reload"));
        assert_eq!(
            parameter_value(&loaded, property, "property_type"),
            ParamValue::Str(property_type.into()),
            "{label} property type should survive reload"
        );
        assert_eq!(
            parameter_value(&loaded, property, "value"),
            expected_value,
            "{label} property value parameter should keep its type"
        );
    }
}

#[test]
fn formula_property_getter_keeps_property_id_after_project_reload() {
    let (mut engine, formula) = engine_with_formula();
    let properties = find_child_by_decl(&engine, formula, PROPERTIES_DECL_ID)
        .expect("Formula should own a Properties manager");

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: format!("{PROPERTY_CREATE_PREFIX}float"),
        label: Some("Amount".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Property creation should succeed: {ack:?}");
    let property = direct_children(&engine, properties)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistProperty::NODE_TYPE
                    && node.node_data().meta.label == "Amount"
            })
        })
        .expect("Amount property should exist");
    let property_uuid = node_uuid_string(&engine, property);

    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula,
        node_type: format!("{ANODE_CREATE_PREFIX}property"),
        label: Some("Amount".into()),
        initial_params: vec![UiCreateUserItemInitialParam {
            decl_id: DeclId("config/property_id".into()),
            value: reference_to_node(&engine, property),
        }],
    });
    assert!(
        ack.success,
        "Property getter creation should succeed: {ack:?}"
    );

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("Project should encode");
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json)
        .expect("Project should decode");
    let loaded_formula = loaded
        .nodes
        .iter()
        .find(|(_, node)| {
            node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
        })
        .map(|(id, _)| id)
        .expect("Formula should reload");
    loaded
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Reloaded Formula should validate");
    loaded
        .apply_edits()
        .expect("Reloaded Formula validation edits should apply");

    let loaded_properties = find_child_by_decl(&loaded, loaded_formula, PROPERTIES_DECL_ID)
        .expect("Reloaded Formula should own a Properties manager");
    let loaded_property = direct_children(&loaded, loaded_properties)
        .into_iter()
        .find(|node| {
            loaded.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistProperty::NODE_TYPE
                    && node.node_data().meta.label == "Amount"
            })
        })
        .expect("Reloaded Amount property should exist");
    let loaded_getter = find_anode_by_type(&loaded, loaded_formula, "property");
    assert!(
        !node_has_warning_id(
            &loaded,
            loaded_getter,
            "alchemist_formula_diagnostic"
        ),
        "Reloaded property getter should not warn about a missing property_id"
    );
    let loaded_getter_config =
        find_child_by_decl(&loaded, loaded_getter, "config")
            .expect("Property getter Config should reload");
    assert_eq!(
        parameter_value(
            &loaded,
            loaded_getter_config,
            "config/property_id"
        ),
        reference_to_node(&loaded, loaded_property)
    );

    let materialized =
        formula_from_snapshot(&loaded.process_tree_snapshot(), loaded_formula)
            .expect("Reloaded Formula should materialize");
    let getter = materialized
        .graph
        .nodes
        .values()
        .find(|node| node.type_id.as_str() == "property")
        .expect("Property getter should materialize");
    assert!(
        matches!(
            getter.config.get("property_id"),
            Some(golden_alchemist::RuntimeValue::Ref(value))
                if value.stable_id.as_ref() == property_uuid
        ),
        "Reloaded property getter should keep its stable property_id"
    );
}

#[test]
fn duplicate_property_anode_inside_edit_session_is_one_undo_step() {
    let (mut engine, formula) = engine_with_formula();
    let properties = find_child_by_decl(&engine, formula, PROPERTIES_DECL_ID)
        .expect("Formula properties root should exist");
    let property = create_property(&mut engine, properties, "Float", "float");
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula,
        node_type: format!("{ANODE_CREATE_PREFIX}property"),
        label: Some("Float".into()),
        initial_params: vec![UiCreateUserItemInitialParam {
            decl_id: DeclId("config/property_id".into()),
            value: reference_to_node(&engine, property),
        }],
    });
    assert!(
        ack.success,
        "Property getter creation should succeed: {ack:?}"
    );

    let source = find_anode_by_type(&engine, formula, "property");
    let before_property_getters = count_anodes_by_type(&engine, formula, "property");
    let before_undo_len = engine.undo_len();
    let session_id = "duplicate-property-getter";

    engine.edits.push(Edit::BeginEditSession {
        origin: EditOrigin::Ui,
        label: Some("Duplicate ANode".to_owned()),
        client_edit_id: session_id.to_owned(),
        ui_client_instance_id: None,
    });
    engine
        .apply_edits()
        .expect("duplicate session should begin");

    let duplicate = duplicate_anode(
        &mut engine,
        formula,
        source,
        Some(source),
        "Float Copy",
    );
    assert_eq!(
        count_anodes_by_type(&engine, formula, "property"),
        before_property_getters + 1,
        "duplicate_subtree_with should create exactly one property getter"
    );
    assert!(
        engine.has_active_edit_session(),
        "duplicate_subtree_with must preserve the active edit session"
    );
    assert_eq!(
        engine.undo_len(),
        before_undo_len,
        "session should not commit before end"
    );

    engine.edits.push(Edit::EndEditSession {
        client_edit_id: session_id.to_owned(),
    });
    engine
        .apply_edits()
        .expect("duplicate session should end");

    assert!(!engine.has_active_edit_session());
    assert_eq!(
        engine.undo_len(),
        before_undo_len + 1,
        "duplicate should be one undo step"
    );
    assert!(engine.nodes.get(duplicate).is_some());

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        count_anodes_by_type(&engine, formula, "property"),
        before_property_getters,
        "one undo should remove the duplicated getter"
    );
}

#[test]
fn undoing_property_anode_removal_restores_getter_without_stale_history() {
    let (mut engine, formula) = engine_with_formula();
    let properties = find_child_by_decl(&engine, formula, PROPERTIES_DECL_ID)
        .expect("Formula properties root should exist");
    let property = create_property(&mut engine, properties, "Float", "float");
    let getter = create_property_anode(&mut engine, formula, property, 2.0, 3.0);
    let before_undo_len = engine.undo_len();

    let ack = engine.apply_ui_intent(UiEditIntent::RemoveNode { node: getter });
    assert!(
        ack.success,
        "Property getter removal should succeed: {ack:?}"
    );
    assert_eq!(count_anodes_by_type(&engine, formula, "property"), 0);

    let ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(
        ack.success,
        "Undoing property getter removal should succeed: {ack:?}"
    );
    assert_eq!(count_anodes_by_type(&engine, formula, "property"), 1);
    assert!(
        engine.nodes.get(getter).is_some(),
        "Undo should restore the removed getter at its original node id"
    );

    let ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(
        ack.success,
        "Undoing property getter creation should succeed without stale child history: {ack:?}"
    );
    assert_eq!(engine.undo_len(), before_undo_len - 1);
    assert_eq!(count_anodes_by_type(&engine, formula, "property"), 0);
}

#[test]
fn undoing_property_anode_duplicate_removes_duplicate_before_position_history() {
    let (mut engine, formula) = engine_with_formula();
    let properties = find_child_by_decl(&engine, formula, PROPERTIES_DECL_ID)
        .expect("Formula properties root should exist");
    let property = create_property(&mut engine, properties, "Float", "float");
    let source = create_property_anode(&mut engine, formula, property, 2.0, 3.0);
    let before_property_getters = count_anodes_by_type(&engine, formula, "property");
    let before_undo_len = engine.undo_len();
    let session_id = "duplicate-property-getter-with-position";

    engine.edits.push(Edit::BeginEditSession {
        origin: EditOrigin::Ui,
        label: Some("Duplicate ANode".to_owned()),
        client_edit_id: session_id.to_owned(),
        ui_client_instance_id: None,
    });
    engine
        .apply_edits()
        .expect("duplicate session should begin");

    let duplicate = duplicate_anode(
        &mut engine,
        formula,
        source,
        Some(source),
        "Float Copy",
    );
    engine
        .ui_apply_initial_params_to_node(
            duplicate,
            vec![UiCreateUserItemInitialParam {
                decl_id: DeclId("position".into()),
                value: ParamValue::Vec2(18.0, 12.0),
            }],
            "DuplicateNode",
        )
        .expect("duplicate position initializer should apply");

    engine.edits.push(Edit::EndEditSession {
        client_edit_id: session_id.to_owned(),
    });
    engine
        .apply_edits()
        .expect("duplicate session should end");

    assert_eq!(
        engine.undo_len(),
        before_undo_len + 1,
        "duplicate and position initializer should be one undo step"
    );
    assert_eq!(
        count_anodes_by_type(&engine, formula, "property"),
        before_property_getters + 1
    );

    assert!(engine.undo().expect("undo duplicate should succeed"));
    assert_eq!(
        count_anodes_by_type(&engine, formula, "property"),
        before_property_getters,
        "undo should remove the duplicate instead of only reverting its position"
    );
    assert!(
        engine.nodes.get(duplicate).is_none(),
        "duplicate node should be detached by the first undo"
    );

    assert!(engine.redo().expect("redo duplicate should succeed"));
    assert_eq!(
        anode_position(&engine, duplicate),
        Some((18.0, 12.0)),
        "redo should restore the duplicate at its initialized UI position"
    );
}

#[test]
fn removing_source_property_anode_preserves_duplicate_getter() {
    let (mut engine, formula) = engine_with_formula();
    let properties = find_child_by_decl(&engine, formula, PROPERTIES_DECL_ID)
        .expect("Formula properties root should exist");
    let property = create_property(&mut engine, properties, "Float", "float");
    let source = create_property_anode(&mut engine, formula, property, 2.0, 3.0);
    let session_id = "duplicate-property-getter-source-removal";

    engine.edits.push(Edit::BeginEditSession {
        origin: EditOrigin::Ui,
        label: Some("Duplicate ANode".to_owned()),
        client_edit_id: session_id.to_owned(),
        ui_client_instance_id: None,
    });
    engine
        .apply_edits()
        .expect("duplicate session should begin");
    let duplicate = duplicate_anode(
        &mut engine,
        formula,
        source,
        Some(source),
        "Float Copy",
    );
    engine.edits.push(Edit::EndEditSession {
        client_edit_id: session_id.to_owned(),
    });
    engine
        .apply_edits()
        .expect("duplicate session should end");

    let ack = engine.apply_ui_intent(UiEditIntent::RemoveNode { node: source });
    assert!(
        ack.success,
        "Removing the source property getter should succeed: {ack:?}"
    );
    assert!(
        engine.nodes.get(source).is_none(),
        "source getter should be removed"
    );
    assert!(
        engine.nodes.get(duplicate).is_some(),
        "duplicate getter should remain when the source getter is removed"
    );
    assert_eq!(count_anodes_by_type(&engine, formula, "property"), 1);
}

#[test]
fn anode_creation_materializes_visible_config_and_socket_nodes() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);

    let anode = direct_children(&engine, formula)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
            })
        })
        .expect("Constant ANode should be visible under the Formula");

    assert!(
        find_child_by_decl(&engine, anode, "anode_type").is_none(),
        "ANode type is internal and must not be a child parameter"
    );
    assert_eq!(anode_type(&engine, anode).as_deref(), Some("constant"));
    assert!(engine
        .nodes
        .get(anode)
        .expect("Constant ANode should exist")
        .node_data()
        .meta
        .presentation
        .color
        .is_some());
    assert_eq!(
        parameter_value(&engine, anode, "position"),
        ParamValue::Vec2(2.0, 3.0)
    );
    let config = find_child_by_decl(&engine, anode, "config")
        .expect("Config folder should be visible");
    let outputs = find_child_by_decl(&engine, anode, "outputs")
        .expect("Outputs folder should be visible");
    assert!(
        find_child_by_decl(&engine, config, "config/value").is_some(),
        "Constant value must be a real parameter"
    );
    assert_eq!(
        parameter_value(&engine, config, "config/log"),
        ParamValue::Bool(false),
        "Every ANode should materialize the common Log parameter"
    );
    assert!(
        direct_children(&engine, outputs).into_iter().any(|node| {
            engine.nodes.get(node).is_some_and(|node| {
                node.get_type() == "alchemist_output_socket"
            })
        }),
        "Constant output socket must be a real node"
    );
}

#[test]
fn anode_config_changes_reconcile_real_children() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    let anode = direct_children(&engine, formula)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
            })
        })
        .expect("Constant ANode should exist");
    let config =
        find_child_by_decl(&engine, anode, "config").expect("Config should exist");
    let value_type = find_child_by_decl(
        &engine,
        config,
        "config/value__type",
    )
    .expect("runtime value type should be a real parameter");

    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: value_type,
        value: ParamValue::Enum("vec3".into()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "value type edit should succeed: {ack:?}");
    assert_eq!(
        parameter_value(&engine, config, "config/value"),
        ParamValue::Vec3(0.0, 0.0, 0.0)
    );
    let outputs =
        find_child_by_decl(&engine, anode, "outputs").expect("Outputs should exist");
    let output = direct_children(&engine, outputs)
        .into_iter()
        .next()
        .expect("Constant output should exist");
    assert_eq!(
        parameter_value(&engine, output, "outputs/value/value_type"),
        ParamValue::Str("vec3".into())
    );
}

#[test]
fn forced_value_type_updates_all_forceable_numeric_nodes() {
    for spec in numeric_forceable_node_specs() {
        for value_type in numeric_value_types() {
            let (mut engine, formula) = engine_with_formula();
            create_anode(&mut engine, formula, spec.type_id, 2.0, 3.0);
            let anode = find_anode_by_type(&engine, formula, spec.type_id);
            let config =
                find_child_by_decl(&engine, anode, "config").expect("Config should exist");
            let value_type_param = find_child_by_decl(&engine, config, "config/value_type")
                .unwrap_or_else(|| panic!("{} should expose Value Type", spec.type_id));

            assert_eq!(
                parameter_options(&engine, value_type_param),
                string_vec(numeric_value_types()),
                "{} should expose every numeric-like type and nothing else",
                spec.type_id
            );

            let ack = engine.apply_ui_intent(UiEditIntent::PatchMeta {
                node: value_type_param,
                patch: NodeMetaPatch {
                    enabled: Some(true),
                    ..NodeMetaPatch::default()
                },
            });
            assert!(ack.success, "Value Type enable should succeed: {ack:?}");
            let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
                node: value_type_param,
                value: ParamValue::Enum((*value_type).into()),
                behaviour: ParameterEventBehaviour::Coalesce,
            });
            assert!(ack.success, "Value Type edit should succeed: {ack:?}");
            assert_eq!(
                parameter_value(&engine, config, "config/value_type"),
                ParamValue::Enum((*value_type).into())
            );
            assert!(
                engine
                    .process_tree_snapshot()
                    .node(value_type_param)
                    .is_some_and(|node| node.enabled
                        && node.param_value
                            == Some(ParamValue::Enum((*value_type).into()))),
                "{} Value Type selector should remain enabled after editing",
                spec.type_id
            );
            assert_eq!(
                forced_value_type(&engine, formula, spec.type_id),
                Some((*value_type).to_owned()),
                "{} should materialize the forced type binding",
                spec.type_id
            );

            assert_anode_socket_types(
                &engine,
                anode,
                spec.inputs,
                spec.outputs,
                value_type,
            );
        }
    }
}

#[test]
fn numeric_nodes_infer_each_supported_connected_type_when_selector_is_disabled() {
    for spec in numeric_forceable_node_specs() {
        for value_type in numeric_value_types() {
            let (mut engine, formula) = engine_with_formula();
            create_anode(&mut engine, formula, "constant", 2.0, 3.0);
            create_anode(&mut engine, formula, spec.type_id, 8.0, 3.0);
            let constant = find_anode_by_type(&engine, formula, "constant");
            let anode = find_anode_by_type(&engine, formula, spec.type_id);
            set_constant_value_type(&mut engine, constant, value_type);

            create_connection(&mut engine, formula, constant, "value", anode, spec.inputs[0]);

            let config =
                find_child_by_decl(&engine, anode, "config").expect("Config should exist");
            assert_eq!(
                parameter_value(&engine, config, "config/value_type"),
                ParamValue::Enum((*value_type).into()),
                "{} should show its auto-inferred Value Type while the selector is disabled",
                spec.type_id
            );
            assert_anode_socket_types(
                &engine,
                anode,
                spec.inputs,
                spec.outputs,
                value_type,
            );
        }
    }
}

#[test]
fn first_input_decides_auto_value_type_when_multiple_inputs_are_connected() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    create_anode(&mut engine, formula, "constant", 6.0, 3.0);
    create_anode(&mut engine, formula, "math", 10.0, 3.0);
    let constants = direct_children(&engine, formula)
        .into_iter()
        .filter(|node| anode_type(&engine, *node).as_deref() == Some("constant"))
        .collect::<Vec<_>>();
    let math = find_anode_by_type(&engine, formula, "math");
    set_constant_value_type(&mut engine, constants[0], "vec3");
    set_constant_value_type(&mut engine, constants[1], "float");

    create_connection(&mut engine, formula, constants[0], "value", math, "value2");
    create_connection(&mut engine, formula, constants[1], "value", math, "value1");

    let config = find_child_by_decl(&engine, math, "config").expect("Math config should exist");
    assert_eq!(
        parameter_value(&engine, config, "config/value_type"),
        ParamValue::Enum("float".into())
    );
    assert_anode_socket_types(&engine, math, &["value1", "value2"], &["result"], "float");
}

#[test]
fn disabled_math_input_count_tracks_connected_trailing_socket() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    create_anode(&mut engine, formula, "constant", 6.0, 3.0);
    create_anode(&mut engine, formula, "math", 10.0, 3.0);
    let constants = direct_children(&engine, formula)
        .into_iter()
        .filter(|node| anode_type(&engine, *node).as_deref() == Some("constant"))
        .collect::<Vec<_>>();
    let math = find_anode_by_type(&engine, formula, "math");

    create_connection(&mut engine, formula, constants[0], "value", math, "value1");
    create_connection(&mut engine, formula, constants[1], "value", math, "value2");

    let config = find_child_by_decl(&engine, math, "config").expect("Math config should exist");
    let inputs = find_child_by_decl(&engine, math, "inputs").expect("Math inputs should exist");
    assert_eq!(
        parameter_value(&engine, config, "config/num_inputs"),
        ParamValue::Int(3),
        "Disabled Math num_inputs should auto-grow when all visible inputs are connected"
    );
    assert!(
        find_child_by_decl(&engine, inputs, "inputs/value3").is_some(),
        "Math should materialize one free trailing dynamic input"
    );
    assert_eq!(
        count_children_by_decl(&engine, inputs, "inputs/value3"),
        1,
        "Math should not materialize duplicate trailing dynamic inputs"
    );

    let connection_to_value2 = direct_children(&engine, formula)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistConnection::NODE_TYPE
            }) && parameter_value(&engine, *node, "target_socket")
                == ParamValue::Str("value2".into())
        })
        .expect("Connection to value2 should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::RemoveNode {
        node: connection_to_value2,
    });
    assert!(ack.success, "Connection removal should succeed: {ack:?}");

    assert_eq!(
        parameter_value(&engine, config, "config/num_inputs"),
        ParamValue::Int(2),
        "Disabled Math num_inputs should shrink when the trailing connected socket is removed"
    );
    assert!(
        find_child_by_decl(&engine, inputs, "inputs/value3").is_none(),
        "Math should remove obsolete trailing dynamic inputs"
    );
}

#[test]
fn disabled_concatenate_input_count_tracks_connected_trailing_socket() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    create_anode(&mut engine, formula, "constant", 6.0, 3.0);
    create_anode(&mut engine, formula, "concatenate", 10.0, 3.0);
    let constants = direct_children(&engine, formula)
        .into_iter()
        .filter(|node_id| {
            engine.nodes.get(*node_id).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
                    && anode_type(&engine, *node_id).as_deref() == Some("constant")
            })
        })
        .collect::<Vec<_>>();
    let concatenate = find_anode_by_type(&engine, formula, "concatenate");
    set_constant_value_type(&mut engine, constants[0], "string");
    set_constant_value_type(&mut engine, constants[1], "string");

    create_connection(&mut engine, formula, constants[0], "value", concatenate, "part1");
    create_connection(&mut engine, formula, constants[1], "value", concatenate, "part2");

    let config =
        find_child_by_decl(&engine, concatenate, "config").expect("Concatenate config should exist");
    let inputs =
        find_child_by_decl(&engine, concatenate, "inputs").expect("Concatenate inputs should exist");
    assert_eq!(
        parameter_value(&engine, config, "config/num_inputs"),
        ParamValue::Int(3),
        "Disabled Concatenate num_inputs should auto-grow when all visible inputs are connected"
    );
    assert_eq!(
        count_children_by_decl(&engine, inputs, "inputs/part3"),
        1,
        "Concatenate should materialize exactly one free trailing dynamic input"
    );
    assert_anode_socket_types(
        &engine,
        concatenate,
        &["part1", "part2", "part3"],
        &["result"],
        "string",
    );
}

#[test]
fn generic_nodes_infer_each_connected_primitive_type_without_forced_selector() {
    for value_type in primitive_value_types() {
        let (mut engine, formula) = engine_with_formula();
        create_anode(&mut engine, formula, "constant", 2.0, 3.0);
        create_anode(&mut engine, formula, "compare", 8.0, 3.0);
        let constant = find_anode_by_type(&engine, formula, "constant");
        let compare = find_anode_by_type(&engine, formula, "compare");
        set_constant_value_type(&mut engine, constant, value_type);

        create_connection(&mut engine, formula, constant, "value", compare, "left");

        assert_anode_socket_types(&engine, compare, &["left", "right"], &[], value_type);
        assert_socket_type(&engine, compare, "outputs", "result", "bool");

        let (mut engine, formula) = engine_with_formula();
        create_anode(&mut engine, formula, "constant", 2.0, 3.0);
        create_anode(&mut engine, formula, "delay_one_tick", 8.0, 3.0);
        let constant = find_anode_by_type(&engine, formula, "constant");
        let delay = find_anode_by_type(&engine, formula, "delay_one_tick");
        let delay_config =
            find_child_by_decl(&engine, delay, "config").expect("Delay config should exist");
        assert!(
            find_child_by_decl(&engine, delay_config, "config/value_type").is_none(),
            "passthrough nodes should infer from connections without a forced Value Type UI"
        );
        set_constant_value_type(&mut engine, constant, value_type);

        create_connection(&mut engine, formula, constant, "value", delay, "value");

        assert_anode_socket_types(&engine, delay, &["value"], &["value"], value_type);
    }
}

#[test]
fn routing_node_infers_connected_type_without_value_type_config_and_starts_collapsed() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    create_anode(
        &mut engine,
        formula,
        chataigne_state_machine::alchemist::ROUTING_TYPE,
        8.0,
        3.0,
    );
    let constant = find_anode_by_type(&engine, formula, "constant");
    let route = find_anode_by_type(
        &engine,
        formula,
        chataigne_state_machine::alchemist::ROUTING_TYPE,
    );
    let route_config =
        find_child_by_decl(&engine, route, "config").expect("Routing config should exist");

    assert!(
        find_child_by_decl(&engine, route_config, "config/value_type").is_none(),
        "routing nodes should infer from connections without a forced Value Type UI"
    );
    assert!(
        engine
            .nodes
            .get(route)
            .expect("Routing node should exist")
            .node_data()
            .meta
            .presentation
            .collapsed,
        "routing nodes should start collapsed"
    );
    set_constant_value_type(&mut engine, constant, "vec3");

    create_connection(&mut engine, formula, constant, "value", route, "in");

    assert_anode_socket_types(&engine, route, &["in"], &["out"], "vec3");
}

#[test]
fn authored_formula_subtree_survives_reload_and_compiles() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    create_anode(&mut engine, formula, "debug_log", 18.0, 3.0);

    let anodes = direct_children(&engine, formula)
        .into_iter()
        .filter(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(anodes.len(), 2);
    let constant = anodes
        .iter()
        .copied()
        .find(|node| {
            anode_type(&engine, *node).as_deref() == Some("constant")
        })
        .expect("Constant should exist");
    let debug = anodes
        .iter()
        .copied()
        .find(|node| {
            anode_type(&engine, *node).as_deref() == Some("debug_log")
        })
        .expect("Debug Log should exist");
    create_connection(
        &mut engine,
        formula,
        constant,
        "value",
        debug,
        "value",
    );

    let materialized = formula_from_snapshot(
        &engine.process_tree_snapshot(),
        formula,
    )
    .expect("Formula subtree should materialize");
    assert_eq!(materialized.graph.nodes.len(), 2);
    assert_eq!(materialized.graph.edges.len(), 1);
    assert_eq!(
        parameter_value(&engine, formula, "is_valid"),
        ParamValue::Bool(true)
    );

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("Formula project should encode");
    assert!(json.contains("alchemist_anode"));
    assert!(json.contains("alchemist_connection"));
    assert!(!json.contains("formula_json"));

    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json)
        .expect("Formula project should decode");
    let loaded_formula = loaded
        .nodes
        .iter()
        .find(|(_, node)| {
            node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
        })
        .map(|(id, _)| id)
        .expect("Formula should reload");
    let reloaded = formula_from_snapshot(
        &loaded.process_tree_snapshot(),
        loaded_formula,
    )
    .expect("reloaded Formula subtree should materialize");

    assert_eq!(reloaded.graph.nodes.len(), 2);
    assert_eq!(reloaded.graph.edges.len(), 1);

    let formula_asset =
        golden_core::app::to_sparse_subtree_json_pretty(&engine, formula)
            .expect("Formula subtree should encode as an asset");
    let asset: serde_json::Value =
        serde_json::from_str(&formula_asset).expect("Formula asset should be JSON");
    assert_eq!(asset["root"]["type"], "alchemist_formula");

    let root: AppNode = Folder::new("import root").into();
    let mut imported_engine = AppEngine::new(root);
    imported_engine.add_node(FormulaLibrary::new().into(), None);
    imported_engine
        .apply_edits()
        .expect("import Formula Library should attach");
    let imported_library = imported_engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("import Formula Library should exist");
    let imported_formula = golden_core::app::insert_sparse_subtree_json(
        &mut imported_engine,
        imported_library,
        None,
        &formula_asset,
    )
    .expect("Formula asset should import as a normal node subtree");
    assert_ne!(
        imported_engine
            .nodes
            .get(imported_formula)
            .expect("imported Formula should exist")
            .node_data()
            .meta
            .uuid,
        engine
            .nodes
            .get(formula)
            .expect("source Formula should exist")
            .node_data()
            .meta
            .uuid,
        "asset import should remap persistent identities"
    );
    let imported = formula_from_snapshot(
        &imported_engine.process_tree_snapshot(),
        imported_formula,
    )
    .expect("imported Formula subtree should materialize");
    assert_eq!(imported.graph.nodes.len(), 2);
    assert_eq!(imported.graph.edges.len(), 1);
}

#[test]
fn anode_layout_changes_do_not_revalidate_formula() {
    let (mut engine, formula) = engine_with_formula();
    let constant = create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    assert_eq!(
        parameter_value(&engine, formula, "is_valid"),
        ParamValue::Bool(true)
    );

    let valid_param =
        find_child_by_decl(&engine, formula, "is_valid").expect("Valid parameter should exist");
    engine.edits.push(Edit::SetParam {
        node: valid_param,
        value: ParamValue::Bool(false),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine
        .apply_edits()
        .expect("stale validity sentinel should apply");

    let position =
        find_child_by_decl(&engine, constant, "position").expect("position should exist");
    engine.edits.push(Edit::SetParam {
        node: position,
        value: ParamValue::Vec2(12.0, 8.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine
        .apply_edits()
        .expect("position edit should apply");
    assert_eq!(
        parameter_value(&engine, formula, "is_valid"),
        ParamValue::Bool(false),
        "Moving an ANode must not revalidate the formula"
    );

    let size = find_child_by_decl(&engine, constant, "size").expect("size should exist");
    engine.edits.push(Edit::PatchMeta {
        node: size,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..NodeMetaPatch::default()
        },
    });
    engine.apply_edits().expect("size meta edit should apply");
    assert_eq!(
        parameter_value(&engine, formula, "is_valid"),
        ParamValue::Bool(false),
        "Resizing an ANode must not revalidate the formula"
    );
}

#[test]
fn removing_anode_removes_its_dangling_connections() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    create_anode(&mut engine, formula, "debug_log", 18.0, 3.0);
    let anodes = direct_children(&engine, formula)
        .into_iter()
        .filter(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
            })
        })
        .collect::<Vec<_>>();
    create_connection(
        &mut engine,
        formula,
        anodes[0],
        "value",
        anodes[1],
        "value",
    );

    let ack = engine.apply_ui_intent(UiEditIntent::RemoveNode {
        node: anodes[0],
    });
    assert!(ack.success, "ANode removal should succeed: {ack:?}");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("Formula should receive the child removal event");
    engine
        .apply_edits()
        .expect("Formula connection cleanup should stabilize");
    assert!(
        direct_children(&engine, formula).into_iter().all(|node| {
            engine.nodes.get(node).is_some_and(|node| {
                node.get_type() != AlchemistConnection::NODE_TYPE
            })
        }),
        "Formula should own cleanup of connections whose endpoint was removed"
    );
}

#[test]
fn undoing_anode_removal_restores_its_connections_in_one_step() {
    let (mut engine, formula) = engine_with_formula();
    create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    create_anode(&mut engine, formula, "debug_log", 18.0, 3.0);
    let anodes = direct_children(&engine, formula)
        .into_iter()
        .filter(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistANode::NODE_TYPE
            })
        })
        .collect::<Vec<_>>();
    create_connection(
        &mut engine,
        formula,
        anodes[0],
        "value",
        anodes[1],
        "value",
    );

    let ack = engine.apply_ui_intent(UiEditIntent::RemoveNode { node: anodes[1] });
    assert!(ack.success, "ANode removal should succeed: {ack:?}");
    assert_eq!(
        formula_from_snapshot(&engine.process_tree_snapshot(), formula)
            .expect("Formula should materialize after removal")
            .graph
            .edges
            .len(),
        0,
        "Removing an ANode should remove incident connections during UI stabilization"
    );

    let ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(ack.success, "Undoing ANode removal should succeed: {ack:?}");
    let materialized = formula_from_snapshot(&engine.process_tree_snapshot(), formula)
        .expect("Formula should materialize after undo");
    assert_eq!(materialized.graph.nodes.len(), 2);
    assert_eq!(materialized.graph.edges.len(), 1);

    let ack = engine.apply_ui_intent(UiEditIntent::RemoveNode { node: anodes[1] });
    assert!(
        ack.success,
        "Removing the restored ANode again should not reference stale nodes: {ack:?}"
    );
}

#[test]
fn duplicating_multiple_anodes_offsets_positions_without_corrupting_sockets() {
    let (mut engine, formula) = engine_with_formula();
    let type_ids = [
        "constant",
        "math",
        "one_minus",
        "inverse",
        "negate",
        "constant",
        "math",
        "one_minus",
        "inverse",
        "negate",
        "constant",
        "math",
        "one_minus",
        "inverse",
        "negate",
    ];
    let originals = type_ids
        .iter()
        .enumerate()
        .map(|(index, type_id)| {
            create_anode(
                &mut engine,
                formula,
                type_id,
                2.0 + index as f64 * 3.0,
                3.0 + (index % 3) as f64 * 2.0,
            )
        })
        .collect::<Vec<_>>();

    let mut prev_sibling = originals.last().copied();
    for (index, source) in originals.iter().copied().enumerate() {
        let copy = duplicate_anode(
            &mut engine,
            formula,
            source,
            prev_sibling,
            &format!("ANode Copy {index}"),
        );
        let ParamValue::Vec2(x, y) = parameter_value(&engine, source, "position") else {
            panic!("source ANode position should be vec2");
        };
        assert_eq!(
            parameter_value(&engine, copy, "position"),
            ParamValue::Vec2(x + 1.5, y + 1.5)
        );
        assert_unique_socket_ids(&engine, copy, "inputs");
        assert_unique_socket_ids(&engine, copy, "outputs");
        prev_sibling = Some(copy);
    }
}

#[test]
fn duplicating_connected_anodes_copies_edge_and_undoes_as_one_block() {
    let (mut engine, formula) = engine_with_formula();
    let source = create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    let target = create_anode(&mut engine, formula, "debug_log", 18.0, 3.0);
    create_connection(&mut engine, formula, source, "value", target, "value");

    let edit_id = "test-duplicate-connected-anodes".to_string();
    engine.edits.push(Edit::BeginEditSession {
        origin: EditOrigin::Ui,
        label: Some("Duplicate connected ANodes".into()),
        client_edit_id: edit_id.clone(),
        ui_client_instance_id: None,
    });
    engine
        .apply_edits()
        .expect("Duplicate edit session should begin");

    let copied = engine
        .ui_apply_duplicate_nodes_with_dependent_user_items(
            vec![
                UiDuplicateNodeSpec {
                    source,
                    new_parent: formula,
                    new_prev_sibling: Some(target),
                    initial_params: vec![UiCreateUserItemInitialParam {
                        decl_id: DeclId("position".into()),
                        value: ParamValue::Vec2(22.0, 3.0),
                    }],
                },
                UiDuplicateNodeSpec {
                    source: target,
                    new_parent: formula,
                    new_prev_sibling: None,
                    initial_params: vec![UiCreateUserItemInitialParam {
                        decl_id: DeclId("position".into()),
                        value: ParamValue::Vec2(38.0, 3.0),
                    }],
                },
            ],
            Vec::new(),
            vec![UiDuplicateDependentUserItem {
                parent: formula,
                node_type: AlchemistConnection::NODE_TYPE.to_owned(),
                label: Some("Connection".into()),
                initial_params: vec![
                    UiDuplicateDependentUserItemInitialParam {
                        decl_id: DeclId("source_node".into()),
                        value: UiDuplicateDependentInitialParamValue::DuplicatedNodeReference {
                            source,
                        },
                    },
                    UiDuplicateDependentUserItemInitialParam {
                        decl_id: DeclId("source_socket".into()),
                        value: UiDuplicateDependentInitialParamValue::Literal {
                            value: ParamValue::Str("value".into()),
                        },
                    },
                    UiDuplicateDependentUserItemInitialParam {
                        decl_id: DeclId("target_node".into()),
                        value: UiDuplicateDependentInitialParamValue::DuplicatedNodeReference {
                            source: target,
                        },
                    },
                    UiDuplicateDependentUserItemInitialParam {
                        decl_id: DeclId("target_socket".into()),
                        value: UiDuplicateDependentInitialParamValue::Literal {
                            value: ParamValue::Str("value".into()),
                        },
                    },
                ],
            }],
            |node| node.project_encode_data(),
            <AppNode as ProjectNode>::project_decode_node,
        )
        .expect("Connected ANode duplicate batch should apply");

    engine.edits.push(Edit::EndEditSession {
        client_edit_id: edit_id,
    });
    engine
        .apply_edits()
        .expect("Duplicate edit session should end");

    assert_eq!(copied.len(), 2);
    assert_eq!(
        parameter_value(&engine, copied[0], "position"),
        ParamValue::Vec2(22.0, 3.0)
    );
    assert_eq!(
        parameter_value(&engine, copied[1], "position"),
        ParamValue::Vec2(38.0, 3.0)
    );

    let materialized = formula_from_snapshot(&engine.process_tree_snapshot(), formula)
        .expect("Formula should materialize after connected duplicate");
    assert_eq!(materialized.graph.nodes.len(), 4);
    assert_eq!(materialized.graph.edges.len(), 2);
    let copied_source = materialized
        .graph
        .nodes
        .values()
        .find(|node| node.label == "Constant 2")
        .expect("Copied source ANode should materialize")
        .id
        .clone();
    let copied_target = materialized
        .graph
        .nodes
        .values()
        .find(|node| node.label == "Debug Log 2")
        .expect("Copied target ANode should materialize")
        .id
        .clone();
    assert!(
        materialized.graph.edges.iter().any(|edge| {
            edge.from.node == copied_source
                && edge.from.socket.to_string() == "value"
                && edge.to.node == copied_target
                && edge.to.socket.to_string() == "value"
        }),
        "Copied edge should connect the copied source and target ANodes"
    );

    let ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(
        ack.success,
        "Undoing connected duplicate should remove the whole copied block: {ack:?}"
    );
    let materialized_after_undo = formula_from_snapshot(&engine.process_tree_snapshot(), formula)
        .expect("Formula should materialize after duplicate undo");
    assert_eq!(materialized_after_undo.graph.nodes.len(), 2);
    assert_eq!(materialized_after_undo.graph.edges.len(), 1);
    assert!(engine.nodes.get(copied[0]).is_none());
    assert!(engine.nodes.get(copied[1]).is_none());
}

#[derive(Clone, Copy)]
struct ANodeSocketSpec {
    type_id: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
}

const NUMERIC_FORCEABLE_NODE_SPECS: &[ANodeSocketSpec] = &[
    ANodeSocketSpec {
        type_id: "math",
        inputs: &["value1", "value2"],
        outputs: &["result"],
    },
    ANodeSocketSpec {
        type_id: "one_minus",
        inputs: &["value"],
        outputs: &["result"],
    },
    ANodeSocketSpec {
        type_id: "inverse",
        inputs: &["value"],
        outputs: &["result"],
    },
    ANodeSocketSpec {
        type_id: "negate",
        inputs: &["value"],
        outputs: &["result"],
    },
];

fn numeric_forceable_node_specs() -> &'static [ANodeSocketSpec] {
    NUMERIC_FORCEABLE_NODE_SPECS
}

fn numeric_value_types() -> &'static [&'static str] {
    &["int", "float", "vec2", "vec3", "color"]
}

fn primitive_value_types() -> &'static [&'static str] {
    &[
        "unit", "bool", "trigger", "int", "float", "string", "vec2",
        "vec3", "color", "duration",
    ]
}

fn string_vec(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn engine_with_formula() -> (AppEngine, NodeId) {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.apply_edits().expect("Formula Library should attach");
    let library = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Formula Library should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: library,
        node_type: AlchemistFormulaDefinition::NODE_TYPE.to_owned(),
        label: Some("Custom Formula".into()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Formula creation should succeed: {ack:?}");
    let formula = direct_children(&engine, library)
        .into_iter()
        .find(|node| {
            engine.nodes.get(*node).is_some_and(|node| {
                node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
            })
        })
        .expect("Formula should exist");
    (engine, formula)
}

fn create_anode(
    engine: &mut AppEngine,
    formula: NodeId,
    type_id: &str,
    x: f64,
    y: f64,
) -> NodeId {
    let before = direct_children(engine, formula);
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula,
        node_type: format!("{ANODE_CREATE_PREFIX}{type_id}"),
        label: None,
        initial_params: vec![UiCreateUserItemInitialParam {
            decl_id: DeclId("position".into()),
            value: ParamValue::Vec2(x, y),
        }],
    });
    assert!(ack.success, "ANode creation should succeed: {ack:?}");
    direct_children(engine, formula)
        .into_iter()
        .find(|node| !before.contains(node) && anode_type(engine, *node).is_some())
        .unwrap_or_else(|| panic!("created ANode `{type_id}` should exist"))
}

fn duplicate_anode(
    engine: &mut AppEngine,
    formula: NodeId,
    source: NodeId,
    new_prev_sibling: Option<NodeId>,
    label: &str,
) -> NodeId {
    let duplicate = engine
        .duplicate_subtree_with(
            source,
            formula,
            new_prev_sibling,
            Some(label.to_owned()),
            |node| node.project_encode_data(),
            <AppNode as ProjectNode>::project_decode_node,
        )
        .unwrap_or_else(|err| panic!("ANode duplicate should succeed: {err:?}"));

    assert!(
        anode_type(engine, duplicate).is_some(),
        "duplicated node `{label}` should be an ANode"
    );
    duplicate
}

fn create_property(
    engine: &mut AppEngine,
    properties: NodeId,
    label: &str,
    property_type: &str,
) -> NodeId {
    let before = direct_children(engine, properties);
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: properties,
        node_type: format!("{PROPERTY_CREATE_PREFIX}{property_type}"),
        label: Some(label.to_owned()),
        initial_params: Vec::new(),
    });
    assert!(ack.success, "Property creation should succeed: {ack:?}");
    direct_children(engine, properties)
        .into_iter()
        .find(|node| {
            !before.contains(node)
                && engine
                    .nodes
                    .get(*node)
                    .is_some_and(|node| node.get_type() == AlchemistProperty::NODE_TYPE)
        })
        .unwrap_or_else(|| panic!("created Property `{label}` should exist"))
}

fn create_property_anode(
    engine: &mut AppEngine,
    formula: NodeId,
    property: NodeId,
    x: f64,
    y: f64,
) -> NodeId {
    let before = direct_children(engine, formula);
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula,
        node_type: format!("{ANODE_CREATE_PREFIX}property"),
        label: Some("Float".into()),
        initial_params: vec![
            UiCreateUserItemInitialParam {
                decl_id: DeclId("config/property_id".into()),
                value: reference_to_node(engine, property),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("position".into()),
                value: ParamValue::Vec2(x, y),
            },
        ],
    });
    assert!(
        ack.success,
        "Property getter creation should succeed: {ack:?}"
    );
    direct_children(engine, formula)
        .into_iter()
        .find(|node| !before.contains(node) && anode_type(engine, *node).as_deref() == Some("property"))
        .unwrap_or_else(|| panic!("created property ANode should exist"))
}

fn reference_to_node(engine: &AppEngine, node: NodeId) -> ParamValue {
    let uuid = engine
        .nodes
        .get(node)
        .expect("Referenced node should exist")
        .node_data()
        .meta
        .uuid;
    ParamValue::Reference(NodeReference::new(uuid))
}

fn node_uuid_string(engine: &AppEngine, node: NodeId) -> String {
    engine
        .nodes
        .get(node)
        .expect("Node should exist")
        .node_data()
        .meta
        .uuid
        .0
        .to_string()
}

fn create_connection(
    engine: &mut AppEngine,
    formula: NodeId,
    source: NodeId,
    source_socket: &str,
    target: NodeId,
    target_socket: &str,
) {
    let source_uuid = engine
        .nodes
        .get(source)
        .expect("source should exist")
        .node_data()
        .meta
        .uuid;
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target should exist")
        .node_data()
        .meta
        .uuid;
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula,
        node_type: AlchemistConnection::NODE_TYPE.to_owned(),
        label: None,
        initial_params: vec![
            UiCreateUserItemInitialParam {
                decl_id: DeclId("source_node".into()),
                value: ParamValue::Reference(NodeReference::new(source_uuid)),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("source_socket".into()),
                value: ParamValue::Str(source_socket.into()),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("target_node".into()),
                value: ParamValue::Reference(NodeReference::new(target_uuid)),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("target_socket".into()),
                value: ParamValue::Str(target_socket.into()),
            },
        ],
    });
    assert!(ack.success, "Connection creation should succeed: {ack:?}");
}

fn set_constant_value_type(engine: &mut AppEngine, constant: NodeId, value_type: &str) {
    let config =
        find_child_by_decl(engine, constant, "config").expect("Constant config should exist");
    let value_type_param = find_child_by_decl(engine, config, "config/value__type")
        .expect("Constant value type should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: value_type_param,
        value: ParamValue::Enum(value_type.into()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "Constant type edit should succeed: {ack:?}");
}

fn forced_value_type(engine: &AppEngine, formula: NodeId, type_id: &str) -> Option<String> {
    let materialized = formula_from_snapshot(&engine.process_tree_snapshot(), formula).ok()?;
    let value_type = materialized
        .graph
        .nodes
        .values()
        .find(|node| node.type_id.as_str() == type_id)?
        .forced_type_bindings
        .iter()
        .next()
        .map(|(_, binding)| binding.value_type.to_string());
    value_type
}

fn direct_children(engine: &AppEngine, parent: NodeId) -> Vec<NodeId> {
    engine.process_tree_snapshot().child_ids(parent)
}

fn find_child_by_decl(
    engine: &AppEngine,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    engine
        .process_tree_snapshot()
        .find_child_by_decl_id(parent, decl_id)
}

fn count_children_by_decl(engine: &AppEngine, parent: NodeId, decl_id: &str) -> usize {
    let snapshot = engine.process_tree_snapshot();
    snapshot
        .child_ids(parent)
        .into_iter()
        .filter(|child| {
            snapshot
                .node(*child)
                .is_some_and(|node| node.decl_id == decl_id)
        })
        .count()
}

fn find_descendant_by_decl(
    engine: &AppEngine,
    root: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    let snapshot = engine.process_tree_snapshot();
    let mut stack = vec![root];
    while let Some(parent) = stack.pop() {
        for child in snapshot.child_ids(parent) {
            let child_snapshot = snapshot.node(child)?;
            if child_snapshot.decl_id == decl_id {
                return Some(child);
            }
            stack.push(child);
        }
    }
    None
}

fn find_anode_by_type(engine: &AppEngine, formula: NodeId, type_id: &str) -> NodeId {
    direct_children(engine, formula)
        .into_iter()
        .find(|node| anode_type(engine, *node).as_deref() == Some(type_id))
        .unwrap_or_else(|| panic!("ANode `{type_id}` should exist"))
}

fn count_anodes_by_type(engine: &AppEngine, formula: NodeId, type_id: &str) -> usize {
    direct_children(engine, formula)
        .into_iter()
        .filter(|node| anode_type(engine, *node).as_deref() == Some(type_id))
        .count()
}

fn anode_position(engine: &AppEngine, anode: NodeId) -> Option<(f64, f64)> {
    let position = find_child_by_decl(engine, anode, "position")?;
    let parameter = engine
        .nodes
        .get(position)?
        .as_any()
        .downcast_ref::<Parameter>()?;
    parameter.value.as_vec2().map(|value| (value.0, value.1))
}

fn anode_type(engine: &AppEngine, node: NodeId) -> Option<String> {
    engine
        .nodes
        .get(node)?
        .node_data()
        .meta
        .tags
        .iter()
        .find_map(|tag| tag.strip_prefix(ANODE_TYPE_TAG_PREFIX))
        .map(ToOwned::to_owned)
}

fn parameter_options(engine: &AppEngine, node: NodeId) -> Vec<String> {
    engine
        .nodes
        .get(node)
        .and_then(Node::engine_param_snapshot)
        .map(|snapshot| {
            snapshot
                .constraints
                .enum_options
                .into_iter()
                .map(|option| option.variant_id)
                .collect()
        })
        .unwrap_or_else(|| panic!("node `{node:?}` should be a parameter"))
}

fn assert_anode_socket_types(
    engine: &AppEngine,
    anode: NodeId,
    inputs: &[&str],
    outputs: &[&str],
    expected_type: &str,
) {
    for input in inputs {
        assert_socket_type(engine, anode, "inputs", input, expected_type);
    }
    for output in outputs {
        assert_socket_type(engine, anode, "outputs", output, expected_type);
    }
}

fn assert_socket_type(
    engine: &AppEngine,
    anode: NodeId,
    direction: &str,
    socket: &str,
    expected_type: &str,
) {
    let folder =
        find_child_by_decl(engine, anode, direction).expect("socket folder should exist");
    let socket_node = find_child_by_decl(
        engine,
        folder,
        &format!("{direction}/{socket}"),
    )
    .unwrap_or_else(|| panic!("{direction} socket `{socket}` should exist"));
    assert_eq!(
        parameter_value(
            engine,
            socket_node,
            &format!("{direction}/{socket}/value_type"),
        ),
        ParamValue::Str(expected_type.into()),
        "{direction} socket `{socket}` should have type `{expected_type}`"
    );
}

fn assert_unique_socket_ids(engine: &AppEngine, anode: NodeId, direction: &str) {
    let ids = socket_ids(engine, anode, direction);
    let mut unique = std::collections::HashSet::new();
    for id in &ids {
        assert!(
            unique.insert(id.clone()),
            "{direction} socket ids should be unique for `{anode:?}`: {ids:?}"
        );
    }
}

fn socket_ids(engine: &AppEngine, anode: NodeId, direction: &str) -> Vec<String> {
    let folder =
        find_child_by_decl(engine, anode, direction).expect("socket folder should exist");
    direct_children(engine, folder)
        .into_iter()
        .map(|socket| {
            let socket_decl_id = engine
                .nodes
                .get(socket)
                .expect("socket node should exist")
                .node_data()
                .meta
                .decl_id
                .0
                .clone();
            match parameter_value(engine, socket, &format!("{socket_decl_id}/socket_id")) {
                ParamValue::Str(value) => value,
                other => panic!("socket id should be a string, got {other:?}"),
            }
        })
        .collect()
}

fn parameter_value(
    engine: &AppEngine,
    parent: NodeId,
    decl_id: &str,
) -> ParamValue {
    let child = find_child_by_decl(engine, parent, decl_id)
        .unwrap_or_else(|| panic!("parameter `{decl_id}` should exist"));
    engine
        .nodes
        .get(child)
        .and_then(Node::engine_param_snapshot)
        .map(|snapshot| snapshot.value)
        .unwrap_or_else(|| panic!("`{decl_id}` should be a parameter"))
}

fn node_has_warning_id(engine: &AppEngine, node: NodeId, warning_id: &str) -> bool {
    engine
        .process_tree_snapshot()
        .node(node)
        .is_some_and(|snapshot| {
            snapshot
                .presentation
                .warnings
                .iter()
                .any(|warning| warning.id == warning_id)
        })
}
