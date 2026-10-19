use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use golden_engine::{
    edit::{Edit, EditCompletionStatus, NodeTree, UserItemTreeInsertion},
    engine::{Engine, EngineEditError, LifecycleLimits, NodeWaker},
    node::{Node, NodeCreationContext, NodeData, NodeId, UserContainerRules},
    process_ctx::ProcessCtx,
};

struct LifecycleProbe {
    data: NodeData,
    remaining: usize,
    expanded: bool,
    active: bool,
    events: Arc<Mutex<Vec<String>>>,
    tree_width: usize,
    forest_mode: u8,
    reject_restore: Arc<AtomicBool>,
    wake: Option<NodeWaker>,
}

impl LifecycleProbe {
    fn new(label: &str, remaining: usize, events: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            data: NodeData::new(label.into()),
            remaining,
            expanded: false,
            active: false,
            events,
            tree_width: 0,
            forest_mode: 0,
            reject_restore: Arc::new(AtomicBool::new(false)),
            wake: None,
        }
    }

    fn record(&self, phase: &str) {
        self.events
            .lock()
            .unwrap()
            .push(format!("{phase}:{}", self.data.meta.label));
    }
}

impl Node for LifecycleProbe {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "lifecycle_probe"
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

    fn engine_on_attached(&mut self, _ctx: &mut ProcessCtx) {
        self.record("attached");
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[]))
    }
    fn user_container_accepts_item(&self, _item_type: &str, _item_kind: &str) -> bool {
        true
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.record("init");
        if self.tree_width > 0 {
            let mut tree = NodeTree::new(Self::new("forest", 0, self.events.clone()));
            for _ in 0..self.tree_width {
                tree.push_child(NodeTree::new(Self::new("leaf", 0, self.events.clone())));
            }
            ctx.edits.push(match self.forest_mode {
                0 => Edit::AddNodeTree {
                    tree,
                    parent: self.id(),
                    prev_sibling: None,
                },
                1 => Edit::AddUserItemTree {
                    tree,
                    parent: self.id(),
                    prev_sibling: None,
                },
                _ => Edit::AddUserItemTrees {
                    items: vec![
                        UserItemTreeInsertion::new(
                            self.id(),
                            NodeTree::new(Self::new("first", 0, self.events.clone())),
                        ),
                        UserItemTreeInsertion::new(self.id(), tree),
                    ],
                },
            });
        }
        if !self.expanded && self.remaining > 0 {
            self.expanded = true;
            ctx.add_child(
                self.id(),
                Self::new("generated", self.remaining - 1, self.events.clone()),
                None,
            );
        }
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        self.active = true;
        self.record("ready");
        self.wake = Some(ctx.node_waker().unwrap());
        if context == NodeCreationContext::HistoryRestore && self.reject_restore.load(Ordering::Relaxed) {
            ctx.add_event_listener(NodeId(u64::MAX), self.id());
        }
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.active = false;
        self.wake = None;
        self.record("destroy");
    }
}

#[test]
fn lifecycle_limits_stop_callback_expansion_without_rejecting_known_input_trees() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut engine = Engine::new(LifecycleProbe::new("root", 0, events.clone()));
    engine.set_lifecycle_limits(LifecycleLimits {
        max_passes: NonZeroUsize::new(2).unwrap(),
        ..Default::default()
    });
    engine.add_node(LifecycleProbe::new("expanding", 10, events), None);
    assert!(
        engine.apply_edits().is_err(),
        "lifecycle expansion must respect its admitted pass budget"
    );
    assert!(engine.nodes.len() <= 4);
    assert!(
        engine.edits.pending.is_empty(),
        "reject the generated suffix instead of continuing it next tick"
    );
    let accepted = engine.nodes.len();
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    assert_eq!(engine.nodes.len(), accepted);
    assert!(engine.undo().unwrap());
    assert_eq!(engine.nodes.len(), 1);
    assert!(engine.redo().unwrap());
    assert_eq!(engine.nodes.len(), accepted);

    // Known detached inputs are not callback expansion, regardless of their width.
    engine.set_lifecycle_limits(LifecycleLimits {
        max_work: NonZeroUsize::new(1).unwrap(),
        ..Default::default()
    });
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut tree = NodeTree::new(LifecycleProbe::new("known", 0, events.clone()));
    for _ in 0..256 {
        tree.push_child(NodeTree::new(LifecycleProbe::new("leaf", 0, events.clone())));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: engine.root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    assert_eq!(engine.nodes.len(), accepted + 257);
}

