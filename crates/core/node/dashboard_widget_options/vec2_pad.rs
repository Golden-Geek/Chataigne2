use crate::node;
use crate::node::Node;
use crate::parameter::{Enum, Vec2};
use crate::process_ctx::ProcessCtx;

use super::{configure_dashboard_widget_options_node, widget_target_can_be_disabled};

/// Runtime node type id for dashboard vec2-pad widget options.
pub const DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_vec2_pad_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_vec2_pad_options", label = "Widget Options")]
#[children(
    label_placement: Enum = "inside" (
        label = "Label Placement",
        description = "Where the widget label is rendered. Disable to hide it.",
        enum_options = ["left", "right", "top", "bottom", "inside"],
        can_be_disabled = true,
    );
    show_enable_button: bool = true (
        label = "Show Enable Button",
        description = "Whether 2D pad widgets expose the target enable toggle when the target supports disabling.",
        dependency = |node: &Self, ctx: &ProcessCtx| widget_target_can_be_disabled(node.id(), ctx),
    );
    trail_time: f64 = 2.0 [0.0..60.0] (
        label = "Trail Time",
        description = "How many seconds of motion history the 2D pad shows. Disable to hide the trail.",
        enabled = false,
        can_be_disabled = true,
    );
    folder(custom_range, label = "Custom Range", enabled = false, can_be_disabled = true) {
        range_min: Vec2 = (0.0, 0.0) (
            label = "Range Min",
            description = "Optional widget-local minimum. Each component is clamped inside the target parameter range when one exists.",
        );
        range_max: Vec2 = (1.0, 1.0) (
            label = "Range Max",
            description = "Optional widget-local maximum. Each component is clamped inside the target parameter range when one exists.",
        );
    }
)]
pub struct DashboardNodeWidgetVec2PadOptionsNode {}

#[node("dashboard_node_widget_vec2_pad_options", from_struct)]
impl Node for DashboardNodeWidgetVec2PadOptionsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        configure_dashboard_widget_options_node(self.node_data_mut());
    }
}
