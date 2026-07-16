//! Elgato Stream Deck module — the reference consumer of the [paging] framework.
//!
//! ## Structure (input vs control, paged on both sides)
//!
//! * **`values/keys`** — flat `pressed` booleans for the *default* page; `values/pages/<id>`
//!   mirrors the inputs for each derived page. Inputs route to the active page.
//! * **`parameters/keys`** — per-key control (`color`, `text`, `image`, `unpaged`) for the
//!   default page; `parameters/pages/<id>` holds the derived control pages (managed by the
//!   [`PageHost`](paging)). `keys` and `pages` are siblings.
//!
//! The `model` parameter (Mini / Standard / XL / Plus / Pedal) drives the key count and
//! resizes every page (default + derived) on both sides. Keys are 1-based.
//!
//! [paging]: crate::app::module::common::paging

mod streamdeck_runtime;

use golden_core::{
    edit::{Edit, NodeTree},
    engine::NodeExecutionRule,
    events::Event,
    node,
    node::{Folder, Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::paging;

use self::streamdeck_runtime::{
    connect, discover_devices, DiscoveredStreamDeck, KeyVisual, StreamDeckDevice, StreamDeckInputEvent,
};

const STREAMDECK_UPDATE_RATE_HZ: u32 = 60;
const STREAMDECK_DISCOVER_INTERVAL_SECS: f64 = 2.0;
const KEYS_FOLDER: &str = "keys";

const NO_DEVICE_VARIANT: &str = "none";
const AUTO_DEVICE_VARIANT: &str = "auto";
const DEFAULT_MODEL_VARIANT: &str = "standard";

const KEY_PRESSED_CALLBACK: &str = "streamDeckKeyPressed";
const KEY_RELEASED_CALLBACK: &str = "streamDeckKeyReleased";
const PAGE_CHANGED_CALLBACK: &str = "streamDeckPageChanged";

const STREAMDECK_SCRIPT_METHODS: &[&str] = &["addPage", "removePage", "pressKey", "releaseKey"];
const STREAMDECK_COMMAND_TYPES: &[&str] = &[];

// Declaration order of a control-key folder's primitives (resolved positionally).
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
        Self::ALL.into_iter().find(|model| model.id() == id).unwrap_or(Self::Standard)
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
    hardware: Option<Box<dyn StreamDeckDevice>>,
    hardware_dirty: bool,
    discover_elapsed: f64,
    known_devices: Vec<DiscoveredStreamDeck>,
    devices_dirty: bool,
    structure_dirty: bool,
    pages_dirty: bool,
    last_active_page: String,
    last_visuals: Vec<KeyVisual>,
    button_down: Vec<bool>,
    /// Editor/script-injected "simulate press" events, drained alongside real device input.
    pending_input: Vec<StreamDeckInputEvent>,
    suppress_hardware: bool,
}

impl StreamDeckModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            true,
            STREAMDECK_DISCOVER_INTERVAL_SECS,
            Vec::new(),
            true,
            true,
            true,
            String::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            false,
        )
    }

    #[cfg(test)]
    pub(crate) fn install_simulated_device(&mut self, device: streamdeck_runtime::SimulatedStreamDeck) {
        self.button_down = vec![false; device.key_count()];
        self.last_visuals = vec![KeyVisual::default(); device.key_count()];
        self.hardware = Some(Box::new(device));
        self.hardware_dirty = false;
        self.suppress_hardware = true;
    }

    #[cfg(test)]
    pub(crate) fn simulated(&self) -> &streamdeck_runtime::SimulatedStreamDeck {
        self.hardware
            .as_ref()
            .and_then(|device| device.as_any().downcast_ref::<streamdeck_runtime::SimulatedStreamDeck>())
            .expect("simulated Stream Deck device should be installed")
    }

    #[cfg(test)]
    pub(crate) fn simulated_mut(&mut self) -> &mut streamdeck_runtime::SimulatedStreamDeck {
        self.hardware
            .as_mut()
            .and_then(|device| device.as_any_mut().downcast_mut::<streamdeck_runtime::SimulatedStreamDeck>())
            .expect("simulated Stream Deck device should be installed")
    }

    // ---- structural lookups ------------------------------------------------

    fn model(&self) -> StreamDeckModel {
        StreamDeckModel::from_id(self.model.get_ref().as_str())
    }

    fn control_keys_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        snapshot.find_child(self.base.parameters_id()?, KEYS_FOLDER)
    }

    fn value_keys_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        snapshot.find_child(self.base.values_id()?, KEYS_FOLDER)
    }

    // ---- model-driven structure generation ---------------------------------

    fn sync_structure(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let target = self.model().key_count();

        // Control side: default page + every derived control page.
        if let Some(control_keys) = self.control_keys_id(snapshot) {
            resize(ctx, control_keys, &key_folders(snapshot, control_keys), target, build_control_key);
        }
        if let Some(params_id) = self.base.parameters_id() {
            if let Some(container) = paging::container_id(snapshot, params_id) {
                for page in snapshot.child_ids(container) {
                    resize(ctx, page, &key_folders(snapshot, page), target, build_control_key);
                }
            }
        }

        // Input side: default page + every derived input page.
        if let Some(value_keys) = self.value_keys_id(snapshot) {
            resize(ctx, value_keys, &snapshot.child_ids(value_keys), target, build_value_key);
        }
        if let Some(values_id) = self.base.values_id() {
            if let Some(container) = paging::container_id(snapshot, values_id) {
                for page in snapshot.child_ids(container) {
                    resize(ctx, page, &snapshot.child_ids(page), target, build_value_key);
                }
            }
        }

        self.button_down = vec![false; target];
        if self.last_visuals.len() < target {
            self.last_visuals.resize(target, KeyVisual::default());
        }
    }

    /// Resolves the flat input folder for the active page (`values/keys` or `values/pages/<id>`).
    fn active_value_keys(&self, snapshot: &ProcessTreeSnapshot, active: &str) -> Option<NodeId> {
        if active.is_empty() || active == paging::DEFAULT_PAGE_ID {
            return self.value_keys_id(snapshot);
        }
        let values_id = self.base.values_id()?;
        let container = paging::container_id(snapshot, values_id)?;
        snapshot
            .child_ids(container)
            .into_iter()
            .find(|id| snapshot.node(*id).is_some_and(|node| paging::page_id_of(node) == active))
            .or_else(|| self.value_keys_id(snapshot))
    }

    // ---- device lifecycle --------------------------------------------------

    fn ensure_hardware(&mut self, ctx: &mut ProcessCtx) {
        if self.suppress_hardware {
            self.hardware_dirty = false;
            return;
        }
        let selection = self.device.get_ref().as_str().to_string();
        if selection == NO_DEVICE_VARIANT {
            self.disconnect_hardware();
            return;
        }
        if let Some(hardware) = self.hardware.as_ref() {
            if selection != AUTO_DEVICE_VARIANT && selection != hardware.serial() {
                self.disconnect_hardware();
            } else {
                return;
            }
        }
        if !self.hardware_dirty && self.discover_elapsed < STREAMDECK_DISCOVER_INTERVAL_SECS {
            return;
        }
        self.hardware_dirty = false;
        self.discover_elapsed = 0.0;

        let target_serial = if selection == AUTO_DEVICE_VARIANT {
            self.known_devices.first().map(|device| device.serial.clone())
        } else {
            Some(selection.clone())
        };
        let Some(serial) = target_serial else {
            return;
        };
        match connect(&serial) {
            Ok(device) => {
                golden_core::logsuccess!(origin = self.id(); format!("Connected to Stream Deck: {} ({})", device.product(), device.serial()));
                self.button_down = vec![false; device.key_count()];
                self.last_visuals = vec![KeyVisual::default(); device.key_count()];
                self.hardware = Some(device);
                self.apply_brightness();
            }
            Err(error) => {
                self.device.set_warning_with(ctx, Some("streamdeck_connect"), error.as_str(), None);
            }
        }
    }

    fn disconnect_hardware(&mut self) {
        if let Some(mut hardware) = self.hardware.take() {
            let _ = hardware.clear();
        }
        self.button_down.iter_mut().for_each(|state| *state = false);
    }

    fn apply_brightness(&mut self) {
        if let Some(hardware) = self.hardware.as_mut() {
            let percent = (self.brightness.get() * 100.0).clamp(0.0, 100.0) as u8;
            let _ = hardware.set_brightness(percent);
        }
    }

    fn refresh_devices(&mut self, ctx: &mut ProcessCtx) {
        if self.suppress_hardware {
            return;
        }
        self.known_devices = discover_devices();
        if self.devices_dirty {
            self.sync_device_options(ctx);
            self.devices_dirty = false;
        }
    }

    fn sync_device_options(&self, ctx: &mut ProcessCtx) {
        if !self.device.is_bound() {
            return;
        }
        let mut options = vec![
            enum_option(AUTO_DEVICE_VARIANT, "Auto", 0),
            enum_option(NO_DEVICE_VARIANT, "No Device", 1),
        ];
        options.extend(
            self.known_devices
                .iter()
                .enumerate()
                .map(|(index, device)| enum_option(&device.serial, &device.product, 10 + index as i32)),
        );
        sync_enum_options(ctx, self.device.id(), options, AUTO_DEVICE_VARIANT);
    }

    fn sync_model_options(&self, ctx: &mut ProcessCtx) {
        if !self.model.is_bound() {
            return;
        }
        let options = vec![
            enum_option("mini", "Mini (6 keys)", 0),
            enum_option("standard", "Standard / MK.2 (15 keys)", 1),
            enum_option("xl", "XL (32 keys)", 2),
            enum_option("plus", "Plus (8 keys)", 3),
            enum_option("pedal", "Pedal (3 keys)", 4),
        ];
        sync_enum_options(ctx, self.model.id(), options, DEFAULT_MODEL_VARIANT);
    }

    // ---- per-tick synchronization -----------------------------------------

    fn sync_pages(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let (Some(params_id), Some(control_keys)) = (self.base.parameters_id(), self.control_keys_id(snapshot)) else {
            return;
        };
        paging::ensure_container(ctx, snapshot, params_id);
        // `Unpaged` is a default-only flag; it is never cloned into derived pages.
        paging::complete_pages(ctx, snapshot, params_id, control_keys, &["Unpaged"]);
        if let Some(values_id) = self.base.values_id() {
            let count = self.model().key_count();
            paging::mirror_pages(ctx, snapshot, params_id, values_id, || (0..count).map(build_value_key).collect());
        }
        paging::sync_selector(ctx, snapshot, params_id, params_id);

        let active = paging::active_page_value(snapshot, params_id);
        if active != self.last_active_page {
            self.last_active_page = active.clone();
            self.last_visuals.iter_mut().for_each(|visual| *visual = KeyVisual::default());
            self.emit_callback(ctx, PAGE_CHANGED_CALLBACK, vec![serde_json::json!(active)]);
            self.base.emit_incoming_traffic(ctx);
        }
        self.pages_dirty = false;
    }

    fn push_feedback(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if self.hardware.is_none() {
            return;
        }
        let (Some(params_id), Some(control_keys)) = (self.base.parameters_id(), self.control_keys_id(snapshot)) else {
            return;
        };
        let device_keys = self.hardware.as_ref().map(|h| h.key_count()).unwrap_or(0);
        let count = device_keys.min(self.model().key_count());

        let active = paging::active_page_value(snapshot, params_id);
        let active_root = paging::active_page_root(snapshot, control_keys, params_id, &active);
        let active_keys = key_folders(snapshot, active_root);
        let default_keys = key_folders(snapshot, control_keys);

        for slot in 0..count {
            let active_key = active_keys.get(slot).copied();
            let default_key = default_keys.get(slot).copied();
            // `Unpaged` lives on the default key only; a default-unpaged slot always shows the
            // default appearance, on every page.
            let unpaged = default_key.is_some_and(|key| read_bool_field(snapshot, key, FIELD_UNPAGED));
            let source = if unpaged { default_key } else { active_key };
            let visual = source.map(|key| read_key_visual(snapshot, key)).unwrap_or_default();

            if self.last_visuals.get(slot) == Some(&visual) {
                continue;
            }
            if let Some(hardware) = self.hardware.as_mut() {
                if hardware.render_key(slot, &visual).is_ok() && slot < self.last_visuals.len() {
                    self.last_visuals[slot] = visual;
                }
            }
        }
        let _ = ctx;
    }

    fn process_input(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        // Editor-injected presses (no device required) are merged with real device input.
        let mut events = std::mem::take(&mut self.pending_input);
        if let Some(hardware) = self.hardware.as_mut() {
            events.extend(hardware.poll_input());
        }
        if events.is_empty() {
            return;
        }
        let active = paging::active_page_value(snapshot, self.base.parameters_id().unwrap_or(snapshot.root()));
        let value_keys = self.active_value_keys(snapshot, &active);
        for event in events {
            let (index, pressed) = match event {
                StreamDeckInputEvent::ButtonDown(index) => (index, true),
                StreamDeckInputEvent::ButtonUp(index) => (index, false),
            };
            if index < self.button_down.len() {
                self.button_down[index] = pressed;
            }
            if let Some(value_keys) = value_keys {
                if let Some(param) = snapshot.child_ids(value_keys).get(index).copied() {
                    ctx.set_param(param, ParamValue::Bool(pressed));
                }
            }
            self.base.emit_incoming_traffic(ctx);
            let callback = if pressed { KEY_PRESSED_CALLBACK } else { KEY_RELEASED_CALLBACK };
            self.emit_callback(ctx, callback, vec![serde_json::json!(index), serde_json::json!(active)]);
            if self.base.log_incoming_enabled() {
                golden_core::log!(origin = self.id(); format!("Stream Deck key {index} {}", if pressed { "pressed" } else { "released" }));
            }
        }
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        let can_receive = self.device.get_ref().as_str() != NO_DEVICE_VARIANT;
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(can_receive, can_receive),
        );
    }

    fn emit_callback(&self, ctx: &mut ProcessCtx, callback: &str, args: Vec<serde_json::Value>) {
        crate::app::module::script_api::emit_script_callback(ctx, self.id(), callback, args);
    }
}

