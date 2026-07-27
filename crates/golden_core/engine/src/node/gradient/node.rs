use std::collections::HashSet;

use super::helpers::read_child_param_f64;
use super::prelude::*;
use super::snapshot::gradient_from_snapshot;
use super::stop::GradientStopNode;

const STOP_ORDER_POSITION_EPSILON: f64 = 1e-10;

/// Node hosting one gradient color-stop list.
pub struct GradientNode {
    node_data: NodeData,
    default_stops_seeded: bool,
}

impl Default for GradientNode {
    fn default() -> Self {
        Self::new()
    }
}

impl GradientNode {
    /// Creates one gradient node labelled `Gradient`.
    pub fn new() -> Self {
        Self::new_with_label("Gradient")
    }

    /// Creates one gradient node with a custom label.
    pub fn new_with_label(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(GRADIENT_DECL_ID.to_string());
        Self {
            node_data,
            default_stops_seeded: false,
        }
    }

    /// Inserts multiple stops using `(position, color, interpolation)` tuples.
    ///
    /// Returns created stop node ids in insertion order.
    pub fn insert_stops(
        &mut self,
        ctx: &mut ProcessCtx,
        stops: Vec<(f64, Color, GradientInterpolation)>,
    ) -> Vec<NodeId> {
        let mut created = Vec::<NodeId>::with_capacity(stops.len());
        for (position, color, interpolation) in stops {
            if !position.is_finite() {
                continue;
            }
            let stop = GradientStopNode::new_with_values(position, color, interpolation);
            let stop_id = stop.id();
            self.add_child_boxed(ctx, Box::new(stop), None);
            created.push(stop_id);
        }
        created
    }

    /// Samples the color at `position` from the current gradient snapshot.
    ///
    /// Returns opaque black when gradient data is unavailable.
    pub fn get_color_at(&self, snapshot: &ProcessTreeSnapshot, position: f64) -> Color {
        gradient_from_snapshot(snapshot, self.id())
            .map(|gradient| gradient.sample_or_black(position))
            .unwrap_or(Color::new(0.0, 0.0, 0.0, 1.0))
    }

    fn stop_child_count(&self, snapshot: &ProcessTreeSnapshot) -> usize {
        snapshot
            .child_ids(self.id())
            .into_iter()
            .filter(|node_id| {
                snapshot
                    .node(*node_id)
                    .is_some_and(|node| node.node_type == GRADIENT_STOP_NODE_TYPE)
            })
            .count()
    }

    fn pending_stop_add_count(&self, ctx: &ProcessCtx) -> usize {
        ctx.edits
            .pending
            .iter()
            .filter(|request| {
                matches!(
                    &request.edit,
                    Edit::AddNode { parent, node, .. } | Edit::AddUserItem { parent, node, .. }
                        if *parent == self.id() && node.get_type() == GRADIENT_STOP_NODE_TYPE
                ) || matches!(
                    &request.edit,
                    Edit::AddNodeTree { parent, tree, .. } | Edit::AddUserItemTree { parent, tree, .. }
                        if *parent == self.id() && tree.node_type() == GRADIENT_STOP_NODE_TYPE
                )
            })
            .count()
    }

    fn seed_default_stops_if_empty(&mut self, ctx: &mut ProcessCtx) {
        let existing_stop_count = ctx
            .tree_snapshot()
            .map(|snapshot| self.stop_child_count(snapshot))
            .unwrap_or(0);
        if existing_stop_count > 0 {
            self.default_stops_seeded = true;
        }

        let pending_stop_count = self.pending_stop_add_count(ctx);
        if existing_stop_count + pending_stop_count == 0 && !self.default_stops_seeded {
            ctx.add_child_boxed(
                self.id(),
                Box::new(GradientStopNode::new_with_values(
                    0.0,
                    Color::new(0.0, 0.0, 0.0, 1.0),
                    GradientInterpolation::Linear,
                )),
                None,
            );
            ctx.add_child_boxed(
                self.id(),
                Box::new(GradientStopNode::new_with_values(
                    1.0,
                    Color::new(1.0, 1.0, 1.0, 1.0),
                    GradientInterpolation::Linear,
                )),
                None,
            );
            self.default_stops_seeded = true;
        }
    }

