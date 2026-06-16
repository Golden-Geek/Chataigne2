//! Shared framework for *paged key-grid* controller modules (Stream Deck, Loupedeck, …).
//!
//! A "deck" is a hardware surface exposing a grid of keys that each carry **control**
//! (color / text / image appearance, plus an `unpaged` flag) and produce a boolean
//! **input** (pressed). The grid is paged on both sides through the generic
//! [`paging`](super::paging) framework: `parameters/keys` is the default control page and
//! `values/keys` its flat input mirror; derived pages live beside them under `pages/`.
//!
//! This module owns everything that is identical between deck modules:
//!
//! * the hardware contract ([`DeckDevice`]) plus the always-available [`SimulatedDeck`],
//! * the per-tick orchestration (structure sync, page sync, feedback push, input routing,
//!   device discovery/connection) carried by [`DeckSurface`], and
//! * the node-tree helpers that build/read the control and value key folders.
//!
//! A concrete module (see the Stream Deck or Loupedeck modules) only supplies its model
//! list (key counts), its parameter declarations, and a [`DeckConfig`] naming its hardware
//! backend and script callbacks. No per-module copy of the paging glue is needed.

use golden_core::{
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeHandle, NodeId, NodeMetaPatch},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use super::paging;
use crate::app::ModuleBase;

/// Decl id of the default-page key folder on both the control and value sides.
pub(crate) const KEYS_FOLDER: &str = "keys";

/// Device-selector enum variant meaning "no hardware".
pub(crate) const NO_DEVICE_VARIANT: &str = "none";
/// Device-selector enum variant meaning "first connected device".
pub(crate) const AUTO_DEVICE_VARIANT: &str = "auto";

/// Script methods every deck exposes (page management lives in [`paging`]).
pub(crate) const PAGE_SCRIPT_METHODS: &[&str] = &["addPage", "removePage"];
/// Decks carry no module commands.
pub(crate) const DECK_COMMAND_TYPES: &[&str] = &[];

// Declaration order of a control-key folder's primitives (resolved positionally).
const FIELD_COLOR: usize = 0;
const FIELD_TEXT: usize = 1;
const FIELD_IMAGE: usize = 2;
const FIELD_UNPAGED: usize = 3;

/// A normalized button transition produced by polling a device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeckInputEvent {
    ButtonDown(usize),
    ButtonUp(usize),
}

/// Outbound visual state for one key (the resolved *feedback* primitives of a control shape).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KeyVisual {
    /// Background RGBA in 0.0..=1.0. Shown wherever `image` is absent or transparent.
    pub color: (f64, f64, f64, f64),
    /// Caption text (rendered on hardware when font rendering is available).
    pub text: String,
    /// Optional image file path. Transparent pixels composite over `color`.
    pub image: String,
}

impl Default for KeyVisual {
    fn default() -> Self {
        Self {
            color: (0.0, 0.0, 0.0, 1.0),
            text: String::new(),
            image: String::new(),
        }
    }
}

/// A connected device discovered on the bus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiscoveredDevice {
    pub serial: String,
    pub product: String,
    pub key_count: usize,
}

