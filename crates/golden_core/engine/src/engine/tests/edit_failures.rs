use super::*;
use crate::{
    contexts::{UserContextLookup, UserContextValueType},
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeCreationContext, NodeId, NodeReference, UserContextNode},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
};

#[crate::node("failing_ready_probe")]
struct FailingReadyProbe {}

#[crate::node("failing_ready_probe", from_struct)]
impl Node for FailingReadyProbe {
    fn on_node_ready(&mut self, ctx: &mut crate::process_ctx::ProcessCtx, _context: NodeCreationContext) {
        ctx.set_param(NodeId(u64::MAX), ParamValue::Int(1));
    }
}

crate::define_node_enum!(
    enum FailureTestNode {
        FailingReadyProbe,
    }
);

#[test]
fn committed_insertion_retains_history_when_ready_callback_fails() {
    for use_tree in [false, true] {
        let mut engine = Engine::new(FailureTestNode::from(Folder::new("root")));
        engine.add_node(Folder::new("prefix").into(), None);
        if use_tree {
            engine.edits.push(Edit::AddNodeTree {
                tree: NodeTree::new(FailingReadyProbe::new()),
                parent: engine.root,
                prev_sibling: None,
            });
        } else {
            engine.add_node(FailingReadyProbe::new().into(), None);
        }
        engine.add_node(Folder::new("suffix").into(), None);
        assert!(engine.apply_edits().is_err());
        assert_eq!(engine.nodes.len(), 3);
        assert_eq!(engine.undo_len(), 1);
        assert!(engine.edits.pending.is_empty());
        let snapshot = engine.process_tree_snapshot();
        assert_eq!(snapshot.child_ids(engine.root).len(), 2);
        assert!(engine.undo().unwrap());
        assert_eq!(engine.nodes.len(), 1);
        engine.add_node(Folder::new("next").into(), None);
        engine.apply_edits().unwrap();
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    }
}

#[test]
fn invalid_insertion_placement_does_not_leave_an_orphan() {
    for use_tree in [false, true] {
        let mut engine = Engine::new(Folder::new("root"));
        let child = Folder::new("invalid");
        engine.edits.push(if use_tree {
            Edit::AddNodeTree {
                tree: NodeTree::new(child),
                parent: engine.root,
                prev_sibling: Some(NodeId(u64::MAX)),
            }
        } else {
            Edit::AddNode {
                node: Box::new(child),
                parent: engine.root,
                prev_sibling: Some(NodeId(u64::MAX)),
            }
        });
        assert!(matches!(
            engine.apply_edits(),
            Err(EngineEditError::SiblingNotFound { .. })
        ));
        assert_eq!(engine.nodes.len(), 1);
        assert_eq!(engine.undo_len(), 0);
    }
}

#[test]
fn invalid_move_placement_does_not_detach_the_node() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.add_node(Folder::new("child"), None);
    engine.apply_edits().unwrap();
    let child = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    let history_before = engine.ui_history_state();
    engine.edits.push(Edit::MoveNode {
        node: child,
        new_parent: engine.root,
        new_prev_sibling: Some(NodeId(u64::MAX)),
    });
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::SiblingNotFound { .. })
    ));
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert_eq!(engine.ui_history_state(), history_before);
}

fn reject_folder_parameter<T: Node>(engine: &mut Engine<T>) {
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

#[test]
fn failed_edit_retains_only_the_applied_prefix_in_history() {
    for failure_index in 0..3 {
        let mut engine = Engine::new(Folder::new("root"));
        for index in 0..3 {
            if index == failure_index {
                engine.edits.push(Edit::SetParam {
                    node: engine.root,
                    value: ParamValue::Int(1),
                    behaviour: crate::parameter::ParameterEventBehaviour::Coalesce,
                });
            } else {
                engine.add_node(Folder::new(format!("child {index}")), None);
            }
        }

        assert!(matches!(
            engine.apply_edits(),
            Err(EngineEditError::ParamEditTargetMismatch { edit_index, .. }) if edit_index == failure_index
        ));
        assert_eq!(engine.nodes.len(), failure_index + 1);
        assert_eq!(engine.undo_len(), usize::from(failure_index > 0));
        assert!(engine.edits.pending.is_empty(), "rejected suffix must not replay");
        if failure_index > 0 {
            assert!(engine.undo().unwrap());
            assert_eq!(engine.nodes.len(), 1);
            assert!(engine.redo().unwrap());
            assert_eq!(engine.nodes.len(), failure_index + 1);
        }

        engine.add_node(Folder::new("next"), None);
        engine.apply_edits().unwrap();
        assert_eq!(engine.nodes.len(), failure_index + 2);
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    }
}

#[test]
fn rejection_before_any_mutation_preserves_redo() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.add_node(Folder::new("child"), None);
    engine.apply_edits().unwrap();
    engine.undo().unwrap();
    let history_before = engine.ui_history_state();
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(1),
        behaviour: crate::parameter::ParameterEventBehaviour::Coalesce,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(engine.ui_history_state(), history_before);
    assert!(engine.redo().unwrap());
    assert_eq!(engine.nodes.len(), 2);
}

