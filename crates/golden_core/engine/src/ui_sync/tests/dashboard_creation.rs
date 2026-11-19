use super::dashboard_layout::{fixture, placement, set_layout};
use crate::node::{
    DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_NODE_WIDGET_NODE_TYPE, DASHBOARD_WIDGET_CONTAINER_NODE_TYPE, Node,
};
use crate::parameter::ParamValue;
use crate::ui_sync::UiEditIntent;

#[test]
fn dashboard_drop_initializes_placement_for_all_widget_kinds() {
    for layout in ["free", "horizontal", "vertical", "grid", "accordion", "tabs"] {
        for widget_type in [
            DASHBOARD_NODE_WIDGET_NODE_TYPE,
            DASHBOARD_GENERIC_WIDGET_NODE_TYPE,
            DASHBOARD_WIDGET_CONTAINER_NODE_TYPE,
        ] {
            let (mut engine, page) = fixture();
            set_layout(&mut engine, page, layout);
            let hint = placement();
            let intent = match widget_type {
                DASHBOARD_GENERIC_WIDGET_NODE_TYPE => UiEditIntent::CreateDashboardGenericWidget {
                    parent: page,
                    target: engine.ui_dashboard_layout_field(page, "layout_kind").unwrap(),
                    placement: Some(hint.clone()),
                    prev_sibling: None,
                },
                DASHBOARD_NODE_WIDGET_NODE_TYPE => UiEditIntent::CreateDashboardNodeWidget {
                    parent: page,
                    target: page,
                    placement: Some(hint.clone()),
                    prev_sibling: None,
                },
                _ => UiEditIntent::CreateDashboardContainerWidget {
                    parent: page,
                    label: Some("Dropped container".into()),
                    placement: Some(hint.clone()),
                    layout_kind: Some("grid".into()),
                    prev_sibling: None,
                },
            };
            let before = engine.to_project_json_with(Node::project_encode_data).unwrap();
            engine.clear_history();
            let ack = engine.apply_ui_intent(intent);
            assert!(ack.success, "layout={layout}, widget_type={widget_type}: {ack:?}");
            let widget = engine
                .nodes
                .iter()
                .find(|(_, node)| node.get_type() == widget_type)
                .unwrap()
                .0;
            for (axis, value, present, enabled) in [
                ("width", hint.width, matches!(layout, "free" | "horizontal"), true),
                (
                    "height",
                    hint.height,
                    matches!(layout, "free" | "vertical"),
                    layout == "free",
                ),
            ] {
                let field = engine.ui_dashboard_layout_field(widget, axis);
                assert_eq!(field.is_some(), present, "{layout}/{axis}");
                if let Some(field) = field {
                    let node = engine.nodes.get(field).unwrap();
                    assert_eq!(node.engine_param_snapshot().unwrap().value, ParamValue::CssValue(value));
                    assert_eq!(node.node_data().meta.enabled, enabled);
                }
            }
            if layout == "free" {
                for (field, value) in [
                    ("anchor", ParamValue::Enum(hint.anchor.clone())),
                    ("position", ParamValue::Vec2(hint.position.0, hint.position.1)),
                ] {
                    let field = engine.ui_dashboard_layout_field(widget, field).unwrap();
                    assert_eq!(
                        engine.nodes.get(field).unwrap().engine_param_snapshot().unwrap().value,
                        value
                    );
                }
            }
            let after = engine.to_project_json_with(Node::project_encode_data).unwrap();
            assert_eq!(engine.undo_len(), 1);
            assert!(engine.undo().unwrap());
            assert_eq!(engine.to_project_json_with(Node::project_encode_data).unwrap(), before);
            assert!(engine.redo().unwrap());
            assert_eq!(engine.to_project_json_with(Node::project_encode_data).unwrap(), after);
        }
    }
}
