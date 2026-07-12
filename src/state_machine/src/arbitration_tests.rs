use std::time::Duration;

use golden_alchemist::{RuntimeValue, StableRef, ValueTypeId};

use golden_alchemist::ContextKey;

use crate::{CommandIntent, CommandIntentArbiter, CommandPolicy, IntentOrigin, ProcessorId, RateLimitScope};

fn intent(processor: ProcessorId, priority: i32, value: f64) -> CommandIntent {
    lane_intent(processor, None, priority, value)
}

fn lane_intent(processor: ProcessorId, context_key: Option<ContextKey>, priority: i32, value: f64) -> CommandIntent {
    CommandIntent {
        origin: IntentOrigin::processor(processor, context_key),
        target: StableRef::new(ValueTypeId::new("chataigne.command_target"), "target"),
        payload: RuntimeValue::Float(value),
        priority,
        policy: CommandPolicy::HighestPriorityWins,
        logical_tick: 1,
    }
}

#[test]
fn conflicting_processors_resolve_deterministically_with_trace() {
    let low = intent(ProcessorId::new(), 1, 1.0);
    let high = intent(ProcessorId::new(), 10, 2.0);
    let mut arbiter = CommandIntentArbiter::default();

    let result = arbiter.arbitrate(vec![low.clone(), high.clone()]);

    assert_eq!(result.dispatch, vec![high.clone()]);
    assert_eq!(result.decisions[0].winner, Some(high));
    assert_eq!(result.decisions[0].losers, vec![low]);
    assert!(result.decisions[0].explanation.contains("priority"));
}

#[test]
fn duplicate_payload_can_be_suppressed() {
    let mut command = intent(ProcessorId::new(), 0, 1.0);
    command.policy = CommandPolicy::DropIfSameAsPrevious;
    let mut arbiter = CommandIntentArbiter::default();

    assert_eq!(arbiter.arbitrate(vec![command.clone()]).dispatch.len(), 1);
    let second = arbiter.arbitrate(vec![command]);

    assert!(second.dispatch.is_empty());
    assert!(second.decisions[0].winner.is_none());
}

#[test]
fn arbitration_preserves_processor_lane_origin() {
    let processor = ProcessorId::new();
    let lane_a = lane_intent(processor, Some(ContextKey::single("device", "a")), 0, 1.0);
    let lane_b = lane_intent(processor, Some(ContextKey::single("device", "b")), 0, 2.0);
    let mut arbiter = CommandIntentArbiter::default();

    let result = arbiter.arbitrate(vec![lane_b.clone(), lane_a.clone()]);

    assert_eq!(result.dispatch, vec![lane_a.clone()]);
    assert_eq!(result.decisions[0].winner, Some(lane_a));
    assert_eq!(result.decisions[0].losers, vec![lane_b]);
}

#[test]
fn last_writer_wins_is_deterministic_across_lanes() {
    let processor = ProcessorId::new();
    let lane_a = lane_intent(processor, Some(ContextKey::single("device", "a")), 0, 1.0);
    let lane_b = lane_intent(processor, Some(ContextKey::single("device", "b")), 0, 2.0);

    let mut first_arbiter = CommandIntentArbiter::default();
    let first = first_arbiter.arbitrate(vec![lane_a.clone(), lane_b.clone()]);
    let mut second_arbiter = CommandIntentArbiter::default();
    let second = second_arbiter.arbitrate(vec![lane_b, lane_a.clone()]);

    assert_eq!(first.dispatch, second.dispatch);
    assert_eq!(first.dispatch, vec![lane_a]);
}

#[test]
fn queue_policy_keeps_all_lane_intents() {
    let processor = ProcessorId::new();
    let mut lane_a = lane_intent(processor, Some(ContextKey::single("device", "a")), 0, 1.0);
    let mut lane_b = lane_intent(processor, Some(ContextKey::single("device", "b")), 0, 2.0);
    lane_a.policy = CommandPolicy::Queue;
    lane_b.policy = CommandPolicy::Queue;
    let mut arbiter = CommandIntentArbiter::default();

    let result = arbiter.arbitrate(vec![lane_b.clone(), lane_a.clone()]);

    assert_eq!(result.dispatch, vec![lane_a, lane_b]);
    assert!(result.decisions.is_empty());
}

#[test]
fn rate_limit_can_be_target_scoped_or_origin_scoped() {
    let processor = ProcessorId::new();
    let mut lane_a = lane_intent(processor, Some(ContextKey::single("device", "a")), 0, 1.0);
    let mut lane_b = lane_intent(processor, Some(ContextKey::single("device", "b")), 0, 2.0);
    lane_a.policy = CommandPolicy::RateLimit {
        interval: Duration::from_millis(10),
        scope: RateLimitScope::Target,
    };
    lane_b.policy = lane_a.policy.clone();
    lane_b.logical_tick = 2;
    let mut target_arbiter = CommandIntentArbiter::default();

    assert_eq!(target_arbiter.arbitrate(vec![lane_a.clone()]).dispatch.len(), 1);
    assert!(target_arbiter.arbitrate(vec![lane_b.clone()]).dispatch.is_empty());

    lane_a.policy = CommandPolicy::RateLimit {
        interval: Duration::from_millis(10),
        scope: RateLimitScope::Origin,
    };
    lane_b.policy = lane_a.policy.clone();
    let mut origin_arbiter = CommandIntentArbiter::default();

    assert_eq!(origin_arbiter.arbitrate(vec![lane_a]).dispatch.len(), 1);
    assert_eq!(origin_arbiter.arbitrate(vec![lane_b]).dispatch.len(), 1);
}
