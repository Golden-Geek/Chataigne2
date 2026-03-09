use crate::node;
use crate::node::Node;
use crate::parameter::{Enum, Vec2};
use crate::process_ctx::ProcessCtx;

use super::{configure_dashboard_widget_options_node, widget_target_can_be_disabled};

/// Runtime node type id for dashboard rotary widget options.
pub const DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_number_rotary_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_number_rotary_options")]
#[children(
    label_placement: Enum = "inside" (
        label = "Label Placement",
        description = "Where the widget label is rendered. Disable to hide it.",
        enum_options = ["left", "right", "top", "bottom", "inside"],
        can_be_disabled = true,
    );
    show_enable_button: bool = true (
        label = "Show Enable Button",
        description = "Whether rotary widgets expose the target enable toggle when the target supports disabling.",
        dependency = |node: &Self, ctx: &ProcessCtx| widget_target_can_be_disabled(node.id(), ctx),
    );
    rotary_show_value_field: bool = true (
        label = "Show Value Field",
        description = "Whether rotary widgets show the numeric value field below the knob.",
    );
    rotary_max_decimals: i32 = 3 [0..8] (
        label = "Max Decimals",
        description = "Maximum number of decimals shown by rotary widgets.",
    );
    rotary_centered_fill: bool = true (
        label = "Centered Fill",
        description = "Whether bipolar ranges fill the knob from the center instead of from the minimum edge.",
    );
    custom_range: Vec2 = (0.0, 1.0) (
        label = "Custom Range",
        description = "Optional widget-local min/max pair. It is intersected with the target parameter range when one exists.",
        enabled = false,
        can_be_disabled = true,
    );
)]
pub struct DashboardNodeWidgetNumberRotaryOptionsNode {}

#[node("dashboard_node_widget_number_rotary_options", from_struct)]
impl Node for DashboardNodeWidgetNumberRotaryOptionsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        configure_dashboard_widget_options_node(self.node_data_mut());
    }
}
