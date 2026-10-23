//! App-owned Stream Deck pages, script intents and passive native service.
mod input;
mod layout;
mod output;
// Native implementations stay available for hardware-feature and fake-provider builds.
#[cfg_attr(not(any(feature = "streamdeck-hid", test)), allow(dead_code))]
mod runtime;
mod service;
use crate::app::module::common::paging;
use golden_core::{
    edit::{Edit, NodeTree},
    engine::NodeExecutionRule,
    events::Event,
    node,
    node::{Folder, Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
const KEYS_FOLDER: &str = "keys";
const NO_DEVICE_VARIANT: &str = "none";
const AUTO_DEVICE_VARIANT: &str = "auto";
const DEFAULT_MODEL_VARIANT: &str = "standard";
const KEY_PRESSED_CALLBACK: &str = "streamDeckKeyPressed";
const KEY_RELEASED_CALLBACK: &str = "streamDeckKeyReleased";
const PAGE_CHANGED_CALLBACK: &str = "streamDeckPageChanged";
const STREAMDECK_SCRIPT_METHODS: &[&str] = &["addPage", "removePage", "pressKey", "releaseKey"];
const STREAMDECK_COMMAND_TYPES: &[&str] = &[];
const FIELD_COLOR: usize = 0;
const FIELD_TEXT: usize = 1;
const FIELD_IMAGE: usize = 2;
const FIELD_UNPAGED: usize = 3;
/// Supported Stream Deck families and their physical key counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StreamDeckModel {
    Mini,
    Standard,
    Xl,
    Plus,
    Pedal,
}

impl StreamDeckModel {
    const ALL: [Self; 5] = [Self::Mini, Self::Standard, Self::Xl, Self::Plus, Self::Pedal];

    fn id(self) -> &'static str {
        match self {
            Self::Mini => "mini",
            Self::Standard => "standard",
            Self::Xl => "xl",
            Self::Plus => "plus",
            Self::Pedal => "pedal",
        }
    }

    fn key_count(self) -> usize {
        match self {
            Self::Mini => 6,
            Self::Standard => 15,
            Self::Xl => 32,
            Self::Plus => 8,
            Self::Pedal => 3,
        }
    }

    fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|model| model.id() == id)
            .unwrap_or(Self::Standard)
    }
}

