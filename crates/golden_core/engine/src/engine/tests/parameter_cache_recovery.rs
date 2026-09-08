use super::*;

use crate::edit::Edit;
use crate::events::CustomEvent;
use crate::node::{Folder, Node, NodeData, NodeId};
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum InjectedFailure {
    #[default]
    None,
    UpdateEditAbsorption,
    InboxEditAbsorption,
}

#[derive(Debug)]
struct CacheRecoveryProbe {
    node_data: NodeData,
    execution_rule: NodeExecutionRule,
    stable_param: NodeId,
    changed_param: NodeId,
    injected_failure: InjectedFailure,
    queue_accepted_update_edits: bool,
    resolved_stable: Option<ParamValue>,
    resolved_changed: Option<ParamValue>,
    update_observations: Vec<(Option<ParamValue>, Option<ParamValue>)>,
    inbox_observations: Vec<(Option<ParamValue>, Option<ParamValue>)>,
}

impl CacheRecoveryProbe {
    fn new(label: &str, update_rate_hz: Option<u32>, stable_param: NodeId, changed_param: NodeId) -> Self {
        Self {
            node_data: NodeData::new(label.to_owned()),
            execution_rule: update_rate_hz
                .map(NodeExecutionRule::periodic)
                .unwrap_or_else(NodeExecutionRule::passive),
            stable_param,
            changed_param,
            injected_failure: InjectedFailure::None,
            queue_accepted_update_edits: false,
            resolved_stable: None,
            resolved_changed: None,
            update_observations: Vec::new(),
            inbox_observations: Vec::new(),
        }
    }
}

impl Node for CacheRecoveryProbe {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "cache_recovery_probe"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.execution_rule.clone()
    }

    fn engine_sync_bound_param_handles(&mut self, resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {
        self.resolved_stable = resolve(self.stable_param);
        self.resolved_changed = resolve(self.changed_param);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.update_observations
            .push((self.resolved_stable.clone(), self.resolved_changed.clone()));

        if self.queue_accepted_update_edits {
            ctx.set_param_with_behaviour(self.changed_param, ParamValue::Int(7), ParameterEventBehaviour::Append);
            ctx.add_child(self.id(), Folder::new("accepted member"), None);
        }

        if self.injected_failure == InjectedFailure::UpdateEditAbsorption {
            ctx.add_child(self.id(), ForeignNode::new("invalid update member"), None);
        }
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.inbox_observations
            .push((self.resolved_stable.clone(), self.resolved_changed.clone()));

        if self.injected_failure == InjectedFailure::InboxEditAbsorption {
            ctx.add_child(self.id(), ForeignNode::new("invalid inbox member"), None);
        }
    }
}

#[derive(Debug)]
struct ForeignNode {
    node_data: NodeData,
}

impl ForeignNode {
    fn new(label: &str) -> Self {
        Self {
            node_data: NodeData::new(label.to_owned()),
        }
    }
}

impl Node for ForeignNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "foreign_cache_recovery_node"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

crate::define_node_enum!(
    enum CacheRecoveryNode {
        Probe(CacheRecoveryProbe),
    }
);

struct RecoveryFixture {
    engine: Engine<CacheRecoveryNode>,
    stable_param: NodeId,
    changed_param: NodeId,
    probes: Vec<NodeId>,
}

impl RecoveryFixture {
    fn new(probe_count: usize, update_rate_hz: Option<u32>) -> Self {
        let root: CacheRecoveryNode = Folder::new("root").into();
        let mut engine = Engine::new(root);
        engine.add_node(
            Parameter::new("stable", ParamValue::Int(11), ParameterChangeCheck::ValueChange).into(),
            None,
        );
        engine.add_node(
            Parameter::new("changed", ParamValue::Int(3), ParameterChangeCheck::ValueChange).into(),
            None,
        );
        engine.apply_edits().expect("fixture parameters should attach");

        let stable_param = first_child(&engine, engine.root);
        let changed_param = next_sibling(&engine, stable_param);
        for index in 0..probe_count {
            engine.add_node(
                CacheRecoveryProbe::new(&format!("probe {index}"), update_rate_hz, stable_param, changed_param).into(),
                None,
            );
        }
        engine.apply_edits().expect("fixture probes should attach");
        engine.resolve().expect("fixture schedule should resolve");

        let mut probes = Vec::with_capacity(probe_count);
        let mut current = engine
            .nodes
            .get(changed_param)
            .and_then(|node| node.node_data().next_sibling);
        while let Some(node_id) = current {
            probes.push(node_id);
            current = engine.nodes.get(node_id).and_then(|node| node.node_data().next_sibling);
        }
        assert_eq!(probes.len(), probe_count);

        Self {
            engine,
            stable_param,
            changed_param,
            probes,
        }
    }

