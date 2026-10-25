use crate::engine::Engine;
use crate::node::{DASHBOARD_PAGE_NODE_TYPE, DashboardNode, Node, NodeId};
use crate::parameter::{CssUnit, CssValue, ParamValue, ParameterEventBehaviour};
use crate::ui_sync::{UiDashboardWidgetPlacement, UiDashboardWidgetSizeEnabled, UiEditIntent};

crate::define_node_enum!(
    enum TestNode {}
);

fn fixture() -> (Engine<TestNode>, NodeId) {
    let mut engine: Engine<TestNode> = Engine::new(DashboardNode::new().into());
    let root = engine.root;
    engine
        .queue_catalog_create(root, DASHBOARD_PAGE_NODE_TYPE, Some("Page".into()), None)
        .unwrap();
    engine.apply_edits().unwrap();
    let page = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    (engine, page)
}

fn container(engine: &mut Engine<TestNode>, parent: NodeId, name: &str, layout: &str) -> NodeId {
    let ack = engine.apply_ui_intent(UiEditIntent::CreateDashboardContainerWidget {
        parent,
        label: Some(name.into()),
        placement: None,
        layout_kind: Some(layout.into()),
        prev_sibling: None,
    });
    assert!(ack.success, "{ack:?}");
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == name)
        .unwrap()
        .0
}

fn set_layout(engine: &mut Engine<TestNode>, owner: NodeId, layout: &str) {
    let node = engine.ui_dashboard_layout_field(owner, "layout_kind").unwrap();
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node,
                value: ParamValue::Enum(layout.into()),
                behaviour: ParameterEventBehaviour::Coalesce
            })
            .success
    );
}

fn placement() -> UiDashboardWidgetPlacement {
    UiDashboardWidgetPlacement {
        anchor: "center".into(),
        position: (135.0, -22.0),
        width: CssValue::new(25.0, CssUnit::Percent),
        height: CssValue::new(7.0, CssUnit::Em),
        size_enabled: Some(UiDashboardWidgetSizeEnabled {
            width: true,
            height: false,
        }),
    }
}

fn authored(engine: &Engine<TestNode>) -> String {
    engine.to_project_json_with(|node| node.project_encode_data()).unwrap()
}

fn reject_without_changes(engine: &mut Engine<TestNode>, intent: UiEditIntent) {
    let before = authored(engine);
    let history = engine.ui_history_state();
    let events = engine.ui_event_log().to_vec();
    let ack = engine.apply_ui_intent(intent);
    assert!(!ack.success, "{ack:?}");
    assert_eq!(authored(engine), before);
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(engine.ui_event_log(), events);
}

#[test]
fn resize_enables_and_sets_only_the_requested_axis_in_one_history_entry() {
    for (layout, axis) in [("horizontal", "width"), ("vertical", "height"), ("free", "width")] {
        let (mut engine, page) = fixture();
        let widget = container(&mut engine, page, "Widget", "free");
        set_layout(&mut engine, page, layout);
        let field = engine.ui_dashboard_layout_field(widget, axis).unwrap();
        let other = engine.ui_dashboard_layout_field(widget, if axis == "width" { "height" } else { "width" });
        let untouched = other.and_then(|id| engine.nodes.get(id).unwrap().engine_param_snapshot());
        engine.clear_history();
        let before = authored(&engine);
        let value = CssValue::new(34.0, CssUnit::Percent);
        let ack = engine.apply_ui_intent(UiEditIntent::ResizeDashboardWidget {
            widget,
            width: (axis == "width").then_some(value),
            height: (axis == "height").then_some(value),
        });
        assert!(ack.success, "{ack:?}");
        assert!(engine.nodes.get(field).unwrap().node_data().meta.enabled);
        assert_eq!(
            engine.nodes.get(field).unwrap().engine_param_snapshot().unwrap().value,
            ParamValue::CssValue(value)
        );
        assert_eq!(
            other.and_then(|id| engine.nodes.get(id).and_then(Node::engine_param_snapshot)),
            untouched
        );
        let after = authored(&engine);
        assert_eq!(engine.undo_len(), 1);
        assert!(engine.undo().unwrap());
        assert_eq!(authored(&engine), before);
        assert!(engine.redo().unwrap());
        assert_eq!(authored(&engine), after);
    }
}

#[test]
fn resize_rejects_wrong_axis_non_widgets_readonly_and_invalid_values_atomically() {
    let (mut engine, page) = fixture();
    let widget = container(&mut engine, page, "Widget", "free");
    set_layout(&mut engine, page, "horizontal");
    for (target, width, height) in [
        (widget, None, None),
        (widget, None, Some(CssValue::new(1.0, CssUnit::Rem))),
        (widget, Some(CssValue::new(-1.0, CssUnit::Rem)), None),
        (widget, Some(CssValue::new(f64::NAN, CssUnit::Rem)), None),
        (page, Some(CssValue::new(1.0, CssUnit::Rem)), None),
    ] {
        reject_without_changes(
            &mut engine,
            UiEditIntent::ResizeDashboardWidget {
                widget: target,
                width,
                height,
            },
        );
    }
    let field = engine.ui_dashboard_layout_field(widget, "width").unwrap();
    // Read-only is a backend parameter invariant, independent of a widget's presentation flags.
    engine
        .nodes
        .get_mut(field)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<crate::parameter::Parameter>()
        .unwrap()
        .read_only = true;
    reject_without_changes(
        &mut engine,
        UiEditIntent::ResizeDashboardWidget {
            widget,
            width: Some(CssValue::new(4.0, CssUnit::Rem)),
            height: None,
        },
    );
}

