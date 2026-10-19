use super::*;
use crate::{
    edit::{EditCompletion, EditCompletionStatus as Status},
    events::CustomEvent,
    node::{EventSubscription, Folder, NodeData, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
};

fn rename(node: NodeId, label: &str) -> Edit {
    Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            label: Some(label.into()),
            ..Default::default()
        },
    }
}

fn invalid_edit() -> Edit {
    Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    }
}

fn enqueue<T: Node>(engine: &mut Engine<T>, edit: Edit) -> EditCompletion {
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.edits.push(edit);
    let completion = ctx.edits.completion();
    engine.absorb_edits(&mut ctx).unwrap();
    completion
}

#[test]
fn edit_completion_distinguishes_applied_prefix_rejection_and_discarded_suffix() {
    let mut engine = Engine::new(Folder::new("root"));
    assert_eq!(engine.edits.completion().status(), Status::Applied);
    let root = engine.root;
    let prefix = enqueue(&mut engine, rename(root, "accepted"));
    let rejected = enqueue(&mut engine, invalid_edit());
    let suffix = enqueue(&mut engine, rename(root, "discarded"));
    let aggregate = engine.edits.completion();
    assert_eq!(aggregate.status(), Status::Pending);
    assert!(engine.apply_edits().is_err());
    assert_eq!(prefix.status(), Status::Applied);
    assert_eq!(rejected.status(), Status::Failed);
    assert_eq!(suffix.status(), Status::Failed);
    assert_eq!(aggregate.status(), Status::Failed);
    assert_eq!(engine.nodes.get(root).unwrap().node_data().meta.label, "accepted");
    assert!(engine.undo().unwrap());
    // Application receipts are historical observations; a later undo does not change them.
    assert_eq!(prefix.status(), Status::Applied);
}

#[test]
fn edit_completion_follows_atomic_commit_and_rollback_without_publishing_early_success() {
    for reject in [false, true] {
        let mut engine = Engine::new(Folder::new("root"));
        let root = engine.root;
        let previous = enqueue(&mut engine, rename(root, "previous"));
        let mut receipt = None;
        let result: Result<(), EngineEditError> = engine.apply_atomic_edit_operation(|engine| {
            receipt = Some(enqueue(engine, rename(root, "candidate")));
            engine.apply_edits()?;
            assert_eq!(receipt.as_ref().unwrap().status(), Status::Pending);
            if reject {
                engine.edits.push(invalid_edit());
                engine.apply_edits()?;
            }
            Ok(())
        });
        assert_eq!(result.is_err(), reject);
        assert_eq!(
            receipt.unwrap().status(),
            if reject { Status::Failed } else { Status::Applied }
        );
        assert_eq!(previous.status(), Status::Pending);
        assert_eq!(
            engine.nodes.get(root).unwrap().node_data().meta.label,
            if reject { "root" } else { "candidate" }
        );
        engine.apply_edits().unwrap();
        assert_eq!(previous.status(), Status::Applied);
    }
}

#[test]
fn observed_no_op_waits_for_application_and_cannot_escape_a_rejected_prefix() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.edits.push(invalid_edit());
    let root = engine.root;
    let completion = enqueue(
        &mut engine,
        Edit::ClearNodeWarning {
            node: root,
            warning_id: None,
        },
    );
    assert_eq!(completion.status(), Status::Pending);
    assert!(engine.apply_edits().is_err());
    assert_eq!(completion.status(), Status::Failed);

    let completion = enqueue(
        &mut engine,
        Edit::ClearNodeWarning {
            node: root,
            warning_id: None,
        },
    );
    engine.apply_edits().unwrap();
    assert_eq!(completion.status(), Status::Applied);
}