    fn probe(&self, node: NodeId) -> &CacheRecoveryProbe {
        self.engine
            .nodes
            .get(node)
            .and_then(|node| node.as_any().downcast_ref())
            .expect("probe should retain its concrete type")
    }

    fn probe_mut(&mut self, node: NodeId) -> &mut CacheRecoveryProbe {
        self.engine
            .nodes
            .get_mut(node)
            .and_then(|node| node.as_any_mut().downcast_mut())
            .expect("probe should retain its concrete type")
    }

    fn assert_values_and_cache(&self, changed: i32) {
        assert_eq!(parameter_value(&self.engine, self.stable_param), ParamValue::Int(11));
        assert_eq!(
            parameter_value(&self.engine, self.changed_param),
            ParamValue::Int(changed)
        );
        assert_eq!(
            self.engine.parameter_values_cache.get(&self.stable_param),
            Some(&ParamValue::Int(11))
        );
        assert_eq!(
            self.engine.parameter_values_cache.get(&self.changed_param),
            Some(&ParamValue::Int(changed))
        );
        assert_eq!(
            self.engine.parameter_values_cache.len(),
            2,
            "only the two parameter nodes should occupy the cache"
        );
    }
}

fn first_child<T: Node>(engine: &Engine<T>, parent: NodeId) -> NodeId {
    engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child)
        .expect("parent should have a child")
}

fn next_sibling<T: Node>(engine: &Engine<T>, node: NodeId) -> NodeId {
    engine
        .nodes
        .get(node)
        .and_then(|node| node.node_data().next_sibling)
        .expect("node should have a next sibling")
}

fn child_count<T: Node>(engine: &Engine<T>, parent: NodeId) -> usize {
    let mut count = 0;
    let mut current = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(node_id) = current {
        count += 1;
        current = engine.nodes.get(node_id).and_then(|node| node.node_data().next_sibling);
    }
    count
}

fn parameter_value<T: Node>(engine: &Engine<T>, node: NodeId) -> ParamValue {
    engine
        .nodes
        .get(node)
        .and_then(Node::engine_param_snapshot)
        .map(|snapshot| snapshot.value)
        .expect("node should be a parameter")
}

fn assert_last_observation(observations: &[(Option<ParamValue>, Option<ParamValue>)], changed: i32) {
    assert_eq!(
        observations.last(),
        Some(&(Some(ParamValue::Int(11)), Some(ParamValue::Int(changed))))
    );
}

