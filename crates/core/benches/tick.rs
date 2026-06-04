use std::time::Duration;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use golden_engine::define_node_enum;
use golden_engine::engine::{Engine, NodeExecutionRule};
use golden_engine::node::{Folder, Node, NodeData};
use golden_engine::process_ctx::ProcessCtx;

// ── Minimal benchmark node types ───────────────────────────────────────────

/// Passive node with no update rate — represents the bulk of the graph.
struct PassiveNode {
    data: NodeData,
}

impl PassiveNode {
    fn new(label: &str) -> Self {
        Self { data: NodeData::new(label.to_string()) }
    }
}

impl Node for PassiveNode {
    fn node_data(&self) -> &NodeData { &self.data }
    fn node_data_mut(&mut self) -> &mut NodeData { &mut self.data }
    fn get_type(&self) -> &str { "bench_passive" }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

/// Active node running at 200 Hz — represents a sparse set of updating nodes.
struct ActiveNode {
    data: NodeData,
    counter: u64,
}

impl ActiveNode {
    fn new(label: &str) -> Self {
        Self { data: NodeData::new(label.to_string()), counter: 0 }
    }
}

impl Node for ActiveNode {
    fn node_data(&self) -> &NodeData { &self.data }
    fn node_data_mut(&mut self) -> &mut NodeData { &mut self.data }
    fn get_type(&self) -> &str { "bench_active" }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn execution_rule(&self) -> NodeExecutionRule { NodeExecutionRule::periodic(200) }
    fn needs_update(&self) -> bool { true }
    fn update(&mut self, _ctx: &mut ProcessCtx) {
        self.counter = black_box(self.counter.wrapping_add(1));
    }
}

define_node_enum! {
    enum BenchNode {
        Passive(PassiveNode),
        Active(ActiveNode),
    }
}

// ── Setup helpers ───────────────────────────────────────────────────────────

fn build_passive_engine(node_count: usize) -> Engine<BenchNode> {
    let root = Folder::new("root".to_string());
    let mut engine = Engine::new(BenchNode::from(root));
    for i in 0..node_count {
        engine.add_node(BenchNode::from(PassiveNode::new(&format!("n{i}"))), None);
    }
    engine.apply_edits().unwrap();
    // Warm up: run one tick so the scheduler is resolved and all internal caches are hot.
    engine.run_tick(Duration::from_millis(5)).unwrap();
    engine
}

fn build_sparse_active_engine(total: usize, active: usize) -> Engine<BenchNode> {
    let root = Folder::new("root".to_string());
    let mut engine = Engine::new(BenchNode::from(root));
    for i in 0..active {
        engine.add_node(BenchNode::from(ActiveNode::new(&format!("a{i}"))), None);
    }
    for i in 0..(total - active) {
        engine.add_node(BenchNode::from(PassiveNode::new(&format!("p{i}"))), None);
    }
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(5)).unwrap();
    engine
}

// ── Benchmarks ──────────────────────────────────────────────────────────────

/// 20 000 passive nodes, no due buckets.
/// Measures the pure tick-loop overhead when nothing is scheduled.
fn bench_tick_20k_passive(c: &mut Criterion) {
    let mut engine = build_passive_engine(20_000);
    let elapsed = Duration::from_millis(5);

    c.bench_function("tick_20k_passive", |b| {
        b.iter(|| engine.run_tick(black_box(elapsed)).unwrap())
    });
}

/// 20 000 nodes, 200 active at 200 Hz.
/// Simulates a realistic sparse-update graph: 1% of nodes fire per tick.
fn bench_tick_20k_sparse_active(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick_20k_sparse_active");

    for active_count in [200usize] {
        let mut engine = build_sparse_active_engine(20_000, active_count);
        let elapsed = Duration::from_millis(5); // 200 Hz step

        group.bench_with_input(
            BenchmarkId::from_parameter(active_count),
            &active_count,
            |b, _| b.iter(|| engine.run_tick(black_box(elapsed)).unwrap()),
        );
    }
    group.finish();
}

criterion_group!(tick_benches, bench_tick_20k_passive, bench_tick_20k_sparse_active);
criterion_main!(tick_benches);
