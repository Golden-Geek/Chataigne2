use crate::node;
use crate::node::Node;
use crate::parameter::Enum;
use crate::process_ctx::ProcessCtx;

use super::{configure_dashboard_widget_options_node, widget_target_can_be_disabled};

/// Runtime node type id for dashboard inspector widget options.
pub const DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_inspector_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_inspector_options")]
#[children(
    label_placement: Enum = "inside" (
        label = "Label Placement",
        description = "Where the widget label is rendered. Disable to hide it.",
        enum_options = ["left", "right", "top", "bottom", "inside"],
        can_be_disabled = true,
    );
    max_child_level: i32 = 2 [0..16] (
        label = "Max Child Level",
        description = "Deepest descendant level rendered by inspector widgets relative to the target root.",
    );
    show_enable_button: bool = true (
        label = "Show Enable Button",
        description = "Whether inspector widgets expose the target enable toggle when the target supports disabling.",
        dependency = |node: &Self, ctx: &ProcessCtx| widget_target_can_be_disabled(node.id(), ctx),
    );
)]
pub struct DashboardNodeWidgetInspectorOptionsNode {}

#[node("dashboard_node_widget_inspector_options", from_struct)]
impl Node for DashboardNodeWidgetInspectorOptionsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        configure_dashboard_widget_options_node(self.node_data_mut());
    }
}
