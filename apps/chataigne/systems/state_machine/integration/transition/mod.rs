use std::collections::HashMap;

use golden_core::{
    item, node,
    node::{
        Node, NodeId, NodeMetaPatch, NodeReference, NodeUserPermissions, UserContainerRules,
        UserCreatableItem,
    },
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

pub(crate) const TRANSITION_ITEM_KIND: &str = "state_transition";

#[node(
    "state_transition_manager",
    label = "Transitions",
    presentation = golden_core::node::PresentationHint {
        show_in_nested_inspector: false,
        ..Default::default()
    }
)]
pub struct StateTransitionManager {}

#[node("state_transition_manager", from_struct)]
impl Node for StateTransitionManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[TRANSITION_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == TRANSITION_ITEM_KIND
            && crate::app::declared_user_item_type_matches(item_type, TRANSITION_ITEM_KIND)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items(TRANSITION_ITEM_KIND)
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, TRANSITION_ITEM_KIND)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        reconcile_state_networks(ctx, None, None, None);
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        reconcile_state_networks(ctx, None, None, None);
    }
}

#[node("state_transition", label = "Transition")]
#[children(
    target: NodeReference (
        label = "Target State",
        description = "State entered when this transition fires.",
        callback = Self::target_changed
    );
)]
pub struct StateTransition {}

impl StateTransition {
    fn target_changed(&mut self, ctx: &mut ProcessCtx, _old_value: ParamValue) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        let Some(source_state) = transition_source_state(snapshot, self.id()) else {
            return;
        };
        let target_state = self.target.get_ref().cached_id();
        reconcile_state_networks(
            ctx,
            None,
            None,
            target_state.map(|target| (source_state, target)),
        );
    }
}

#[item("state_transition", node = "state_transition", from_struct)]
impl Node for StateTransition {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

pub(crate) fn reconcile_state_networks(
    ctx: &mut ProcessCtx,
    preferred_active: Option<NodeId>,
    forced_inactive: Option<NodeId>,
    extra_transition: Option<(NodeId, NodeId)>,
) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    let actions = state_network_enabled_updates(
        snapshot.as_ref(),
        preferred_active,
        forced_inactive,
        extra_transition,
    );
    for (state, enabled) in actions {
        ctx.patch_node_meta(
            state,
            NodeMetaPatch {
                enabled: Some(enabled),
                ..NodeMetaPatch::default()
            },
        );
    }
}

fn state_network_enabled_updates(
    snapshot: &ProcessTreeSnapshot,
    preferred_active: Option<NodeId>,
    forced_inactive: Option<NodeId>,
    extra_transition: Option<(NodeId, NodeId)>,
) -> Vec<(NodeId, bool)> {
    let states = collect_state_nodes(snapshot);
    if states.is_empty() {
        return Vec::new();
    }

    let state_activity: Vec<(NodeId, bool)> = states
        .iter()
        .map(|state| (*state, state_enabled(snapshot, *state).unwrap_or(false)))
        .collect();
    let mut transitions = Vec::new();
    for source in &states {
        let Some(transition_manager) = snapshot.find_child_by_decl_id(*source, "transitions") else {
            continue;
        };
        for transition in snapshot.child_ids(transition_manager) {
            let Some(target) = transition_target_state(snapshot, transition) else {
                continue;
            };
            transitions.push((*source, target));
        }
    }
    if let Some((source, target)) = extra_transition {
        transitions.push((source, target));
    }
    let desired_activity =
        reconciled_state_activity(&state_activity, &transitions, preferred_active, forced_inactive);

    states
        .into_iter()
        .filter_map(|state| {
            let desired = desired_activity.get(&state).copied()?;
            if state_enabled(snapshot, state) == Some(desired) {
                return None;
            }
            Some((state, desired))
        })
        .collect()
}

