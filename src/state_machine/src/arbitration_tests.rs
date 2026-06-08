use golden_alchemist::{RuntimeValue, StableRef, ValueTypeId};

use crate::{CommandIntent, CommandIntentArbiter, CommandPolicy, IntentOrigin, ProcessorId};

fn intent(processor: ProcessorId, priority: i32, value: f64) -> CommandIntent {
    CommandIntent {
        origin: IntentOrigin::Processor(processor),
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