#[golden_core::item(
    "module",
    node = "streamdeck_module",
    via = base,
    from_struct,
    menu_path = ["Controllers"]
)]
impl Node for StreamDeckModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, STREAMDECK_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.structure_dirty = true;
        self.sync_model_options(ctx);
        self.refresh_devices(ctx);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.discover_elapsed += ctx.delta_time.as_secs_f64();
        if !self.node_data().effective_enabled {
            self.disconnect_hardware();
            return;
        }

        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };

        self.refresh_devices(ctx);
        self.ensure_hardware(ctx);
        if self.structure_dirty {
            self.sync_structure(ctx, snapshot.as_ref());
            self.structure_dirty = false;
        }
        self.sync_pages(ctx, snapshot.as_ref());
        self.process_input(ctx, snapshot.as_ref());
        self.push_feedback(ctx, snapshot.as_ref());
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.disconnect_hardware();
    }

    fn needs_update(&self) -> bool {
        self.hardware.is_some()
            || self.hardware_dirty
            || self.devices_dirty
            || self.structure_dirty
            || self.pages_dirty
            || !self.pending_input.is_empty()
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(STREAMDECK_UPDATE_RATE_HZ)
            .with_compiled_kernel("chataigne.runtime.streamdeck")
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn on_structure_changed(&mut self, _ctx: &mut ProcessCtx) {
        self.pages_dirty = true;
    }

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, patch: NodeMetaPatch) {
        // A page rename (label change) must re-sync the values mirror's labels, even when no
        // device is connected (otherwise `update` would not run).
        if patch.label.is_some() {
            self.pages_dirty = true;
        }
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), STREAMDECK_SCRIPT_METHODS)
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        match method {
            "addPage" => {
                let name = args.first().and_then(ParamValue::as_str).unwrap_or_default();
                if let Some(snapshot) = ctx.tree_snapshot_arc() {
                    if let Some(params_id) = self.base.parameters_id() {
                        paging::add_page(ctx, snapshot.as_ref(), params_id, &name);
                    }
                }
                Ok(true)
            }
            "removePage" => {
                let page_id = args.first().and_then(ParamValue::as_str).unwrap_or_default();
                if let Some(snapshot) = ctx.tree_snapshot_arc() {
                    if let Some(params_id) = self.base.parameters_id() {
                        paging::remove_page(ctx, snapshot.as_ref(), params_id, &page_id);
                    }
                }
                Ok(true)
            }
            // Editor "simulate hardware press": injected through the same path as real input.
            "pressKey" => {
                if let Some(index) = args.first().and_then(ParamValue::as_int) {
                    self.pending_input.push(StreamDeckInputEvent::ButtonDown(index.max(0) as usize));
                }
                Ok(true)
            }
            "releaseKey" => {
                if let Some(index) = args.first().and_then(ParamValue::as_int) {
                    self.pending_input.push(StreamDeckInputEvent::ButtonUp(index.max(0) as usize));
                }
                Ok(true)
            }
            _ => self.base.engine_call_script_method(ctx, method, args),
        }
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.base.emit_script_param_callback(ctx, snapshot_arc.as_ref(), param, &old_value);

            if self.device.is_bound() && self.device.id() == param {
                self.hardware_dirty = true;
                self.refresh_data_capabilities(ctx);
            } else if self.model.is_bound() && self.model.id() == param {
                self.structure_dirty = true;
            } else if self.brightness.is_bound() && self.brightness.id() == param {
                self.apply_brightness();
            }
        }
    }

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.hardware_dirty = true;
            self.devices_dirty = true;
            self.structure_dirty = true;
            self.discover_elapsed = STREAMDECK_DISCOVER_INTERVAL_SECS;
        } else {
            self.disconnect_hardware();
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

// ---- free helpers ----------------------------------------------------------

/// Returns the ordered key folders under a page root (folder children only).
fn key_folders(snapshot: &ProcessTreeSnapshot, page_root: NodeId) -> Vec<NodeId> {
    snapshot
        .child_ids(page_root)
        .into_iter()
        .filter(|child| {
            snapshot
                .node(*child)
                .is_some_and(|node| node.param_value.is_none() && node.node_type != paging::PAGE_HOST_TYPE)
        })
        .collect()
}

/// Adds/removes trailing children of `parent` so it holds exactly `target` of them.
fn resize(ctx: &mut ProcessCtx, parent: NodeId, existing: &[NodeId], target: usize, build: impl Fn(usize) -> NodeTree) {
    if existing.len() < target {
        for index in existing.len()..target {
            ctx.add_child_tree(parent, build(index), None);
        }
    } else {
        for &extra in &existing[target..] {
            ctx.edits.push(Edit::RemoveNode { node: extra });
        }
    }
}

fn build_control_key(slot: usize) -> NodeTree {
    let number = slot + 1; // keys are 1-based
    let mut tree = NodeTree::new(authored_named_folder(&format!("Key {number}"), &format!("key_{number}")));
    tree.push_child(NodeTree::new(authored_param("Color", ParamValue::Color(0.05, 0.05, 0.05, 1.0), false)));
    tree.push_child(NodeTree::new(authored_param("Text", ParamValue::Str(String::new()), false)));
    tree.push_child(NodeTree::new(authored_param("Image", ParamValue::File(String::new()), false)));
    tree.push_child(NodeTree::new(authored_param("Unpaged", ParamValue::Bool(false), false)));
    tree
}

fn build_value_key(slot: usize) -> NodeTree {
    let number = slot + 1;
    // Inputs are writable so the editor can "simulate press" by setting the value directly.
    let mut param = authored_param(&format!("Key {number}"), ParamValue::Bool(false), false);
    param.node_data_mut().meta.decl_id = golden_core::node::DeclId(format!("key_{number}"));
    NodeTree::new(param)
}

fn authored_named_folder(label: &str, id: &str) -> Folder {
    let mut folder = authored_folder(label);
    folder.node_data_mut().meta.short_name = id.to_string();
    folder.node_data_mut().meta.decl_id = golden_core::node::DeclId(id.to_string());
    folder
}

fn authored_folder(label: &str) -> Folder {
    let mut folder = Folder::new(label);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    folder
}

fn authored_param(label: &str, value: ParamValue, read_only: bool) -> Parameter {
    let mut param = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    param.read_only = read_only;
    crate::app::module::enable_module_authoring(param.node_data_mut());
    param
}

fn read_bool_field(snapshot: &ProcessTreeSnapshot, key_folder: NodeId, field_index: usize) -> bool {
    child_param_at(snapshot, key_folder, field_index)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_bool)
        .unwrap_or(false)
}

