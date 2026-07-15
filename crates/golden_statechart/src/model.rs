use golden_graph::{GraphEdge, GraphNode, GraphTransaction, NodePresentation, PortRef};
use indexmap::{IndexMap, IndexSet};

use crate::{
    RegionId, StateId, StatechartEdgeData, StatechartGraphData, StatechartGraphDocument, StatechartGraphDomain,
    StatechartId, StatechartNodeData, TransitionId,
};

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

/// Canonical statechart authoring document and runtime state.
///
/// Generic identity, topology, revisions, and editor presentation stay inside the
/// `golden_graph` document. Statechart-specific hierarchy and active configuration
/// live in the domain data of the same revisioned document.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Statechart {
    #[cfg_attr(feature = "serde", serde(with = "crate::domain::document_serde"))]
    document: StatechartGraphDocument,
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
            document: StatechartGraphDomain::new_document(StatechartId::new(), root_region),
        }
    }

    pub fn from_document(document: StatechartGraphDocument) -> Result<Self, StatechartError> {
        let diagnostics = StatechartGraphDomain.validate_document(&document);
        if diagnostics.is_empty() {
            Ok(Self { document })
        } else {
            Err(StatechartError::InvalidDocument(
                diagnostics.into_iter().map(|diagnostic| diagnostic.message).collect(),
            ))
        }
    }

    #[must_use]
    pub const fn document(&self) -> &StatechartGraphDocument {
        &self.document
    }

    #[must_use]
    pub fn into_document(self) -> StatechartGraphDocument {
        self.document
    }

    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.document.data().schema_version
    }

    #[must_use]
    pub fn id(&self) -> StatechartId {
        StatechartId::from_uuid(self.document.id().as_uuid())
    }

    #[must_use]
    pub fn root_region(&self) -> RegionId {
        self.document.data().root_region
    }

    #[must_use]
    pub fn regions(&self) -> &IndexMap<RegionId, Region> {
        &self.document.data().regions
    }

    #[must_use]
    pub fn active(&self) -> &ActiveConfiguration {
        &self.document.data().active
    }

    #[must_use]
    pub fn state(&self, id: StateId) -> Option<StateNode> {
        let graph_id = StatechartGraphDomain::node_id(id);
        let node = self.document.node(graph_id)?;
        let ui_layout =
            self.document
                .presentation()
                .nodes
                .get(&graph_id)
                .map_or_else(StateUiLayout::default, |presentation| StateUiLayout {
                    position: presentation.position,
                    size: presentation.size,
                });
        Some(node.data.clone().into_state(id, ui_layout))
    }

    pub fn states(&self) -> impl Iterator<Item = StateNode> + '_ {
        self.document.nodes().map(|node| {
            let id = StatechartGraphDomain::state_id(node.id);
            self.state(id)
                .expect("statechart node identity is derived from its graph node")
        })
    }

    #[must_use]
    pub fn transition(&self, id: TransitionId) -> Option<Transition> {
        self.document
            .edge(StatechartGraphDomain::edge_id(id))
            .map(transition_from_edge)
    }

    pub fn transitions(&self) -> impl Iterator<Item = Transition> + '_ {
        self.document.edges().map(transition_from_edge)
    }

    pub fn add_leaf(&mut self, region: RegionId, label: impl Into<String>) -> Result<StateId, StatechartError> {
        self.require_region(region)?;
        let state = StateNode::leaf(label, region);
        let id = state.id;
        let mut data = self.document.data().clone();
        data.regions[&region].states.push(id);
        let mut transaction = GraphTransaction::for_document(&self.document);
        transaction.replace_graph_data(data);
        transaction.insert_node(graph_node(&state), Some(node_presentation(state.ui_layout)));
        self.commit(transaction)?;
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
        let state = StateNode {
            id: state_id,
            label: label.into(),
            parent_region: region,
            kind: StateKind::Composite {
                regions: vec![child_region],
                history,
                enter,
            },
            ui_layout: StateUiLayout::default(),
        };
        let mut data = self.document.data().clone();
        data.regions.insert(
            child_region,
            Region {
                id: child_region,
                parent_state: Some(state_id),
                states: Vec::new(),
                initial: None,
            },
        );
        data.regions[&region].states.push(state_id);
        let mut transaction = GraphTransaction::for_document(&self.document);
        transaction.replace_graph_data(data);
        transaction.insert_node(graph_node(&state), Some(node_presentation(state.ui_layout)));
        self.commit(transaction)?;
        Ok((state_id, child_region))
    }

    pub fn set_initial(&mut self, region: RegionId, state: StateId) -> Result<(), StatechartError> {
        self.require_region(region)?;
        if self.state(state).is_none_or(|node| node.parent_region != region) {
            return Err(StatechartError::StateOutsideRegion { state, region });
        }
        let mut data = self.document.data().clone();
        data.regions[&region].initial = Some(state);
        let mut transaction = GraphTransaction::for_document(&self.document);
        transaction.replace_graph_data(data);
        self.commit(transaction)
    }

    pub fn set_state_ui_layout(&mut self, state: StateId, layout: StateUiLayout) -> Result<(), StatechartError> {
        self.require_state(state)?;
        let node = StatechartGraphDomain::node_id(state);
        let mut presentation = self
            .document
            .presentation()
            .nodes
            .get(&node)
            .cloned()
            .unwrap_or_default();
        presentation.position = layout.position;
        presentation.size = layout.size;
        let mut transaction = GraphTransaction::for_document(&self.document);
        transaction.set_node_presentation(node, Some(presentation));
        self.commit(transaction)
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
        let mut data = self.document.data().clone();
        let transition = Transition {
            id,
            source,
            target,
            priority,
            creation_order: data.next_transition_order,
        };
        data.next_transition_order += 1;
        let mut transaction = GraphTransaction::for_document(&self.document);
        transaction.replace_graph_data(data);
        transaction.connect(graph_edge(&transition));
        self.commit(transaction)?;
        Ok(id)
    }

    pub fn initialize(&mut self) -> Result<Vec<LifecycleEvent>, StatechartError> {
        let initial = self.regions()[&self.root_region()]
            .initial
            .ok_or(StatechartError::MissingInitial(self.root_region()))?;
        let path = self.descend_from(initial)?;
        let lifecycle = path.iter().copied().map(LifecycleEvent::Enter).collect();
        let mut data = self.document.data().clone();
        set_active_path(&mut data.active, path);
        let mut transaction = GraphTransaction::for_document(&self.document);
        transaction.replace_graph_data(data);
        self.commit(transaction)?;
        Ok(lifecycle)
    }

    pub fn step(
        &mut self,
        mut eligible: impl FnMut(&Transition) -> bool,
    ) -> Result<Option<TransitionOutcome>, StatechartError> {
        let Some(active_path) = self.active().active_leaf_paths.first().cloned() else {
            return Err(StatechartError::NotInitialized);
        };
        let mut candidates: Vec<_> = self
            .transitions()
            .filter(|transition| self.active().active_scopes.contains(&transition.source) && eligible(transition))
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
        let exited: Vec<_> = active_path[common..].iter().rev().copied().collect();
        let entered = entered_path[common..].to_vec();
        let lifecycle = exited
            .iter()
            .copied()
            .map(LifecycleEvent::Exit)
            .chain(entered.iter().copied().map(LifecycleEvent::Enter))
            .collect();
        let mut data = self.document.data().clone();
        self.record_history(&mut data.active, &active_path);
        set_active_path(&mut data.active, entered_path);
        let mut transaction = GraphTransaction::for_document(&self.document);
        transaction.replace_graph_data(data);
        self.commit(transaction)?;
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
        loop {
            let node = self.state(current).ok_or(StatechartError::MissingState(current))?;
            let StateKind::Composite {
                regions,
                history,
                enter,
            } = node.kind
            else {
                break;
            };
            let region = *regions
                .first()
                .ok_or(StatechartError::CompositeWithoutRegion(current))?;
            let next = match enter {
                EnterPolicy::Explicit(state) => Some(state),
                EnterPolicy::LastActiveChild if history != HistoryPolicy::None => self
                    .active()
                    .history
                    .get(&current)
                    .and_then(|entry| entry.shallow)
                    .or(self.regions()[&region].initial),
                EnterPolicy::InitialChild | EnterPolicy::LastActiveChild => self.regions()[&region].initial,
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
        loop {
            let node = self.state(current).ok_or(StatechartError::MissingState(current))?;
            let Some(parent) = self.regions()[&node.parent_region].parent_state else {
                break;
            };
            reverse.push(parent);
            current = parent;
        }
        reverse.reverse();
        Ok(reverse)
    }

    fn depth(&self, state: StateId) -> usize {
        self.path_to(state).map_or(0, |path| path.len())
    }

    fn record_history(&self, active: &mut ActiveConfiguration, path: &[StateId]) {
        for (index, state) in path.iter().enumerate() {
            if matches!(
                self.state(*state).map(|node| node.kind),
                Some(StateKind::Composite { .. })
            ) {
                active.history.insert(
                    *state,
                    StateHistory {
                        shallow: path.get(index + 1).copied(),
                        deep: Some(path[index + 1..].to_vec()),
                    },
                );
            }
        }
    }

    fn require_region(&self, region: RegionId) -> Result<(), StatechartError> {
        self.regions()
            .contains_key(&region)
            .then_some(())
            .ok_or(StatechartError::MissingRegion(region))
    }

    fn require_state(&self, state: StateId) -> Result<(), StatechartError> {
        self.document
            .node(StatechartGraphDomain::node_id(state))
            .is_some()
            .then_some(())
            .ok_or(StatechartError::MissingState(state))
    }

    fn commit(
        &mut self,
        transaction: GraphTransaction<StatechartGraphData, StatechartNodeData, StatechartEdgeData>,
    ) -> Result<(), StatechartError> {
        transaction
            .commit(&mut self.document, &StatechartGraphDomain)
            .map(|_| ())
            .map_err(|error| StatechartError::Edit(error.to_string()))
    }
}

fn graph_node(state: &StateNode) -> GraphNode<StatechartNodeData> {
    GraphNode {
        id: StatechartGraphDomain::node_id(state.id),
        data: StatechartNodeData::from_state(state),
    }
}

fn node_presentation(layout: StateUiLayout) -> NodePresentation {
    NodePresentation {
        position: layout.position,
        size: layout.size,
        collapsed: false,
    }
}

fn graph_edge(transition: &Transition) -> GraphEdge<StatechartEdgeData> {
    GraphEdge {
        id: StatechartGraphDomain::edge_id(transition.id),
        from: PortRef::new(
            StatechartGraphDomain::node_id(transition.source),
            StatechartGraphDomain::outgoing_port(),
        ),
        to: PortRef::new(
            StatechartGraphDomain::node_id(transition.target),
            StatechartGraphDomain::incoming_port(),
        ),
        data: StatechartEdgeData {
            priority: transition.priority,
            creation_order: transition.creation_order,
        },
    }
}

fn transition_from_edge(edge: &GraphEdge<StatechartEdgeData>) -> Transition {
    Transition {
        id: StatechartGraphDomain::transition_id(edge.id),
        source: StatechartGraphDomain::state_id(edge.from.node),
        target: StatechartGraphDomain::state_id(edge.to.node),
        priority: edge.data.priority,
        creation_order: edge.data.creation_order,
    }
}

fn set_active_path(active: &mut ActiveConfiguration, path: StatePath) {
    active.active_scopes = path.iter().copied().collect();
    active.active_leaf_paths = vec![path];
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
    #[error("statechart document edit failed: {0}")]
    Edit(String),
    #[error("statechart document is invalid: {0:?}")]
    InvalidDocument(Vec<String>),
}
