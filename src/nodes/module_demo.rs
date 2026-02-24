use golden_core::{
    color::Color, item, log, update, node, node::{Node, NodeId, NodeReference}, parameter::{Enum, ParamValue, Vec2, Vec3}, process_ctx::ProcessCtx
};
use uuid::uuid;

pub const MODULE_MANAGER_UUID: golden_core::node::NodeUuid =
	golden_core::node::NodeUuid(uuid!("3f0d7ac2-5c7a-4d8f-85e2-2c6e6cf3b451"));

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
        connected: bool = true (label = "Connected", description = "Whether the module is currently connected", read_only = true);
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
        int_param: i32 = 4  (label = "Integer Parameter", description = "An integer parameter with range", can_be_disabled = true, enabled = false);
        float_param: f64 = 0.75 [0.0..10.0] (label = "Float Parameter", description = "A floating-point parameter with range");
        string_param: String = "/example/address".to_string() (label = "String Parameter", description = "A string parameter");
        vec2_param: Vec2 = (0.5, 0.25) (label = "Vec2 Parameter", description = "A 2D vector parameter", read_only = false);
    }
    folder(values, label = "Values", reuse = true) {
       
        vec3_param: Vec3 = (0.1, 0.2, 0.3) (label = "Vec3 Parameter", description = "A 3D vector parameter");
        color_param: Color = (0.9, 0.4, 0.2, 1.0) (label = "Color Parameter", description = "An RGBA color parameter");
        reference_param: NodeReference (
            label = "Reference Parameter",
            description = "A node reference parameter",
            reference_root = golden_core::parameter::ReferenceRoot::Uuid(MODULE_MANAGER_UUID),
            // reference_default_search_filter = Some("values".to_string()),
            reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly,
            reference_custom_filter_key = Some("module_values_parameters".to_string()),
        );
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

// #[update(50)]
#[item("module", via = base, from_struct)]
impl Node for OscModule {

    fn init(&mut self, ctx: &mut ProcessCtx) {
        log!("Initializing OSC Module: ", self.node_data().meta.label);
        // Surface warnings coming from generated parameter descendants on the modu le row.
        self.set_child_warning_depth(ctx, 2);
        // Enable typical user-edit permissions so the UI can offer context-menu actions
        self.node_data_mut().meta.user_permissions = node::NodeUserPermissions::all();
        // self.float_param.set_warning_with(
        //     ctx,
        //     None,
        //     format!("Can't bind port {}", self.float_param.get()),
        //     Some("This is some additional info that can be shown in the UI when hovering the warning icon."),
        // );
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let val = (self.float_param.get() + 0.2) % 10.0;
        self.float_param.set(ctx, val);
        // if val > 7.5 {
        //     log!(level= error; "New value :",  self.float_param.get());

        // } else if val > 5.0 {
        //     log!(level= warning; "New value :",  self.float_param.get());

        // } else {
        //     log!("New value :",  self.float_param.get());
        // };
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, node_id: NodeId, _old_value: ParamValue) {
        if node_id == self.bool_param.id() {
            if self.bool_param.get() {
                // self.bool_param.set_warning(ctx, "bool_param is true!");
            } else {
                self.bool_param.clear_warning(ctx, None);
            }
        }
    }
}

#[node]
#[params(
folder(values, label = "Values", reuse = true) {
       
        vec3_param: Vec3 = (0.1, 0.2, 0.3) (label = "Vec3 Parameter", description = "A 3D vector parameter");
        color_param: Color = (0.9, 0.4, 0.2, 1.0) (label = "Color Parameter", description = "An RGBA color parameter");
        reference_param: NodeReference (
            label = "Reference Parameter",
            description = "A node reference parameter",
            reference_root = golden_core::parameter::ReferenceRoot::Uuid(MODULE_MANAGER_UUID),
            reference_default_search_filter = Some("values".to_string()),
            reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly,
            reference_custom_filter_key = Some("module_values_parameters".to_string()),
        );
        enum_param: Enum (
            label = "Enum Parameter",
            read_only = true,
            description = "An enum parameter with simple string-list options",
            enum_options = ["off", "on", "auto (default)", "Super long option label"],
        );
    }
)]
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
impl Node for MidiModule {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        // Allow UI actions (color, delete/duplicate, constraints)
        self.node_data_mut().meta.user_permissions = node::NodeUserPermissions::all();
    }
}

#[node]
#[params(
folder(values, label = "Values", reuse = true) {
       
        vec3_param: Vec3 = (0.1, 0.2, 0.3) (label = "Vec3 Parameter", description = "A 3D vector parameter");
        color_param: Color = (0.9, 0.4, 0.2, 1.0) (label = "Color Parameter", description = "An RGBA color parameter");
        reference_param: NodeReference (
            label = "Reference Parameter",
            description = "A node reference parameter",
            reference_root = golden_core::parameter::ReferenceRoot::Uuid(MODULE_MANAGER_UUID),
            // reference_default_search_filter = Some("values".to_string()),
            reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly,
            reference_custom_filter_key = Some("module_values_parameters".to_string()),
        );
        enum_param: Enum (
            label = "Enum Parameter",
            read_only = true,
            description = "An enum parameter with simple string-list options",
            enum_options = ["off", "on", "auto (default)", "Super long option label"],
        );
    }
)]
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
impl Node for DmxModule {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        // Allow UI actions (color, delete/duplicate, constraints)
        self.node_data_mut().meta.user_permissions = node::NodeUserPermissions::all();
    }
}
