use std::time::{SystemTime, UNIX_EPOCH};

use golden_core::{
    engine::Engine,
    // animation_curve::{CurveEasing, CurveHandle},
    color::Color,
    item,
    log,
    node,
    node::{AnimationCurveNode, Node, NodeId, NodeReference},
    parameter::{Enum, File, ParamValue, Vec2, Vec3},
    process_ctx::ProcessCtx,
};

#[node(label = "Module Manager")]
pub struct ModuleManager {
    #[state(default = true, persist)]
    allow_dmx: bool,
}

#[node(from_struct)]
impl Node for ModuleManager {
    golden_core::define_user_item_factory_methods! {
        accepts = ["module"];
        items = [
            {
                node_type: "osc_module",
                item_kind: "module",
                label: OscModule::default_label(),
                create: |_: &Self| OscModule::create(),
            },
            {
                node_type: "midi_module",
                item_kind: "module",
                label: MidiModule::default_label(),
                create: |_: &Self| MidiModule::create(),
            },
            {
                node_type: "dmx_module",
                item_kind: "module",
                label: DmxModule::default_label(),
                when: |this: &Self| this.allow_dmx,
                create: |_: &Self| DmxModule::create(),
            },
        ];
    }
}

#[node(label = "Module")]
#[children(
    folder(infos, label = "Infos") {
        connected: bool = true (label = "Connected", description = "Whether the module is currently connected", read_only = true);
    }
    folder(parameters, label = "Parameters") {}
    folder(values, label = "Values") {}
)]
pub struct ModuleBase {}

#[node(from_struct, scriptable, contextualizable)]
impl Node for ModuleBase {
    fn user_item_kind(&self) -> &str {
        "module"
    }
}

#[node(label = "OSC Module")]
#[children(
    folder(infos, label = "Infos", reuse = true) {
    }

    folder(parameters, label = "Parameters", reuse = true) {
        trigger_param: ParamValue = ParamValue::Trigger() (label = "Test Trigger Parameter", description = "A trigger parameter using ParamValue::Trigger()");
        bool_param: bool = true (label = "Test Boolean Parameter", description = "Test boolean parameter");
        int_param: i32 = 4  (label = "Test Integer Parameter", description = "Test integer parameter with range", can_be_disabled = true, enabled = false);
        float_param: f64 = 0.75 [0.0..10.0] (label = "Test Float Parameter", description = "Test floating-point parameter with range");
        string_param: String = "/example/address".to_string() (label = "Test String Parameter", description = "Test string parameter");
        file_param: File = "" (
            label = "Test File Parameter",
            description = "Test file path parameter",
            file_allowed_types = ["script"],
            file_allowed_extensions = ["js", "mjs", "cjs"],
        );
        node animation_curve: AnimationCurveNode = AnimationCurveNode::new() (
            label = "Test Animation Curve",
            description = "Test animation curve parameter with predefined keyframes"
        );
        vec2_param: Vec2 = (0.5, 0.25) (label = "Test Vec2 Parameter", description = "Test 2D vector parameter", read_only = false);
        vec2_range_param: Vec2 = (0.5, 0.25) [(-1.0, -1.0)..(1.0, 2.0)] (label = "Test Vec2 Range Parameter", description = "Test 2D vector parameter with component ranges", read_only = false);
    }
    folder(values, label = "Values", reuse = true) {
       
        vec3_param: Vec3 = (0.1, 0.2, 0.3) (label = "Test Vec3 Parameter", description = "Test 3D vector parameter");
        vec3_range_param: Vec3 = (0.1, 0.2, 0.3) [(0.0, -1.0, 0.2)..(1.0, 2.0, 0.8)] (label = "Test Vec3 Range Parameter", description = "Test 3D vector parameter with component ranges");
        color_param: Color = (0.9, 0.4, 0.2, 1.0) (label = "Test Color Parameter", description = "Test RGBA color parameter");
        reference_param: NodeReference (
            label = "Test Reference Parameter",
            description = "Test node reference parameter",
            reference_root = golden_core::parameter::ReferenceRoot::EngineRoot,
            // reference_default_search_filter = Some("values".to_string()),
            reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly,
            reference_custom_filter_key = Some("module_values_parameters".to_string()),
        );
        enum_param: Enum (
            label = "Test Enum Parameter",
            description = "Test enum parameter with simple string-list options",
            enum_options = ["off", "on", "auto (default)", "Super long option label"],
        );
    }
)]
pub struct OscModule {
    base: ModuleBase,
}