#[test]
fn edit_completion_reports_coalescing_and_transfers_duplicate_removal_ownership() {
    let mut engine = Engine::new(Folder::new("root"));
    let root = engine.root;
    engine.edits.push(Edit::SetParam {
        node: root,
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    let superseded = engine.edits.completion();
    engine.edits.push(Edit::SetParam {
        node: root,
        value: ParamValue::Int(2),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert_eq!(superseded.status(), Status::Failed);
    engine.edits.pending.clear();

    engine.add_node(Folder::new("child"), None);
    engine.apply_edits().unwrap();
    let child = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let first = enqueue(&mut engine, Edit::RemoveNode { node: child });
    let second = enqueue(&mut engine, Edit::RemoveNode { node: child });
    assert_eq!(engine.edits.pending.len(), 1);
    engine.apply_edits().unwrap();
    assert_eq!(first.status(), Status::Applied);
    assert_eq!(second.status(), Status::Applied);
}

#[test]
fn edit_completion_fails_the_entire_rejected_ingress_set_and_engine_teardown() {
    let completion = {
        let mut engine = Engine::new(Folder::new("root"));
        let root = engine.root;
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        ctx.edits.push(rename(root, "must not leak"));
        ctx.edits.push(Edit::AddNode {
            node: Box::new(CompletionProbe::new(false)),
            parent: root,
            prev_sibling: None,
        });
        let rejected = ctx.edits.completion();
        assert!(engine.absorb_edits(&mut ctx).is_err());
        assert_eq!(rejected.status(), Status::Failed);
        assert!(engine.edits.pending.is_empty());
        enqueue(&mut engine, rename(root, "never applied"))
    };
    assert_eq!(completion.status(), Status::Failed);
}

struct CompletionProbe {
    data: NodeData,
    fire: bool,
    fail_ready: bool,
    completion: Option<EditCompletion>,
}

impl CompletionProbe {
    fn new(fail_ready: bool) -> Self {
        Self {
            data: NodeData::new("probe".into()),
            fire: false,
            fail_ready,
            completion: None,
        }
    }
}

impl Node for CompletionProbe {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "completion_probe"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(100)
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        if std::mem::take(&mut self.fire) {
            ctx.emit_custom_event(CustomEvent::new(
                "completion_defer",
                Some(self.id()),
                serde_json::Value::Null,
            ));
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic == "completion_defer" {
            ctx.add_child(self.id(), Self::new(false), None);
            self.completion = Some(ctx.edits.completion());
        }
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if self.fail_ready {
            ctx.edits.push(invalid_edit());
        }
    }
}

#[test]
fn edit_completion_remains_pending_across_real_structural_deferral() {
    let mut probe = CompletionProbe::new(false);
    probe.fire = true;
    let mut engine = Engine::new(probe);
    engine.edits.push(Edit::AddEventListener {
        subscriber: engine.root,
        subscription: EventSubscription::node(engine.root),
    });
    engine.apply_edits().unwrap();
    engine.resolve().unwrap();
    engine.run_tick(Duration::from_millis(20)).unwrap();
    let receipt = engine
        .nodes
        .get(engine.root)
        .unwrap()
        .completion
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(receipt.status(), Status::Pending);
    assert_eq!(engine.nodes.len(), 1);
    assert_eq!(engine.deferred_structural_edits.len(), 1);
    engine.run_tick(Duration::from_millis(20)).unwrap();
    assert_eq!(receipt.status(), Status::Applied);
    assert_eq!(engine.nodes.len(), 2);
}

#[test]
fn edit_completion_fails_when_insertion_mutates_but_its_ready_completion_fails() {
    let mut engine = Engine::new(CompletionProbe::new(false));
    let root = engine.root;
    let completion = enqueue(
        &mut engine,
        Edit::AddNodeTree {
            tree: NodeTree::new(CompletionProbe::new(true)),
            parent: root,
            prev_sibling: None,
        },
    );
    assert!(engine.apply_edits().is_err());
    assert_eq!(completion.status(), Status::Failed);
    assert_eq!(
        engine.nodes.len(),
        2,
        "failure must preserve P01's committed-prefix contract"
    );
    assert!(engine.undo().unwrap());
    assert_eq!(engine.nodes.len(), 1);
}