#[test]
fn scheduled_callback_budget_rejects_excess_work_and_preserves_accepted_edits() {
    let mut fixture = RecoveryFixture::new(2, Some(1_000));
    let probes = fixture.probes.clone();
    for probe in &probes {
        fixture.probe_mut(*probe).queue_accepted_update_edits = true;
    }
    fixture.engine.set_runtime_limits(RuntimeLimits {
        max_update_callbacks_per_tick: 1,
        ..RuntimeLimits::default()
    });

    let result = fixture.engine.run_tick(Duration::from_millis(1));
    assert!(matches!(
        result,
        Err(EngineRuntimeError::UpdateBudgetExceeded {
            callbacks: 2,
            limit: 1,
            ..
        })
    ));
    assert_eq!(
        probes
            .iter()
            .map(|probe| fixture.probe(*probe).update_observations.len())
            .sum::<usize>(),
        1,
        "the callback beyond the limit must not execute"
    );
    fixture.assert_values_and_cache(3);
    assert_eq!(
        fixture.engine.edits.pending.len(),
        2,
        "the accepted callback's edits should remain queued"
    );

    for probe in &probes {
        fixture.probe_mut(*probe).queue_accepted_update_edits = false;
    }
    fixture.engine.set_runtime_limits(RuntimeLimits {
        max_update_callbacks_per_tick: 8,
        ..RuntimeLimits::default()
    });
    fixture
        .engine
        .run_tick(Duration::from_millis(1))
        .expect("the next tick should apply accepted edits and continue");

    fixture.assert_values_and_cache(7);
    assert_eq!(
        probes
            .iter()
            .map(|probe| child_count(&fixture.engine, *probe))
            .sum::<usize>(),
        1,
        "only the callback admitted before the budget failure should add a member"
    );
    for probe in probes {
        assert_last_observation(&fixture.probe(probe).update_observations, 7);
    }
}

#[test]
fn scheduled_edit_absorption_failure_restores_cache_for_the_next_tick() {
    let mut fixture = RecoveryFixture::new(1, Some(1_000));
    let probe = fixture.probes[0];
    fixture.probe_mut(probe).injected_failure = InjectedFailure::UpdateEditAbsorption;

    let result = fixture.engine.run_tick(Duration::from_millis(1));
    assert!(matches!(
        result,
        Err(EngineRuntimeError::Edit(EngineEditError::NodeTypeMismatch {
            operation: "AddNode",
            ..
        }))
    ));
    fixture.assert_values_and_cache(3);
    assert_eq!(child_count(&fixture.engine, probe), 0);
    assert!(fixture.engine.edits.pending.is_empty());

    fixture.probe_mut(probe).injected_failure = InjectedFailure::None;
    fixture.engine.edits.push(Edit::SetParam {
        node: fixture.changed_param,
        value: ParamValue::Int(9),
        behaviour: ParameterEventBehaviour::Append,
    });
    fixture
        .engine
        .run_tick(Duration::from_millis(1))
        .expect("the next tick should apply new work after the rejected callback edit");

    fixture.assert_values_and_cache(9);
    assert_eq!(child_count(&fixture.engine, probe), 0);
    assert_eq!(fixture.probe(probe).update_observations.len(), 2);
    assert_last_observation(&fixture.probe(probe).update_observations, 9);
}

#[test]
fn inbox_edit_absorption_failure_restores_cache_for_the_next_tick() {
    let mut fixture = RecoveryFixture::new(1, None);
    let probe = fixture.probes[0];
    fixture.engine.inbox.clear();
    fixture.probe_mut(probe).injected_failure = InjectedFailure::InboxEditAbsorption;
    fixture.engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("cache.recovery", Some(probe), serde_json::Value::Null),
    });
    fixture.engine.apply_edits().expect("custom event should be emitted");

    let result = fixture.engine.dispatch_inbox(ExecutionPhase::EngineTick);
    assert!(matches!(
        result,
        Err(EngineEditError::NodeTypeMismatch {
            operation: "AddNode",
            ..
        })
    ));
    fixture.assert_values_and_cache(3);
    assert_eq!(child_count(&fixture.engine, probe), 0);
    assert_eq!(fixture.probe(probe).inbox_observations.len(), 1);

    fixture.probe_mut(probe).injected_failure = InjectedFailure::None;
    fixture.engine.edits.push(Edit::SetParam {
        node: fixture.changed_param,
        value: ParamValue::Int(13),
        behaviour: ParameterEventBehaviour::Append,
    });
    fixture
        .engine
        .run_tick(Duration::from_millis(1))
        .expect("the next tick should dispatch retained events and continue");

    fixture.assert_values_and_cache(13);
    assert_eq!(child_count(&fixture.engine, probe), 0);
    assert_eq!(fixture.probe(probe).inbox_observations.len(), 2);
    assert_last_observation(&fixture.probe(probe).inbox_observations, 13);
    assert!(fixture.engine.inbox.events.is_empty());
}
