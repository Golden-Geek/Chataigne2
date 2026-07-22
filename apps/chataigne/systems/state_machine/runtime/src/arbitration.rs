use std::{cmp::Ordering, collections::HashMap, time::Duration};

use chataigne_alchemist::{ContextKey, RuntimeIntent, StableRef};
use chataigne_state_machine_model::TransitionId;
use golden_values::Value as RuntimeValue;

use crate::ProcessorId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IntentOrigin {
    Processor {
        processor_id: ProcessorId,
        context_key: Option<Box<ContextKey>>,
    },
    Transition {
        transition_id: TransitionId,
    },
    System,
}

impl IntentOrigin {
    #[must_use]
    pub fn processor(processor_id: ProcessorId, context_key: Option<ContextKey>) -> Self {
        Self::Processor {
            processor_id,
            context_key: context_key.map(Box::new),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlendPolicy {
    Average,
    Add,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateLimitScope {
    Target,
    Origin,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CommandPolicy {
    FireAndForget,
    LastWriterWins,
    HighestPriorityWins,
    Queue,
    DropIfSameAsPrevious,
    RateLimit { interval: Duration, scope: RateLimitScope },
    Blend(BlendPolicy),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandIntent {
    pub origin: IntentOrigin,
    pub target: StableRef,
    pub payload: RuntimeValue,
    pub priority: i32,
    pub policy: CommandPolicy,
    pub logical_tick: u64,
}

impl CommandIntent {
    #[must_use]
    pub fn from_runtime(intent: RuntimeIntent, origin: IntentOrigin) -> Option<Self> {
        Self::from_runtime_with_policy(intent, origin, 0, CommandPolicy::LastWriterWins)
    }

    #[must_use]
    pub fn from_runtime_with_policy(
        intent: RuntimeIntent,
        origin: IntentOrigin,
        priority: i32,
        policy: CommandPolicy,
    ) -> Option<Self> {
        if intent.kind.as_ref() != "chataigne.command" {
            return None;
        }
        Some(Self {
            origin,
            target: intent.target?,
            payload: intent.payload,
            priority,
            policy,
            logical_tick: intent.logical_tick,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArbitrationDecision {
    pub target: StableRef,
    pub winner: Option<CommandIntent>,
    pub losers: Vec<CommandIntent>,
    pub explanation: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArbitrationResult {
    pub dispatch: Vec<CommandIntent>,
    pub decisions: Vec<ArbitrationDecision>,
}

#[derive(Default)]
pub struct CommandIntentArbiter {
    previous_payloads: HashMap<StableRef, RuntimeValue>,
    last_dispatch_tick: HashMap<RateLimitKey, u64>,
}

impl CommandIntentArbiter {
    pub fn arbitrate(&mut self, intents: Vec<CommandIntent>) -> ArbitrationResult {
        let mut groups = Vec::<(StableRef, Vec<(usize, CommandIntent)>)>::new();
        for (index, intent) in intents.into_iter().enumerate() {
            if let Some((_, group)) = groups.iter_mut().find(|(target, _)| *target == intent.target) {
                group.push((index, intent));
            } else {
                groups.push((intent.target.clone(), vec![(index, intent)]));
            }
        }
        let mut result = ArbitrationResult::default();
        for (target, mut group) in groups {
            if group
                .iter()
                .all(|(_, intent)| matches!(intent.policy, CommandPolicy::Queue))
            {
                group.sort_by(command_intent_order);
                for (_, intent) in group {
                    self.record_dispatch(&intent);
                    result.dispatch.push(intent);
                }
                continue;
            }
            group.sort_by(command_intent_order);
            let (_, candidate) = group.remove(0);
            let mut losers: Vec<CommandIntent> = group.into_iter().map(|(_, intent)| intent).collect();
            let suppression = self.suppression_reason(&candidate);
            let winner = if suppression.is_none() {
                self.record_dispatch(&candidate);
                result.dispatch.push(candidate.clone());
                Some(candidate)
            } else {
                losers.push(candidate);
                None
            };
            let explanation = suppression.unwrap_or_else(|| {
                format!(
                    "selected by priority, origin, logical tick, then stable input order; {} competing intent(s) lost",
                    losers.len()
                )
            });
            result.decisions.push(ArbitrationDecision {
                target,
                winner,
                losers,
                explanation,
            });
        }
        result
    }

    fn suppression_reason(&self, intent: &CommandIntent) -> Option<String> {
        match intent.policy {
            CommandPolicy::DropIfSameAsPrevious
                if self.previous_payloads.get(&intent.target) == Some(&intent.payload) =>
            {
                Some("dropped because payload matches the previous dispatch".into())
            }
            CommandPolicy::RateLimit { interval, scope } => {
                let minimum_ticks = interval.as_millis().min(u128::from(u64::MAX)) as u64;
                let key = RateLimitKey::from_intent(intent, scope);
                self.last_dispatch_tick.get(&key).and_then(|last| {
                    (intent.logical_tick.saturating_sub(*last) < minimum_ticks)
                        .then(|| format!("dropped by {scope:?} rate limit"))
                })
            }
            _ => None,
        }
    }

    fn record_dispatch(&mut self, intent: &CommandIntent) {
        self.previous_payloads
            .insert(intent.target.clone(), intent.payload.clone());
        self.last_dispatch_tick
            .insert(RateLimitKey::Target(intent.target.clone()), intent.logical_tick);
        self.last_dispatch_tick
            .insert(RateLimitKey::Origin(intent.origin.clone()), intent.logical_tick);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum RateLimitKey {
    Target(StableRef),
    Origin(IntentOrigin),
}

impl RateLimitKey {
    fn from_intent(intent: &CommandIntent, scope: RateLimitScope) -> Self {
        match scope {
            RateLimitScope::Target => Self::Target(intent.target.clone()),
            RateLimitScope::Origin => Self::Origin(intent.origin.clone()),
        }
    }
}

fn command_intent_order(
    (left_index, left): &(usize, CommandIntent),
    (right_index, right): &(usize, CommandIntent),
) -> Ordering {
    right
        .priority
        .cmp(&left.priority)
        .then_with(|| compare_origins(&left.origin, &right.origin))
        .then_with(|| right.logical_tick.cmp(&left.logical_tick))
        .then_with(|| left_index.cmp(right_index))
}

fn compare_origins(left: &IntentOrigin, right: &IntentOrigin) -> Ordering {
    origin_bucket(left)
        .cmp(&origin_bucket(right))
        .then_with(|| match (left, right) {
            (
                IntentOrigin::Processor {
                    processor_id: left_processor,
                    context_key: left_context,
                },
                IntentOrigin::Processor {
                    processor_id: right_processor,
                    context_key: right_context,
                },
            ) => left_processor
                .cmp(right_processor)
                .then_with(|| left_context.cmp(right_context)),
            _ => Ordering::Equal,
        })
}

fn origin_bucket(origin: &IntentOrigin) -> u8 {
    match origin {
        IntentOrigin::System => 0,
        IntentOrigin::Transition { .. } => 1,
        IntentOrigin::Processor { .. } => 2,
    }
}

pub trait CommandDispatcher {
    type Error;

    fn dispatch(&mut self, intent: &CommandIntent) -> Result<(), Self::Error>;
}

impl ArbitrationResult {
    pub fn dispatch_with<D: CommandDispatcher>(&self, dispatcher: &mut D) -> Result<(), D::Error> {
        for intent in &self.dispatch {
            dispatcher.dispatch(intent)?;
        }
        Ok(())
    }
}
