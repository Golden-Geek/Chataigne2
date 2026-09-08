use super::*;

use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::events::EventKind;
use crate::node::{Node, NodeData, NodeId, NodeUuid};
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour};
use crate::process_ctx::ProcessCtx;

#[derive(Debug)]
struct TopologyNode {
    node_data: NodeData,
    rule: NodeExecutionRule,
}

impl TopologyNode {
    fn new(label: impl Into<String>, uuid: u128, rule: NodeExecutionRule) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.uuid = NodeUuid(Uuid::from_u128(uuid));
        Self { node_data, rule }
    }
}

impl Node for TopologyNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "topology_test_node"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.rule.clone()
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}

const TOPOLOGY_SPECS: [(&str, u128, &[&str]); 9] = [
    ("a", 10, &[]),
    ("g", 15, &[]),
    ("b", 20, &[]),
    ("h", 25, &["b"]),
    ("c", 30, &["a"]),
    ("d", 40, &["a"]),
    ("i", 45, &["g"]),
    ("e", 50, &["c", "d"]),
    ("f", 60, &["b"]),
];

fn build_topology(insertion_order: &[&str]) -> Engine<TopologyNode> {
    let mut engine = Engine::new(TopologyNode::new("root", 1, NodeExecutionRule::passive()));
    for label in insertion_order {
        let (_, uuid, _) = TOPOLOGY_SPECS
            .iter()
            .find(|(candidate, _, _)| candidate == label)
            .expect("insertion label should have a topology specification");
        engine.add_node(TopologyNode::new(*label, *uuid, NodeExecutionRule::periodic(100)), None);
    }
    engine.apply_edits().expect("topology fixture should attach");

    for (label, _, dependencies) in TOPOLOGY_SPECS {
        let node_id = node_id_by_label(&engine, label);
        let dependencies = dependencies
            .iter()
            .map(|dependency| node_id_by_label(&engine, dependency))
            .collect::<Vec<_>>();
        engine.nodes.get_mut(node_id).expect("topology node should exist").rule =
            NodeExecutionRule::periodic(100).with_dependencies(dependencies);
    }
    engine.resolve().expect("acyclic topology should resolve");
    engine
}

fn node_id_by_label<T: Node>(engine: &Engine<T>, label: &str) -> NodeId {
    engine
        .nodes
        .iter()
        .find_map(|(node_id, node)| (node.node_data().meta.label == label).then_some(node_id))
        .unwrap_or_else(|| panic!("node '{label}' should exist"))
}

fn scheduled_labels<T: Node>(engine: &Engine<T>) -> Vec<String> {
    engine
        .schedule_topology()
        .iter()
        .map(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .expect("scheduled node should exist")
                .node_data()
                .meta
                .label
                .clone()
        })
        .collect()
}

#[test]
fn equivalent_graphs_compile_one_canonical_topology_for_every_ready_tie() {
    const EXPECTED: [&str; 9] = ["a", "g", "b", "h", "c", "d", "i", "e", "f"];
    let insertion_orders = [
        ["a", "g", "b", "h", "c", "d", "i", "e", "f"],
        ["f", "e", "i", "d", "c", "h", "b", "g", "a"],
        ["d", "b", "f", "a", "i", "c", "g", "e", "h"],
    ];

    for insertion_order in insertion_orders {
        let engine = build_topology(&insertion_order);
        assert_eq!(scheduled_labels(&engine), EXPECTED);
        assert_eq!(
            engine
                .schedule_bucket_nodes(100)
                .expect("the shared schedule bucket should exist"),
            engine.schedule_topology(),
            "the live tick bucket should retain the compiled canonical order"
        );
    }
}

#[test]
fn cycle_diagnostics_use_stable_identity_order_across_materialization_orders() {
    let mut cycle_labels = Vec::new();
    for insertion_order in [["x", "y"], ["y", "x"]] {
        let mut engine = Engine::new(TopologyNode::new("root", 1, NodeExecutionRule::passive()));
        for label in insertion_order {
            let uuid = if label == "x" { 80 } else { 90 };
            engine.add_node(TopologyNode::new(label, uuid, NodeExecutionRule::periodic(100)), None);
        }
        engine.apply_edits().expect("cycle fixture should attach");
        let x = node_id_by_label(&engine, "x");
        let y = node_id_by_label(&engine, "y");
        engine.nodes.get_mut(x).expect("x should exist").rule = NodeExecutionRule::periodic(100).with_dependencies([y]);
        engine.nodes.get_mut(y).expect("y should exist").rule = NodeExecutionRule::periodic(100).with_dependencies([x]);

        let EngineRuntimeError::DependencyCycle { nodes } = engine.resolve().expect_err("cycle should be rejected")
        else {
            panic!("cycle fixture should report DependencyCycle");
        };
        cycle_labels.push(
            nodes
                .iter()
                .map(|node_id| {
                    engine
                        .nodes
                        .get(*node_id)
                        .expect("cycle node should exist")
                        .node_data()
                        .meta
                        .label
                        .clone()
                })
                .collect::<Vec<_>>(),
        );
    }

    assert_eq!(cycle_labels, vec![vec!["x", "y"], vec!["x", "y"]]);
}

#[derive(Debug)]
struct EffectObserver {
    node_data: NodeData,
    effects: Vec<String>,
}

impl EffectObserver {
    fn new() -> Self {
        let mut node_data = NodeData::new("root".to_owned());
        node_data.meta.uuid = NodeUuid(Uuid::from_u128(1));
        Self {
            node_data,
            effects: Vec::new(),
        }
    }
}

