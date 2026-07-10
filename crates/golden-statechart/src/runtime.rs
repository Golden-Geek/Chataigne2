use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use golden_graph::GraphNodeId;
use smol_str::SmolStr;

use crate::StatechartPlan;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActiveConfiguration {
    pub states: BTreeSet<GraphNodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessorInvocation {
    pub processor: SmolStr,
    pub entering: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatechartStep {
    pub transitioned: bool,
    pub exited: Vec<GraphNodeId>,
    pub entered: Vec<GraphNodeId>,
    pub processors: Vec<ProcessorInvocation>,
}

pub struct StatechartRuntime {
    plan: Arc<StatechartPlan>,
    active: ActiveConfiguration,
    leaf: GraphNodeId,
}

impl StatechartRuntime {
    pub fn new(plan: Arc<StatechartPlan>) -> Self {
        let leaf = plan.initial_target;
        let states = ancestry(&plan, leaf).into_iter().collect();
        Self {
            plan,
            active: ActiveConfiguration { states },
            leaf,
        }
    }

    pub fn active(&self) -> &ActiveConfiguration {
        &self.active
    }

    pub fn step(&mut self, event: Option<&str>, guards: &HashMap<SmolStr, bool>) -> StatechartStep {
        let selected = ancestry(&self.plan, self.leaf).into_iter().find_map(|source| {
            self.plan.outgoing.get(&source)?.iter().find_map(|index| {
                let transition = &self.plan.transitions[*index];
                let event_matches = transition.event.as_deref() == event;
                let guard_matches = transition
                    .guard
                    .as_ref()
                    .is_none_or(|guard| guards.get(guard).copied().unwrap_or(false));
                (event_matches && guard_matches).then_some(*index)
            })
        });
        let Some(index) = selected else {
            return StatechartStep::default();
        };
        let transition = &self.plan.transitions[index];
        if transition.internal {
            return StatechartStep {
                transitioned: true,
                ..Default::default()
            };
        }

        let previous = ancestry(&self.plan, self.leaf);
        let next = ancestry(&self.plan, transition.target);
        let shared = previous
            .iter()
            .rev()
            .zip(next.iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let exited = previous[..previous.len() - shared].to_vec();
        let mut entered = next[..next.len() - shared].to_vec();
        entered.reverse();
        let mut processors = Vec::new();
        for state in &exited {
            if let Some(compiled) = self.plan.states.get(state) {
                processors.extend(
                    compiled
                        .exit_processors
                        .iter()
                        .cloned()
                        .map(|processor| ProcessorInvocation {
                            processor,
                            entering: false,
                        }),
                );
            }
        }
        for state in &entered {
            if let Some(compiled) = self.plan.states.get(state) {
                processors.extend(
                    compiled
                        .entry_processors
                        .iter()
                        .cloned()
                        .map(|processor| ProcessorInvocation {
                            processor,
                            entering: true,
                        }),
                );
            }
        }
        self.leaf = transition.target;
        self.active.states = next.into_iter().collect();
        StatechartStep {
            transitioned: true,
            exited,
            entered,
            processors,
        }
    }
}

fn ancestry(plan: &StatechartPlan, state: GraphNodeId) -> Vec<GraphNodeId> {
    let mut result = Vec::new();
    let mut current = Some(state);
    while let Some(id) = current {
        result.push(id);
        current = plan.states.get(&id).and_then(|state| state.parent);
    }
    result
}
