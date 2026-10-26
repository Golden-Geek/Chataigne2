use super::*;
use crate::{
    node::NodeWarning,
    parameter::{ParameterConstraints, RangeConstraint},
    process_ctx::SubtreeStructureRevision,
};

fn structure(engine: &Engine<RevisionNode>, node: NodeId) -> SubtreeStructureRevision {
    engine
        .build_process_tree_snapshot()
        .subtree_structure_revision(node)
        .unwrap()
}

#[test]
fn value_constraints_and_warnings_preserve_binding_structure_but_advance_authored_state() {
    let mut engine = Engine::new(RevisionNode::from(Folder::new("root")));
    let root = engine.root;
    let branch = folder(&mut engine, root, "branch");
    let param = add(
        &mut engine,
        branch,
        Parameter::new("level", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
    );
    let before = structure(&engine, branch);
    let authored = revision(&engine, branch);
    engine.edits.push(Edit::SetParam {
        node: param,
        value: ParamValue::Float(0.5),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.edits.push(Edit::SetParamConstraints {
        node: param,
        constraints: ParameterConstraints {
            range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
            ..Default::default()
        },
    });
    engine.edits.push(Edit::SetNodeWarning {
        node: param,
        warning: NodeWarning {
            id: "probe".into(),
            message: "not a binding change".into(),
            detail: None,
        },
    });
    engine.apply_edits().unwrap();
    assert_eq!(before, structure(&engine, branch));
    assert_ne!(authored, revision(&engine, branch));
    let snapshot = engine.build_process_tree_snapshot();
    let overlay = snapshot.with_param_values([(param, ParamValue::Float(0.8))]);
    assert_ne!(
        snapshot.subtree_structure_revision(branch),
        overlay.subtree_structure_revision(branch)
    );
}

#[test]
fn named_aliases_enablement_and_same_uuid_replacement_invalidate_structure() {
    let mut engine = Engine::new(RevisionNode::from(Folder::new("root")));
    let root = engine.root;
    let branch = folder(&mut engine, root, "branch");
    let leaf = folder(&mut engine, branch, "leaf");
    let mut before = structure(&engine, branch);
    for patch in [
        NodeMetaPatch {
            label: Some("renamed".into()),
            ..Default::default()
        },
        NodeMetaPatch {
            short_name: Some("alias".into()),
            ..Default::default()
        },
        NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    ] {
        engine.edits.push(Edit::PatchMeta { node: leaf, patch });
        engine.apply_edits().unwrap();
        let after = structure(&engine, branch);
        assert_ne!(before, after);
        before = after;
    }
    let uuid = engine.nodes.get(leaf).unwrap().node_data().meta.uuid;
    let mut replacement = Parameter::new("replacement", ParamValue::Int(0), ParameterChangeCheck::ValueChange);
    replacement.node_data_mut().meta.uuid = uuid;
    engine.edits.push(Edit::ReplaceNode {
        node: leaf,
        new_node: Box::new(replacement),
    });
    engine.apply_edits().unwrap();
    assert_eq!(engine.nodes.get(leaf).unwrap().node_data().meta.uuid, uuid);
    assert_ne!(before, structure(&engine, branch));
}

#[test]
fn moves_removal_and_history_cover_both_parents_without_mutating_retained_views() {
    let mut engine = Engine::new(RevisionNode::from(Folder::new("root")));
    let root = engine.root;
    let left = folder(&mut engine, root, "left");
    let right = folder(&mut engine, root, "right");
    let other = folder(&mut engine, root, "other");
    let child = folder(&mut engine, left, "child");
    let retained = engine.build_process_tree_snapshot();
    let old = (structure(&engine, left), structure(&engine, right));
    let disjoint = structure(&engine, other);
    engine.edits.push(Edit::MoveNode {
        node: child,
        new_parent: right,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    assert_ne!(old.0, structure(&engine, left));
    assert_ne!(old.1, structure(&engine, right));
    assert_eq!(disjoint, structure(&engine, other));
    let moved = structure(&engine, right);
    assert!(engine.undo().unwrap());
    assert_ne!(moved, structure(&engine, right));
    assert!(engine.redo().unwrap());
    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().unwrap();
    assert!(
        engine
            .build_process_tree_snapshot()
            .subtree_structure_revision(child)
            .is_none()
    );
    assert_eq!(retained.subtree_structure_revision(left), Some(old.0));
    assert_eq!(disjoint, structure(&engine, other));
}

#[test]
fn structure_tokens_are_root_specific_even_with_the_same_folded_sequence() {
    let mut clock = super::super::super::tree_revisions::TreeRevisions::default();
    let mut engine = Engine::new(RevisionNode::from(Folder::new("root")));
    let root = engine.root;
    let leaf = folder(&mut engine, root, "leaf");
    let source = engine.build_process_tree_snapshot();
    let first = source.node(root).unwrap().clone();
    let second = source.node(leaf).unwrap().clone();
    let sequence = clock.next_change();
    clock.mark_structure(leaf, sequence);
    let snapshot = ProcessTreeSnapshot::from_indexed_nodes(
        root,
        std::collections::HashMap::from([(root, Box::new(first)), (leaf, Box::new(second))]),
        Default::default(),
        clock.snapshot(),
    );
    assert_eq!(snapshot.subtree_revision(root), snapshot.subtree_revision(leaf));
    assert_ne!(
        snapshot.subtree_structure_revision(root),
        snapshot.subtree_structure_revision(leaf)
    );
}

#[test]
fn ui_value_transactions_do_not_invalidate_bindings_but_alias_transactions_do() {
    use crate::ui_sync::{UiGraphOp, UiGraphTransaction, UiNodeMetaPatch, UiParamPatch};
    let mut engine = Engine::new(RevisionNode::from(Folder::new("root")));
    let root = engine.root;
    let leaf = folder(&mut engine, root, "leaf");
    let before = structure(&engine, root);
    let transaction = |ops| crate::events::EventKind::GraphTransaction {
        transaction: UiGraphTransaction {
            tx_id: 1,
            epoch: 1,
            base_graph_version: 0,
            next_graph_version: 1,
            ops,
        },
    };
    engine.emit_event(transaction(vec![UiGraphOp::ParamPatched {
        node: leaf,
        param: leaf,
        patch: UiParamPatch {
            value: Some(ParamValue::Float(0.1)),
            control: None,
            constraints: None,
        },
    }]));
    assert_eq!(before, structure(&engine, root));
    let patch: UiNodeMetaPatch = serde_json::from_value(serde_json::json!({"label":"alias"})).unwrap();
    engine.emit_event(transaction(vec![UiGraphOp::NodeMetaPatched { node: leaf, patch }]));
    assert_ne!(before, structure(&engine, root));
}