/// Hardware-facing contract. Implementations must be `Send` so a module can own one.
pub(crate) trait DeckDevice: Send {
    /// Number of physical keys exposed by this device.
    fn key_count(&self) -> usize;
    /// Stable serial used to re-select the device.
    fn serial(&self) -> &str;
    /// Human-readable product name.
    fn product(&self) -> &str;
    /// Non-blocking poll returning button transitions since the last call.
    fn poll_input(&mut self) -> Vec<DeckInputEvent>;
    /// Pushes one key's feedback primitives to the device.
    fn render_key(&mut self, index: usize, visual: &KeyVisual) -> Result<(), String>;
    /// Sets global brightness (0..=100).
    fn set_brightness(&mut self, percent: u8) -> Result<(), String>;
    /// Clears all keys to black.
    fn clear(&mut self) -> Result<(), String>;
    /// Downcast hook (used by tests to reach the simulated device behind the trait object).
    fn as_any(&self) -> &dyn std::any::Any;
    /// Mutable downcast hook (used by tests to inject button presses).
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Per-module configuration: hardware backend hooks plus naming for callbacks and logs.
#[derive(Clone, Copy)]
pub(crate) struct DeckConfig {
    /// Minimum seconds between hardware discovery sweeps.
    pub discover_interval_secs: f64,
    /// Human-readable surface name used in logs (e.g. "Stream Deck").
    pub product_label: &'static str,
    /// Warning id raised on the device selector when a connection fails.
    pub connect_warning_id: &'static str,
    /// Script callback fired on key press.
    pub key_pressed_callback: &'static str,
    /// Script callback fired on key release.
    pub key_released_callback: &'static str,
    /// Script callback fired when the active page changes.
    pub page_changed_callback: &'static str,
    /// Lists devices currently visible on the host bus.
    pub discover: fn() -> Vec<DiscoveredDevice>,
    /// Connects to a device by serial, returning a boxed driver.
    pub connect: fn(&str) -> Result<Box<dyn DeckDevice>, String>,
}

/// Generic per-module deck state plus the shared per-tick orchestration. A concrete module
/// holds exactly one `DeckSurface` beside its [`ModuleBase`] and forwards the `Node` lifecycle
/// hooks to it.
pub(crate) struct DeckSurface {
    config: DeckConfig,
    hardware: Option<Box<dyn DeckDevice>>,
    hardware_dirty: bool,
    discover_elapsed: f64,
    known_devices: Vec<DiscoveredDevice>,
    devices_dirty: bool,
    structure_dirty: bool,
    pages_dirty: bool,
    last_active_page: String,
    last_visuals: Vec<KeyVisual>,
    button_down: Vec<bool>,
    suppress_hardware: bool,
}

impl DeckSurface {
    pub(crate) fn new(config: DeckConfig) -> Self {
        Self {
            discover_elapsed: config.discover_interval_secs,
            config,
            hardware: None,
            hardware_dirty: true,
            known_devices: Vec::new(),
            devices_dirty: true,
            structure_dirty: true,
            pages_dirty: true,
            last_active_page: String::new(),
            last_visuals: Vec::new(),
            button_down: Vec::new(),
            suppress_hardware: false,
        }
    }

    // ---- lifecycle hooks (called by the concrete module) -------------------

    /// Accumulates frame time toward the next discovery sweep.
    pub(crate) fn add_elapsed(&mut self, seconds: f64) {
        self.discover_elapsed += seconds;
    }

    pub(crate) fn needs_update(&self) -> bool {
        self.hardware.is_some()
            || self.hardware_dirty
            || self.devices_dirty
            || self.structure_dirty
            || self.pages_dirty
    }

    pub(crate) fn mark_structure_dirty(&mut self) {
        self.structure_dirty = true;
    }

    pub(crate) fn mark_pages_dirty(&mut self) {
        self.pages_dirty = true;
    }

    pub(crate) fn mark_hardware_dirty(&mut self) {
        self.hardware_dirty = true;
    }

    /// Reacts to an effective-enabled transition: rearm discovery/structure on enable,
    /// drop the device on disable.
    pub(crate) fn set_effective_enabled(&mut self, enabled: bool) {
        if enabled {
            self.hardware_dirty = true;
            self.devices_dirty = true;
            self.structure_dirty = true;
            self.discover_elapsed = self.config.discover_interval_secs;
        } else {
            self.disconnect_hardware();
        }
    }

    /// Refreshes the known-device list and (when dirty) the device-selector options.
    pub(crate) fn refresh_devices(&mut self, ctx: &mut ProcessCtx, device_param_id: Option<NodeId>) {
        if self.suppress_hardware {
            return;
        }
        self.known_devices = (self.config.discover)();
        if self.devices_dirty {
            self.sync_device_options(ctx, device_param_id);
            self.devices_dirty = false;
        }
    }

    /// The full per-tick sequence shared by every deck module. The caller resolves the
    /// model-driven `key_count`, the current device `selection`, the device parameter id and
    /// the brightness percentage from its own typed handles.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn tick(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        base: &ModuleBase,
        module_id: NodeId,
        key_count: usize,
        brightness: u8,
        selection: &str,
        device_param_id: Option<NodeId>,
    ) {
        self.refresh_devices(ctx, device_param_id);
        self.ensure_hardware(ctx, module_id, selection, device_param_id, brightness);
        if self.structure_dirty {
            self.sync_structure(ctx, snapshot, base, key_count);
            self.structure_dirty = false;
        }
        self.sync_pages(ctx, snapshot, base, module_id, key_count);
        self.process_input(ctx, snapshot, base, module_id);
        self.push_feedback(ctx, snapshot, base, key_count);
    }

