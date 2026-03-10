use std::time::Duration;

use crate::define_node_enum;
use crate::edit::Edit;
use crate::engine::Engine;
use crate::node::{DeclId, Folder, Node, NodeId, NodeReference};
use crate::parameter::{
    CssUnit, CssValue, ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour, ParameterSnapshot,
    RangeConstraint, ReferenceTargetKind,
};
use crate::ui_sync::{UiCreateUserItemInitialParam, UiEditIntent};

use super::{
    DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_NODE_TYPE, DASHBOARD_NODE_WIDGET_NODE_TYPE,
    DASHBOARD_PAGE_NODE_TYPE, DASHBOARD_WIDGET_CONTAINER_NODE_TYPE, DashboardNode,
};

define_node_enum!(
    enum DashboardTestNode {}
);

fn first_child<T: Node>(engine: &Engine<T>, parent: NodeId) -> NodeId {
    engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child)
        .expect("parent should have one child")
}

fn direct_child_by_type<T: Node>(engine: &Engine<T>, parent: NodeId, node_type: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let child_node = engine.nodes.get(child_id)?;
        if child_node.get_type() == node_type {
            return Some(child_id);
        }
        child = child_node.node_data().next_sibling;
    }
    None
}

fn find_descendant_by_decl<T: Node>(engine: &Engine<T>, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    let mut stack = vec![parent];
    while let Some(node_id) = stack.pop() {
        let Some(node) = engine.nodes.get(node_id) else {
            continue;
        };

        let mut child = node.node_data().first_child;
        while let Some(child_id) = child {
            let Some(child_node) = engine.nodes.get(child_id) else {
                break;
            };
            let child_decl_id = child_node.node_data().meta.decl_id.0.as_str();
            if child_decl_id == decl_id || child_decl_id.rsplit('/').next() == Some(decl_id) {
                return Some(child_id);
            }
            stack.push(child_id);
            child = child_node.node_data().next_sibling;
        }
    }
    None
}

fn count_descendants_by_decl<T: Node>(engine: &Engine<T>, parent: NodeId, decl_id: &str) -> usize {
    let mut count = 0usize;
    let mut stack = vec![parent];
    while let Some(node_id) = stack.pop() {
        let Some(node) = engine.nodes.get(node_id) else {
            continue;
        };

        let mut child = node.node_data().first_child;
        while let Some(child_id) = child {
            let Some(child_node) = engine.nodes.get(child_id) else {
                break;
            };
            let child_decl_id = child_node.node_data().meta.decl_id.0.as_str();
            if child_decl_id == decl_id || child_decl_id.rsplit('/').next() == Some(decl_id) {
                count += 1;
            }
            stack.push(child_id);
            child = child_node.node_data().next_sibling;
        }
    }

    count
}

fn param_snapshot<T: Node>(engine: &Engine<T>, node_id: NodeId) -> ParameterSnapshot {
    engine
        .nodes
        .get(node_id)
        .and_then(Node::engine_param_snapshot)
        .expect("node should expose a parameter snapshot")
}

fn direct_child_decl_ids<T: Node>(engine: &Engine<T>, parent: NodeId) -> Vec<String> {
    let mut decl_ids = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let child_node = engine.nodes.get(child_id).expect("child should exist");
        decl_ids.push(child_node.node_data().meta.decl_id.0.clone());
        child = child_node.node_data().next_sibling;
    }
    decl_ids
}

#[test]
fn dashboard_catalog_exposes_pages_and_widgets() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("dashboard creation should apply");

    let dashboard = first_child(&engine, engine.root);
    assert_eq!(
        engine.nodes.get(dashboard).expect("dashboard should exist").get_type(),
        DASHBOARD_NODE_TYPE
    );

    let dashboard_creatable = engine.catalog_creatable_items(dashboard);
    assert!(
        dashboard_creatable
            .iter()
            .any(|item| item.node_type == DASHBOARD_PAGE_NODE_TYPE),
        "dashboard should expose page creation"
    );

    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    let page_creatable = engine.catalog_creatable_items(page);
    assert!(
        page_creatable
            .iter()
            .any(|item| item.node_type == DASHBOARD_WIDGET_CONTAINER_NODE_TYPE),
        "page should expose container widget creation"
    );
    assert!(
        page_creatable
            .iter()
            .any(|item| item.node_type == DASHBOARD_NODE_WIDGET_NODE_TYPE),
        "page should expose node widget creation"
    );
    assert!(
        page_creatable
            .iter()
            .any(|item| item.node_type == DASHBOARD_GENERIC_WIDGET_NODE_TYPE),
        "page should expose generic widget creation"
    );
}

