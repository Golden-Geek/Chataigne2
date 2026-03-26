use golden_core::{node, node::Node, process_ctx::ProcessCtx};

#[node("osc_output", label = "Output", show_in_nested_inspector = true)]
#[children(
    remote_host: String = "127.0.0.1".to_string() (
        label = "Remote Host",
        description = "Remote OSC destination hostname or IP address."
    );
    remote_port: i32 = 8000 [0..65535] (
        label = "Remote Port",
        description = "UDP port used for outgoing OSC messages.",
        widget = "text"
    );
)]
pub struct OscOutput {}

#[golden_core::item("osc_output", node = "osc_output", from_struct)]
impl Node for OscOutput {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}