    // ---- device lifecycle --------------------------------------------------

    fn ensure_hardware(
        &mut self,
        ctx: &mut ProcessCtx,
        module_id: NodeId,
        selection: &str,
        device_param_id: Option<NodeId>,
        brightness: u8,
    ) {
        if self.suppress_hardware {
            self.hardware_dirty = false;
            return;
        }
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
        if !self.hardware_dirty && self.discover_elapsed < self.config.discover_interval_secs {
            return;
        }
        self.hardware_dirty = false;
        self.discover_elapsed = 0.0;

        let target_serial = if selection == AUTO_DEVICE_VARIANT {
            self.known_devices.first().map(|device| device.serial.clone())
        } else {
            Some(selection.to_string())
        };
        let Some(serial) = target_serial else {
            return;
        };
        match (self.config.connect)(&serial) {
            Ok(device) => {
                golden_core::logsuccess!(origin = module_id; format!("Connected to {}: {} ({})", self.config.product_label, device.product(), device.serial()));
                self.button_down = vec![false; device.key_count()];
                self.last_visuals = vec![KeyVisual::default(); device.key_count()];
                self.hardware = Some(device);
                self.apply_brightness(brightness);
            }
            Err(error) => {
                if let Some(param_id) = device_param_id {
                    NodeHandle::new(param_id).set_warning_with(ctx, Some(self.config.connect_warning_id), error.as_str(), None);
                }
            }
        }
    }

    pub(crate) fn disconnect_hardware(&mut self) {
        if let Some(mut hardware) = self.hardware.take() {
            let _ = hardware.clear();
        }
        self.button_down.iter_mut().for_each(|state| *state = false);
    }

    pub(crate) fn apply_brightness(&mut self, percent: u8) {
        if let Some(hardware) = self.hardware.as_mut() {
            let _ = hardware.set_brightness(percent.min(100));
        }
    }

    fn sync_device_options(&self, ctx: &mut ProcessCtx, device_param_id: Option<NodeId>) {
        let Some(param_id) = device_param_id else {
            return;
        };
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
        sync_enum_options(ctx, param_id, options, AUTO_DEVICE_VARIANT);
    }

    // ---- structural lookups ------------------------------------------------

    fn control_keys_id(&self, snapshot: &ProcessTreeSnapshot, base: &ModuleBase) -> Option<NodeId> {
        snapshot.find_child(base.parameters_id()?, KEYS_FOLDER)
    }

    fn value_keys_id(&self, snapshot: &ProcessTreeSnapshot, base: &ModuleBase) -> Option<NodeId> {
        snapshot.find_child(base.values_id()?, KEYS_FOLDER)
    }

    /// Resolves the flat input folder for the active page (`values/keys` or `values/pages/<id>`).
    fn active_value_keys(&self, snapshot: &ProcessTreeSnapshot, base: &ModuleBase, active: &str) -> Option<NodeId> {
        if active.is_empty() || active == paging::DEFAULT_PAGE_ID {
            return self.value_keys_id(snapshot, base);
        }
        let values_id = base.values_id()?;
        let container = paging::container_id(snapshot, values_id)?;
        snapshot
            .child_ids(container)
            .into_iter()
            .find(|id| snapshot.node(*id).is_some_and(|node| paging::page_id_of(node) == active))
            .or_else(|| self.value_keys_id(snapshot, base))
    }

    // ---- model-driven structure generation ---------------------------------

