use super::*;
use crate::{
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
};
use golden_io::{PendingReceiver, PendingSender, bounded_pending_channel};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy)]
enum WakeFailure {
    None,
    Ingress,
    Apply,
}

#[crate::node("worker_wake_probe")]
struct WorkerWakeProbe {
    input: PendingReceiver<ParamValue>,
    target: NodeId,
    calls: Arc<AtomicUsize>,
    rearm: Option<NodeWaker>,
    keep_rearming: bool,
    failure: WakeFailure,
    scheduled: bool,
    create_tree: bool,
}

#[crate::node("worker_wake_probe", from_struct)]
impl Node for WorkerWakeProbe {
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        assert_eq!(ctx.delta_time, Duration::ZERO);
        self.calls.fetch_add(1, Ordering::Relaxed);
        match self.failure {
            WakeFailure::Ingress => {
                ctx.add_child(self.id(), ForeignWakeProbe::new(), None);
                return;
            }
            WakeFailure::Apply => {
                ctx.set_param(NodeId(u64::MAX), ParamValue::Int(1));
                return;
            }
            WakeFailure::None => {}
        }
        let mut values = Vec::new();
        self.input.drain_into(&mut values, NonZeroUsize::new(8).unwrap());
        for value in values {
            ctx.set_param_with_behaviour(self.target, value, ParameterEventBehaviour::Append);
        }
        if let Some(waker) = self.rearm.take() {
            waker.wake().unwrap();
            waker.wake().unwrap();
            if self.keep_rearming {
                self.rearm = Some(waker);
            }
        }
        if std::mem::take(&mut self.create_tree) {
            ctx.add_child_tree(
                self.id(),
                NodeTree::new(Folder::new("worker tree")).with_child(NodeTree::new(Folder::new("child"))),
                None,
            );
        }
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        assert_eq!(
            ctx.delta_time,
            Duration::from_millis(1),
            "worker wakes cannot advance the scheduled clock"
        );
        self.calls.fetch_add(1, Ordering::Relaxed);
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        if self.scheduled {
            NodeExecutionRule::periodic(1000)
        } else {
            NodeExecutionRule::passive()
        }
    }

    fn needs_update(&self) -> bool {
        self.scheduled
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}

#[crate::node("foreign_wake_probe")]
struct ForeignWakeProbe {}
#[crate::node("foreign_wake_probe", from_struct)]
impl Node for ForeignWakeProbe {}

crate::define_node_enum!(
    enum WakeNode {
        WorkerWakeProbe,
    }
);

fn add_probe(
    engine: &mut Engine<WakeNode>,
    target: NodeId,
    scheduled: bool,
) -> (NodeId, PendingSender<ParamValue>, Arc<AtomicUsize>) {
    let (sender, receiver) = bounded_pending_channel(8, 8);
    let calls = Arc::new(AtomicUsize::new(0));
    engine.add_node(
        WorkerWakeProbe::new(
            receiver,
            target,
            calls.clone(),
            None,
            false,
            WakeFailure::None,
            scheduled,
            false,
        )
        .into(),
        None,
    );
    engine.apply_edits().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().last_child.unwrap();
    (id, sender, calls)
}

fn fixture() -> (
    Engine<WakeNode>,
    NodeId,
    NodeId,
    PendingSender<ParamValue>,
    Arc<AtomicUsize>,
) {
    fixture_with_value(ParamValue::Int(0))
}

fn fixture_with_value(
    initial: ParamValue,
) -> (
    Engine<WakeNode>,
    NodeId,
    NodeId,
    PendingSender<ParamValue>,
    Arc<AtomicUsize>,
) {
    let mut engine = Engine::new(WakeNode::from(Folder::new("root")));
    engine.add_node(
        Parameter::new("target", initial, ParameterChangeCheck::None).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let target = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let (probe, sender, calls) = add_probe(&mut engine, target, false);
    (engine, probe, target, sender, calls)
}

fn probe(engine: &mut Engine<WakeNode>, id: NodeId) -> &mut WorkerWakeProbe {
    let WakeNode::WorkerWakeProbe(probe) = engine.nodes.get_mut(id).unwrap() else {
        panic!("worker probe")
    };
    probe
}

#[test]
fn worker_wakes_coalesce_but_ordered_mailbox_values_retain_their_occurrences() {
    for pulses in [false, true] {
        let initial = if pulses {
            ParamValue::Trigger()
        } else {
            ParamValue::Int(0)
        };
        let (mut engine, id, target, sender, calls) = fixture_with_value(initial);
        let waker = engine.node_waker(id).unwrap();
        assert_eq!(waker, engine.node_waker(id).unwrap(), "reuse the reservation");
        std::thread::spawn(move || {
            for value in [11, 22, 33] {
                assert!(
                    sender
                        .send(if pulses {
                            ParamValue::Trigger()
                        } else {
                            ParamValue::Int(value)
                        })
                        .is_ok()
                );
                waker.wake().unwrap();
            }
        })
        .join()
        .unwrap();
        engine.run_tick(Duration::from_millis(1)).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            engine.nodes.get(target).unwrap().engine_param_snapshot().unwrap().value,
            if pulses {
                ParamValue::Trigger()
            } else {
                ParamValue::Int(33)
            }
        );
        let changes = engine
            .ui_event_log()
            .iter()
            .filter(
                |event| matches!(event.kind, crate::events::EventKind::ParamChanged { param, .. } if param == target),
            )
            .count();
        assert_eq!(
            changes,
            if pulses { 3 } else { 1 },
            "UI retains latest scalar state and every trigger edge"
        );
        assert_eq!(engine.undo_len(), 2, "runtime inputs add no undo entry");
        engine.run_tick(Duration::from_millis(1)).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1, "idle nodes do not poll");
    }
}