#[test]
fn move_and_placement_follow_destination_policy_and_undo_as_one_transaction() {
    for source_layout in ["free", "horizontal", "vertical"] {
        for layout in ["free", "horizontal", "vertical", "grid", "tabs", "accordion"] {
            let (mut engine, page) = fixture();
            set_layout(&mut engine, page, source_layout);
            let widget = container(&mut engine, page, "Widget", "free");
            let destination = container(&mut engine, page, "Destination", layout);
            engine.clear_history();
            let before = authored(&engine);
            let hint = placement();
            let ack = engine.apply_ui_intent(UiEditIntent::MoveDashboardWidget {
                widget,
                parent: destination,
                target_index: Some(0),
                placement: Some(hint.clone()),
            });
            assert!(ack.success, "{layout}: {ack:?}");
            assert_eq!(engine.nodes.get(widget).unwrap().node_data().parent, Some(destination));
            for (axis, value, expected) in [
                ("width", hint.width, matches!(layout, "free" | "horizontal")),
                ("height", hint.height, matches!(layout, "free" | "vertical")),
            ] {
                let id = engine.ui_dashboard_layout_field(widget, axis);
                if !expected {
                    assert!(id.is_none(), "{layout}: unsupported {axis} must stay absent");
                    continue;
                }
                let id = id.unwrap();
                let parameter = engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap();
                if expected {
                    assert_eq!(parameter.value, ParamValue::CssValue(value));
                }
                if layout == "free" {
                    assert!(engine.nodes.get(id).unwrap().node_data().meta.enabled);
                }
                if layout == "horizontal" && axis == "width" {
                    assert!(engine.nodes.get(id).unwrap().node_data().meta.enabled);
                }
                if layout == "vertical" && axis == "height" {
                    assert!(!engine.nodes.get(id).unwrap().node_data().meta.enabled);
                }
            }
            if layout == "free" {
                let id = engine.ui_dashboard_layout_field(widget, "position").unwrap();
                assert_eq!(
                    engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value,
                    ParamValue::Vec2(135.0, -22.0)
                );
            }
            let after = authored(&engine);
            assert_eq!(engine.undo_len(), 1);
            assert!(engine.undo().unwrap());
            assert_eq!(authored(&engine), before);
            assert!(engine.redo().unwrap());
            assert_eq!(authored(&engine), after);
        }
    }
}

#[test]
fn resize_rejects_the_whole_two_axis_request_when_the_second_field_is_readonly() {
    let (mut engine, page) = fixture();
    let widget = container(&mut engine, page, "Widget", "free");
    let height = engine.ui_dashboard_layout_field(widget, "height").unwrap();
    engine
        .nodes
        .get_mut(height)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<crate::parameter::Parameter>()
        .unwrap()
        .read_only = true;
    reject_without_changes(
        &mut engine,
        UiEditIntent::ResizeDashboardWidget {
            widget,
            width: Some(CssValue::new(24.0, CssUnit::Rem)),
            height: Some(CssValue::new(10.0, CssUnit::Rem)),
        },
    );
}

#[test]
fn repeated_resize_samples_preserve_the_callers_group_and_one_step_undo() {
    let (mut engine, page) = fixture();
    let widget = container(&mut engine, page, "Widget", "free");
    set_layout(&mut engine, page, "horizontal");
    engine.clear_history();
    let before = authored(&engine);
    let client_edit_id = "resize-gesture".to_string();
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::BeginEdit {
                client_edit_id: client_edit_id.clone(),
                label: Some("Resize".into())
            })
            .success
    );
    for value in [20.0, 30.0] {
        assert!(
            engine
                .apply_ui_intent(UiEditIntent::ResizeDashboardWidget {
                    widget,
                    width: Some(CssValue::new(value, CssUnit::Percent)),
                    height: None
                })
                .success
        );
        assert!(engine.has_active_edit_session());
    }
    assert!(engine.apply_ui_intent(UiEditIntent::EndEdit { client_edit_id }).success);
    let after = authored(&engine);
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(authored(&engine), before);
    assert!(engine.redo().unwrap());
    assert_eq!(authored(&engine), after);
}

#[test]
fn invalid_move_placement_destination_and_cycle_leave_no_partial_move() {
    let (mut engine, page) = fixture();
    let widget = container(&mut engine, page, "Widget", "free");
    let destination = container(&mut engine, page, "Destination", "free");
    let child = container(&mut engine, widget, "Child", "free");
    let mut invalid = placement();
    invalid.anchor = "invalid-anchor".into();
    for (parent, placement) in [
        (destination, Some(invalid)),
        (widget, None),
        (child, None),
        (engine.root, None),
        (NodeId(u64::MAX), None),
    ] {
        reject_without_changes(
            &mut engine,
            UiEditIntent::MoveDashboardWidget {
                widget,
                parent,
                target_index: None,
                placement,
            },
        );
    }
}

#[test]
fn move_indexes_exclude_structural_children_and_clamp_to_the_end() {
    let (mut engine, page) = fixture();
    let a = container(&mut engine, page, "A", "free");
    let b = container(&mut engine, page, "B", "free");
    let c = container(&mut engine, page, "C", "free");
    for (target_index, expected) in [(0, vec![b, a, c]), (usize::MAX, vec![a, c, b]), (1, vec![a, b, c])] {
        let ack = engine.apply_ui_intent(UiEditIntent::MoveDashboardWidget {
            widget: b,
            parent: page,
            target_index: Some(target_index),
            placement: None,
        });
        assert!(ack.success, "{ack:?}");
        let snapshot = engine.process_tree_snapshot();
        let items: Vec<_> = snapshot
            .child_ids(page)
            .into_iter()
            .filter(|id| [a, b, c].contains(id))
            .collect();
        assert_eq!(items, expected);
        assert!(engine.ui_dashboard_layout_field(page, "layout_kind").is_some());
    }
}
