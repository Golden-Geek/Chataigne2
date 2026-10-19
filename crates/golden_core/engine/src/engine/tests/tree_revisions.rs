use super::*;
use crate::{
    edit::Edit,
    node::Folder,
    parameter::{Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    process_ctx::SubtreeRevision,
};

#[crate::node("revision_probe")]
struct RevisionProbe {
    #[state(default = 0, persist)]
    authored: i32,
    target: NodeId,
    observed: Vec<SubtreeRevision>,
}

#[crate::node("revision_probe", from_struct)]
impl Node for RevisionProbe {
    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        _args: &[ParamValue],
    ) -> Result<bool, String> {
        if method == "increment" {
            self.authored += 1;
            return Ok(true);
        }
        if method != "observe" {
            return Ok(false);
        }
        self.observed
            .push(ctx.tree_snapshot().unwrap().subtree_revision(self.target).unwrap());
        Ok(true)
    }
}

crate::define_node_enum!(
    enum RevisionNode {
        RevisionProbe,
    }
);

fn add(engine: &mut Engine<RevisionNode>, parent: NodeId, node: RevisionNode) -> NodeId {
    engine.add_node(node, Some(parent));
    engine.apply_edits().unwrap();
    engine.nodes.get(parent).unwrap().node_data().last_child.unwrap()
}

fn folder(engine: &mut Engine<RevisionNode>, parent: NodeId, name: &str) -> NodeId {
    add(engine, parent, Folder::new(name).into())
}

fn revision(engine: &Engine<RevisionNode>, node: NodeId) -> SubtreeRevision {
    engine.build_process_tree_snapshot().subtree_revision(node).unwrap()
}

#[test]
fn applied_parameter_write_fences_read_models_before_the_following_script_call() {
    let mut engine = Engine::new(RevisionNode::from(Folder::new("root")));
    let root = engine.root;
    let left = folder(&mut engine, root, "left");
    let right = folder(&mut engine, root, "right");
    let parameter = add(
        &mut engine,
        left,
        Parameter::new("value", ParamValue::Int(1), ParameterChangeCheck::None).into(),
    );
    let probe = add(&mut engine, right, RevisionProbe::new(left, Vec::new()).into());
    let old = revision(&engine, left);
    let unrelated = revision(&engine, right);
    let retained = engine.get_or_build_tick_snapshot();
    engine.edits.push(Edit::SetParam {
        node: parameter,
        value: ParamValue::Int(2),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: probe,
        call: crate::script::ScriptMethodCall::prepare("observe", Vec::new()).unwrap(),
    });
    engine.apply_edits().unwrap();
    let current = revision(&engine, left);
    assert_ne!(old, current);
    assert_eq!(unrelated, revision(&engine, right));
    let RevisionNode::RevisionProbe(probe) = engine.nodes.get(probe).unwrap() else {
        panic!("probe");
    };
    assert_eq!(probe.observed, [current]);
    assert_eq!(retained.subtree_revision(left), Some(old));
    engine.emit_event(crate::events::EventKind::Custom(crate::events::CustomEvent::new(
        "traffic",
        Some(left),
        serde_json::Value::Null,
    )));
    assert_eq!(current, revision(&engine, left));
}

#[test]
fn topology_and_history_changes_fence_both_sides_of_a_move_and_keep_other_subtrees_stable() {
    let mut engine = Engine::new(RevisionNode::from(Folder::new("root")));
    let root = engine.root;
    let left = folder(&mut engine, root, "left");
    let right = folder(&mut engine, root, "right");
    let unrelated = folder(&mut engine, root, "unrelated");
    let child = folder(&mut engine, left, "child");
    let before = (
        revision(&engine, left),
        revision(&engine, right),
        revision(&engine, unrelated),
    );
    engine.edits.push(Edit::MoveNode {
        node: child,
        new_parent: right,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let moved = (revision(&engine, left), revision(&engine, right));
    assert_ne!(before.0, moved.0);
    assert_ne!(before.1, moved.1);
    assert_eq!(before.2, revision(&engine, unrelated));
    assert!(engine.undo().unwrap());
    assert_ne!(moved.0, revision(&engine, left));
    assert_ne!(moved.1, revision(&engine, right));
    assert!(engine.redo().unwrap());
    assert_eq!(before.2, revision(&engine, unrelated));
    let previous = revision(&engine, right);
    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().unwrap();
    assert_ne!(previous, revision(&engine, right));
    assert!(engine.build_process_tree_snapshot().subtree_revision(child).is_none());
}

#[test]
fn engine_lifetimes_and_parameter_overlays_have_distinct_revision_namespaces() {
    let first = Engine::new(RevisionNode::from(Folder::new("root")));
    let second = Engine::new(RevisionNode::from(Folder::new("root")));
    assert_ne!(revision(&first, first.root), revision(&second, second.root));
    let mut engine = first;
    let root = engine.root;
    let parameter = add(
        &mut engine,
        root,
        Parameter::new("value", ParamValue::Int(1), ParameterChangeCheck::None).into(),
    );
    let snapshot = engine.build_process_tree_snapshot();
    assert_eq!(snapshot.clone().subtree_revision(root), snapshot.subtree_revision(root));
    let overlay = snapshot.with_param_values([(parameter, ParamValue::Int(9))]);
    assert_ne!(snapshot.subtree_revision(root), overlay.subtree_revision(root));
}

#[test]
fn opaque_authored_changes_and_metadata_replay_advance_revisions_without_ui_event_dependence() {
    let mut engine = Engine::new(RevisionNode::from(Folder::new("root")));
    let root = engine.root;
    let target = add(&mut engine, root, RevisionProbe::new(root, Vec::new()).into());
    let other = folder(&mut engine, root, "other");
    let original = revision(&engine, target);
    let unrelated = revision(&engine, other);
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: target,
        call: crate::script::ScriptMethodCall::prepare("increment", Vec::new()).unwrap(),
    });
    engine.apply_edits().unwrap();
    let changed = revision(&engine, target);
    assert_ne!(original, changed);
    assert_eq!(unrelated, revision(&engine, other));
    assert!(engine.undo().unwrap());
    assert_ne!(changed, revision(&engine, target));
    let before_meta = revision(&engine, target);
    engine.edits.push(Edit::PatchMeta {
        node: target,
        patch: NodeMetaPatch {
            label: Some("renamed".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert_ne!(before_meta, revision(&engine, target));
    assert_eq!(unrelated, revision(&engine, other));
}