    fn sync_structure(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, base: &ModuleBase, target: usize) {
        // Control side: default page + every derived control page.
        if let Some(control_keys) = self.control_keys_id(snapshot, base) {
            resize(ctx, control_keys, &key_folders(snapshot, control_keys), target, build_control_key);
        }
        if let Some(params_id) = base.parameters_id() {
            if let Some(container) = paging::container_id(snapshot, params_id) {
                for page in snapshot.child_ids(container) {
                    resize(ctx, page, &key_folders(snapshot, page), target, build_control_key);
                }
            }
        }

        // Input side: default page + every derived input page.
        if let Some(value_keys) = self.value_keys_id(snapshot, base) {
            resize(ctx, value_keys, &snapshot.child_ids(value_keys), target, build_value_key);
        }
        if let Some(values_id) = base.values_id() {
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

    // ---- per-tick synchronization -----------------------------------------

    fn sync_pages(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        base: &ModuleBase,
        module_id: NodeId,
        key_count: usize,
    ) {
        let (Some(params_id), Some(control_keys)) = (base.parameters_id(), self.control_keys_id(snapshot, base)) else {
            return;
        };
        paging::ensure_container(ctx, snapshot, params_id);
        paging::complete_pages(ctx, snapshot, params_id, control_keys);
        if let Some(values_id) = base.values_id() {
            paging::mirror_pages(ctx, snapshot, params_id, values_id, || (0..key_count).map(build_value_key).collect());
        }
        paging::sync_selector(ctx, snapshot, params_id, params_id);

        let active = paging::active_page_value(snapshot, params_id);
        if active != self.last_active_page {
            self.last_active_page = active.clone();
            self.last_visuals.iter_mut().for_each(|visual| *visual = KeyVisual::default());
            emit_callback(ctx, module_id, self.config.page_changed_callback, vec![serde_json::json!(active)]);
            base.emit_incoming_traffic(ctx);
        }
        self.pages_dirty = false;
    }

    fn push_feedback(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, base: &ModuleBase, key_count: usize) {
        if self.hardware.is_none() {
            return;
        }
        let (Some(params_id), Some(control_keys)) = (base.parameters_id(), self.control_keys_id(snapshot, base)) else {
            return;
        };
        let device_keys = self.hardware.as_ref().map(|h| h.key_count()).unwrap_or(0);
        let count = device_keys.min(key_count);

        let active = paging::active_page_value(snapshot, params_id);
        let active_root = paging::active_page_root(snapshot, control_keys, params_id, &active);
        let active_keys = key_folders(snapshot, active_root);
        let default_keys = key_folders(snapshot, control_keys);

        for slot in 0..count {
            let active_key = active_keys.get(slot).copied();
            let unpaged = active_key.is_some_and(|key| read_bool_field(snapshot, key, FIELD_UNPAGED));
            let source = if unpaged { default_keys.get(slot).copied() } else { active_key };
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

    fn process_input(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, base: &ModuleBase, module_id: NodeId) {
        let events = match self.hardware.as_mut() {
            Some(hardware) => hardware.poll_input(),
            None => return,
        };
        if events.is_empty() {
            return;
        }
        let active = paging::active_page_value(snapshot, base.parameters_id().unwrap_or(snapshot.root()));
        let value_keys = self.active_value_keys(snapshot, base, &active);
        for event in events {
            let (index, pressed) = match event {
                DeckInputEvent::ButtonDown(index) => (index, true),
                DeckInputEvent::ButtonUp(index) => (index, false),
            };
            if index < self.button_down.len() {
                self.button_down[index] = pressed;
            }
            if let Some(value_keys) = value_keys {
                if let Some(param) = snapshot.child_ids(value_keys).get(index).copied() {
                    ctx.set_param(param, ParamValue::Bool(pressed));
                }
            }
            base.emit_incoming_traffic(ctx);
            let callback = if pressed { self.config.key_pressed_callback } else { self.config.key_released_callback };
            emit_callback(ctx, module_id, callback, vec![serde_json::json!(index), serde_json::json!(active)]);
            if base.log_incoming_enabled() {
                golden_core::log!(origin = module_id; format!("{} key {index} {}", self.config.product_label, if pressed { "pressed" } else { "released" }));
            }
        }
    }

    // ---- test hooks --------------------------------------------------------

    #[cfg(test)]
    pub(crate) fn install_simulated_device(&mut self, device: SimulatedDeck) {
        self.button_down = vec![false; device.key_count()];
        self.last_visuals = vec![KeyVisual::default(); device.key_count()];
        self.hardware = Some(Box::new(device));
        self.hardware_dirty = false;
        self.suppress_hardware = true;
    }

    #[cfg(test)]
    pub(crate) fn simulated(&self) -> &SimulatedDeck {
        self.hardware
            .as_ref()
            .and_then(|device| device.as_any().downcast_ref::<SimulatedDeck>())
            .expect("simulated deck device should be installed")
    }

    #[cfg(test)]
    pub(crate) fn simulated_mut(&mut self) -> &mut SimulatedDeck {
        self.hardware
            .as_mut()
            .and_then(|device| device.as_any_mut().downcast_mut::<SimulatedDeck>())
            .expect("simulated deck device should be installed")
    }
}

/// Dispatches the shared page-management script methods. Returns `None` when `method` is not a
/// page method, so the caller can fall through to its base handler.
pub(crate) fn handle_page_script_method(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    params_id: NodeId,
    method: &str,
    args: &[ParamValue],
) -> Option<bool> {
    match method {
        "addPage" => {
            let name = args.first().and_then(ParamValue::as_str).unwrap_or_default();
            paging::add_page(ctx, snapshot, params_id, &name);
            Some(true)
        }
        "removePage" => {
            let page_id = args.first().and_then(ParamValue::as_str).unwrap_or_default();
            paging::remove_page(ctx, snapshot, params_id, &page_id);
            Some(true)
        }
        _ => None,
    }
}

// ---- free helpers ----------------------------------------------------------

fn emit_callback(ctx: &mut ProcessCtx, module_id: NodeId, callback: &str, args: Vec<serde_json::Value>) {
    crate::app::module::script_api::emit_script_callback(ctx, module_id, callback, args);
}

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
    let mut param = authored_param(&format!("Key {number}"), ParamValue::Bool(false), true);
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
pub(crate) fn sync_enum_options(ctx: &mut ProcessCtx, param_id: NodeId, options: Vec<ParameterEnumOption>, fallback: &str) {
    let fallback = fallback.to_string();
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("deck enum target is not a parameter".to_string());
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

// ---------------------------------------------------------------------------
// Simulated device (always available — powers the test suite and headless use).
// ---------------------------------------------------------------------------

/// In-memory device that records what was rendered and replays injected presses. Shared by
/// every deck module's test suite.
pub(crate) struct SimulatedDeck {
    serial: String,
    product: String,
    pressed: Vec<bool>,
    queued: Vec<DeckInputEvent>,
    rendered: Vec<KeyVisual>,
    brightness: u8,
}

impl SimulatedDeck {
    pub(crate) fn new(serial: impl Into<String>, key_count: usize) -> Self {
        Self {
            serial: serial.into(),
            product: "Simulated Deck".to_string(),
            pressed: vec![false; key_count],
            queued: Vec::new(),
            rendered: vec![KeyVisual::default(); key_count],
            brightness: 100,
        }
    }

    /// Injects a physical button press (test helper).
    pub(crate) fn press(&mut self, index: usize) {
        if index < self.pressed.len() && !self.pressed[index] {
            self.pressed[index] = true;
            self.queued.push(DeckInputEvent::ButtonDown(index));
        }
    }

    /// Injects a physical button release (test helper).
    pub(crate) fn release(&mut self, index: usize) {
        if index < self.pressed.len() && self.pressed[index] {
            self.pressed[index] = false;
            self.queued.push(DeckInputEvent::ButtonUp(index));
        }
    }

    /// Returns the last visual rendered to a key (test introspection).
    pub(crate) fn rendered(&self, index: usize) -> Option<&KeyVisual> {
        self.rendered.get(index)
    }

    #[cfg(test)]
    pub(crate) fn brightness(&self) -> u8 {
        self.brightness
    }
}

impl DeckDevice for SimulatedDeck {
    fn key_count(&self) -> usize {
        self.pressed.len()
    }

    fn serial(&self) -> &str {
        &self.serial
    }

    fn product(&self) -> &str {
        &self.product
    }

    fn poll_input(&mut self) -> Vec<DeckInputEvent> {
        std::mem::take(&mut self.queued)
    }

    fn render_key(&mut self, index: usize, visual: &KeyVisual) -> Result<(), String> {
        if index >= self.rendered.len() {
            return Err(format!("key index {index} out of range"));
        }
        self.rendered[index] = visual.clone();
        Ok(())
    }

    fn set_brightness(&mut self, percent: u8) -> Result<(), String> {
        self.brightness = percent.min(100);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), String> {
        for visual in &mut self.rendered {
            *visual = KeyVisual::default();
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