#[test]
fn failed_batch_finalizes_missing_reference_warnings_and_dirty_nodes() {
    let mut engine = Engine::new(FailureTestNode::from(Folder::new("root")));
    engine.add_node(
        Parameter::new("target", ParamValue::Int(1), ParameterChangeCheck::None).into(),
        None,
    );
    engine.add_node(
        Parameter::new(
            "reference",
            ParamValue::Reference(NodeReference::default()),
            ParameterChangeCheck::None,
        )
        .into(),
        None,
    );
    engine.apply_edits().unwrap();
    let target = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let reference = engine.nodes.get(target).unwrap().node_data().next_sibling.unwrap();
    let target_uuid = engine.nodes.get(target).unwrap().node_data().meta.uuid;
    engine.edits.push(Edit::SetParam {
        node: reference,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    engine.clear_history();
    engine.take_project_dirty_nodes();

    engine.edits.push(Edit::RemoveNode { node: target });
    reject_folder_parameter(&mut engine);
    assert!(engine.apply_edits().is_err());
    assert!(
        engine
            .nodes
            .get(reference)
            .unwrap()
            .node_data()
            .meta
            .presentation
            .warning(Some("missing-reference"))
            .is_some()
    );
    let dirty = engine.take_project_dirty_nodes();
    assert!(dirty.contains(&target));
    assert!(dirty.contains(&engine.root));
    assert!(engine.undo().unwrap());
    assert!(
        engine
            .nodes
            .get(reference)
            .unwrap()
            .node_data()
            .meta
            .presentation
            .warning(Some("missing-reference"))
            .is_none()
    );
}

#[test]
fn failed_batch_invalidates_cached_context_resolution() {
    let mut engine = Engine::new(FailureTestNode::from(Folder::new("root")));
    engine.add_node(Folder::new("owner").into(), None);
    engine.apply_edits().unwrap();
    let owner = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.add_node(UserContextNode::new("scope").into(), Some(owner));
    engine.apply_edits().unwrap();
    let scope = engine.nodes.get(owner).unwrap().node_data().first_child.unwrap();
    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(120.0), ParameterChangeCheck::None).into(),
        Some(scope),
    );
    engine.add_node(Folder::new("consumer").into(), Some(owner));
    engine.apply_edits().unwrap();
    let consumer = engine.nodes.get(scope).unwrap().node_data().next_sibling.unwrap();
    assert!(matches!(
        engine.resolve_user_context_symbol(consumer, "tempo", Some(UserContextValueType::Float)),
        UserContextLookup::Resolved(_)
    ));

    engine.edits.push(Edit::MoveNode {
        node: consumer,
        new_parent: engine.root,
        new_prev_sibling: None,
    });
    reject_folder_parameter(&mut engine);
    assert!(engine.apply_edits().is_err());
    assert!(matches!(
        engine.resolve_user_context_symbol(consumer, "tempo", Some(UserContextValueType::Float)),
        UserContextLookup::Missing { .. }
    ));
    engine.edits.push(Edit::MoveNode {
        node: consumer,
        new_parent: NodeId(u64::MAX),
        new_prev_sibling: None,
    });
    assert!(engine.apply_edits().is_err());
    engine.undo().unwrap();
    assert!(matches!(
        engine.resolve_user_context_symbol(consumer, "tempo", Some(UserContextValueType::Float)),
        UserContextLookup::Resolved(_)
    ));
}