    fn collect_stop_reorder_moves_by_position(&self, snapshot: &ProcessTreeSnapshot) -> Vec<(NodeId, Option<NodeId>)> {
        let children = snapshot.child_ids(self.id());
        if children.len() < 2 {
            return Vec::new();
        }

        let mut stop_entries = Vec::<(usize, NodeId, f64)>::new();
        for (index, child) in children.iter().copied().enumerate() {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type != GRADIENT_STOP_NODE_TYPE {
                continue;
            }
            let position = read_child_param_f64(snapshot, child, GRADIENT_STOP_POSITION_DECL_ID, 0.0);
            stop_entries.push((index, child, position));
        }
        if stop_entries.len() < 2 {
            return Vec::new();
        }

        let mut sorted_stop_entries = stop_entries.clone();
        sorted_stop_entries.sort_by(|left, right| {
            if (left.2 - right.2).abs() <= STOP_ORDER_POSITION_EPSILON {
                left.0.cmp(&right.0)
            } else {
                left.2.total_cmp(&right.2)
            }
        });

        let current_stop_order = stop_entries.iter().map(|(_, stop_id, _)| *stop_id).collect::<Vec<_>>();
        let sorted_stop_order = sorted_stop_entries
            .iter()
            .map(|(_, stop_id, _)| *stop_id)
            .collect::<Vec<_>>();
        if current_stop_order == sorted_stop_order {
            return Vec::new();
        }

        let sorted_stop_set = sorted_stop_order.iter().copied().collect::<HashSet<NodeId>>();
        let mut sorted_stop_iter = sorted_stop_order.into_iter();
        let mut desired_children = Vec::<NodeId>::with_capacity(children.len());
        for child in children.iter().copied() {
            if sorted_stop_set.contains(&child) {
                if let Some(next_stop) = sorted_stop_iter.next() {
                    desired_children.push(next_stop);
                } else {
                    desired_children.push(child);
                }
            } else {
                desired_children.push(child);
            }
        }

        let mut simulated_children = children;
        let mut moves = Vec::<(NodeId, Option<NodeId>)>::new();
        for index in 0..desired_children.len() {
            let desired_child = desired_children[index];
            if !sorted_stop_set.contains(&desired_child) {
                continue;
            }
            if simulated_children.get(index).copied() == Some(desired_child) {
                continue;
            }
            let Some(current_index) = simulated_children.iter().position(|node_id| *node_id == desired_child) else {
                continue;
            };
            let moved_child = simulated_children.remove(current_index);
            simulated_children.insert(index, moved_child);
            let after = if index == 0 {
                None
            } else {
                Some(simulated_children[index - 1])
            };
            moves.push((desired_child, after));
        }

        moves
    }
}

impl Node for GradientNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        GRADIENT_NODE_TYPE
    }

    fn type_description(&self) -> Option<&str> {
        Some("Internal node hosting one gradient color-stop list.")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn user_item_kind(&self) -> &str {
        GRADIENT_ITEM_KIND
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[GRADIENT_STOP_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new(
            GRADIENT_STOP_NODE_TYPE,
            GRADIENT_STOP_ITEM_KIND,
            "Stop",
        )]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        let normalized = node_type.trim().to_ascii_lowercase();
        if normalized == GRADIENT_STOP_NODE_TYPE || normalized == "stop" {
            return Some(Box::new(GradientStopNode::new()));
        }
        None
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        self.seed_default_stops_if_empty(ctx);
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.seed_default_stops_if_empty(ctx);
    }

    fn engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        let reorder_moves = {
            let Some(snapshot) = ctx.tree_snapshot() else {
                return;
            };

            let mut should_reorder_stops = false;
            for event in &ctx.events {
                match &event.kind {
                    EventKind::ChildAdded { parent, .. }
                    | EventKind::ChildRemoved { parent, .. }
                    | EventKind::ChildReordered { parent, .. }
                        if *parent == self.id() =>
                    {
                        should_reorder_stops = true;
                    }
                    EventKind::ChildReplaced { parent, .. } if *parent == self.id() => {
                        should_reorder_stops = true;
                    }
                    EventKind::ParamChanged { param, .. } => {
                        if let Some(param_snapshot) = snapshot.node(*param)
                            && let Some(stop_node) = param_snapshot.parent
                            && let Some(stop_snapshot) = snapshot.node(stop_node)
                            && stop_snapshot.parent == Some(self.id())
                            && stop_snapshot.node_type == GRADIENT_STOP_NODE_TYPE
                            && param_snapshot.decl_id == GRADIENT_STOP_POSITION_DECL_ID
                        {
                            should_reorder_stops = true;
                        }
                    }
                    _ => {}
                }
            }

            if should_reorder_stops {
                self.collect_stop_reorder_moves_by_position(snapshot)
            } else {
                Vec::new()
            }
        };

        for (child, after) in reorder_moves {
            self.move_child(ctx, child, self.id(), after);
        }
    }

    fn engine_child_event_interest_depth(&self, _event: &Event) -> u32 {
        2
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}
