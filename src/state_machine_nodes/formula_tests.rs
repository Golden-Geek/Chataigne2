use golden_core::{
    color::Color,
    node::{
        DeclId, Folder, Node, NodeId, NodeMetaPatch, NodeReference,
    },
    parameter::{ParamValue, Parameter, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    ui_sync::{UiCreateUserItemInitialParam, UiEditIntent},
};

use super::{
    ANODE_CREATE_PREFIX, ANODE_ITEM_KIND, ANODE_TYPE_TAG_PREFIX,
    AlchemistANode, AlchemistConnection, AlchemistFormulaDefinition,
    AlchemistFormulaFolder, AlchemistProperty, AlchemistPropertyManager,
    AlchemistPropertiesManager, FORMULA_FOLDER_ITEM_KIND,
    FORMULA_FOLDER_NODE_TYPE, FORMULA_ITEM_KIND, FormulaLibrary,
    PROPERTIES_DECL_ID,
    PROPERTY_CREATE_PREFIX, PROPERTY_MANAGER_CREATE_PREFIX,
    formula_from_snapshot,
};
use crate::app::{AppEngine, AppNode};

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
            item.node_type == FORMULA_FOLDER_NODE_TYPE
                && item.item_kind == FORMULA_FOLDER_ITEM_KIND
        }));
    }
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
        .color;
    let value_color = engine
        .nodes
        .get(value)
        .expect("Property value should exist")
        .node_data()
        .meta
        .presentation
        .color;
    assert!(property_color.is_some());
    assert_eq!(property_color, value_color);
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: value,
        value: ParamValue::Float(3.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "Property value edit should succeed: {ack:?}");

    let property_uuid = engine
        .nodes
        .get(property)
        .expect("Property should exist")
        .node_data()
        .meta
        .uuid
        .0
        .to_string();
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: formula,
        node_type: format!("{ANODE_CREATE_PREFIX}property"),
        label: Some("Amount".into()),
        initial_params: vec![
            UiCreateUserItemInitialParam {
                decl_id: DeclId("config/property_id".into()),
                value: ParamValue::Str(property_uuid),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("config/value__type".into()),
                value: ParamValue::Enum("float".into()),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("config/value".into()),
                value: ParamValue::Float(3.5),
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
    assert!(
        !engine
            .nodes
            .get(getter_node)
            .expect("Property getter should exist")
            .node_data()
            .meta
            .user_permissions
            .can_edit_name,
        "Property getter labels must stay synchronized with their property"
    );
    assert!(
        !engine
            .nodes
            .get(getter_node)
            .expect("Property getter should exist")
            .node_data()
            .meta
            .user_permissions
            .can_edit_color,
        "Property getter colors must come from their source Property"
    );
    let getter_config = find_child_by_decl(&engine, getter_node, "config")
        .expect("Property getter Config should exist");
    assert!(
        direct_children(&engine, getter_config)
            .into_iter()
            .filter_map(|child| {
                engine
                    .nodes
                    .get(child)
                    .and_then(|node| node.as_any().downcast_ref::<Parameter>())
            })
            .all(|parameter| {
                parameter.read_only
                    && !parameter
                        .node_data()
                        .meta
                        .presentation
                        .show_in_inspector_content
            }),
        "Property getter configuration must be hidden and read-only"
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
            .color,
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
) {
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