#[node("streamdeck_module", label = "Stream Deck")]
#[children(
    folder(connection) {
        device: Enum = AUTO_DEVICE_VARIANT (
            label = "Device",
            description = "Stream Deck to drive. Auto uses the first connected device.",
            enum_options = ["auto", "none"]
        );
        [base_children];
    }
    folder(parameters) {
        model: Enum = DEFAULT_MODEL_VARIANT (
            label = "Model",
            description = "Stream Deck family. Sets the number of keys and rebuilds the layout.",
            enum_options = ["mini", "standard", "xl", "plus", "pedal"]
        );
        brightness: f64 = 0.8 [0..1] (
            label = "Brightness",
            description = "Global key backlight brightness.",
        );
        // Default-page control surface (appearance). `pages/` is created beside this at runtime.
        folder(keys, label = "Keys", tags = vec![paging::PAGEABLE_TAG.to_string()]) {}
        [base_children];
    }
    folder(values) {
        // Default-page inputs (flat booleans). `pages/` mirror is created beside this on demand.
        folder(keys, label = "Keys") {}
        [base_children];
    }
)]
pub struct StreamDeckModule {
    base: crate::app::ModuleBase,
    service: service::Service,
    layout: layout::Layout,
    output: output::Output,
    last_active_page: String,
}
impl StreamDeckModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            service::Service::new(),
            layout::Layout::default(),
            output::Output::default(),
            String::new(),
        )
    }
    fn model(&self) -> StreamDeckModel {
        StreamDeckModel::from_id(self.model.get_ref().as_str())
    }
    fn layout_owner(&self) -> Result<layout::Owner, String> {
        Ok(layout::Owner {
            module: self.id(),
            parameters: self.base.parameters_id().ok_or("Stream Deck parameters unavailable")?,
            values: self.base.values_id().ok_or("Stream Deck values unavailable")?,
            count: self.model().key_count(),
            model: self.model.id(),
        })
    }
    fn warning(&self, ctx: &mut ProcessCtx, error: &str) {
        NodeHandle::new(self.id()).set_warning_with(ctx, Some("streamdeck_runtime"), error, None);
    }
    fn emit_callback(&self, ctx: &mut ProcessCtx, name: &str, args: Vec<serde_json::Value>) {
        crate::app::module::script_api::emit_script_callback(ctx, self.id(), name, args);
    }
    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        let enabled = self.device.get_ref().as_str() != NO_DEVICE_VARIANT;
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(enabled, enabled));
    }
    fn sync_initial_options(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let models = vec![
            enum_option("mini", "Mini (6 keys)", 0),
            enum_option("standard", "Standard / MK.2 (15 keys)", 1),
            enum_option("xl", "XL (32 keys)", 2),
            enum_option("plus", "Plus (8 keys)", 3),
            enum_option("pedal", "Pedal (3 keys)", 4),
        ];
        choice_options(ctx, &snapshot, self.model.id(), models);
        let mut devices = vec![
            enum_option(AUTO_DEVICE_VARIANT, "Auto", 0),
            enum_option(NO_DEVICE_VARIANT, "No Device", 1),
        ];
        let selected = self.device.get_ref();
        let selected = output::selection(selected.as_str());
        if selected.len() <= runtime::MAX_SERIAL && selected != AUTO_DEVICE_VARIANT && selected != NO_DEVICE_VARIANT {
            devices.push(enum_option(selected, &format!("Missing: {selected}"), 2));
        }
        choice_options(ctx, &snapshot, self.device.id(), devices);
    }
}
#[golden_core::item("module",node="streamdeck_module",via=base,from_struct,menu_path=["Controllers"])]
impl Node for StreamDeckModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, STREAMDECK_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
    }
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.sync_initial_options(ctx);
        if let Err(error) = self.register_service(ctx) {
            self.warning(ctx, &error);
        }
        self.request_service();
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service_turn(ctx);
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.streamdeck")
    }
    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.layout.cancel();
        self.stop_runtime();
    }
    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }
    fn on_structure_changed(&mut self, _ctx: &mut ProcessCtx) {
        self.layout.mark();
        self.output.invalidate();
        self.request_service();
    }
    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, patch: NodeMetaPatch) {
        if patch.label.is_some() || patch.short_name.is_some() {
            self.layout.mark();
            self.output.invalidate();
            self.request_service();
        }
    }
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            STREAMDECK_SCRIPT_METHODS,
        )
    }
    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        match method {
            "addPage" | "removePage" => {
                let name = match args.first() {
                    Some(ParamValue::Str(value) | ParamValue::Enum(value)) => value.as_str(),
                    _ => "",
                };
                if name.len() > layout::MAX_LABEL {
                    return Err("Stream Deck page name/id exceeds 512 bytes".into());
                }
                let snapshot = ctx
                    .tree_snapshot_arc()
                    .ok_or("Stream Deck page authoring requires a snapshot")?;
                let owner = self.layout_owner()?;
                if method == "addPage" {
                    self.layout.add(&snapshot, owner, name)?;
                } else {
                    self.layout.remove(&snapshot, owner, name)?;
                }
                self.request_service();
                Ok(true)
            }
            "pressKey" | "releaseKey" => {
                let index = args
                    .first()
                    .and_then(ParamValue::as_int)
                    .ok_or("Stream Deck key index must be an integer")?;
                if index < 0 {
                    return Err("Stream Deck key index must be between 0 and 63".into());
                }
                self.script_input(ctx, index as usize, method == "pressKey")?;
                Ok(true)
            }
            _ => self.base.engine_call_script_method(ctx, method, args),
        }
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.base.emit_script_param_callback(ctx, &snapshot, param, &old_value);
            if self.device.is_bound() && self.device.id() == param {
                let current = self.device.get_ref();
                let previous = match &old_value {
                    ParamValue::Enum(value) => output::selection(value),
                    _ => "",
                };
                if output::selection(current.as_str()) != previous {
                    self.output.cancel();
                    self.base.set_connected(ctx, false);
                }
                self.refresh_data_capabilities(ctx);
            }
            if self.model.is_bound() && self.model.id() == param {
                self.layout.mark();
                self.layout.ready = false;
            }
            let mut current = Some(param);
            let mut control = false;
            for _ in 0..20 {
                let Some(id) = current else {
                    break;
                };
                if Some(id) == self.base.parameters_id() || self.device.id() == param {
                    control = true;
                    break;
                }
                current = snapshot.node(id).and_then(|n| n.parent);
            }
            if control {
                self.output.invalidate();
                self.request_service();
            }
        }
    }
    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.layout.mark();
            self.output.invalidate();
            self.refresh_data_capabilities(ctx);
            self.request_service();
        } else {
            self.layout.cancel();
            self.stop_runtime();
            self.service.pressure = false;
            NodeHandle::new(self.id()).clear_warning(ctx, Some("streamdeck_pressure"));
            self.base.set_connected(ctx, false);
        }
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}
fn enum_option(variant_id: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.into(),
        value: ParamValue::Enum(variant_id.into()),
        label: label.into(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}
fn choice_options(ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, id: NodeId, options: Vec<ParameterEnumOption>) {
    let Some(node) = snapshot.node(id) else {
        return;
    };
    let Some(value) = &node.param_value else {
        return;
    };
    if node
        .param_constraints
        .as_ref()
        .is_some_and(|c| c.enum_options.as_slice() == options.as_slice())
    {
        return;
    }
    ctx.edits.push(Edit::SetParamEnumOptions {
        node: id,
        expected_uuid: node.uuid,
        expected_value: value.clone(),
        selected_value: None,
        options: options.into(),
    });
}
#[cfg(test)]
mod tests;
