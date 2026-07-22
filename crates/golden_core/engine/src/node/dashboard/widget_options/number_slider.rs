use crate::node;
use crate::node::Node;
use crate::parameter::{Enum, Vec2};
use crate::process_ctx::ProcessCtx;

use super::configure_dashboard_widget_options_node;

/// Runtime node type id for dashboard slider widget options.
pub const DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_number_slider_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_number_slider_options", label = "Widget Options")]
#[children(
    label_placement: Enum = "inside" (
        label = "Label Placement",
        description = "Where the widget label is rendered. Disable to hide it.",
        enum_options = ["left", "right", "top", "bottom", "inside"],
        can_be_disabled = true,
    );
    show_enable_button: bool = true (
        label = "Show Enable Button",
        description = "Whether slider widgets expose the target enable toggle when the target supports disabling.",
    );
    slider_show_value_field: bool = true (
        label = "Show Value Field",
        description = "Whether slider widgets keep the numeric value field visible next to the slider.",
    );
    slider_max_decimals: i32 = 3 [0..8] (
        label = "Max Decimals",
        description = "Maximum number of decimals shown by slider widgets.",
    );
    custom_range: Vec2 = (0.0, 1.0) (
        label = "Custom Range",
        description = "Optional widget-local min/max pair. It is intersected with the target parameter range when one exists.",
        enabled = false,
        can_be_disabled = true,
    );
)]
pub struct DashboardNodeWidgetNumberSliderOptionsNode {}

#[node("dashboard_node_widget_number_slider_options", from_struct)]
impl Node for DashboardNodeWidgetNumberSliderOptionsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        configure_dashboard_widget_options_node(self.node_data_mut());
    }
}
