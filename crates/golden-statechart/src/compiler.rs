use std::collections::{BTreeMap, BTreeSet};

use golden_graph::{GraphDocument, GraphEdgeId, GraphNodeId};
use smol_str::SmolStr;
use thiserror::Error;

use crate::{StateKind, StatechartGraphDomain};

#[derive(Clone, Debug)]
pub struct CompiledState {
    pub id: GraphNodeId,
    pub parent: Option<GraphNodeId>,
    pub kind: StateKind,
    pub entry_processors: Vec<SmolStr>,
    pub exit_processors: Vec<SmolStr>,
}

#[derive(Clone, Debug)]
pub struct CompiledTransition {
    pub id: GraphEdgeId,
    pub source: GraphNodeId,
    pub target: GraphNodeId,
    pub event: Option<SmolStr>,
    pub guard: Option<SmolStr>,
    pub priority: u16,
    pub internal: bool,
}

#[derive(Clone, Debug)]
pub struct StatechartPlan {
    pub states: BTreeMap<GraphNodeId, CompiledState>,
    pub transitions: Vec<CompiledTransition>,
    pub outgoing: BTreeMap<GraphNodeId, Vec<usize>>,
    pub initial_target: GraphNodeId,
}

#[derive(Default)]
pub struct StatechartCompiler;

impl StatechartCompiler {
    pub fn compile(
        &self,
        graph: &GraphDocument<StatechartGraphDomain>,
    ) -> Result<StatechartPlan, StatechartCompileError> {
        let initial = graph
            .nodes()
            .filter(|node| node.data.kind == StateKind::Initial)
            .map(|node| node.id)
            .collect::<Vec<_>>();
        if initial.len() != 1 {
            return Err(StatechartCompileError::InitialCount(initial.len()));
        }
        let initial_edges = graph.outgoing_edges(initial[0]).collect::<Vec<_>>();
        if initial_edges.len() != 1 {
            return Err(StatechartCompileError::InitialTransitionCount(initial_edges.len()));
        }
        let initial_target = initial_edges[0].to.node;

        let states = graph
            .nodes()
            .map(|node| {
                (
                    node.id,
                    CompiledState {
                        id: node.id,
                        parent: node.data.parent,
                        kind: node.data.kind,
                        entry_processors: node.data.entry_processors.clone(),
                        exit_processors: node.data.exit_processors.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        validate_parents(&states)?;

        let mut transitions = graph
            .edges()
            .filter(|edge| edge.from.node != initial[0])
            .map(|edge| CompiledTransition {
                id: edge.id,
                source: edge.from.node,
                target: edge.to.node,
                event: edge.data.event.clone(),
                guard: edge.data.guard.clone(),
                priority: edge.data.priority,
                internal: edge.data.internal,
            })
            .collect::<Vec<_>>();
        transitions.sort_by_key(|transition| (transition.source, transition.priority, transition.id));
        let mut outgoing = BTreeMap::<GraphNodeId, Vec<usize>>::new();
        for (index, transition) in transitions.iter().enumerate() {
            outgoing.entry(transition.source).or_default().push(index);
        }
        Ok(StatechartPlan {
            states,
            transitions,
            outgoing,
            initial_target,
        })
    }
}

fn validate_parents(states: &BTreeMap<GraphNodeId, CompiledState>) -> Result<(), StatechartCompileError> {
    for state in states.values() {
        let mut seen = BTreeSet::new();
        let mut parent = state.parent;
        while let Some(current) = parent {
            if !seen.insert(current) {
                return Err(StatechartCompileError::ParentCycle(state.id));
            }
            parent = states
                .get(&current)
                .ok_or(StatechartCompileError::MissingParent(current))?
                .parent;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum StatechartCompileError {
    #[error("statechart requires exactly one initial state, found {0}")]
    InitialCount(usize),
    #[error("initial state requires exactly one outgoing transition, found {0}")]
    InitialTransitionCount(usize),
    #[error("state parent is missing: {0:?}")]
    MissingParent(GraphNodeId),
    #[error("state parent chain contains a cycle from {0:?}")]
    ParentCycle(GraphNodeId),
}
