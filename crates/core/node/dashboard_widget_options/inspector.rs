use crate::node;
use crate::node::Node;
use crate::process_ctx::ProcessCtx;

use super::configure_dashboard_widget_options_node;

pub const DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_inspector_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_inspector_options")]
#[children(
    include_children: bool = true (
        label = "Include Children",
        description = "Whether inspector widgets render child parameters and subnodes.",
    );
)]
pub struct DashboardNodeWidgetInspectorOptionsNode {}

#[node("dashboard_node_widget_inspector_options", from_struct)]
impl Node for DashboardNodeWidgetInspectorOptionsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        configure_dashboard_widget_options_node(self.node_data_mut());
    }
}