impl Node for EffectObserver {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "effect_observer"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        for event in &ctx.events {
            if let EventKind::ParamChanged { new_value, .. } = &event.kind {
                match new_value {
                    ParamValue::Int(value) => self.effects.push(format!("numeric:{value}")),
                    ParamValue::Trigger() => self.effects.push("trigger".to_owned()),
                    _ => {}
                }
            }
        }
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}

#[derive(Debug)]
struct EffectWriter {
    node_data: NodeData,
    numeric_param: NodeId,
    trigger_param: NodeId,
    value: i32,
}

impl EffectWriter {
    fn new(label: &str, uuid: u128, numeric_param: NodeId, trigger_param: NodeId, value: i32) -> Self {
        let mut node_data = NodeData::new(label.to_owned());
        node_data.meta.uuid = NodeUuid(Uuid::from_u128(uuid));
        Self {
            node_data,
            numeric_param,
            trigger_param,
            value,
        }
    }
}

impl Node for EffectWriter {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "effect_writer"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(1_000)
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        ctx.set_param_with_behaviour(
            self.numeric_param,
            ParamValue::Int(self.value),
            ParameterEventBehaviour::Append,
        );
        ctx.set_param_with_behaviour(
            self.trigger_param,
            ParamValue::Trigger(),
            ParameterEventBehaviour::Append,
        );
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}

crate::define_node_enum!(
    enum EffectNode {
        Observer(EffectObserver),
        Writer(EffectWriter),
    }
);

fn parameter_with_uuid(label: &str, value: ParamValue, uuid: u128) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::None);
    parameter.node_data_mut().meta.uuid = NodeUuid(Uuid::from_u128(uuid));
    parameter
}

fn run_conflicting_effect_fixture(reverse_parameters: bool, reverse_writers: bool) -> (Vec<String>, Vec<String>, i32) {
    let mut engine: Engine<EffectNode> = Engine::new(EffectObserver::new().into());
    let parameters = if reverse_parameters {
        [
            ("trigger", ParamValue::Trigger(), 1_100),
            ("numeric", ParamValue::Int(0), 1_000),
        ]
    } else {
        [
            ("numeric", ParamValue::Int(0), 1_000),
            ("trigger", ParamValue::Trigger(), 1_100),
        ]
    };
    for (label, value, uuid) in parameters {
        engine.add_node(parameter_with_uuid(label, value, uuid).into(), None);
    }
    engine.apply_edits().expect("effect parameters should attach");
    let numeric_param = node_id_by_label(&engine, "numeric");
    let trigger_param = node_id_by_label(&engine, "trigger");

    let writers = if reverse_writers {
        [("second", 200, 2), ("first", 100, 1)]
    } else {
        [("first", 100, 1), ("second", 200, 2)]
    };
    for (label, uuid, value) in writers {
        engine.add_node(
            EffectWriter::new(label, uuid, numeric_param, trigger_param, value).into(),
            None,
        );
    }
    engine.apply_edits().expect("effect writers should attach");
    engine.resolve().expect("effect schedule should resolve");
    engine.inbox.clear();
    engine
        .run_tick(Duration::from_millis(1))
        .expect("conflicting effects should execute");

    let schedule = scheduled_labels(&engine);
    let effects = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.as_any().downcast_ref::<EffectObserver>())
        .expect("root should remain the effect observer")
        .effects
        .clone();
    let final_value = engine
        .nodes
        .get(numeric_param)
        .and_then(Node::engine_param_snapshot)
        .and_then(|snapshot| match snapshot.value {
            ParamValue::Int(value) => Some(value),
            _ => None,
        })
        .expect("numeric target should retain an integer value");
    (schedule, effects, final_value)
}

#[test]
fn canonical_schedule_preserves_same_target_write_and_trigger_order() {
    let forward = run_conflicting_effect_fixture(false, false);
    let reversed = run_conflicting_effect_fixture(true, true);
    let expected_schedule = vec!["first".to_owned(), "second".to_owned()];
    let expected_effects = vec![
        "numeric:1".to_owned(),
        "trigger".to_owned(),
        "numeric:2".to_owned(),
        "trigger".to_owned(),
    ];

    assert_eq!(forward.0, expected_schedule);
    assert_eq!(reversed.0, expected_schedule);
    assert_eq!(forward.1, expected_effects);
    assert_eq!(reversed.1, expected_effects);
    assert_eq!(forward.2, 2);
    assert_eq!(reversed.2, 2);
}

#[test]
#[ignore = "manual schedule-compilation measurement"]
fn bench_canonical_schedule_resolve_twenty_thousand_nodes() {
    const NODE_COUNT: usize = 20_000;
    const SAMPLE_COUNT: usize = 10;
    let mut engine = Engine::new(TopologyNode::new("root", 1, NodeExecutionRule::passive()));
    for index in 0..NODE_COUNT {
        engine.add_node(
            TopologyNode::new(
                format!("node {index}"),
                index as u128 + 10_000,
                NodeExecutionRule::periodic(200),
            ),
            None,
        );
    }
    engine.apply_edits().expect("benchmark fixture should attach");
    engine.resolve().expect("warm-up resolve should succeed");

    let mut samples_us = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = Instant::now();
        engine.resolve().expect("measured resolve should succeed");
        samples_us.push(started.elapsed().as_micros());
    }

    assert_eq!(engine.schedule_topology().len(), NODE_COUNT);
    println!("schedule_resolve fixture=independent-uuid-ascending-v1 nodes={NODE_COUNT} samples_us={samples_us:?}");
}
