use crate::node;
use crate::node::Node;
use crate::parameter::Enum;
use crate::process_ctx::ProcessCtx;

use super::configure_dashboard_widget_options_node;

/// Runtime node type id for dashboard parameter-editor widget options.
pub const DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_parameter_editor_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_parameter_editor_options", label = "Widget Options")]
#[children(
    label_placement: Enum = "inside" (
        label = "Label Placement",
        description = "Where the widget label is rendered. Disable to hide it.",
        enum_options = ["left", "right", "top", "bottom", "inside"],
        can_be_disabled = true,
    );
    show_enable_button: bool = true (
        label = "Show Enable Button",
        description = "Whether parameter widgets expose the target enable toggle when the target supports disabling.",
    );
)]
pub struct DashboardNodeWidgetParameterEditorOptionsNode {}

#[node("dashboard_node_widget_parameter_editor_options", from_struct)]
impl Node for DashboardNodeWidgetParameterEditorOptionsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        configure_dashboard_widget_options_node(self.node_data_mut());
    }
}