impl OscModule {
    pub fn create() -> Self {
        Self::new(ModuleBase::new())
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
        // self.seed_animation_curve(ctx);
        // self.float_param.set_warning_with(
        //     ctx,
        //     None,
        //     format!("Can't bind port {}", self.float_param.get()),
        //     Some("This is some additional info that can be shown in the UI when hovering the warning icon."),
        // );
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        // self.float_param.set(ctx, (self.float_param.get() + 0.2) % 10.0);

        //get current time
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
        let noise = (now *2.73).cos() * 0.5 + 0.5;

        let tx = (now + noise).cos() ;
        let ty = (now * 1.13+noise*0.32).cos() ;
        let tz = (now * 0.72+noise*0.54).cos() ;
        // println!("now: {}, noise: {}", now, noise);
        self.vec2_param.set(ctx, Vec2::new(tx, ty));
        self.vec2_range_param.set(ctx, Vec2::new(tx, ty));
        self.vec3_param.set(ctx, Vec3::new(tx, ty, tz));

        
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, node_id: NodeId, _old_value: ParamValue) {
        if node_id == self.bool_param.id() {
            if self.bool_param.get() {
                // self.bool_param.set_warning(ctx, "bool_param is true!");
            } else {
                // self.bool_param.clear_warning(ctx, None);
            }
        }

        if node_id == self.float_param.id() {
            let val = self.float_param.get();

           self.string_param.set(ctx, format!("Value: {:.2}", val));
           if self.bool_param.get() {
            if val > 7.5 {
                log!(level= error; "New value :",  self.float_param.get());

            } else if val > 5.0 {
                log!(level= info; "New value :",  self.float_param.get());

            } else {
                log!(level= success; "New value :",  self.float_param.get());
            };
        };
        }
    }

}

#[node(label = "MIDI Module")]
#[children(
folder(values, label = "Values", reuse = true) {
       
        vec3_param: Vec3 = (0.1, 0.2, 0.3) (label = "Vec3 Parameter", description = "A 3D vector parameter");
        color_param: Color = (0.9, 0.4, 0.2, 1.0) (label = "Color Parameter", description = "An RGBA color parameter");
        reference_param: NodeReference (
            label = "Reference Parameter",
            description = "A node reference parameter",
            reference_root = golden_core::parameter::ReferenceRoot::EngineRoot,
            // reference_default_search_filter = Some("values".to_string()),
            reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly,
            reference_custom_filter_key = Some("module_values_parameters".to_string()),
        );
        float_reference_param: NodeReference (
            label = "Float Reference Parameter",
            description = "A node reference parameter that only accepts float parameters",
            reference_root = golden_core::parameter::ReferenceRoot::EngineRoot,
            // reference_default_search_filter = Some("values".to_string()),
            reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly,
            reference_allowed_parameter_types = vec!["float".to_string()],
            reference_allow_projections = true,
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
    pub fn create() -> Self {
        Self::new(ModuleBase::new())
    }
}

#[item("module", via = base, from_struct)]
impl Node for MidiModule {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        // Allow UI actions (color, delete/duplicate, constraints)
        self.node_data_mut().meta.user_permissions = node::NodeUserPermissions::all();
    }

}

#[node(label = "DMX Module")]
#[children(
folder(values, label = "Values", reuse = true) {
       
        vec3_param: Vec3 = (0.1, 0.2, 0.3) (label = "Vec3 Parameter", description = "A 3D vector parameter");
        color_param: Color = (0.9, 0.4, 0.2, 1.0) (label = "Color Parameter", description = "An RGBA color parameter");
        reference_param: NodeReference (
            label = "Reference Parameter",
            description = "A node reference parameter",
            reference_root = golden_core::parameter::ReferenceRoot::EngineRoot,
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
    pub fn create() -> Self {
        Self::new(ModuleBase::new())
    }
}

#[item("module", via = base, from_struct)]
impl Node for DmxModule {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        // Allow UI actions (color, delete/duplicate, constraints)
        self.node_data_mut().meta.user_permissions = node::NodeUserPermissions::all();
    }

}

pub fn register_demo_reference_filters<T: Node>(engine: &mut Engine<T>) {
    engine.register_reference_filter("module_values_parameters", |engine, _param_node, _root, candidate| {
        let Some(candidate_node) = engine.nodes.get(candidate) else {
            return false;
        };
        if candidate_node.engine_param_snapshot().is_none() {
            return false;
        }

        let Some(parent_id) = candidate_node.node_data().parent else {
            return false;
        };
        let Some(parent_node) = engine.nodes.get(parent_id) else {
            return false;
        };
        if parent_node.node_data().meta.decl_id.0 != "values" {
            return false;
        }

        let mut current = Some(parent_id);
        let mut has_module_ancestor = false;
        let mut has_module_manager_ancestor = false;
        while let Some(node_id) = current {
            let Some(node) = engine.nodes.get(node_id) else {
                break;
            };
            if node.user_item_kind() == "module" {
                has_module_ancestor = true;
            }
            if node.get_type() == "module_manager" {
                has_module_manager_ancestor = true;
            }
            current = node.node_data().parent;
        }

        has_module_ancestor && has_module_manager_ancestor
    });
}
