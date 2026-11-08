use golden_engine::{
    edit::Edit,
    engine::Engine,
    node::{Node, NodeData, NodeId},
    process_ctx::ProcessCtx,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

type Observations = Arc<Mutex<Vec<(String, bool, bool)>>>;
struct Probe {
    data: NodeData,
    observations: Observations,
    bound: Arc<AtomicBool>,
}
impl Probe {
    fn new(label: &str, observations: &Observations) -> Self {
        Self {
            data: NodeData::new(label.into()),
            observations: observations.clone(),
            bound: Arc::new(AtomicBool::new(false)),
        }
    }
}
impl Node for Probe {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "destroy_scope_probe"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn destroy(&mut self, ctx: &mut ProcessCtx) {
        assert!(ctx.tree_snapshot().is_none());
        self.observations.lock().unwrap().push((
            self.data.meta.label.clone(),
            ctx.is_destroying(self.id()),
            self.data.parent.is_some_and(|parent| ctx.is_destroying(parent)),
        ));
    }
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _: golden_engine::node::NodeCreationContext) {
        assert!(!ctx.is_destroying(self.id()));
        if let Some(parent) = self.data.parent {
            self.bound.store(false, Ordering::Release);
            let bound = self.bound.clone();
            ctx.register_ancestor_runtime(self.id(), move |node| {
                assert_eq!(node.id(), parent);
                bound.store(true, Ordering::Release);
                Ok(true)
            });
        }
    }
}

#[test]
fn destroy_scope_distinguishes_channel_and_ancestor_removal_through_history_without_a_snapshot() {
    let observations = Observations::default();
    let mut engine = Engine::new(Probe::new("root", &observations));
    engine.add_node(Probe::new("parent", &observations), None);
    engine.apply_edits().unwrap();
    let parent = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.add_node(Probe::new("child", &observations), Some(parent));
    engine.apply_edits().unwrap();
    let child = engine.nodes.get(parent).unwrap().node_data().first_child.unwrap();
    assert!(engine.nodes.get(child).unwrap().bound.load(Ordering::Acquire));
    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().unwrap();
    assert_eq!(*observations.lock().unwrap(), [("child".into(), true, false)]);
    assert!(engine.undo().unwrap());
    assert!(
        engine.nodes.get(child).unwrap().bound.load(Ordering::Acquire),
        "history-ready registrations settle synchronously"
    );
    observations.lock().unwrap().clear();
    engine.edits.push(Edit::RemoveNode { node: parent });
    engine.apply_edits().unwrap();
    assert_eq!(
        *observations.lock().unwrap(),
        [("child".into(), true, true), ("parent".into(), true, false)]
    );
    assert!(engine.undo().unwrap());
    observations.lock().unwrap().clear();
    assert!(engine.redo().unwrap());
    assert_eq!(
        *observations.lock().unwrap(),
        [("child".into(), true, true), ("parent".into(), true, false)]
    );
    assert!(
        !ProcessCtx::new(
            golden_engine::process_ctx::ExecutionPhase::EngineTick,
            golden_engine::engine::EngineTime {
                tick: 0,
                micro: 0,
                seq: 0
            }
        )
        .is_destroying(NodeId(1))
    );
}
