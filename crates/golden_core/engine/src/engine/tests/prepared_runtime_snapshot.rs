use std::{any::Any, time::Duration};

use crate::{
    app::prepare_engine_for_runtime,
    engine::{Engine, NodeExecutionRule},
    events::{CustomEvent, EventKind},
    node::{Node, NodeData},
    process_ctx::ProcessCtx,
};

struct SnapshotUpdateNode {
    data: NodeData,
    rule: NodeExecutionRule,
}

impl SnapshotUpdateNode {
    fn new(label: &str, rule: NodeExecutionRule) -> Self {
        Self {
            data: NodeData::new(label.to_owned()),
            rule,
        }
    }
}

impl Node for SnapshotUpdateNode {
    fn node_data(&self) -> &NodeData {
        &self.data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }

    fn get_type(&self) -> &str {
        "snapshot_update_test"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.rule.clone()
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let snapshot = ctx.tree_snapshot().expect("scheduled update should receive a snapshot");
        if self.data.meta.label == "later" {
            assert!(snapshot.find_child(snapshot.root(), "later").is_some());
        }
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}

fn prepared_engine() -> Engine<SnapshotUpdateNode> {
    let mut engine = Engine::new(SnapshotUpdateNode::new("root", NodeExecutionRule::passive()));
    engine.add_node(
        SnapshotUpdateNode::new("periodic", NodeExecutionRule::periodic(125)),
        None,
    );
    prepare_engine_for_runtime(&mut engine).expect("engine should prepare");
    engine
}

#[test]
fn first_runtime_tick_reuses_snapshot_from_preparation() {
    let mut engine = prepared_engine();
    assert!(engine.prepared_first_tick_snapshot.is_some());

    engine.run_tick(Duration::from_millis(8)).expect("tick should run");

    let stats = engine.tick_stats();
    assert_eq!(stats.callbacks_fired, 1);
    assert_eq!(stats.snapshot_builds, 0);
    assert!(engine.prepared_first_tick_snapshot.is_none());
}

#[test]
fn passive_snapshot_capable_node_does_not_trigger_preparation() {
    let mut engine = Engine::new(SnapshotUpdateNode::new("root", NodeExecutionRule::passive()));
    prepare_engine_for_runtime(&mut engine).expect("passive engine should prepare");
    assert!(engine.prepared_first_tick_snapshot.is_none());
}

#[test]
fn applied_edits_invalidate_prepared_snapshot() {
    let mut engine = prepared_engine();
    engine.add_node(SnapshotUpdateNode::new("later", NodeExecutionRule::periodic(125)), None);
    engine.apply_edits().expect("edit should apply");
    assert!(engine.prepared_first_tick_snapshot.is_none());

    engine.run_tick(Duration::from_millis(8)).expect("tick should run");
    assert!(engine.tick_stats().snapshot_builds >= 1);
}

#[test]
fn pending_edits_prevent_first_tick_snapshot_reuse() {
    let mut engine = prepared_engine();
    engine.add_node(SnapshotUpdateNode::new("later", NodeExecutionRule::periodic(125)), None);

    engine.run_tick(Duration::from_millis(8)).expect("tick should run");
    assert!(engine.tick_stats().snapshot_builds >= 1);
}

#[test]
fn custom_events_do_not_rebuild_the_prepared_control_index() {
    let mut engine = prepared_engine();
    engine
        .run_tick(Duration::from_millis(8))
        .expect("first tick should run");
    assert_eq!(engine.tick_stats().control_index_rebuilds, 0);

    engine.emit_event(EventKind::Custom(CustomEvent::new(
        "signal",
        None,
        serde_json::Value::Null,
    )));
    engine
        .run_tick(Duration::from_millis(8))
        .expect("signal tick should run");
    assert_eq!(engine.tick_stats().control_index_rebuilds, 0);
}