#[test]
fn dashboard_pages_expose_pixel_page_size_as_vec2() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("dashboard creation should apply");

    let dashboard = first_child(&engine, engine.root);
    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    let page_size = find_descendant_by_decl(&engine, page, "page_size").expect("page size parameter should exist");
    let page_size_snapshot = param_snapshot(&engine, page_size);
    let page_size_node = engine.nodes.get(page_size).expect("page size node should exist");

    assert_eq!(page_size_snapshot.value, ParamValue::Vec2(1920.0, 1080.0));
    assert_eq!(page_size_snapshot.constraints.step, Some(1.0));
    assert_eq!(page_size_snapshot.constraints.step_base, Some(0.0));
    assert_eq!(
        page_size_snapshot.constraints.range,
        RangeConstraint::components(Some(vec![1.0, 1.0]), None),
        "page size should clamp both components to positive pixel dimensions"
    );
    assert!(
        !page_size_node.node_data().meta.enabled,
        "page size should stay opt-in by default"
    );
    assert!(
        page_size_node.node_data().meta.can_be_disabled,
        "page size should remain disableable"
    );
    assert!(
        find_descendant_by_decl(&engine, page, "page_width").is_none(),
        "legacy page width parameter should be removed"
    );
    assert!(
        find_descendant_by_decl(&engine, page, "page_height").is_none(),
        "legacy page height parameter should be removed"
    );
}

#[test]
fn dashboard_widgets_materialize_binding_constraints() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("dashboard creation should apply");

    let dashboard = first_child(&engine, engine.root);
    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    engine
        .queue_catalog_create(page, DASHBOARD_NODE_WIDGET_NODE_TYPE, Some("Auto UI".to_string()), None)
        .expect("node widget creation should queue");
    engine
        .queue_catalog_create(
            page,
            DASHBOARD_GENERIC_WIDGET_NODE_TYPE,
            Some("Button".to_string()),
            None,
        )
        .expect("generic widget creation should queue");
    engine.apply_edits().expect("widget creation should apply");

    let node_widget = direct_child_by_type(&engine, page, DASHBOARD_NODE_WIDGET_NODE_TYPE)
        .expect("page should contain a node widget child");
    let generic_widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE)
        .expect("page should contain a generic widget child");

    let target_node_param = find_descendant_by_decl(&engine, node_widget, "target_node")
        .expect("node widget target reference should exist");
    let target_param_param = find_descendant_by_decl(&engine, generic_widget, "target_param")
        .expect("generic widget target reference should exist");

    assert_eq!(
        param_snapshot(&engine, target_node_param)
            .constraints
            .reference
            .target_kind,
        ReferenceTargetKind::AnyNode,
        "node widgets should accept references to any node"
    );
    assert_eq!(
        param_snapshot(&engine, target_param_param)
            .constraints
            .reference
            .target_kind,
        ReferenceTargetKind::ParameterOnly,
        "generic widgets should bind to parameters by default"
    );
}

