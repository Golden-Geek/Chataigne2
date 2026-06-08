use indexmap::{IndexMap, IndexSet};

use crate::{RegionId, STATECHART_SCHEMA_VERSION, StateId, StatechartId, TransitionId};

pub type StatePath = Vec<StateId>;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StateUiLayout {
    pub position: [f64; 2],
    pub size: Option<[f64; 2]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HistoryPolicy {
    #[default]
    None,
    Shallow,
    Deep,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EnterPolicy {
    #[default]
    InitialChild,
    LastActiveChild,
    Explicit(StateId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StateKind {
    Leaf,
    Composite {
        regions: Vec<RegionId>,
        history: HistoryPolicy,
        enter: EnterPolicy,
    },
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StateNode {
    pub id: StateId,
    pub label: String,
    pub parent_region: RegionId,
    pub kind: StateKind,
    pub ui_layout: StateUiLayout,
}

impl StateNode {
    #[must_use]
    pub fn leaf(label: impl Into<String>, parent_region: RegionId) -> Self {
        Self {
            id: StateId::new(),
            label: label.into(),
            parent_region,
            kind: StateKind::Leaf,
            ui_layout: StateUiLayout::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Region {
    pub id: RegionId,
    pub parent_state: Option<StateId>,
    pub states: Vec<StateId>,
    pub initial: Option<StateId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transition {
    pub id: TransitionId,
    pub source: StateId,
    pub target: StateId,
    pub priority: i32,
    pub creation_order: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StateHistory {
    pub shallow: Option<StateId>,
    pub deep: Option<StatePath>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActiveConfiguration {
    pub active_leaf_paths: Vec<StatePath>,
    pub active_scopes: IndexSet<StateId>,
    pub history: IndexMap<StateId, StateHistory>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    Enter(StateId),
    Exit(StateId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionOutcome {
    pub transition: TransitionId,
    pub exited: Vec<StateId>,
    pub entered: Vec<StateId>,
    pub lifecycle: Vec<LifecycleEvent>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Statechart {
    pub schema_version: u32,
    pub id: StatechartId,
    pub root_region: RegionId,
    pub regions: IndexMap<RegionId, Region>,
    pub states: IndexMap<StateId, StateNode>,
    pub transitions: Vec<Transition>,
    pub active: ActiveConfiguration,
    next_transition_order: u64,
}

impl Default for Statechart {
    fn default() -> Self {
        Self::new()
    }
}

impl Statechart {
    #[must_use]
    pub fn new() -> Self {
        let root_region = RegionId::new();
        Self {
            schema_version: STATECHART_SCHEMA_VERSION,
            id: StatechartId::new(),
            root_region,
            regions: IndexMap::from([(
                root_region,
                Region {
                    id: root_region,
                    parent_state: None,
                    states: Vec::new(),
                    initial: None,
                },
            )]),
            states: IndexMap::new(),
            transitions: Vec::new(),
            active: ActiveConfiguration::default(),
            next_transition_order: 0,
        }
    }

    pub fn add_leaf(&mut self, region: RegionId, label: impl Into<String>) -> Result<StateId, StatechartError> {
        self.require_region(region)?;
        let state = StateNode::leaf(label, region);
        let id = state.id;
        self.states.insert(id, state);
        self.regions[&region].states.push(id);
        Ok(id)
    }

    pub fn add_composite(
        &mut self,
        region: RegionId,
        label: impl Into<String>,
        history: HistoryPolicy,
        enter: EnterPolicy,
    ) -> Result<(StateId, RegionId), StatechartError> {
        self.require_region(region)?;
        let state_id = StateId::new();
        let child_region = RegionId::new();
        self.regions.insert(
            child_region,
            Region {
                id: child_region,
                parent_state: Some(state_id),
                states: Vec::new(),
                initial: None,
            },
        );
        self.states.insert(
            state_id,
            StateNode {
                id: state_id,
                label: label.into(),
                parent_region: region,
                kind: StateKind::Composite {
                    regions: vec![child_region],
                    history,
                    enter,
                },
                ui_layout: StateUiLayout::default(),
            },
        );
        self.regions[&region].states.push(state_id);
        Ok((state_id, child_region))
    }

    pub fn set_initial(&mut self, region: RegionId, state: StateId) -> Result<(), StatechartError> {
        self.require_region(region)?;
        if self.states.get(&state).is_none_or(|node| node.parent_region != region) {
            return Err(StatechartError::StateOutsideRegion { state, region });
        }
        self.regions[&region].initial = Some(state);
        Ok(())
    }

    pub fn add_transition(
        &mut self,
        source: StateId,
        target: StateId,
        priority: i32,
    ) -> Result<TransitionId, StatechartError> {
        self.require_state(source)?;
        self.require_state(target)?;
        let id = TransitionId::new();
        self.transitions.push(Transition {
            id,
            source,
            target,
            priority,
            creation_order: self.next_transition_order,
        });
        self.next_transition_order += 1;
        Ok(id)
    }

    pub fn initialize(&mut self) -> Result<Vec<LifecycleEvent>, StatechartError> {
        let initial = self.regions[&self.root_region]
            .initial
            .ok_or(StatechartError::MissingInitial(self.root_region))?;
        let path = self.descend_from(initial)?;
        let lifecycle = path.iter().copied().map(LifecycleEvent::Enter).collect();
        self.set_active_path(path);
        Ok(lifecycle)
    }

    pub fn step(
        &mut self,
        mut eligible: impl FnMut(&Transition) -> bool,
    ) -> Result<Option<TransitionOutcome>, StatechartError> {
        let Some(active_path) = self.active.active_leaf_paths.first().cloned() else {
            return Err(StatechartError::NotInitialized);
        };
        let mut candidates: Vec<Transition> = self
            .transitions
            .iter()
            .filter(|transition| self.active.active_scopes.contains(&transition.source) && eligible(transition))
            .cloned()
            .collect();
        candidates.sort_by(|left, right| {
            self.depth(right.source)
                .cmp(&self.depth(left.source))
                .then_with(|| right.priority.cmp(&left.priority))
                .then_with(|| left.creation_order.cmp(&right.creation_order))
        });
        let Some(selected) = candidates.first() else {
            return Ok(None);
        };
        let target_path = self.path_to(selected.target)?;
        let mut entered_path = target_path.clone();
        let descendants = self.descend_from(selected.target)?;
        entered_path.extend(descendants.into_iter().skip(1));
        let common = common_prefix_len(&active_path, &target_path);
        let exited: Vec<StateId> = active_path[common..].iter().rev().copied().collect();
        self.record_history(&active_path);
        let entered = entered_path[common..].to_vec();
        let lifecycle = exited
            .iter()
            .copied()
            .map(LifecycleEvent::Exit)
            .chain(entered.iter().copied().map(LifecycleEvent::Enter))
            .collect();
        self.set_active_path(entered_path);
        Ok(Some(TransitionOutcome {
            transition: selected.id,
            exited,
            entered,
            lifecycle,
        }))
    }

    fn descend_from(&self, state: StateId) -> Result<StatePath, StatechartError> {
        let mut path = vec![state];
        let mut current = state;
        while let StateKind::Composite {
            regions,
            history,
            enter,
        } = &self.states[&current].kind
        {
            let region = *regions
                .first()
                .ok_or(StatechartError::CompositeWithoutRegion(current))?;
            let next = match enter {
                EnterPolicy::Explicit(state) => Some(*state),
                EnterPolicy::LastActiveChild if *history != HistoryPolicy::None => self
                    .active
                    .history
                    .get(&current)
                    .and_then(|entry| entry.shallow)
                    .or(self.regions[&region].initial),
                EnterPolicy::InitialChild | EnterPolicy::LastActiveChild => self.regions[&region].initial,
            }
            .ok_or(StatechartError::MissingInitial(region))?;
            path.push(next);
            current = next;
        }
        Ok(path)
    }

    fn path_to(&self, state: StateId) -> Result<StatePath, StatechartError> {
        self.require_state(state)?;
        let mut reverse = vec![state];
        let mut current = state;
        while let Some(parent) = self.regions[&self.states[&current].parent_region].parent_state {
            reverse.push(parent);
            current = parent;
        }
        reverse.reverse();
        Ok(reverse)
    }

    fn depth(&self, state: StateId) -> usize {
        self.path_to(state).map_or(0, |path| path.len())
    }

    fn record_history(&mut self, path: &[StateId]) {
        for (index, state) in path.iter().enumerate() {
            if matches!(self.states[state].kind, StateKind::Composite { .. }) {
                self.active.history.insert(
                    *state,
                    StateHistory {
                        shallow: path.get(index + 1).copied(),
                        deep: Some(path[index + 1..].to_vec()),
                    },
                );
            }
        }
    }

    fn set_active_path(&mut self, path: StatePath) {
        self.active.active_scopes = path.iter().copied().collect();
        self.active.active_leaf_paths = vec![path];
    }

    fn require_region(&self, region: RegionId) -> Result<(), StatechartError> {
        self.regions
            .contains_key(&region)
            .then_some(())
            .ok_or(StatechartError::MissingRegion(region))
    }

    fn require_state(&self, state: StateId) -> Result<(), StatechartError> {
        self.states
            .contains_key(&state)
            .then_some(())
            .ok_or(StatechartError::MissingState(state))
    }
}

fn common_prefix_len(left: &[StateId], right: &[StateId]) -> usize {
    left.iter().zip(right).take_while(|(left, right)| left == right).count()
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum StatechartError {
    #[error("region `{0}` does not exist")]
    MissingRegion(RegionId),
    #[error("state `{0}` does not exist")]
    MissingState(StateId),
    #[error("state `{state}` is not inside region `{region}`")]
    StateOutsideRegion { state: StateId, region: RegionId },
    #[error("region `{0}` has no initial state")]
    MissingInitial(RegionId),
    #[error("composite state `{0}` has no child region")]
    CompositeWithoutRegion(StateId),
    #[error("statechart has not been initialized")]
    NotInitialized,
}