#[test]
fn worker_signal_during_callback_is_retained_for_the_next_tick() {
    let (mut engine, id, _, _, calls) = fixture();
    let waker = engine.node_waker(id).unwrap();
    probe(&mut engine, id).rearm = Some(waker.clone());
    waker.wake().unwrap();
    for expected in [1, 2, 2] {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), expected);
    }
}

#[test]
fn rejected_worker_callback_returns_itself_and_unprocessed_wakes_to_the_queue() {
    for ingress in [true, false] {
        let (mut engine, first, target, sender, calls) = fixture();
        let (second, _, second_calls) = add_probe(&mut engine, target, false);
        let first_wake = engine.node_waker(first).unwrap();
        let second_wake = engine.node_waker(second).unwrap();
        assert!(sender.send(ParamValue::Int(42)).is_ok());
        probe(&mut engine, first).failure = if ingress {
            WakeFailure::Ingress
        } else {
            WakeFailure::Apply
        };
        first_wake.wake().unwrap();
        second_wake.wake().unwrap();
        assert!(engine.run_tick(Duration::from_millis(1)).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(second_calls.load(Ordering::Relaxed), usize::from(!ingress));
        probe(&mut engine, first).failure = WakeFailure::None;
        engine.run_tick(Duration::from_millis(1)).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(second_calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            engine.nodes.get(target).unwrap().engine_param_snapshot().unwrap().value,
            ParamValue::Int(42)
        );
    }
}

#[test]
fn continuously_rearmed_worker_has_bounded_turns_without_starving_scheduled_work() {
    let (mut engine, id, target, _, wake_calls) = fixture();
    let (scheduled, _, scheduled_calls) = add_probe(&mut engine, target, true);
    let scheduled_wake = engine.node_waker(scheduled).unwrap();
    let waker = engine.node_waker(id).unwrap();
    probe(&mut engine, id).rearm = Some(waker.clone());
    probe(&mut engine, id).keep_rearming = true;
    engine.set_runtime_limits(RuntimeLimits {
        max_wake_callbacks_per_tick: 2,
        max_update_callbacks_per_tick: 1,
        ..Default::default()
    });
    waker.wake().unwrap();
    for _ in 0..10 {
        scheduled_wake.wake().unwrap();
        engine.run_tick(Duration::from_millis(1)).unwrap();
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 2);
        assert_eq!(engine.tick_stats().callbacks_fired, 1);
    }
    assert_eq!(wake_calls.load(Ordering::Relaxed), 10);
    assert_eq!(scheduled_calls.load(Ordering::Relaxed), 20);
}

#[test]
fn node_removal_and_replacement_close_old_sources_even_when_identity_is_restored() {
    for replace in [false, true] {
        let (mut engine, id, _, _, _) = fixture();
        let old = engine.node_waker(id).unwrap();
        old.wake().unwrap();
        engine.edits.push(if replace {
            Edit::ReplaceNode {
                node: id,
                new_node: Box::new(Folder::new("replacement")),
            }
        } else {
            Edit::RemoveNode { node: id }
        });
        engine.apply_edits().unwrap();
        assert_eq!(old.wake(), Err(NodeWakeError::Closed));
        assert!(engine.undo().unwrap());
        let restored = engine.node_waker(id).unwrap();
        assert_ne!(old, restored);
        restored.wake().unwrap();
        engine.run_tick(Duration::from_millis(1)).unwrap();
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 1);
    }
}

#[test]
fn wake_registration_capacity_reclaims_dropped_sources_and_engine_drop_closes_queued_handles() {
    let mut engine = Engine::new(Folder::new("root"));
    let mut tree = NodeTree::new(Folder::new("workers"));
    for _ in 0..1025 {
        tree.push_child(NodeTree::new(Folder::new("worker")));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: engine.root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let children = snapshot.child_ids(snapshot.child_ids(engine.root)[0]);
    let mut handles = children[..1024]
        .iter()
        .map(|id| engine.node_waker(*id).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(engine.node_waker(children[1024]).unwrap_err(), NodeWakeError::Capacity);
    assert_eq!(engine.node_waker(children[0]).unwrap(), handles[0]);
    handles.pop();
    let recovered = engine.node_waker(children[1024]).unwrap();
    recovered.wake().unwrap();
    drop(engine);
    assert_eq!(recovered.wake(), Err(NodeWakeError::Closed));
    assert_eq!(handles[0].wake(), Err(NodeWakeError::Closed));
}

#[test]
fn worker_callback_owns_structural_edits_and_zero_budget_retains_accepted_signals() {
    let (mut engine, id, _, _, calls) = fixture();
    probe(&mut engine, id).create_tree = true;
    let waker = engine.node_waker(id).unwrap();
    engine.set_runtime_limits(RuntimeLimits {
        max_wake_callbacks_per_tick: 0,
        ..Default::default()
    });
    waker.wake().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    engine.set_runtime_limits(Default::default());
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let snapshot = engine.process_tree_snapshot();
    let tree = snapshot.child_ids(id)[0];
    assert_eq!(snapshot.child_ids(tree).len(), 1);
    assert!(
        ProcessCtx::new(ExecutionPhase::EngineTick, engine.time)
            .node_waker()
            .is_err()
    );
}
