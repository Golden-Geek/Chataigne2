use golden_core::{item, node, node::Node, process_ctx::ProcessCtx, update};

#[node]
pub struct ModuleManager {
    allow_dmx: bool,
}

impl ModuleManager {
    pub fn create(label: impl Into<String>, allow_dmx: bool) -> Self {
        Self::new(label.into(), allow_dmx)
    }
}

#[node(from_struct)]
impl Node for ModuleManager {
    golden_core::define_user_item_factory_methods! {
        accepts = ["module"];
        items = [
            {
                node_type: "osc_module",
                item_kind: "module",
                label: "OSC Module",
                create: |_: &Self, label: String| OscModule::create(label),
            },
            {
                node_type: "midi_module",
                item_kind: "module",
                label: "MIDI Module",
                create: |_: &Self, label: String| MidiModule::create(label),
            },
            {
                node_type: "dmx_module",
                item_kind: "module",
                label: "DMX Module",
                when: |this: &Self| this.allow_dmx,
                create: |_: &Self, label: String| DmxModule::create(label),
            },
        ];
    }
}

#[node]
#[params(
    folder(infos, label = "Infos") {
        connected: bool = false (label = "Connected", description = "Whether the module is currently connected");
    }
    folder(parameters, label = "Parameters") {}
    folder(values, label = "Values") {}
)]
pub struct ModuleBase {}

impl ModuleBase {
    pub fn send_command(&mut self, _ctx: &mut ProcessCtx, command: impl AsRef<str>) {
        println!("[{}] send_command: {}", self.node_data().meta.label, command.as_ref());
    }
}

#[node(from_struct)]
impl Node for ModuleBase {
    fn user_item_kind(&self) -> &str {
        "module"
    }
}

#[node]
#[params(
    folder(values, label = "Values", reuse = true) {
    bool_param:bool = true (label = "Boolean Parameter", description = "A boolean parameter stored on a node using the #[param] attribute macro");
    dummy_param: f64 = 2.2 [0.0..10.0] (label = "Dummy Param", description = "A parameter stored on a node using the #[param] attribute macro");
    }
)]
pub struct OscModule {
    base: ModuleBase,
}

impl OscModule {
    pub fn create(label: impl Into<String>) -> Self {
        let label = label.into();
        Self::new(label.clone(), ModuleBase::new(label))
    }

    pub fn send_command(&mut self, ctx: &mut ProcessCtx, command: impl AsRef<str>) {
        self.base.send_command(ctx, command);
    }
}

// #[update(10)]
#[item("module", via = base, from_struct)]
impl Node for OscModule {
    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.dummy_param.set(ctx, (self.dummy_param.get() + 0.5) % 10.0);
        // println!("OscModule update: dummy_param={}", self.dummy_param.get());
    }
}

#[node]
pub struct MidiModule {
    base: ModuleBase,
}

impl MidiModule {
    pub fn create(label: impl Into<String>) -> Self {
        let label = label.into();
        Self::new(label.clone(), ModuleBase::new(label))
    }

    pub fn send_command(&mut self, ctx: &mut ProcessCtx, command: impl AsRef<str>) {
        self.base.send_command(ctx, command);
    }
}

#[item("module", via = base, from_struct)]
impl Node for MidiModule {}

#[node]
pub struct DmxModule {
    base: ModuleBase,
}

impl DmxModule {
    pub fn create(label: impl Into<String>) -> Self {
        let label = label.into();
        Self::new(label.clone(), ModuleBase::new(label))
    }

    pub fn send_command(&mut self, ctx: &mut ProcessCtx, command: impl AsRef<str>) {
        self.base.send_command(ctx, command);
    }
}

#[item("module", via = base, from_struct)]
impl Node for DmxModule {}
