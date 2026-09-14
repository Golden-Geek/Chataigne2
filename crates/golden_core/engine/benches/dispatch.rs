use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use golden_engine::define_node_enum;
use golden_engine::edit::{Edit, NodeTree};
use golden_engine::engine::Engine;
use golden_engine::events::CustomEvent;
use golden_engine::node::{EventSubscription, Folder, Node, NodeData, NodeId};
use golden_engine::process_ctx::ProcessCtx;

// ── Minimal node types ──────────────────────────────────────────────────────

struct PassiveNode {
    data: NodeData,
}

impl PassiveNode {
    fn new(label: &str) -> Self {
        Self {
            data: NodeData::new(label.to_string()),
        }
    }
}

impl Node for PassiveNode {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "bench_passive"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

struct ListenerNode {
    data: NodeData,
    receive_count: u64,
}

impl ListenerNode {
    fn new(label: &str) -> Self {
        Self {
            data: NodeData::new(label.to_string()),
            receive_count: 0,
        }
    }
}

impl Node for ListenerNode {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "bench_listener"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn on_custom_event(&mut self, _ctx: &mut ProcessCtx, _event: CustomEvent) {
        self.receive_count = black_box(self.receive_count.wrapping_add(1));
    }
}

define_node_enum! {
    enum BenchNode {
        Passive(PassiveNode),
        Listener(ListenerNode),
    }
}

// ── Setup helper ────────────────────────────────────────────────────────────

/// Builds an engine with `total` nodes, `listeners` of which are subscribed to the root.
fn build_dispatch_engine(total: usize, listeners: usize) -> (Engine<BenchNode>, NodeId) {
    let root = Folder::new("root".to_string());
    let mut engine = Engine::new(BenchNode::from(root));
    let root_id = engine.root;

    let trees = (0..(total - listeners))
        .map(|i| NodeTree::new(BenchNode::from(PassiveNode::new(&format!("p{i}")))))
        .chain((0..listeners).map(|i| NodeTree::new(BenchNode::from(ListenerNode::new(&format!("l{i}"))))))
        .collect();
    engine.apply_project_load_node_trees(trees, root_id, None).unwrap();

    let listener_ids: Vec<_> = engine
        .nodes
        .iter()
        .filter_map(|(id, node)| (node.get_type() == "bench_listener").then_some(id))
        .collect();
    assert_eq!(listener_ids.len(), listeners);
    for lid in listener_ids {
        engine.edits.push(Edit::AddEventListener {
            subscriber: lid,
            subscription: EventSubscription::node(root_id),
        });
    }
    engine.apply_project_load_edits().unwrap();

    // Warm up.
    engine.run_tick(Duration::from_millis(5)).unwrap();
    (engine, root_id)
}

// ── Benchmark ───────────────────────────────────────────────────────────────

/// 10 000 nodes, 1 000 listeners subscribed to root, 100 custom events per tick.
fn bench_dispatch_10k_1k_listeners(c: &mut Criterion) {
    let (mut engine, root_id) = build_dispatch_engine(10_000, 1_000);
    let elapsed = Duration::from_millis(5);

    c.bench_function("dispatch_10k_with_1k_listeners", |b| {
        b.iter(|| {
            for _ in 0..100 {
                engine.edits.push(Edit::EmitCustomEvent {
                    event: CustomEvent::new("ping", Some(root_id), serde_json::Value::Null),
                });
            }
            engine.run_tick(black_box(elapsed)).unwrap();
        })
    });
}

criterion_group!(dispatch_benches, bench_dispatch_10k_1k_listeners);
criterion_main!(dispatch_benches);
