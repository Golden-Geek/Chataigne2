use golden_core::{node, node::Node};

#[node("osc_output", label = "Output")]
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

impl OscOutput {
    pub fn create_with_module_authoring() -> Self {
        let mut output = Self::new();
        crate::app::module::enable_module_authoring(output.node_data_mut());
        output
    }
}

#[golden_core::item("osc_output", node = "osc_output", from_struct)]
impl Node for OscOutput {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create_with_module_authoring)
    }
}
