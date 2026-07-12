use crate::node;
use crate::node::Node;
use crate::parameter::{Enum, Vec3};
use crate::process_ctx::ProcessCtx;

use super::configure_dashboard_widget_options_node;

/// Runtime node type id for dashboard vec3-editor widget options.
pub const DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE: &str = "dashboard_node_widget_vec3_editor_options";

#[allow(missing_docs)]
#[node("dashboard_node_widget_vec3_editor_options", label = "Widget Options")]
#[children(
    label_placement: Enum = "inside" (
        label = "Label Placement",
        description = "Where the widget label is rendered. Disable to hide it.",
        enum_options = ["left", "right", "top", "bottom", "inside"],
        can_be_disabled = true,
    );
    show_enable_button: bool = true (
        label = "Show Enable Button",
        description = "Whether vector editor widgets expose the target enable toggle when the target supports disabling.",
    );
    vector_layout: Enum = "inline" (
        label = "Layout",
        description = "Arrangement used for vector editor widgets.",
        enum_options = ["inline", "column"],
    );
    vector_show_value_fields: bool = true (
        label = "Show Value Fields",
        description = "Whether vector editor widgets keep per-component numeric fields visible.",
    );
    vector_max_decimals: i32 = 2 [0..8] (
        label = "Max Decimals",
        description = "Maximum number of decimals shown by vector editor widget fields.",
    );
    folder(custom_range, label = "Custom Range", enabled = false, can_be_disabled = true) {
        range_min: Vec3 = (0.0, 0.0, 0.0) (
            label = "Range Min",
            description = "Optional widget-local minimum. Each component is clamped inside the target parameter range when one exists.",
        );
        range_max: Vec3 = (1.0, 1.0, 1.0) (
            label = "Range Max",
            description = "Optional widget-local maximum. Each component is clamped inside the target parameter range when one exists.",
        );
    }
)]
pub struct DashboardNodeWidgetVec3EditorOptionsNode {}

#[node("dashboard_node_widget_vec3_editor_options", from_struct)]
impl Node for DashboardNodeWidgetVec3EditorOptionsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        configure_dashboard_widget_options_node(self.node_data_mut());
    }
}