fn child_param_at(snapshot: &ProcessTreeSnapshot, key_folder: NodeId, field_index: usize) -> Option<NodeId> {
    snapshot
        .child_ids(key_folder)
        .into_iter()
        .filter(|child| snapshot.node(*child).is_some_and(|node| node.param_value.is_some()))
        .nth(field_index)
}

fn read_key_visual(snapshot: &ProcessTreeSnapshot, key_folder: NodeId) -> KeyVisual {
    let color = child_param_at(snapshot, key_folder, FIELD_COLOR)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_color)
        .unwrap_or((0.0, 0.0, 0.0, 1.0));
    let text = child_param_at(snapshot, key_folder, FIELD_TEXT)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_str)
        .unwrap_or_default();
    let image = child_param_at(snapshot, key_folder, FIELD_IMAGE)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_str)
        .unwrap_or_default();
    KeyVisual { color, text, image }
}

fn enum_option(variant_id: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

/// Swaps fresh enum options into a live selector parameter (mirrors the gamepad pattern).
fn sync_enum_options(ctx: &mut ProcessCtx, param_id: NodeId, options: Vec<ParameterEnumOption>, fallback: &str) {
    let fallback = fallback.to_string();
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("Stream Deck enum target is not a parameter".to_string());
        };
        let current = parameter
            .value
            .as_enum()
            .filter(|variant| !variant.trim().is_empty())
            .unwrap_or_else(|| fallback.clone());
        let next_value = if options.iter().any(|option| option.variant_id == current) {
            ParamValue::Enum(current)
        } else {
            ParamValue::Enum(fallback.clone())
        };
        if parameter.constraints.enum_options == options && parameter.value == next_value {
            return Ok(());
        }
        let label = parameter.node_data().meta.label.clone();
        let change_check = parameter.change_check.clone();
        let mut replacement = Parameter::new(label.as_str(), next_value, change_check);
        *replacement.node_data_mut() = parameter.node_data().clone();
        replacement.default_value = parameter.default_value.clone();
        replacement.event_behaviour = ParameterEventBehaviour::Coalesce;
        replacement.read_only = parameter.read_only;
        replacement.persist_read_only_value = parameter.persist_read_only_value;
        replacement.constraints = parameter.constraints.clone();
        replacement.constraints.enum_options = options.clone();
        replacement.ui_hints = parameter.ui_hints.clone();
        replacement.control = parameter.control.clone();
        replacement.control_modes_enabled = parameter.control_modes_enabled;
        inner_ctx.replace_node(param_id, replacement);
        Ok(())
    });
}

#[cfg(test)]
mod streamdeck_tests;
