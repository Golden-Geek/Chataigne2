use golden_core::{node, node::Node, process_ctx::ProcessCtx};

#[node("osc_module_base", label = "OSC Module")]
#[children(
    folder(parameters, label = "Parameters", reuse = true) {
        auto_add: bool = true (
            label = "Auto Add",
            description = "Automatically create missing OSC value nodes from incoming addresses."
        );
        node osc: crate::app::OscTransportSettings = crate::app::OscTransportSettings::create() (
            label = "OSC",
            description = "OSC transport configuration for this module."
        );
        node outputs: crate::app::OscOutputManager = crate::app::OscOutputManager::new() (
            label = "Outputs",
            description = "OSC destinations used by this module for outgoing traffic."
        );
    }
)]
pub struct OscModuleBase {
    base: crate::app::ModuleBase,
}

impl OscModuleBase {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleBase::new())
    }
}

#[node("osc_module_base", via = base, from_struct)]
impl Node for OscModuleBase {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "osc_module_base").then(Self::create)
    }
}
