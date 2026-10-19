use super::*;
use crate::edit::Edit;
use crate::node::{EventSubscription, Folder, Node, NodeCreationContext, NodeId};
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck};
use crate::process_ctx::ProcessCtx;
use crate::ui_sync::UiEditIntent;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[crate::node("history_failure_probe")]
struct HistoryFailureProbe {
    #[state(default = 0, persist)]
    authored_changes: i32,
    #[state(default = 0)]
    runtime_calls: usize,
}

#[crate::node("history_failure_probe", from_struct)]
impl Node for HistoryFailureProbe {
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, _param: NodeId, _old: ParamValue) {
        self.authored_changes += 1;
        self.runtime_calls += 1;
        ctx.patch_node_meta(
            self.id(),
            crate::node::NodeMetaPatch {
                label: Some(format!("Change {}", self.authored_changes)),
                ..Default::default()
            },
        );
    }

    fn on_node_ready(&mut self, _ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.authored_changes += 1;
        self.runtime_calls += 1;
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.authored_changes += 10;
        self.runtime_calls += 1;
    }
}

#[crate::node("history_ready_failure_probe")]
struct HistoryReadyFailureProbe {
    reject: Arc<AtomicBool>,
}

#[crate::node("history_ready_failure_probe", from_struct)]
impl Node for HistoryReadyFailureProbe {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        if context == NodeCreationContext::HistoryRestore && self.reject.load(Ordering::Relaxed) {
            ctx.add_event_listener(NodeId(u64::MAX), self.id());
        }
    }
}

crate::define_node_enum!(
    enum HistoryFailureNode {
        HistoryFailureProbe,
        HistoryReadyFailureProbe,
    }
);

fn set_param(engine: &mut Engine<HistoryFailureNode>, node: NodeId, value: i32) {
    engine.edits.push(Edit::SetParam {
        node,
        value: ParamValue::Int(value),
        behaviour: Default::default(),
    });
}

fn two_params() -> (Engine<HistoryFailureNode>, NodeId, NodeId) {
    let mut engine = Engine::new(HistoryFailureNode::from(Folder::new("root")));
    for label in ["first", "second"] {
        engine.add_node(
            Parameter::new(label, ParamValue::Int(0), ParameterChangeCheck::ValueChange).into(),
            None,
        );
    }
    engine.apply_edits().unwrap();
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    engine.clear_history();
    let first = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let second = engine.nodes.get(first).unwrap().node_data().next_sibling.unwrap();
    (engine, first, second)
}

#[test]
fn rejected_undo_and_redo_restore_live_values_and_keep_the_transaction() {
    for undo in [true, false] {
        let (mut engine, first, second) = two_params();
        set_param(&mut engine, first, 1);
        set_param(&mut engine, second, 2);
        engine.apply_edits().unwrap();
        if !undo {
            engine.undo().unwrap();
        }
        // Runtime writes are deliberately outside authored history and can make a replay stale.
        let (missing, survivor) = if undo { (first, second) } else { (second, first) };
        set_param(&mut engine, survivor, 37);
        engine.edits.push(Edit::RemoveNode { node: missing });
        engine.apply_edits_without_history().unwrap();
        let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        let history = engine.ui_history_state();
        let events = engine.ui_event_log().to_vec();
        let dirty = engine.project_dirty_nodes.clone();
        let version = engine.ui_graph_version;
        let result = if undo { engine.undo() } else { engine.redo() };
        assert!(result.is_err());
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            before
        );
        assert_eq!(engine.ui_history_state(), history);
        assert_eq!(engine.ui_event_log(), events);
        assert_eq!(engine.project_dirty_nodes, dirty);
        assert_eq!(engine.ui_graph_version, version);
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        set_param(&mut engine, survivor, 38);
        engine.apply_edits().unwrap();
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    }
}

#[test]
fn missing_restore_parent_keeps_detached_payload_owned_by_history() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.add_node(Folder::new("parent"), None);
    engine.apply_edits().unwrap();
    let parent = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.add_node(Folder::new("child"), Some(parent));
    engine.apply_edits().unwrap();
    let child = engine.nodes.get(parent).unwrap().node_data().first_child.unwrap();
    engine.clear_history();
    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().unwrap();
    // Keep the unrelated runtime removal payload so the missing parent can recover.
    let mut runtime_removal: super::super::history::HistoryStep<_> =
        engine.apply_remove_node(0, parent, None).unwrap().into();
    let history = engine.ui_history_state();
    assert!(engine.undo().is_err());
    assert!(
        !engine.nodes.contains(child),
        "rejected restoration must not leave an orphan"
    );
    assert_eq!(engine.ui_history_state(), history);
    assert!(
        engine.undo().is_err(),
        "retained payload must still reject the missing parent"
    );
    runtime_removal.undo_for_rollback(&mut engine).unwrap();
    assert!(engine.undo().unwrap());
    assert!(engine.nodes.contains(child), "retry uses the original retained payload");
    engine.clear_history();
    assert_eq!(engine.nodes.len(), 3);
}

