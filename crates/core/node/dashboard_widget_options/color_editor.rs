use crate::node;
use crate::node::Node;
use crate::parameter::Enum;
use crate::process_ctx::ProcessCtx;

use super::configure_dashboard_widget_options_node;

/// Runtime node type id for dashboard color-editor widget options.
pub const DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_color_editor_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_color_editor_options", label = "Widget Options")]
#[children(
    label_placement: Enum = "inside" (
        label = "Label Placement",
        description = "Where the widget label is rendered. Disable to hide it.",
        enum_options = ["left", "right", "top", "bottom", "inside"],
        can_be_disabled = true,
    );
    show_enable_button: bool = true (
        label = "Show Enable Button",
        description = "Whether color editor widgets expose the target enable toggle when the target supports disabling.",
    );
    color_force_expanded: bool = true (
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