#[test]
fn replacement_transfers_live_ownership_before_initializing_the_new_payload() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut engine = Engine::new(LifecycleProbe::new("root", 0, events.clone()));
    engine.add_node(LifecycleProbe::new("old", 0, events.clone()), None);
    engine.apply_edits().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    events.lock().unwrap().clear();
    engine.edits.push(Edit::ReplaceNode {
        node: id,
        new_node: Box::new(LifecycleProbe::new("new", 0, events.clone())),
    });
    engine.apply_edits().unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        ["destroy:old", "attached:new", "init:new", "ready:new"]
    );
    assert!(engine.nodes.get(id).unwrap().active);
    assert_eq!(engine.nodes.get(id).unwrap().id(), id);
    events.lock().unwrap().clear();
    assert!(engine.undo().unwrap());
    assert_eq!(*events.lock().unwrap(), ["destroy:new", "ready:old"]);
    assert!(engine.nodes.get(id).unwrap().active);
    events.lock().unwrap().clear();
    assert!(engine.redo().unwrap());
    assert_eq!(*events.lock().unwrap(), ["destroy:old", "ready:new"]);
    assert!(engine.nodes.get(id).unwrap().active);
}

#[test]
fn generated_tree_is_rejected_before_any_partial_insertion() {
    for mode in 0..3 {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut engine = Engine::new(LifecycleProbe::new("root", 0, events.clone()));
        engine.set_lifecycle_limits(LifecycleLimits {
            max_work: NonZeroUsize::new(5).unwrap(),
            ..Default::default()
        });
        let mut node = LifecycleProbe::new("generator", 0, events);
        node.tree_width = 10;
        node.forest_mode = mode;
        engine.add_node(node, None);
        let completion = engine.edits.completion();
        assert!(matches!(
            engine.apply_edits(),
            Err(EngineEditError::LifecycleBudgetExceeded { resource: "work", .. })
        ));
        assert_eq!(completion.status(), EditCompletionStatus::Failed);
        assert_eq!(engine.nodes.len(), 2, "only the accepted generator is retained");
        assert!(engine.edits.pending.is_empty());
        assert!(engine.undo().unwrap());
        assert_eq!(engine.nodes.len(), 1);
        assert!(engine.redo().unwrap());
        assert_eq!(engine.nodes.len(), 2, "history must not rerun generator init");
    }
}

#[test]
fn replacement_retains_children_and_closes_each_retired_worker_generation() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut engine = Engine::new(LifecycleProbe::new("root", 0, events.clone()));
    let tree = NodeTree::new(LifecycleProbe::new("old", 0, events.clone()))
        .with_child(NodeTree::new(LifecycleProbe::new("child", 0, events.clone())));
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: engine.root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let child = engine.nodes.get(id).unwrap().node_data().first_child.unwrap();
    let old_wake = engine.nodes.get(id).unwrap().wake.clone().unwrap();
    events.lock().unwrap().clear();
    engine.edits.push(Edit::ReplaceNode {
        node: id,
        new_node: Box::new(LifecycleProbe::new("new", 0, events.clone())),
    });
    engine.apply_edits().unwrap();
    let new_wake = engine.nodes.get(id).unwrap().wake.clone().unwrap();
    assert!(old_wake.wake().is_err());
    for undo in [true, false, true] {
        if undo {
            engine.undo().unwrap();
        } else {
            engine.redo().unwrap();
        }
        assert_eq!(engine.nodes.get(id).unwrap().node_data().first_child, Some(child));
        assert!(engine.nodes.get(child).unwrap().active);
        assert!(engine.nodes.get(id).unwrap().wake.as_ref().unwrap().wake().is_ok());
    }
    assert!(new_wake.wake().is_err());
    assert!(!events.lock().unwrap().iter().any(|event| event == "destroy:child"));
}

#[test]
fn rejected_history_activation_restores_payload_runtime_and_allows_retry() {
    for undo in [true, false] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut engine = Engine::new(LifecycleProbe::new("root", 0, events.clone()));
        let old = LifecycleProbe::new("old", 0, events.clone());
        let reject_old = old.reject_restore.clone();
        engine.add_node(old, None);
        engine.apply_edits().unwrap();
        let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
        let new = LifecycleProbe::new("new", 0, events);
        let reject_new = new.reject_restore.clone();
        engine.edits.push(Edit::ReplaceNode {
            node: id,
            new_node: Box::new(new),
        });
        engine.apply_edits().unwrap();
        if !undo {
            engine.undo().unwrap();
        }
        let reject = if undo { reject_old } else { reject_new };
        reject.store(true, Ordering::Relaxed);
        let history = engine.ui_history_state();
        let before = engine.ui_event_log().to_vec();
        for _ in 0..2 {
            assert!(if undo { engine.undo() } else { engine.redo() }.is_err());
            let live = engine.nodes.get(id).unwrap();
            assert_eq!(live.data.meta.label, if undo { "new" } else { "old" });
            assert!(live.active, "rollback must reactivate the original payload");
            assert!(live.wake.as_ref().unwrap().wake().is_ok());
            assert_eq!(engine.ui_history_state(), history);
            assert_eq!(engine.ui_event_log(), before);
        }
        reject.store(false, Ordering::Relaxed);
        assert!(if undo { engine.undo() } else { engine.redo() }.unwrap());
        assert!(engine.nodes.get(id).unwrap().active);
    }
}