#[test]
fn failed_batched_ready_restores_history_payloads_and_allows_retry() {
    let mut engine = Engine::new(HistoryFailureNode::from(Folder::new("root")));
    let reject = Arc::new(AtomicBool::new(false));
    for _ in 0..2 {
        engine.add_node(HistoryReadyFailureProbe::new(reject.clone()).into(), None);
    }
    engine.apply_edits().unwrap();
    let full = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.undo().unwrap();
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    let history = engine.ui_history_state();
    let events = engine.ui_event_log().to_vec();
    reject.store(true, Ordering::Relaxed);
    for _ in 0..2 {
        assert!(engine.redo().is_err());
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            before
        );
        assert_eq!(engine.ui_history_state(), history);
        assert_eq!(engine.ui_event_log(), events);
    }
    reject.store(false, Ordering::Relaxed);
    assert!(engine.redo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        full
    );
}

#[test]
fn history_notifications_preserve_authored_callback_state_but_activate_runtime() {
    let mut engine = Engine::new(HistoryFailureNode::from(HistoryFailureProbe::new()));
    let root = engine.root;
    engine.add_node(
        Parameter::new("Value", ParamValue::Int(0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.edits.push(Edit::AddEventListener {
        subscriber: root,
        subscription: EventSubscription::subtree(root, u32::MAX),
    });
    engine.apply_edits().unwrap();
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    engine.clear_history();
    let param = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: param,
                value: ParamValue::Int(1),
                behaviour: Default::default(),
            })
            .success
    );
    let after = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    assert_ne!(after, before);
    for _ in 0..2 {
        engine.undo().unwrap();
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            before
        );
        engine.redo().unwrap();
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            after
        );
    }
    let HistoryFailureNode::HistoryFailureProbe(probe) = engine.nodes.get(root).unwrap() else {
        panic!("probe")
    };
    assert!(probe.runtime_calls >= 5);
}

#[test]
fn ready_history_activation_preserves_live_authored_payload() {
    let mut engine = Engine::new(HistoryFailureNode::from(Folder::new("root")));
    engine.add_node(HistoryFailureProbe::new().into(), None);
    engine.apply_edits().unwrap();
    let child = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.undo().unwrap();
    engine.redo().unwrap();
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    let HistoryFailureNode::HistoryFailureProbe(probe) = engine.nodes.get(child).unwrap() else {
        panic!("probe")
    };
    assert_eq!(probe.runtime_calls, 3);
}

#[test]
fn removal_teardown_preserves_authored_payload_for_undo_and_rejection() {
    for reject in [false, true] {
        let mut engine = Engine::new(HistoryFailureNode::from(Folder::new("root")));
        engine.add_node(HistoryFailureProbe::new().into(), None);
        engine.apply_edits().unwrap();
        let child = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
        engine.clear_history();
        let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        if reject {
            let result: Result<(), EngineEditError> = engine.apply_atomic_edit_operation(|engine| {
                engine.edits.push(Edit::RemoveNode { node: child });
                set_param(engine, engine.root, 1);
                engine.apply_edits()
            });
            assert!(result.is_err());
            assert_eq!(engine.undo_len(), 0);
        } else {
            engine.edits.push(Edit::RemoveNode { node: child });
            engine.apply_edits().unwrap();
            engine.undo().unwrap();
        }
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            before
        );
        let HistoryFailureNode::HistoryFailureProbe(probe) = engine.nodes.get(child).unwrap() else {
            panic!("probe")
        };
        assert_eq!(probe.runtime_calls, 3);
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        assert_eq!(
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
            before
        );
    }
}

#[test]
fn failed_undo_restores_runtime_moved_and_replaced_payloads() {
    for replace in [false, true] {
        let (mut engine, missing, _) = two_params();
        for label in ["old parent", "authored parent", "runtime parent"] {
            engine.add_node(Folder::new(label).into(), None);
        }
        engine.apply_edits().unwrap();
        let parents = engine.ui_direct_children(engine.root).unwrap();
        let (old_parent, authored_parent, runtime_parent) = (parents[2], parents[3], parents[4]);
        engine.add_node(Folder::new("child").into(), Some(old_parent));
        engine.apply_edits().unwrap();
        let child = engine.nodes.get(old_parent).unwrap().node_data().first_child.unwrap();
        engine.clear_history();
        set_param(&mut engine, missing, 1);
        if replace {
            engine.edits.push(Edit::ReplaceNode {
                node: child,
                new_node: Box::new(Folder::new("replacement")),
            });
        } else {
            engine.edits.push(Edit::MoveNode {
                node: child,
                new_parent: authored_parent,
                new_prev_sibling: None,
            });
        }
        engine.apply_edits().unwrap();
        engine.edits.push(Edit::MoveNode {
            node: child,
            new_parent: runtime_parent,
            new_prev_sibling: None,
        });
        engine.edits.push(Edit::RemoveNode { node: missing });
        engine.apply_edits_without_history().unwrap();
        let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        let history = engine.ui_history_state();
        for _ in 0..2 {
            assert!(engine.undo().is_err());
            assert_eq!(
                engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
                before
            );
            assert_eq!(engine.ui_history_state(), history);
            assert_eq!(
                engine.nodes.get(child).unwrap().node_data().parent,
                Some(runtime_parent)
            );
        }
    }
}
