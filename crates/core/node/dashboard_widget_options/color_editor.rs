use crate::node;
use crate::node::Node;
use crate::process_ctx::ProcessCtx;

use super::configure_dashboard_widget_options_node;

pub const DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_color_editor_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_color_editor_options")]
#[children(
    color_force_expanded: bool = false (
        label = "Always Expanded",
        description = "Whether color editor widgets stay expanded instead of collapsing to preview mode.",
    );
    color_show_hex: bool = true (
        label = "Show Hex",
        description = "Whether color editor widgets show the hexadecimal color input.",
    );
    color_show_rgba_fields: bool = true (
        label = "Show RGBA Fields",
        description = "Whether color editor widgets show the RGBA numeric controls.",
    );
)]
pub struct DashboardNodeWidgetColorEditorOptionsNode {}

#[node("dashboard_node_widget_color_editor_options", from_struct)]
impl Node for DashboardNodeWidgetColorEditorOptionsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        configure_dashboard_widget_options_node(self.node_data_mut());
    }
}
