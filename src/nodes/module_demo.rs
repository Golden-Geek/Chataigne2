use golden_core::{
    color::Color,
    item,
    node,
    node::{Node, NodeReference},
    parameter::{Enum, ParamValue, Vec2, Vec3},
    process_ctx::ProcessCtx,
};

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
    folder(infos, label = "Infos", reuse = true) {
    }

    folder(parameters, label = "Parameters", reuse = true) {
         trigger_param: ParamValue = ParamValue::Trigger() (label = "Trigger Parameter", description = "A trigger parameter using ParamValue::Trigger()");
        bool_param: bool = true (label = "Boolean Parameter", description = "A boolean parameter");
        int_param: i32 = 4  (label = "Integer Parameter", description = "An integer parameter with range");
        float_param: f64 = 0.75 [0.0..1.0] (label = "Float Parameter", description = "A floating-point parameter with range");
        string_param: String = "/example/address".to_string() (label = "String Parameter", description = "A string parameter");
        vec2_param: Vec2 = (0.5, 0.25) (label = "Vec2 Parameter", description = "A 2D vector parameter");
    }
    folder(values, label = "Values", reuse = true) {
       
        vec3_param: Vec3 = (0.1, 0.2, 0.3) (label = "Vec3 Parameter", description = "A 3D vector parameter");
        color_param: Color = (0.9, 0.4, 0.2, 1.0) (label = "Color Parameter", description = "An RGBA color parameter");
        reference_param: NodeReference (label = "Reference Parameter", description = "A node reference parameter");
        enum_param: Enum (
            label = "Enum Parameter",
            description = "An enum parameter with simple string-list options",
            enum_options = ["off", "on", "auto (default)", "Super long option label"],
        );
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
    fn update(&mut self, _ctx: &mut ProcessCtx) {
        // self.float_param.set(ctx, (self.float_param.get() + 0.5) % 10.0);
        // println!("OscModule update: float_param={}", self.float_param.get());
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
