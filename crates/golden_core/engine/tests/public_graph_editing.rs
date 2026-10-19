use golden_application::GraphEditing;
use golden_engine::app::ProjectLifecycle;
use golden_engine::application::ProductionRuntime;
use golden_engine::define_node_enum;
use golden_engine::engine::Engine;
use golden_engine::node::{Folder, Node, NodeData};
use golden_engine::ui_sync::{UiEditIntent, UiProjectFileSpec};

define_node_enum!(
    enum ExternalConsumerNode {}
);

impl ProjectLifecycle for ExternalConsumerNode {}

fn apply_through_public_contract<G>(graph: &G, edit: G::Edit) -> Result<G::Revision, G::Error>
where
    G: GraphEditing,
{
    graph.apply_graph_edit(edit)
}

#[test]
fn dependency_consumer_observes_graph_edit_rejection_details() {
    let engine: Engine<ExternalConsumerNode> = Engine::new(Folder::new("Root").into());
    let root = engine.root;
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(ExternalConsumerNode::project_file_spec(), None),
    )
    .unwrap();

    let error = apply_through_public_contract(&runtime, UiEditIntent::RemoveNode { node: root })
        .expect_err("the dependency-facing contract must not report rejected edits as success");

    assert_eq!(error.graph_rejection().unwrap().code(), Some("cannot_mutate_root"));
    assert!(
        error
            .graph_rejection()
            .unwrap()
            .message()
            .is_some_and(|message| message.contains("root"))
    );
    assert_eq!(error.graph_rejection().unwrap().history().current_history_state_id, 0);
    assert_eq!(
        runtime.history_state().unwrap(),
        *error.graph_rejection().unwrap().history()
    );
}
#[test]
fn worker_wake_service_has_a_per_tick_bound_and_retains_its_suffix() {
    use golden_engine::edit::{Edit, NodeTree};
    use std::time::Duration;
    let mut engine = Engine::new(Folder::new("root"));
    let mut forest = NodeTree::new(Folder::new("workers"));
    for _ in 0..512 {
        forest.push_child(NodeTree::new(Folder::new("worker")));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree: forest,
        parent: engine.root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let forest = snapshot.child_ids(engine.root)[0];
    let wakers = snapshot
        .child_ids(forest)
        .iter()
        .map(|id| engine.node_waker(*id).unwrap())
        .collect::<Vec<_>>();
    for waker in &wakers {
        waker.wake().unwrap();
    }
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 256);
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 256);
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
}

struct ExternalWorkerWakeProbe {
    data: NodeData,
    calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    rendezvous: Option<WakeRendezvous>,
}

struct WakeRendezvous {
    entered: std::sync::mpsc::SyncSender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

impl ExternalWorkerWakeProbe {
    fn new(calls: std::sync::Arc<std::sync::atomic::AtomicUsize>, rendezvous: Option<WakeRendezvous>) -> Self {
        Self {
            data: NodeData::new("external worker".into()),
            calls,
            rendezvous,
        }
    }
}

impl Node for ExternalWorkerWakeProbe {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "external_worker_wake_probe"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_worker_wake(&mut self, ctx: &mut golden_engine::process_ctx::ProcessCtx) {
        assert_eq!(ctx.delta_time, std::time::Duration::ZERO);
        self.calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(rendezvous) = self.rendezvous.take() {
            rendezvous.entered.send(()).unwrap();
            rendezvous
                .release
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap();
        }
    }
}

#[test]
fn a_real_producer_waking_during_service_cannot_be_cleared_by_its_acknowledgement() {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        time::Duration,
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let (entered, entered_rx) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel(1);
    let mut engine = Engine::new(ExternalWorkerWakeProbe::new(
        calls.clone(),
        Some(WakeRendezvous {
            entered,
            release: release_rx,
        }),
    ));
    let source = engine.node_waker(engine.root).unwrap();
    source.wake().unwrap();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            source.wake().unwrap();
            release.send(()).unwrap();
        });
        engine.run_tick(Duration::from_millis(1)).unwrap();
    });
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}
