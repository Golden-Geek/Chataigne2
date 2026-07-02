use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;

use super::easing::CurveEasingNode;
use super::helpers::{
    CURVE_RANGE_EPSILON, KEY_ORDER_POSITION_EPSILON, incoming_segment_slope, key_secant_slope, outgoing_segment_slope,
    read_child_param_f64,
};
use super::key::CurveKeyNode;
use super::prelude::*;
use super::range::{
    CurveRangeConstraint, CurveRangeNode, read_range_axis_bounds, read_range_constraint_from_key_param_constraints,
    read_range_constraint_from_range_node,
};
use super::snapshot::{curve_from_snapshot, parse_key_from_snapshot};

/// Internal node hosting one animation-curve key list.
pub struct CurveNode {
    node_data: NodeData,
    user_can_edit_range: bool,
    code_range_constraint: Option<CurveRangeConstraint>,
    range_node: Option<NodeId>,
    default_keys_seeded: bool,
}

impl CurveNode {
    /// Creates one animation-curve node.
    pub fn new() -> Self {
        Self::new_with_label("Curve")
    }

    /// Creates one animation-curve node with custom label.
    pub fn new_with_label(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_CURVE_DECL_ID.to_string());
        Self {
            node_data,
            user_can_edit_range: true,
            code_range_constraint: None,
            range_node: None,
            default_keys_seeded: false,
        }
    }

    /// Enables/disables user-editable range controls.
    pub fn with_user_editable_range(mut self, user_can_edit_range: bool) -> Self {
        self.user_can_edit_range = user_can_edit_range;
        self
    }

    /// Sets whether range controls are user-editable.
    pub fn set_user_editable_range(&mut self, user_can_edit_range: bool) {
        self.user_can_edit_range = user_can_edit_range;
    }

    /// Sets one code-authored range constraint.
    pub fn with_range_constraint(mut self, range: Option<CurveRangeConstraint>) -> Self {
        self.code_range_constraint = range;
        self
    }

    /// Replaces the code-authored range constraint.
    pub fn set_range_constraint(&mut self, range: Option<CurveRangeConstraint>) {
        self.code_range_constraint = range;
    }

    /// Inserts multiple keys using `(position, value)` tuples.
    ///
    /// Inserted keys default to linear easing so code-driven bulk inserts can stay lightweight.
    ///
    /// Returns created key node ids in insertion order.
    pub fn insert_keys(&mut self, ctx: &mut ProcessCtx, keys: Vec<(f64, f64)>) -> Vec<NodeId> {
        let keys_with_easing = keys
            .into_iter()
            .map(|(position, value)| (position, value, CurveEasing::Linear))
            .collect();
        self.insert_keys_with_easing(ctx, keys_with_easing)
    }

    /// Inserts multiple keys with explicit per-key easing.
    ///
    /// Returns created key node ids in insertion order.
    pub fn insert_keys_with_easing(
        &mut self,
        ctx: &mut ProcessCtx,
        mut keys: Vec<(f64, f64, CurveEasing)>,
    ) -> Vec<NodeId> {
        if keys.is_empty() {
            return Vec::new();
        }

        keys.retain(|(position, value, _)| position.is_finite() && value.is_finite());
        if keys.is_empty() {
            return Vec::new();
        }
        keys.sort_by(|left, right| left.0.total_cmp(&right.0));

        let active_range = ctx
            .tree_snapshot()
            .and_then(|snapshot| self.effective_range_constraint(snapshot));
        let child_range = self.initial_key_range_constraint();
        let mut created = Vec::<NodeId>::with_capacity(keys.len());
        for (position, value, easing) in keys {
            let (position, value) = if let Some(range) = active_range {
                (range.clamp_position(position), range.clamp_value(value))
            } else {
                (position, value)
            };

            let key_node = CurveKeyNode::new_with_values_and_range_and_easing(position, value, child_range, easing);
            let key_id = key_node.id();
            self.add_child_boxed(ctx, Box::new(key_node), None);
            created.push(key_id);
        }
        created
    }

    /// Samples one value at `position` from the current curve snapshot.
    ///
    /// Returns `0` when curve data is unavailable or sampling fails.
    pub fn get_value_at(&self, snapshot: &ProcessTreeSnapshot, position: f64) -> f64 {
        if !position.is_finite() {
            return 0.0;
        }
        curve_from_snapshot(snapshot, self.id())
            .and_then(|curve| curve.sample(position))
            .unwrap_or(0.0)
    }

    /// Returns sorted `(key_id, position, value)` tuples inside `range`.
    pub fn get_keys_between(
        &self,
        snapshot: &ProcessTreeSnapshot,
        range: RangeInclusive<f64>,
    ) -> Vec<(NodeId, f64, f64)> {
        let start = *range.start();
        let end = *range.end();
        if !start.is_finite() || !end.is_finite() {
            return Vec::new();
        }
        let (min_position, max_position) = if start <= end { (start, end) } else { (end, start) };
        self.collect_sorted_key_data(snapshot)
            .into_iter()
            .filter(|(_, position, _)| *position >= min_position && *position <= max_position)
            .collect()
    }

    /// Returns the closest key to `position` as `(key_id, position, value)`.
    pub fn get_closest_key(&self, snapshot: &ProcessTreeSnapshot, position: f64) -> Option<(NodeId, f64, f64)> {
        if !position.is_finite() {
            return None;
        }

        self.collect_sorted_key_data(snapshot)
            .into_iter()
            .min_by(|left, right| {
                let left_distance = (left.1 - position).abs();
                let right_distance = (right.1 - position).abs();
                left_distance
                    .total_cmp(&right_distance)
                    .then(left.1.total_cmp(&right.1))
                    .then(left.0.0.cmp(&right.0.0))
            })
    }

    /// Returns the closest strictly-previous key to `position`.
    pub fn get_prev_key(&self, snapshot: &ProcessTreeSnapshot, position: f64) -> Option<(NodeId, f64, f64)> {
        if !position.is_finite() {
            return None;
        }

        let mut previous = None;
        for key in self.collect_sorted_key_data(snapshot) {
            if key.1 < position - KEY_ORDER_POSITION_EPSILON {
                previous = Some(key);
            } else {
                break;
            }
        }
        previous
    }

    /// Returns the closest strictly-next key to `position`.
    pub fn get_next_key(&self, snapshot: &ProcessTreeSnapshot, position: f64) -> Option<(NodeId, f64, f64)> {
        if !position.is_finite() {
            return None;
        }

        self.collect_sorted_key_data(snapshot)
            .into_iter()
            .find(|(_, key_position, _)| *key_position > position + KEY_ORDER_POSITION_EPSILON)
    }

    fn collect_sorted_key_records(&self, snapshot: &ProcessTreeSnapshot) -> Vec<(NodeId, CurveKey)> {
        let range_constraint = self
            .effective_range_constraint(snapshot)
            .or_else(|| read_range_constraint_from_key_param_constraints(snapshot, self.id()));
        let mut key_entries = Vec::<(usize, NodeId, CurveKey)>::new();
        for (source_index, child) in snapshot.child_ids(self.id()).into_iter().enumerate() {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
                continue;
            }
            let Some(key) = parse_key_from_snapshot(snapshot, child, range_constraint) else {
                continue;
            };
            key_entries.push((source_index, child, key));
        }

        key_entries.sort_by(|left, right| {
            if (left.2.position - right.2.position).abs() <= KEY_ORDER_POSITION_EPSILON {
                left.0.cmp(&right.0)
            } else {
                left.2.position.total_cmp(&right.2.position)
            }
        });
        key_entries
            .into_iter()
            .map(|(_, node_id, key)| (node_id, key))
            .collect()
    }

    fn queue_key_easing_replace(
        &self,
        snapshot: &ProcessTreeSnapshot,
        key_node: NodeId,
        easing: CurveEasing,
    ) -> Result<Edit, String> {
        let easing_node = snapshot
            .find_child(key_node, PARAMETER_ANIMATION_EASING_DECL_ID)
            .ok_or_else(|| format!("missing easing node for animation-curve key {}", key_node.0))?;
        let label = snapshot
            .node(easing_node)
            .map(|node| node.label.clone())
            .unwrap_or_else(|| "Easing".to_string());
        Ok(Edit::ReplaceNode {
            node: easing_node,
            new_node: Box::new(CurveEasingNode::new_with_easing(label, easing)),
        })
    }

    /// Replaces all keys inside the sampled x-range with a sparse bezier fit of `samples`.
    pub fn replace_range_with_fitted_samples(
        &mut self,
        ctx: &mut ProcessCtx,
        samples: &[CurveFitPoint],
        options: CurveBezierFitOptions,
    ) -> Result<Vec<NodeId>, String> {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return Err("animation curve fit requires an active tree snapshot".to_string());
        };

        let active_range = self
            .effective_range_constraint(snapshot)
            .or_else(|| read_range_constraint_from_key_param_constraints(snapshot, self.id()));
        let normalized_samples = samples
            .iter()
            .copied()
            .map(|point| {
                if let Some(range) = active_range {
                    CurveFitPoint::new(range.clamp_position(point.position), range.clamp_value(point.value))
                } else {
                    point
                }
            })
            .collect::<Vec<_>>();
        let mut fitted_keys = fit_points_to_bezier_keys(normalized_samples.as_slice(), options);
        if fitted_keys.len() < 2 {
            return Err("animation curve fit requires at least two unique sample positions".to_string());
        }

        let key_records = self.collect_sorted_key_records(snapshot);
        let range_start = fitted_keys.first().map(|key| key.position).unwrap_or(0.0);
        let range_end = fitted_keys.last().map(|key| key.position).unwrap_or(range_start);
        let keys_to_remove = key_records
            .iter()
            .filter(|(_, key)| {
                key.position >= range_start - KEY_ORDER_POSITION_EPSILON
                    && key.position <= range_end + KEY_ORDER_POSITION_EPSILON
            })
            .map(|(node_id, _)| *node_id)
            .collect::<Vec<_>>();
        let left_index = key_records
            .iter()
            .rposition(|(_, key)| key.position < range_start - KEY_ORDER_POSITION_EPSILON);
        let right_index = key_records
            .iter()
            .position(|(_, key)| key.position > range_end + KEY_ORDER_POSITION_EPSILON);

        let first_path_slope = outgoing_segment_slope(&fitted_keys[0], &fitted_keys[1]).unwrap_or_else(|| {
            key_secant_slope(
                fitted_keys[0].position,
                fitted_keys[0].value,
                fitted_keys[1].position,
                fitted_keys[1].value,
            )
            .unwrap_or(0.0)
        });
        let fitted_len = fitted_keys.len();
        let last_path_slope = incoming_segment_slope(&fitted_keys[fitted_len - 2], &fitted_keys[fitted_len - 1])
            .unwrap_or_else(|| {
                key_secant_slope(
                    fitted_keys[fitted_len - 2].position,
                    fitted_keys[fitted_len - 2].value,
                    fitted_keys[fitted_len - 1].position,
                    fitted_keys[fitted_len - 1].value,
                )
                .unwrap_or(0.0)
            });

        if let Some(right_index) = right_index {
            let (_, right_key) = &key_records[right_index];
            let right_boundary_slope = key_records
                .get(right_index + 1)
                .and_then(|(_, next_key)| outgoing_segment_slope(right_key, next_key))
                .or_else(|| {
                    key_secant_slope(
                        fitted_keys[fitted_len - 1].position,
                        fitted_keys[fitted_len - 1].value,
                        right_key.position,
                        right_key.value,
                    )
                })
                .unwrap_or(0.0);
            if let Some(easing) = bezier_easing_from_endpoint_slopes(
                fitted_keys[fitted_len - 1].position,
                fitted_keys[fitted_len - 1].value,
                right_key.position,
                right_key.value,
                last_path_slope,
                right_boundary_slope,
            ) {
                fitted_keys[fitted_len - 1].easing = easing;
            }
        }

        let mut left_key_easing_replace = None;
        if let Some(left_index) = left_index {
            let (left_node_id, left_key) = &key_records[left_index];
            let left_boundary_slope = key_records
                .get(left_index + 1)
                .and_then(|(_, next_key)| outgoing_segment_slope(left_key, next_key))
                .or_else(|| {
                    key_secant_slope(
                        left_key.position,
                        left_key.value,
                        fitted_keys[0].position,
                        fitted_keys[0].value,
                    )
                })
                .unwrap_or(0.0);
            if let Some(easing) = bezier_easing_from_endpoint_slopes(
                left_key.position,
                left_key.value,
                fitted_keys[0].position,
                fitted_keys[0].value,
                left_boundary_slope,
                first_path_slope,
            ) {
                left_key_easing_replace = Some(self.queue_key_easing_replace(snapshot, *left_node_id, easing)?);
            }
        }

        if let Some(edit) = left_key_easing_replace {
            ctx.edits.push(edit);
        }

        for node_id in keys_to_remove {
            ctx.edits.push(Edit::RemoveNode { node: node_id });
        }

        Ok(self.insert_keys_with_easing(
            ctx,
            fitted_keys
                .into_iter()
                .map(|key| (key.position, key.value, key.easing))
                .collect(),
        ))
    }

    fn collect_sorted_key_data(&self, snapshot: &ProcessTreeSnapshot) -> Vec<(NodeId, f64, f64)> {
        let range_constraint = self
            .effective_range_constraint(snapshot)
            .or_else(|| read_range_constraint_from_key_param_constraints(snapshot, self.id()));
        let mut key_entries = Vec::<(usize, NodeId, f64, f64)>::new();
        for (source_index, child) in snapshot.child_ids(self.id()).into_iter().enumerate() {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
                continue;
            }

            let mut position = read_child_param_f64(snapshot, child, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, 0.0);
            let mut value = read_child_param_f64(snapshot, child, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, 0.0);
            if let Some(range_constraint) = range_constraint {
                position = range_constraint.clamp_position(position);
                value = range_constraint.clamp_value(value);
            }
            key_entries.push((source_index, child, position, value));
        }

        key_entries.sort_by(|left, right| {
            if (left.2 - right.2).abs() <= KEY_ORDER_POSITION_EPSILON {
                left.0.cmp(&right.0)
            } else {
                left.2.total_cmp(&right.2)
            }
        });
        key_entries
            .into_iter()
            .map(|(_, key_id, position, value)| (key_id, position, value))
            .collect()
    }

    fn bind_decl_child(&mut self, decl_id: &str, child: NodeId) {
        if decl_id == PARAMETER_ANIMATION_RANGE_DECL_ID {
            self.range_node = Some(child);
        }
    }

    fn unbind_child(&mut self, child: NodeId) {
        if self.range_node == Some(child) {
            self.range_node = None;
        }
    }

    fn initial_key_range_constraint(&self) -> Option<CurveRangeConstraint> {
        if self.user_can_edit_range {
            None
        } else {
            self.code_range_constraint
        }
    }

    fn pending_key_add_count(&self, ctx: &ProcessCtx) -> usize {
        ctx.edits
            .pending
            .iter()
            .filter(|request| {
                matches!(
                    &request.edit,
                    Edit::AddNode { parent, node, .. } | Edit::AddUserItem { parent, node, .. }
                        if *parent == self.id() && node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE
                ) || matches!(
                    &request.edit,
                    Edit::AddNodeTree { parent, tree, .. } | Edit::AddUserItemTree { parent, tree, .. }
                        if *parent == self.id() && tree.node_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE
                )
            })
            .count()
    }

    fn effective_range_constraint(&self, snapshot: &ProcessTreeSnapshot) -> Option<CurveRangeConstraint> {
        if self.user_can_edit_range {
            let range_node = self
                .range_node
                .or_else(|| snapshot.find_child(self.id(), PARAMETER_ANIMATION_RANGE_DECL_ID))?;
            return read_range_constraint_from_range_node(snapshot, range_node);
        }
        self.code_range_constraint
    }

    fn editable_range_constraint(
        enabled: bool,
        x_bounds: Option<(f64, f64)>,
        y_bounds: Option<(f64, f64)>,
    ) -> Option<CurveRangeConstraint> {
        if !enabled {
            return None;
        }

        let (x_min, x_max) = x_bounds?;
        let (y_min, y_max) = y_bounds?;
        CurveRangeConstraint::new(x_min, x_max, y_min, y_max)
    }

    fn clamped_key_param_update(
        &self,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
        new_value: &ParamValue,
        range: CurveRangeConstraint,
    ) -> Option<(NodeId, f64)> {
        let Some(param_snapshot) = snapshot.node(param) else {
            return None;
        };
        let Some(key_node) = param_snapshot.parent else {
            return None;
        };
        let Some(key_snapshot) = snapshot.node(key_node) else {
            return None;
        };
        if key_snapshot.parent != Some(self.id()) || key_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
            return None;
        }

        let Some(raw_value) = new_value
            .as_float()
            .or_else(|| new_value.as_int().map(|int_value| int_value as f64))
        else {
            return None;
        };

        let clamped = if param_snapshot.decl_id == PARAMETER_ANIMATION_KEY_POSITION_DECL_ID {
            range.clamp_position(raw_value)
        } else if param_snapshot.decl_id == PARAMETER_ANIMATION_KEY_VALUE_DECL_ID {
            range.clamp_value(raw_value)
        } else {
            return None;
        };

        if (clamped - raw_value).abs() <= CURVE_RANGE_EPSILON {
            return None;
        }

        Some((param, clamped))
    }

    fn collect_range_clamp_updates_for_all_keys(
        &self,
        snapshot: &ProcessTreeSnapshot,
        range: CurveRangeConstraint,
    ) -> Vec<(NodeId, f64)> {
        let mut updates = Vec::<(NodeId, f64)>::new();
        for child in snapshot.child_ids(self.id()) {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
                continue;
            }

            if let Some(position_param) = snapshot.find_child(child, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID) {
                if let Some(new_value) = snapshot
                    .node(position_param)
                    .and_then(|entry| entry.param_value.as_ref())
                {
                    if let Some(update) = self.clamped_key_param_update(snapshot, position_param, new_value, range) {
                        updates.push(update);
                    }
                }
            }
            if let Some(value_param) = snapshot.find_child(child, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID) {
                if let Some(new_value) = snapshot.node(value_param).and_then(|entry| entry.param_value.as_ref()) {
                    if let Some(update) = self.clamped_key_param_update(snapshot, value_param, new_value, range) {
                        updates.push(update);
                    }
                }
            }
        }
        updates
    }

    fn collect_key_reorder_moves_by_position(&self, snapshot: &ProcessTreeSnapshot) -> Vec<(NodeId, Option<NodeId>)> {
        let children = snapshot.child_ids(self.id());
        if children.len() < 2 {
            return Vec::new();
        }

        let mut key_entries = Vec::<(usize, NodeId, f64)>::new();
        for (index, child) in children.iter().copied().enumerate() {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
                continue;
            }
            let position = read_child_param_f64(snapshot, child, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, 0.0);
            key_entries.push((index, child, position));
        }
        if key_entries.len() < 2 {
            return Vec::new();
        }

        let mut sorted_key_entries = key_entries.clone();
        sorted_key_entries.sort_by(|left, right| {
            if (left.2 - right.2).abs() <= KEY_ORDER_POSITION_EPSILON {
                left.0.cmp(&right.0)
            } else {
                left.2.total_cmp(&right.2)
            }
        });

        let current_key_order = key_entries.iter().map(|(_, key_id, _)| *key_id).collect::<Vec<_>>();
        let sorted_key_order = sorted_key_entries
            .iter()
            .map(|(_, key_id, _)| *key_id)
            .collect::<Vec<_>>();
        if current_key_order == sorted_key_order {
            return Vec::new();
        }

        let sorted_key_set = sorted_key_order.iter().copied().collect::<HashSet<NodeId>>();
        let mut sorted_key_iter = sorted_key_order.into_iter();
        let mut desired_children = Vec::<NodeId>::with_capacity(children.len());
        for child in children.iter().copied() {
            if sorted_key_set.contains(&child) {
                if let Some(next_key) = sorted_key_iter.next() {
                    desired_children.push(next_key);
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
            if !sorted_key_set.contains(&desired_child) {
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

    fn sync_range_node_presence(&mut self, ctx: &mut ProcessCtx) {
        let mut range_children = Vec::<NodeId>::new();
        if let Some(snapshot) = ctx.tree_snapshot() {
            for child in snapshot.child_ids(self.id()) {
                let Some(child_snapshot) = snapshot.node(child) else {
                    continue;
                };
                if child_snapshot.decl_id == PARAMETER_ANIMATION_RANGE_DECL_ID {
                    range_children.push(child);
                }
            }
        }

        if self.user_can_edit_range {
            self.range_node = range_children.first().copied();
            for duplicate in range_children.iter().skip(1).copied() {
                self.remove_child(ctx, duplicate);
            }
            if self.range_node.is_none() {
                self.add_child_boxed(
                    ctx,
                    Box::new(CurveRangeNode::new(
                        self.code_range_constraint,
                        self.code_range_constraint.is_some(),
                    )),
                    None,
                );
            }
            return;
        }

        self.range_node = None;
        for child in range_children {
            self.remove_child(ctx, child);
        }
    }

    fn sync_keys_and_clamp_to_active_range(&mut self, ctx: &mut ProcessCtx) {
        self.sync_range_node_presence(ctx);

        let existing_key_count = ctx
            .tree_snapshot()
            .map(|snapshot| {
                snapshot
                    .child_ids(self.id())
                    .into_iter()
                    .filter(|node_id| {
                        snapshot
                            .node(*node_id)
                            .is_some_and(|node| node.node_type == PARAMETER_ANIMATION_KEY_NODE_TYPE)
                    })
                    .count()
            })
            .unwrap_or(0);
        let pending_key_count = self.pending_key_add_count(ctx);
        if existing_key_count > 0 {
            self.default_keys_seeded = true;
        }

        if existing_key_count + pending_key_count == 0 && !self.default_keys_seeded {
            let initial_range = self.initial_key_range_constraint();
            ctx.add_child_boxed(
                self.id(),
                Box::new(CurveKeyNode::new_with_values_and_range(0.0, 0.0, initial_range)),
                None,
            );
            ctx.add_child_boxed(
                self.id(),
                Box::new(CurveKeyNode::new_with_values_and_range(1.0, 1.0, initial_range)),
                None,
            );
            self.default_keys_seeded = true;
        }

        let clamp_updates = ctx
            .tree_snapshot()
            .and_then(|snapshot| {
                self.effective_range_constraint(snapshot)
                    .map(|range| self.collect_range_clamp_updates_for_all_keys(snapshot, range))
            })
            .unwrap_or_default();
        for (param, value) in clamp_updates {
            ctx.set_param_with_behaviour(param, ParamValue::Float(value), ParameterEventBehaviour::Coalesce);
        }

        let reorder_moves = ctx
            .tree_snapshot()
            .map(|snapshot| self.collect_key_reorder_moves_by_position(snapshot))
            .unwrap_or_default();
        for (child, after) in reorder_moves {
            self.move_child(ctx, child, self.id(), after);
        }
    }
}

impl Node for CurveNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_CURVE_NODE_TYPE
    }

    fn type_description(&self) -> Option<&str> {
        Some("Internal node hosting one animation-curve key list.")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn user_item_kind(&self) -> &str {
        PARAMETER_ANIMATION_CURVE_ITEM_KIND
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[PARAMETER_ANIMATION_KEY_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new(
            PARAMETER_ANIMATION_KEY_NODE_TYPE,
            PARAMETER_ANIMATION_KEY_ITEM_KIND,
            "Key",
        )]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        let normalized = node_type.trim().to_ascii_lowercase();
        if normalized == PARAMETER_ANIMATION_KEY_NODE_TYPE || normalized == "key" {
            return Some(Box::new(CurveKeyNode::new_with_label_and_range(
                "Key",
                self.initial_key_range_constraint(),
            )));
        }
        None
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        self.sync_keys_and_clamp_to_active_range(ctx);
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.sync_keys_and_clamp_to_active_range(ctx);
    }

    fn engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        let (mut remove_children, add_range_node, clamp_updates, reorder_moves) = {
            let Some(snapshot) = ctx.tree_snapshot() else {
                return;
            };

            let mut remove_children = Vec::<NodeId>::new();
            let mut add_range_node = false;
            let mut clamp_updates = HashMap::<NodeId, f64>::new();
            let mut pending_key_param_values = HashMap::<NodeId, f64>::new();
            let mut enforce_all = false;
            let mut should_reorder_keys = false;
            let mut tracked_range_node = self
                .range_node
                .or_else(|| snapshot.find_child(self.id(), PARAMETER_ANIMATION_RANGE_DECL_ID));
            let mut editable_range_enabled = tracked_range_node
                .and_then(|range_node| snapshot.node(range_node).map(|node| node.enabled))
                .unwrap_or(false);
            let mut editable_x_bounds = tracked_range_node.and_then(|range_node| {
                read_range_axis_bounds(snapshot, range_node, PARAMETER_ANIMATION_RANGE_X_DECL_ID)
            });
            let mut editable_y_bounds = tracked_range_node.and_then(|range_node| {
                read_range_axis_bounds(snapshot, range_node, PARAMETER_ANIMATION_RANGE_Y_DECL_ID)
            });
            let mut range_constraint = if self.user_can_edit_range {
                Self::editable_range_constraint(editable_range_enabled, editable_x_bounds, editable_y_bounds)
            } else {
                self.code_range_constraint
            };

            for event in &ctx.events {
                match &event.kind {
                    EventKind::ChildAdded { parent, child, decl_id } if *parent == self.id() => {
                        if decl_id.0 == PARAMETER_ANIMATION_RANGE_DECL_ID {
                            if !self.user_can_edit_range {
                                remove_children.push(*child);
                                continue;
                            }
                            if let Some(existing) = self.range_node {
                                if existing != *child {
                                    remove_children.push(*child);
                                    continue;
                                }
                            }
                            tracked_range_node = Some(*child);
                            editable_range_enabled = snapshot.node(*child).is_some_and(|node| node.enabled);
                            editable_x_bounds =
                                read_range_axis_bounds(snapshot, *child, PARAMETER_ANIMATION_RANGE_X_DECL_ID);
                            editable_y_bounds =
                                read_range_axis_bounds(snapshot, *child, PARAMETER_ANIMATION_RANGE_Y_DECL_ID);
                            range_constraint = Self::editable_range_constraint(
                                editable_range_enabled,
                                editable_x_bounds,
                                editable_y_bounds,
                            );
                        }
                        self.bind_decl_child(decl_id.0.as_str(), *child);
                        enforce_all = true;
                        should_reorder_keys = true;
                    }
                    EventKind::ChildReplaced {
                        parent,
                        old,
                        new,
                        decl_id,
                    } if *parent == self.id() => {
                        self.unbind_child(*old);
                        if decl_id.0 == PARAMETER_ANIMATION_RANGE_DECL_ID {
                            if !self.user_can_edit_range {
                                remove_children.push(*new);
                                continue;
                            }
                            if let Some(existing) = self.range_node {
                                if existing != *new {
                                    remove_children.push(*new);
                                    continue;
                                }
                            }
                            tracked_range_node = Some(*new);
                            editable_range_enabled = snapshot.node(*new).is_some_and(|node| node.enabled);
                            editable_x_bounds =
                                read_range_axis_bounds(snapshot, *new, PARAMETER_ANIMATION_RANGE_X_DECL_ID);
                            editable_y_bounds =
                                read_range_axis_bounds(snapshot, *new, PARAMETER_ANIMATION_RANGE_Y_DECL_ID);
                            range_constraint = Self::editable_range_constraint(
                                editable_range_enabled,
                                editable_x_bounds,
                                editable_y_bounds,
                            );
                        }
                        self.bind_decl_child(decl_id.0.as_str(), *new);
                        enforce_all = true;
                        should_reorder_keys = true;
                    }
                    EventKind::ChildRemoved { parent, child } if *parent == self.id() => {
                        let removed_range = self.range_node == Some(*child);
                        self.unbind_child(*child);
                        if removed_range && self.user_can_edit_range {
                            tracked_range_node = None;
                            editable_range_enabled = false;
                            editable_x_bounds = None;
                            editable_y_bounds = None;
                            range_constraint = None;
                            add_range_node = true;
                        }
                        enforce_all = true;
                        should_reorder_keys = true;
                    }
                    EventKind::ChildReordered { parent, .. } if *parent == self.id() => {
                        enforce_all = true;
                        should_reorder_keys = true;
                    }
                    EventKind::ParamChanged { param, new_value, .. } => {
                        if let Some(param_snapshot) = snapshot.node(*param) {
                            if let Some(key_node) = param_snapshot.parent {
                                if let Some(key_snapshot) = snapshot.node(key_node) {
                                    if key_snapshot.parent == Some(self.id())
                                        && key_snapshot.node_type == PARAMETER_ANIMATION_KEY_NODE_TYPE
                                    {
                                        let decl_id = param_snapshot.decl_id.as_str();
                                        if decl_id == PARAMETER_ANIMATION_KEY_POSITION_DECL_ID
                                            || decl_id == PARAMETER_ANIMATION_KEY_VALUE_DECL_ID
                                        {
                                            if let Some(raw_value) = new_value
                                                .as_float()
                                                .or_else(|| new_value.as_int().map(|int_value| int_value as f64))
                                            {
                                                pending_key_param_values.insert(*param, raw_value);
                                            }
                                        }
                                        if decl_id == PARAMETER_ANIMATION_KEY_POSITION_DECL_ID {
                                            should_reorder_keys = true;
                                        }
                                    }
                                }
                            }
                        }

                        if let (true, Some(range_node), Some(param_snapshot)) =
                            (self.user_can_edit_range, tracked_range_node, snapshot.node(*param))
                        {
                            if param_snapshot.parent == Some(range_node) {
                                if param_snapshot.decl_id == PARAMETER_ANIMATION_RANGE_X_DECL_ID {
                                    editable_x_bounds = new_value.as_vec2().map(|bounds| (bounds.0, bounds.1));
                                    range_constraint = Self::editable_range_constraint(
                                        editable_range_enabled,
                                        editable_x_bounds,
                                        editable_y_bounds,
                                    );
                                    enforce_all = true;
                                } else if param_snapshot.decl_id == PARAMETER_ANIMATION_RANGE_Y_DECL_ID {
                                    editable_y_bounds = new_value.as_vec2().map(|bounds| (bounds.0, bounds.1));
                                    range_constraint = Self::editable_range_constraint(
                                        editable_range_enabled,
                                        editable_x_bounds,
                                        editable_y_bounds,
                                    );
                                    enforce_all = true;
                                }
                            }
                        }
                        if let Some(range) = range_constraint {
                            if let Some(update) = self.clamped_key_param_update(snapshot, *param, new_value, range) {
                                clamp_updates.insert(update.0, update.1);
                            }
                        }
                    }
                    EventKind::MetaChanged { node, patch } => {
                        if self.user_can_edit_range && Some(*node) == tracked_range_node && patch.enabled.is_some() {
                            editable_range_enabled = patch.enabled.unwrap_or(editable_range_enabled);
                            range_constraint = Self::editable_range_constraint(
                                editable_range_enabled,
                                editable_x_bounds,
                                editable_y_bounds,
                            );
                            enforce_all = true;
                        }
                    }
                    _ => {}
                }
            }

            if enforce_all {
                if let Some(range) = range_constraint {
                    for (param, value) in self.collect_range_clamp_updates_for_all_keys(snapshot, range) {
                        clamp_updates.insert(param, value);
                    }
                    for (param, raw_value) in pending_key_param_values {
                        if let Some(update) =
                            self.clamped_key_param_update(snapshot, param, &ParamValue::Float(raw_value), range)
                        {
                            clamp_updates.insert(update.0, update.1);
                        }
                    }
                }
            }

            let mut clamp_updates: Vec<(NodeId, f64)> = clamp_updates.into_iter().collect();
            clamp_updates.sort_unstable_by_key(|(param, _)| param.0);
            let reorder_moves = if should_reorder_keys {
                self.collect_key_reorder_moves_by_position(snapshot)
            } else {
                Vec::new()
            };
            (remove_children, add_range_node, clamp_updates, reorder_moves)
        };

        if !remove_children.is_empty() {
            remove_children.sort_unstable_by_key(|node_id| node_id.0);
            remove_children.dedup();
            for child in remove_children {
                self.remove_child(ctx, child);
            }
        }

        if add_range_node {
            self.add_child_boxed(
                ctx,
                Box::new(CurveRangeNode::new(
                    self.code_range_constraint,
                    self.code_range_constraint.is_some(),
                )),
                None,
            );
        }

        if !clamp_updates.is_empty() {
            for (param, value) in clamp_updates {
                ctx.set_param_with_behaviour(param, ParamValue::Float(value), ParameterEventBehaviour::Coalesce);
            }
        }

        if !reorder_moves.is_empty() {
            for (child, after) in reorder_moves {
                self.move_child(ctx, child, self.id(), after);
            }
        }
    }

    fn on_child_added_decl(&mut self, _ctx: &mut ProcessCtx, parent: NodeId, child: NodeId, decl_id: &DeclId) {
        if parent != self.id() {
            return;
        }
        self.bind_decl_child(decl_id.0.as_str(), child);
    }

    fn on_child_replaced_decl(
        &mut self,
        _ctx: &mut ProcessCtx,
        parent: NodeId,
        old: NodeId,
        new: NodeId,
        decl_id: &DeclId,
    ) {
        if parent != self.id() {
            return;
        }
        self.unbind_child(old);
        self.bind_decl_child(decl_id.0.as_str(), new);
    }

    fn on_child_removed(&mut self, _ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        if parent != self.id() {
            return;
        }
        self.unbind_child(child);
    }

    fn engine_child_event_interest_depth(&self, _event: &Event) -> u32 {
        2
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}
