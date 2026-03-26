use golden_core::{node, node::Node, parameter::Enum, process_ctx::ProcessCtx};

#[node("osc_transport_settings", label = "OSC Receiver")]
#[children(
    input_interface: Enum = "any" (
        label = "Input Interface",
        description = "Network interface used to receive OSC and as the source binding for outgoing traffic.",
        enum_options = ["any (default)"]
    );
    bind_port: i32 = 9000 [0..65535] (
        label = "Local Port",
        description = "UDP port used by this module to receive OSC messages.",
        widget = "text"
    );
    receive_enabled: bool = true (
        label = "Receive",
        description = "Whether this module should listen for incoming OSC packets."
    );
)]
pub struct OscTransportSettings {
    interface_refresh_elapsed: f64,
}

impl OscTransportSettings {
    pub fn create() -> Self {
        Self::new(1.0)
    }
}

#[node("osc_transport_settings", from_struct)]
impl Node for OscTransportSettings {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.interface_refresh_elapsed = 1.0;
        self.refresh_interface_options(ctx);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.interface_refresh_elapsed += ctx.delta_time.as_secs_f64();
        if self.interface_refresh_elapsed < 1.0 {
            return;
        }

        self.interface_refresh_elapsed = 0.0;
        self.refresh_interface_options(ctx);
    }
}

impl OscTransportSettings {
    fn refresh_interface_options(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(input_interface_id) = snapshot.as_ref().resolve_path_from(self.id(), "input_interface") else {
            return;
        };

        match crate::app::module::common::network_interfaces::available_interface_options() {
            Ok(options) => {
                crate::app::module::common::network_interfaces::sync_interface_enum_options(
                    ctx,
                    input_interface_id,
                    options,
                );
                self.clear_warnings(ctx);
            }
            Err(error) => {
                self.set_warning(ctx, error.as_str());
            }
        }
    }
}
