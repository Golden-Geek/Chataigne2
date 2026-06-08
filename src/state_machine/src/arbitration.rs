use std::{collections::HashMap, time::Duration};

use golden_alchemist::{RuntimeIntent, RuntimeValue, StableRef};

use crate::ProcessorId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentOrigin {
    Processor(ProcessorId),
    Transition,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlendPolicy {
    Average,
    Add,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CommandPolicy {
    FireAndForget,
    LastWriterWins,
    HighestPriorityWins,
    Queue,
    DropIfSameAsPrevious,
    RateLimit(Duration),
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
        if intent.kind.as_ref() != "chataigne.command" {
            return None;
        }
        Some(Self {
            origin,
            target: intent.target?,
            payload: intent.payload,
            priority: 0,
            policy: CommandPolicy::LastWriterWins,
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
    last_dispatch_tick: HashMap<StableRef, u64>,
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
                for (_, intent) in group {
                    self.record_dispatch(&intent);
                    result.dispatch.push(intent);
                }
                continue;
            }
            group.sort_by(|(left_index, left), (right_index, right)| {
                right
                    .priority
                    .cmp(&left.priority)
                    .then_with(|| right.logical_tick.cmp(&left.logical_tick))
                    .then_with(|| right_index.cmp(left_index))
            });
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
                    "selected by priority, logical tick, then stable input order; {} competing intent(s) lost",
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
            CommandPolicy::RateLimit(interval) => {
                let minimum_ticks = interval.as_millis().min(u128::from(u64::MAX)) as u64;
                self.last_dispatch_tick.get(&intent.target).and_then(|last| {
                    (intent.logical_tick.saturating_sub(*last) < minimum_ticks)
                        .then(|| "dropped by target rate limit".into())
                })
            }
            _ => None,
        }
    }

    fn record_dispatch(&mut self, intent: &CommandIntent) {
        self.previous_payloads
            .insert(intent.target.clone(), intent.payload.clone());
        self.last_dispatch_tick
            .insert(intent.target.clone(), intent.logical_tick);
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