#[test]
fn dashboard_node_widget_types_follow_target_capabilities() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    let mut speed = Parameter::new("Speed", ParamValue::Float(0.5), ParameterChangeCheck::ValueChange);
    speed.constraints.range = RangeConstraint::uniform(Some(0.0), Some(1.0));
    engine.add_node(speed.into(), None);
    engine.add_node(
        Parameter::new("Point", ParamValue::Vec2(0.0, 0.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("initial tree should apply");

    let float_param = direct_child_by_type(&engine, engine.root, "float").expect("float parameter should exist");
    let vec2_param = direct_child_by_type(&engine, engine.root, "vec2").expect("vec2 parameter should exist");
    let dashboard =
        direct_child_by_type(&engine, engine.root, DASHBOARD_NODE_TYPE).expect("dashboard should exist");
    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    engine
        .queue_catalog_create(page, DASHBOARD_NODE_WIDGET_NODE_TYPE, Some("Auto UI".to_string()), None)
        .expect("node widget creation should queue");
    for _ in 0..3 {
        engine.apply_edits().expect("widget creation should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let widget = direct_child_by_type(&engine, page, DASHBOARD_NODE_WIDGET_NODE_TYPE)
        .expect("page should contain a node widget child");
    assert_eq!(
        direct_child_decl_ids(&engine, widget),
        vec!["widget", "layout", "widget_options"],
        "node widgets should expose widget, layout, and widget options nodes in direct-child order"
    );
    let widget_type =
        find_descendant_by_decl(&engine, widget, "widget_type").expect("widget type parameter should exist");
    let target_node =
        find_descendant_by_decl(&engine, widget, "target_node").expect("target node parameter should exist");
    let max_child_level = find_descendant_by_decl(&engine, widget, "max_child_level")
        .expect("inspector widgets should expose max child level");
    let label_placement = find_descendant_by_decl(&engine, widget, "label_placement")
        .expect("inspector widgets should expose label placement inside widget options");

    let initial_widget_type = param_snapshot(&engine, widget_type);
    let initial_types = initial_widget_type
        .constraints
        .enum_options
        .iter()
        .map(|option| option.variant_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        initial_types,
        vec!["inspector"],
        "unbound widgets should expose inspector only"
    );
    assert_eq!(
        initial_widget_type.value,
        ParamValue::Enum("inspector".to_string()),
        "unbound widgets should default to inspector"
    );
    assert_eq!(
        param_snapshot(&engine, max_child_level).value,
        ParamValue::Int(2),
        "inspector widgets should default to a child depth of two levels"
    );
    assert_eq!(
        param_snapshot(&engine, label_placement).value,
        ParamValue::Enum("inside".to_string()),
        "node widget label placement should now live under widget options"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "include_children").is_none(),
        "legacy include children should be removed from inspector widget options"
    );

    let float_uuid = engine
        .nodes
        .get(float_param)
        .expect("float parameter should exist")
        .node_data()
        .meta
        .uuid;
    engine.edits.push(Edit::SetParam {
        node: target_node,
        value: ParamValue::Reference(NodeReference::new(float_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..4 {
        engine.apply_edits().expect("binding float target should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let float_widget_type = param_snapshot(&engine, widget_type);
    let float_types = float_widget_type
        .constraints
        .enum_options
        .iter()
        .map(|option| (option.variant_id.as_str(), option.label.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        float_types,
        vec![
            ("default", "Default"),
            ("inspector", "Inspector"),
            ("slider", "Slider"),
            ("rotary", "Rotary"),
        ],
        "numeric parameters should expose default, inspector, slider, and rotary widget types",
    );
    assert_eq!(
        float_widget_type.value,
        ParamValue::Enum("default".to_string()),
        "parameter widgets should default to the semantic default widget type when first bound",
    );
    assert_eq!(
        count_descendants_by_decl(&engine, widget, "show_enable_button"),
        1,
        "parameter widgets should expose only one enable-button option at a time"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "slider_show_value_field").is_some(),
        "default numeric widgets should materialize slider option nodes",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "custom_range").is_some(),
        "default numeric widgets should expose the custom range parameter",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "rotary_show_value_field").is_none(),
        "default numeric widgets should not materialize rotary options",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "widget_options").is_some(),
        "widget options should remain mounted directly under the node widget",
    );

    engine.edits.push(Edit::SetParam {
        node: widget_type,
        value: ParamValue::Enum("rotary".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("switching float widget to rotary should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert!(
        find_descendant_by_decl(&engine, widget, "slider_show_value_field").is_none(),
        "switching to rotary should replace the slider options node",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "rotary_show_value_field").is_some(),
        "switching to rotary should materialize rotary-specific options",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "custom_range").is_some(),
        "rotary widgets should expose the custom range parameter",
    );

    let vec2_uuid = engine
        .nodes
        .get(vec2_param)
        .expect("vec2 parameter should exist")
        .node_data()
        .meta
        .uuid;
    engine.edits.push(Edit::SetParam {
        node: target_node,
        value: ParamValue::Reference(NodeReference::new(vec2_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..4 {
        engine.apply_edits().expect("binding vec2 target should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let vec2_widget_type = param_snapshot(&engine, widget_type);
    let vec2_types = vec2_widget_type
        .constraints
        .enum_options
        .iter()
        .map(|option| (option.variant_id.as_str(), option.label.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        vec2_types,
        vec![
            ("default", "Default"),
            ("inspector", "Inspector"),
            ("vec2Pad", "2D Pad"),
        ],
        "vec2 parameters should expose the default and 2D Pad widget types",
    );
    assert_eq!(
        vec2_widget_type.value,
        ParamValue::Enum("default".to_string()),
        "unsupported widget types should reset to the target default when rebinding",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "vector_layout").is_some(),
        "rebinding to vec2 should replace rotary options with vector-editor options",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "custom_range").is_some(),
        "vec2 widgets should expose the custom range folder",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "custom_range/range_min").is_some(),
        "vec2 widgets should expose vector custom range bounds",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "rotary_show_value_field").is_none(),
        "rebinding away from numeric rotary mode should remove rotary options",
    );

    engine.edits.push(Edit::SetParam {
        node: widget_type,
        value: ParamValue::Enum("vec2Pad".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("switching vec2 widget to 2D pad should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert!(
        find_descendant_by_decl(&engine, widget, "trail_time").is_some(),
        "vec2 pad widgets should expose trail time controls",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "custom_range").is_some(),
        "vec2 pad widgets should keep the custom range folder available",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "custom_range/range_min").is_some(),
        "vec2 pad widgets should keep custom range bounds available",
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "vector_layout").is_none(),
        "vec2 pad widgets should hide vector-editor-only layout options",
    );
}

#[test]
fn dashboard_widget_layout_dependencies_follow_parent_layout() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("dashboard creation should apply");

    let dashboard = first_child(&engine, engine.root);
    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    engine
        .queue_catalog_create(
            page,
            DASHBOARD_GENERIC_WIDGET_NODE_TYPE,
            Some("Widget".to_string()),
            None,
        )
        .expect("generic widget creation should queue");
    for _ in 0..3 {
        engine.apply_edits().expect("widget creation should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE)
        .expect("page should contain a generic widget child");
    let page_layout =
        find_descendant_by_decl(&engine, page, "layout_kind").expect("page layout parameter should exist");

    assert!(
        find_descendant_by_decl(&engine, widget, "position").is_some(),
        "free-layout widgets should expose position"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "column_span").is_none(),
        "free-layout widgets should hide grid-only spans"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "row_span").is_none(),
        "free-layout widgets should hide grid-only spans"
    );

    engine.edits.push(Edit::SetParam {
        node: page_layout,
        value: ParamValue::Enum("grid".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..3 {
        engine.apply_edits().expect("switching page layout should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert!(
        find_descendant_by_decl(&engine, widget, "position").is_none(),
        "grid-layout widgets should hide free-layout position"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "column_span").is_some(),
        "grid-layout widgets should expose column span"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "row_span").is_some(),
        "grid-layout widgets should expose row span"
    );

    engine.edits.push(Edit::SetParam {
        node: page_layout,
        value: ParamValue::Enum("free".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..3 {
        engine.apply_edits().expect("switching page layout back should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert_eq!(
        direct_child_decl_ids(&engine, widget),
        vec!["binding", "appearance", "layout", "content"],
        "reintroduced free-layout parameters should return to declared order",
    );
}

#[test]
fn dashboard_generic_widget_kind_dependencies_follow_widget_kind() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("dashboard creation should apply");

    let dashboard = first_child(&engine, engine.root);
    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    engine
        .queue_catalog_create(
            page,
            DASHBOARD_GENERIC_WIDGET_NODE_TYPE,
            Some("Widget".to_string()),
            None,
        )
        .expect("generic widget creation should queue");
    for _ in 0..3 {
        engine.apply_edits().expect("widget creation should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE)
        .expect("page should contain a generic widget child");
    let widget_kind =
        find_descendant_by_decl(&engine, widget, "widget_kind").expect("widget kind parameter should exist");

    assert!(
        find_descendant_by_decl(&engine, widget, "placeholder").is_none(),
        "text widgets should hide text-input placeholder"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "multiline").is_none(),
        "text widgets should hide multiline"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "value_range").is_none(),
        "text widgets should hide slider range"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "step").is_none(),
        "text widgets should hide slider step"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "default_checked").is_none(),
        "text widgets should hide checkbox defaults"
    );

    engine.edits.push(Edit::SetParam {
        node: widget_kind,
        value: ParamValue::Enum("textInput".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..3 {
        engine
            .apply_edits()
            .expect("switching widget kind to text input should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert!(
        find_descendant_by_decl(&engine, widget, "placeholder").is_some(),
        "text-input widgets should expose placeholder"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "multiline").is_some(),
        "text-input widgets should expose multiline"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "value_range").is_none(),
        "text-input widgets should hide slider range"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "step").is_none(),
        "text-input widgets should hide slider step"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "default_checked").is_none(),
        "text-input widgets should hide checkbox defaults"
    );

    engine.edits.push(Edit::SetParam {
        node: widget_kind,
        value: ParamValue::Enum("slider".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..3 {
        engine
            .apply_edits()
            .expect("switching widget kind to slider should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert!(
        find_descendant_by_decl(&engine, widget, "placeholder").is_none(),
        "slider widgets should hide text-input placeholder"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "multiline").is_none(),
        "slider widgets should hide multiline"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "value_range").is_some(),
        "slider widgets should expose range"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "step").is_some(),
        "slider widgets should expose step"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "default_checked").is_none(),
        "slider widgets should hide checkbox defaults"
    );

    engine.edits.push(Edit::SetParam {
        node: widget_kind,
        value: ParamValue::Enum("checkbox".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..3 {
        engine
            .apply_edits()
            .expect("switching widget kind to checkbox should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert!(
        find_descendant_by_decl(&engine, widget, "default_checked").is_some(),
        "checkbox widgets should expose default checked"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "placeholder").is_none(),
        "checkbox widgets should hide text-input placeholder"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "value_range").is_none(),
        "checkbox widgets should hide slider range"
    );
    assert!(
        find_descendant_by_decl(&engine, widget, "step").is_none(),
        "checkbox widgets should hide slider step"
    );
}

#[test]
fn dashboard_widgets_expose_disableable_label_placement() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("dashboard creation should apply");

    let dashboard = first_child(&engine, engine.root);
    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    engine
        .queue_catalog_create(
            page,
            DASHBOARD_GENERIC_WIDGET_NODE_TYPE,
            Some("Widget".to_string()),
            None,
        )
        .expect("generic widget creation should queue");
    for _ in 0..3 {
        engine.apply_edits().expect("widget creation should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE)
        .expect("page should contain a generic widget child");
    let label_placement = find_descendant_by_decl(&engine, widget, "label_placement")
        .expect("label placement parameter should exist");
    let label_placement_node = engine
        .nodes
        .get(label_placement)
        .expect("label placement node should exist");

    assert_eq!(
        param_snapshot(&engine, label_placement).value,
        ParamValue::Enum("top".to_string())
    );
    assert!(
        label_placement_node.node_data().meta.enabled,
        "label placement should start enabled"
    );
    assert!(
        label_placement_node.node_data().meta.can_be_disabled,
        "label placement should be disableable so hidden remains available"
    );
}

#[test]
fn dashboard_node_widget_inspector_options_follow_target_disable_capabilities() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    let mut target = Parameter::new(
        "Toggleable",
        ParamValue::Str("Value".to_string()),
        ParameterChangeCheck::ValueChange,
    );
    target.node_data_mut().meta.can_be_disabled = true;
    engine.add_node(target.into(), None);
    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("initial tree should apply");

    let dashboard =
        direct_child_by_type(&engine, engine.root, DASHBOARD_NODE_TYPE).expect("dashboard should exist");
    let target_node = {
        let mut child = engine
            .nodes
            .get(engine.root)
            .and_then(|node| node.node_data().first_child);
        let mut found = None;
        while let Some(child_id) = child {
            let child_node = engine.nodes.get(child_id).expect("child should exist");
            if child_node.get_type() != DASHBOARD_NODE_TYPE {
                found = Some(child_id);
                break;
            }
            child = child_node.node_data().next_sibling;
        }
        found.expect("disableable target parameter should exist")
    };
    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    engine
        .queue_catalog_create(
            page,
            DASHBOARD_NODE_WIDGET_NODE_TYPE,
            Some("Inspector".to_string()),
            None,
        )
        .expect("node widget creation should queue");
    for _ in 0..3 {
        engine.apply_edits().expect("widget creation should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let widget = direct_child_by_type(&engine, page, DASHBOARD_NODE_WIDGET_NODE_TYPE)
        .expect("page should contain a node widget child");
    let widget_target =
        find_descendant_by_decl(&engine, widget, "target_node").expect("target node parameter should exist");
    let widget_type =
        find_descendant_by_decl(&engine, widget, "widget_type").expect("widget type parameter should exist");
    let target_uuid = engine
        .nodes
        .get(target_node)
        .expect("target node should exist")
        .node_data()
        .meta
        .uuid;

    engine.edits.push(Edit::SetParam {
        node: widget_target,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..4 {
        engine.apply_edits().expect("binding inspector target should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let show_enable_button = find_descendant_by_decl(&engine, widget, "show_enable_button").expect(
        "disableable parameter widgets should expose the enable-button toggle in their default widget options",
    );
    assert_eq!(
        param_snapshot(&engine, show_enable_button).value,
        ParamValue::Bool(true),
        "disableable parameter widgets should show the enable button by default"
    );

    engine.edits.push(Edit::SetParam {
        node: widget_type,
        value: ParamValue::Enum("inspector".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("switching widget to inspector should apply");
        engine
            .dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert_eq!(
        count_descendants_by_decl(&engine, widget, "show_enable_button"),
        1,
        "inspector widgets should keep a single enable-button option after replacing parameter widget options"
    );
    let show_enable_button = find_descendant_by_decl(&engine, widget, "show_enable_button")
        .expect("disableable inspector targets should expose the enable-button toggle");
    assert_eq!(
        param_snapshot(&engine, show_enable_button).value,
        ParamValue::Bool(true),
        "disableable inspector targets should show the enable button by default"
    );
}

#[test]
fn ui_create_user_item_initial_params_materialize_dashboard_widget_without_follow_up_edits() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("Tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("initial tree should apply");

    let target_param = direct_child_by_type(&engine, engine.root, "float").expect("target parameter should exist");
    let dashboard =
        direct_child_by_type(&engine, engine.root, DASHBOARD_NODE_TYPE).expect("dashboard should exist");
    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    let target_uuid = engine
        .nodes
        .get(target_param)
        .expect("target parameter should exist")
        .node_data()
        .meta
        .uuid;
    let create_ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: page,
        node_type: DASHBOARD_GENERIC_WIDGET_NODE_TYPE.to_string(),
        label: Some("Tempo Widget".to_string()),
        initial_params: vec![
            UiCreateUserItemInitialParam {
                decl_id: DeclId("binding/target_param".to_string()),
                value: ParamValue::Reference(NodeReference::new(target_uuid)),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("content/widget_kind".to_string()),
                value: ParamValue::Enum("textInput".to_string()),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("content/placeholder".to_string()),
                value: ParamValue::Str("Tempo".to_string()),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("content/multiline".to_string()),
                value: ParamValue::Bool(true),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("layout/anchor".to_string()),
                value: ParamValue::Enum("center".to_string()),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("layout/position".to_string()),
                value: ParamValue::Vec2(240.0, 160.0),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("layout/width".to_string()),
                value: ParamValue::CssValue(CssValue::new(18.0, CssUnit::Rem)),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("layout/height".to_string()),
                value: ParamValue::CssValue(CssValue::new(6.0, CssUnit::Rem)),
            },
        ],
    });
    assert!(create_ack.success, "configured widget creation should succeed");

    let widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE)
        .expect("page should contain the created generic widget");
    let widget_kind =
        find_descendant_by_decl(&engine, widget, "widget_kind").expect("widget kind parameter should exist");
    let placeholder =
        find_descendant_by_decl(&engine, widget, "placeholder").expect("placeholder parameter should exist");
    let multiline =
        find_descendant_by_decl(&engine, widget, "multiline").expect("multiline parameter should exist");
    let anchor = find_descendant_by_decl(&engine, widget, "anchor").expect("anchor parameter should exist");
    let position = find_descendant_by_decl(&engine, widget, "position").expect("position parameter should exist");
    let width = find_descendant_by_decl(&engine, widget, "width").expect("width parameter should exist");
    let height = find_descendant_by_decl(&engine, widget, "height").expect("height parameter should exist");

    assert_eq!(
        param_snapshot(&engine, widget_kind).value,
        ParamValue::Enum("textInput".to_string())
    );
    assert_eq!(
        param_snapshot(&engine, placeholder).value,
        ParamValue::Str("Tempo".to_string())
    );
    assert_eq!(param_snapshot(&engine, multiline).value, ParamValue::Bool(true));
    assert_eq!(
        param_snapshot(&engine, anchor).value,
        ParamValue::Enum("center".to_string())
    );
    assert_eq!(param_snapshot(&engine, position).value, ParamValue::Vec2(240.0, 160.0));
    assert_eq!(
        param_snapshot(&engine, width).value,
        ParamValue::CssValue(CssValue::new(18.0, CssUnit::Rem))
    );
    assert_eq!(
        param_snapshot(&engine, height).value,
        ParamValue::CssValue(CssValue::new(6.0, CssUnit::Rem))
    );
}

#[test]
fn dashboard_node_widget_initial_target_reference_without_cached_id_stabilizes() {
    let root: DashboardTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);

    let mut speed = Parameter::new("Speed", ParamValue::Float(0.5), ParameterChangeCheck::ValueChange);
    speed.constraints.range = RangeConstraint::uniform(Some(0.0), Some(1.0));
    engine.add_node(speed.into(), None);
    engine.add_node(DashboardNode::new().into(), None);
    engine.apply_edits().expect("initial tree should apply");

    let target_param = direct_child_by_type(&engine, engine.root, "float").expect("target parameter should exist");
    let dashboard =
        direct_child_by_type(&engine, engine.root, DASHBOARD_NODE_TYPE).expect("dashboard should exist");
    engine
        .queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None)
        .expect("page creation should queue");
    engine.apply_edits().expect("page creation should apply");

    let page = first_child(&engine, dashboard);
    let target_uuid = engine
        .nodes
        .get(target_param)
        .expect("target parameter should exist")
        .node_data()
        .meta
        .uuid;
    engine
        .queue_catalog_create(
            page,
            DASHBOARD_NODE_WIDGET_NODE_TYPE,
            Some("Speed Widget".to_string()),
            None,
        )
        .expect("node widget creation should queue");
    engine.apply_edits().expect("node widget creation should apply");

    let widget = direct_child_by_type(&engine, page, DASHBOARD_NODE_WIDGET_NODE_TYPE)
        .expect("page should contain the created node widget");
    let target_node =
        find_descendant_by_decl(&engine, widget, "target_node").expect("target node parameter should exist");
    engine.edits.push(Edit::SetParam {
        node: target_node,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("target binding should apply");
    engine.resolve().expect("resolve should succeed");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("runtime tick should stabilize");

    let widget_type =
        find_descendant_by_decl(&engine, widget, "widget_type").expect("widget type parameter should exist");
    let widget_type_snapshot = param_snapshot(&engine, widget_type);
    let widget_types = widget_type_snapshot
        .constraints
        .enum_options
        .iter()
        .map(|option| option.variant_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        widget_types,
        vec!["default", "inspector", "slider", "rotary"],
        "bound numeric widgets should converge to their widget types even when the target reference cached id is initially empty"
    );
    assert_eq!(
        widget_type_snapshot.value,
        ParamValue::Enum("default".to_string()),
        "bound numeric widgets should resolve to the default widget type"
    );
}
