use crate::node;
use crate::node::Node;
use crate::parameter::Vec2;
use crate::process_ctx::ProcessCtx;

use super::configure_dashboard_widget_options_node;

pub const DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_vec2_pad_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_vec2_pad_options")]
#[children(
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
