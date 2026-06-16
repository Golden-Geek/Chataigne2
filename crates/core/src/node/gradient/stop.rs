use super::helpers::{make_color_parameter, make_interpolation_parameter, make_position_parameter};
use super::prelude::*;

/// Internal node representing one gradient color stop.
pub struct GradientStopNode {
    node_data: NodeData,
    default_position: f64,
    default_color: Color,
    default_interpolation: GradientInterpolation,
}

impl GradientStopNode {
    /// Creates one stop node defaulting to opaque black at position `0`.
    pub fn new() -> Self {
        Self::new_with_label_and_values(
            "Stop",
            0.0,
            Color::new(0.0, 0.0, 0.0, 1.0),
            GradientInterpolation::Linear,
        )
    }

    /// Creates one stop node with a custom label and default color/position.
    pub fn new_with_label(label: impl Into<String>) -> Self {
        Self::new_with_label_and_values(
            label,
            0.0,
            Color::new(0.0, 0.0, 0.0, 1.0),
            GradientInterpolation::Linear,
        )
    }

    /// Creates one stop node with explicit position, color, and interpolation.
    pub fn new_with_values(position: f64, color: Color, interpolation: GradientInterpolation) -> Self {
        Self::new_with_label_and_values("Stop", position, color, interpolation)
    }

    fn new_with_label_and_values(
        label: impl Into<String>,
        position: f64,
        color: Color,
        interpolation: GradientInterpolation,
    ) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        // The gradient editor drives stops via the ribbon; keep the inline stop inspector folded by default.
        node_data.meta.presentation.collapsed = true;
        Self {
            node_data,
            default_position: position.clamp(0.0, 1.0),
            default_color: color,
            default_interpolation: interpolation,
        }
    }
}

impl Node for GradientStopNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        GRADIENT_STOP_NODE_TYPE
    }

    fn type_description(&self) -> Option<&str> {
        Some("Internal node representing one gradient color stop.")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn user_item_kind(&self) -> &str {
        GRADIENT_STOP_ITEM_KIND
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        if !parameter_child_exists(ctx, self.id(), GRADIENT_STOP_POSITION_DECL_ID) {
            ctx.add_child_boxed(
                self.id(),
                Box::new(make_position_parameter(self.default_position)),
                None,
            );
        }
        if !parameter_child_exists(ctx, self.id(), GRADIENT_STOP_COLOR_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_color_parameter(self.default_color)), None);
        }
        if !parameter_child_exists(ctx, self.id(), GRADIENT_STOP_INTERPOLATION_DECL_ID) {
            ctx.add_child_boxed(
                self.id(),
                Box::new(make_interpolation_parameter(self.default_interpolation)),
                None,
            );
        }
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}
