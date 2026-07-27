use super::easing::CurveEasingNode;
use super::helpers::{default_curve_easing, make_float_parameter};
use super::prelude::*;
use super::range::CurveRangeConstraint;

/// Internal node representing one animation curve key.
pub struct CurveKeyNode {
    node_data: NodeData,
    default_position: f64,
    default_value: f64,
    range_constraint: Option<CurveRangeConstraint>,
    default_easing: CurveEasing,
}

impl Default for CurveKeyNode {
    fn default() -> Self {
        Self::new()
    }
}

impl CurveKeyNode {
    /// Creates one key node with default position/value set to `0`.
    pub fn new() -> Self {
        Self::new_with_label_and_values_and_range_and_easing("Key", 0.0, 0.0, None, default_curve_easing())
    }

    /// Creates one key node with custom label and default position/value.
    pub fn new_with_label(label: impl Into<String>) -> Self {
        Self::new_with_label_and_values_and_range_and_easing(label, 0.0, 0.0, None, default_curve_easing())
    }

    /// Creates one key node with custom label and optional range constraint.
    pub fn new_with_label_and_range(label: impl Into<String>, range_constraint: Option<CurveRangeConstraint>) -> Self {
        Self::new_with_label_and_values_and_range_and_easing(label, 0.0, 0.0, range_constraint, default_curve_easing())
    }

    /// Creates one key node with explicit initial position/value.
    pub fn new_with_values(position: f64, value: f64) -> Self {
        Self::new_with_label_and_values_and_range_and_easing("Key", position, value, None, default_curve_easing())
    }

    /// Creates one key node with explicit initial position/value and optional range constraint.
    pub fn new_with_values_and_range(
        position: f64,
        value: f64,
        range_constraint: Option<CurveRangeConstraint>,
    ) -> Self {
        Self::new_with_label_and_values_and_range_and_easing(
            "Key",
            position,
            value,
            range_constraint,
            default_curve_easing(),
        )
    }

    /// Creates one key node with explicit initial position/value, optional range, and explicit easing.
    pub fn new_with_values_and_range_and_easing(
        position: f64,
        value: f64,
        range_constraint: Option<CurveRangeConstraint>,
        easing: CurveEasing,
    ) -> Self {
        Self::new_with_label_and_values_and_range_and_easing("Key", position, value, range_constraint, easing)
    }

    fn new_with_label_and_values_and_range_and_easing(
        label: impl Into<String>,
        position: f64,
        value: f64,
        range_constraint: Option<CurveRangeConstraint>,
        default_easing: CurveEasing,
    ) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        let (default_position, default_value) = if let Some(range_constraint) = range_constraint {
            (
                range_constraint.clamp_position(position),
                range_constraint.clamp_value(value),
            )
        } else {
            (position, value)
        };
        Self {
            node_data,
            default_position,
            default_value,
            range_constraint,
            default_easing,
        }
    }
}

impl Node for CurveKeyNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_KEY_NODE_TYPE
    }

    fn type_description(&self) -> Option<&str> {
        Some("Internal node representing one animation curve key.")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn user_item_kind(&self) -> &str {
        PARAMETER_ANIMATION_KEY_ITEM_KIND
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        // `engine_on_attached` consults the tree snapshot via `parameter_child_exists`
        // to avoid recreating declared child parameters.
        true
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_KEY_POSITION_DECL_ID) {
            let mut position_param = make_float_parameter(
                "Position",
                PARAMETER_ANIMATION_KEY_POSITION_DECL_ID,
                self.default_position,
            );
            if let Some(range_constraint) = self.range_constraint {
                position_param.constraints.range = Some(RangeConstraint::Uniform {
                    min: Some(range_constraint.x_min),
                    max: Some(range_constraint.x_max),
                });
            }
            ctx.add_child_boxed(self.id(), Box::new(position_param), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_KEY_VALUE_DECL_ID) {
            let mut value_param =
                make_float_parameter("Value", PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, self.default_value);
            if let Some(range_constraint) = self.range_constraint {
                value_param.constraints.range = Some(RangeConstraint::Uniform {
                    min: Some(range_constraint.y_min),
                    max: Some(range_constraint.y_max),
                });
            }
            ctx.add_child_boxed(self.id(), Box::new(value_param), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_EASING_DECL_ID) {
            ctx.add_child_boxed(
                self.id(),
                Box::new(CurveEasingNode::new_with_easing("Easing", self.default_easing.clone())),
                None,
            );
        }
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}