fn reconciled_state_activity(
    states: &[(NodeId, bool)],
    transitions: &[(NodeId, NodeId)],
    preferred_active: Option<NodeId>,
    forced_inactive: Option<NodeId>,
) -> HashMap<NodeId, bool> {
    let state_indexes: HashMap<NodeId, usize> = states
        .iter()
        .enumerate()
        .map(|(index, (state, _))| (*state, index))
        .collect();
    let current_activity: HashMap<NodeId, bool> = states.iter().copied().collect();
    let mut components = DisjointSet::new(states.len());
    for (source, target) in transitions {
        union_states(&mut components, &state_indexes, *source, *target);
    }

    let mut members_by_component: HashMap<usize, Vec<NodeId>> = HashMap::new();
    for (index, (state, _)) in states.iter().copied().enumerate() {
        members_by_component.entry(components.find(index)).or_default().push(state);
    }

    let mut desired_activity = HashMap::with_capacity(states.len());
    for members in members_by_component.values() {
        // A component with no transitions is an isolated state; it can be freely inactive.
        // Only connected components (linked by at least one transition) require one active state.
        let is_connected = transitions.iter().any(|(src, tgt)| {
            members.contains(src) || members.contains(tgt)
        });
        let chosen = preferred_active
            .filter(|candidate| members.contains(candidate))
            .or_else(|| {
                members.iter().copied().find(|state| {
                    Some(*state) != forced_inactive
                        && current_activity.get(state).copied().unwrap_or(false)
                })
            })
            .or_else(|| {
                if !is_connected {
                    return None;
                }
                members
                    .iter()
                    .copied()
                    .find(|state| Some(*state) != forced_inactive)
            })
            .or_else(|| if is_connected { members.first().copied() } else { None });
        let Some(chosen) = chosen else {
            continue;
        };
        for state in members {
            desired_activity.insert(*state, *state == chosen);
        }
    }
    desired_activity
}

fn collect_state_nodes(snapshot: &ProcessTreeSnapshot) -> Vec<NodeId> {
    let mut states = Vec::new();
    let mut pending = vec![snapshot.root()];
    while let Some(node_id) = pending.pop() {
        let Some(node) = snapshot.node(node_id) else {
            continue;
        };
        if node.node_type == crate::app::StateMachineState::NODE_TYPE {
            states.push(node_id);
        }
        let mut children = snapshot.child_ids(node_id);
        children.reverse();
        pending.extend(children);
    }
    states
}

fn state_enabled(snapshot: &ProcessTreeSnapshot, state: NodeId) -> Option<bool> {
    Some(snapshot.node(state)?.enabled)
}

fn transition_source_state(snapshot: &ProcessTreeSnapshot, transition: NodeId) -> Option<NodeId> {
    let manager = snapshot.node(transition)?.parent?;
    let state = snapshot.node(manager)?.parent?;
    (snapshot.node(state)?.node_type == crate::app::StateMachineState::NODE_TYPE).then_some(state)
}

fn transition_target_state(snapshot: &ProcessTreeSnapshot, transition: NodeId) -> Option<NodeId> {
    let target_param = snapshot.find_child_by_decl_id(transition, "target")?;
    let ParamValue::Reference(reference) = snapshot.node(target_param)?.param_value.as_ref()? else {
        return None;
    };
    reference
        .cached_id()
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
}

fn union_states(
    components: &mut DisjointSet,
    state_indexes: &HashMap<NodeId, usize>,
    source: NodeId,
    target: NodeId,
) {
    let (Some(source), Some(target)) = (state_indexes.get(&source), state_indexes.get(&target)) else {
        return;
    };
    components.union(*source, *target);
}

struct DisjointSet {
    parents: Vec<usize>,
    ranks: Vec<u8>,
}

impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parents: (0..size).collect(),
            ranks: vec![0; size],
        }
    }

    fn find(&mut self, item: usize) -> usize {
        if self.parents[item] != item {
            self.parents[item] = self.find(self.parents[item]);
        }
        self.parents[item]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left == right {
            return;
        }
        match self.ranks[left].cmp(&self.ranks[right]) {
            std::cmp::Ordering::Less => self.parents[left] = right,
            std::cmp::Ordering::Greater => self.parents[right] = left,
            std::cmp::Ordering::Equal => {
                self.parents[right] = left;
                self.ranks[left] = self.ranks[left].saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests;
