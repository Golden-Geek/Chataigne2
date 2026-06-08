//! Elgato Stream Deck module — the reference consumer of the [paging] framework.
//!
//! The control surface (a grid of keys) is declared **once** with the normal
//! `#[children(...)]` DSL. The `keys` folder is tagged `pageable`, which the runtime
//! treats as the fixed `default` page; the user can derive additional pages (clones of
//! that declared layout) and flip between them through the injected `active_page`
//! selector — locally, or project-wide via the Preset/State system.
//!
//! Each key is a *control shape* with explicit feedback primitives (`color`, `text`,
//! pushed to the device) and an activity primitive (`pressed`, written from the device).
//! This example models a 6-key surface; the count is model-specific and the viewport
//! maps `min(device_keys, template_keys)`.
//!
//! [paging]: crate::app::module::common::paging

mod streamdeck_runtime;

use golden_core::{
    engine::NodeExecutionRule,
    events::Event,
    node,
    node::{Node, NodeCreationContext, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, Parameter, ParameterEnumOption, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::paging;

use self::streamdeck_runtime::{
    connect, discover_devices, DiscoveredStreamDeck, KeyVisual, StreamDeckDevice, StreamDeckInputEvent,
};

const STREAMDECK_UPDATE_RATE_HZ: u32 = 60;
const STREAMDECK_DISCOVER_INTERVAL_SECS: f64 = 2.0;
const STREAMDECK_KEY_COUNT: usize = 6;
const STREAMDECK_TEMPLATE_FOLDER: &str = "keys";

const NO_DEVICE_VARIANT: &str = "none";
const AUTO_DEVICE_VARIANT: &str = "auto";

const KEY_PRESSED_CALLBACK: &str = "streamDeckKeyPressed";
const KEY_RELEASED_CALLBACK: &str = "streamDeckKeyReleased";
const PAGE_CHANGED_CALLBACK: &str = "streamDeckPageChanged";

const STREAMDECK_SCRIPT_METHODS: &[&str] = &["addPage", "removePage"];
const STREAMDECK_COMMAND_TYPES: &[&str] = &[];

// Position of each feedback/activity primitive within a declared key folder
// (must match the declaration order in the `#[children]` block below).
const KEY_FIELD_COLOR: usize = 0;
const KEY_FIELD_TEXT: usize = 1;
const KEY_FIELD_IMAGE: usize = 2;
const KEY_FIELD_PRESSED: usize = 3;

#[node("streamdeck_module", label = "Stream Deck")]
#[children(
    folder(connection) {
        device: Enum = AUTO_DEVICE_VARIANT (
            label = "Device",
            description = "Stream Deck to drive. Auto uses the first connected device.",
            enum_options = ["auto (Auto)", "none (No Device)"]
        );
        [base_children];
    }
    folder(parameters) {
        brightness: i32 = 80 [0..100] (
            label = "Brightness",
            description = "Global key backlight brightness percentage."
        );
        new_page: String = String::new() (
            label = "New Page",
            description = "Type a name and confirm to derive a new page from the default layout."
        );
        [base_children];
    }
    folder(values) {
        folder(keys, label = "Keys", tags = vec![paging::PAGEABLE_TAG.to_string()]) {
            folder(key_0, label = "Key 0") {
                key_0_color: ParamValue = ParamValue::Color(0.05, 0.05, 0.05, 1.0) (label = "Color");
                key_0_text: String = String::new() (label = "Text");
                key_0_image: ParamValue = ParamValue::File(String::new()) (label = "Image");
                key_0_pressed: bool = false (label = "Pressed", read_only = true);
            }
            folder(key_1, label = "Key 1") {
                key_1_color: ParamValue = ParamValue::Color(0.05, 0.05, 0.05, 1.0) (label = "Color");
                key_1_text: String = String::new() (label = "Text");
                key_1_image: ParamValue = ParamValue::File(String::new()) (label = "Image");
                key_1_pressed: bool = false (label = "Pressed", read_only = true);
            }
            folder(key_2, label = "Key 2") {
                key_2_color: ParamValue = ParamValue::Color(0.05, 0.05, 0.05, 1.0) (label = "Color");
                key_2_text: String = String::new() (label = "Text");
                key_2_image: ParamValue = ParamValue::File(String::new()) (label = "Image");
                key_2_pressed: bool = false (label = "Pressed", read_only = true);
            }
            folder(key_3, label = "Key 3") {
                key_3_color: ParamValue = ParamValue::Color(0.05, 0.05, 0.05, 1.0) (label = "Color");
                key_3_text: String = String::new() (label = "Text");
                key_3_image: ParamValue = ParamValue::File(String::new()) (label = "Image");
                key_3_pressed: bool = false (label = "Pressed", read_only = true);
            }
            folder(key_4, label = "Key 4") {
                key_4_color: ParamValue = ParamValue::Color(0.05, 0.05, 0.05, 1.0) (label = "Color");
                key_4_text: String = String::new() (label = "Text");
                key_4_image: ParamValue = ParamValue::File(String::new()) (label = "Image");
                key_4_pressed: bool = false (label = "Pressed", read_only = true);
            }
            folder(key_5, label = "Key 5") {
                key_5_color: ParamValue = ParamValue::Color(0.05, 0.05, 0.05, 1.0) (label = "Color");
                key_5_text: String = String::new() (label = "Text");
                key_5_image: ParamValue = ParamValue::File(String::new()) (label = "Image");
                key_5_pressed: bool = false (label = "Pressed", read_only = true);
            }
        }
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
    last_active_page: String,
    last_visuals: Vec<KeyVisual>,
    button_down: Vec<bool>,
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
            String::new(),
            vec![KeyVisual::default(); STREAMDECK_KEY_COUNT],
            vec![false; STREAMDECK_KEY_COUNT],
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

    fn parameters_id(&self) -> Option<NodeId> {
        self.base.parameters_id()
    }

    fn template_folder_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let values_id = self.base.values_id()?;
        snapshot.find_child(values_id, STREAMDECK_TEMPLATE_FOLDER)
    }

    /// Resolves the ordered key folders for the active page (default or a derived clone).
    fn active_key_folders(&self, snapshot: &ProcessTreeSnapshot) -> Vec<NodeId> {
        let Some(params_id) = self.parameters_id() else {
            return Vec::new();
        };
        let Some(template_folder) = self.template_folder_id(snapshot) else {
            return Vec::new();
        };
        let active = paging::active_page_value(snapshot, params_id);
        let page_root = paging::active_page_root(snapshot, template_folder, &active);
        key_folders(snapshot, page_root)
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
            // Reconnect if a specific device was chosen that differs from the live one.
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

        let target_serial = match selection.as_str() {
            AUTO_DEVICE_VARIANT => self.known_devices.first().map(|device| device.serial.clone()),
            other => Some(other.to_string()),
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
                self.apply_brightness(ctx);
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

    fn apply_brightness(&mut self, _ctx: &mut ProcessCtx) {
        if let Some(hardware) = self.hardware.as_mut() {
            let percent = self.brightness.get().clamp(0, 100) as u8;
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
        sync_enum_options(ctx, self.device.id(), options);
    }

    // ---- per-tick synchronization -----------------------------------------

    fn sync_pages(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let (Some(params_id), Some(template_folder)) =
            (self.parameters_id(), self.template_folder_id(snapshot))
        else {
            return;
        };
        paging::sync_selector(ctx, snapshot, params_id, template_folder);

        let active = paging::active_page_value(snapshot, params_id);
        if active != self.last_active_page {
            self.last_active_page = active.clone();
            // Force a full re-render on page flip so the surface reflects the new layer.
            self.last_visuals.iter_mut().for_each(|visual| *visual = KeyVisual::default());
            self.emit_callback(ctx, PAGE_CHANGED_CALLBACK, vec![serde_json::json!(active)]);
            self.base.emit_incoming_traffic(ctx);
        }
    }

    fn push_feedback(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if self.hardware.is_none() {
            return;
        }
        let key_folders = self.active_key_folders(snapshot);
        let key_count = self.hardware.as_ref().map(|h| h.key_count()).unwrap_or(0);
        for slot in 0..key_count {
            let visual = key_folders
                .get(slot)
                .map(|folder| read_key_visual(snapshot, *folder))
                .unwrap_or_default();
            if self.last_visuals.get(slot) == Some(&visual) {
                continue;
            }
            if let Some(hardware) = self.hardware.as_mut() {
                if hardware.render_key(slot, &visual).is_ok() {
                    if slot < self.last_visuals.len() {
                        self.last_visuals[slot] = visual;
                    }
                }
            }
        }
        let _ = ctx;
    }

    fn process_input(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let events = match self.hardware.as_mut() {
            Some(hardware) => hardware.poll_input(),
            None => return,
        };
        if events.is_empty() {
            return;
        }
        let key_folders = self.active_key_folders(snapshot);
        for event in events {
            let (index, pressed) = match event {
                StreamDeckInputEvent::ButtonDown(index) => (index, true),
                StreamDeckInputEvent::ButtonUp(index) => (index, false),
            };
            if index < self.button_down.len() {
                self.button_down[index] = pressed;
            }
            if let Some(folder) = key_folders.get(index) {
                if let Some(pressed_param) = child_param_at(snapshot, *folder, KEY_FIELD_PRESSED) {
                    ctx.set_param(pressed_param, ParamValue::Bool(pressed));
                }
            }
            self.base.emit_incoming_traffic(ctx);
            let callback = if pressed { KEY_PRESSED_CALLBACK } else { KEY_RELEASED_CALLBACK };
            self.emit_callback(ctx, callback, vec![serde_json::json!(index)]);
            if self.base.log_incoming_enabled() {
                golden_core::log!(origin = self.id(); format!("Stream Deck key {index} {}", if pressed { "pressed" } else { "released" }));
            }
        }
    }

    // ---- page management ---------------------------------------------------

    fn handle_new_page_request(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let name = self.new_page.get_ref().trim().to_string();
        if name.is_empty() {
            return;
        }
        if let Some(template_folder) = self.template_folder_id(snapshot) {
            if let Some(page_id) = paging::add_page(ctx, snapshot, template_folder, &name) {
                golden_core::log!(origin = self.id(); format!("Created Stream Deck page '{page_id}'"));
            }
        }
        // Clear the request field so the same name can be reused later.
        self.new_page.set(ctx, String::new());
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
        self.sync_pages(ctx, snapshot.as_ref());
        self.process_input(ctx, snapshot.as_ref());
        self.push_feedback(ctx, snapshot.as_ref());
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.disconnect_hardware();
    }

    fn needs_update(&self) -> bool {
        self.hardware.is_some() || self.hardware_dirty || self.devices_dirty
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(STREAMDECK_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
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
                    if let Some(template_folder) = self.template_folder_id(snapshot.as_ref()) {
                        paging::add_page(ctx, snapshot.as_ref(), template_folder, &name);
                    }
                }
                Ok(true)
            }
            "removePage" => {
                let page_id = args.first().and_then(ParamValue::as_str).unwrap_or_default();
                if let Some(snapshot) = ctx.tree_snapshot_arc() {
                    if let Some(template_folder) = self.template_folder_id(snapshot.as_ref()) {
                        paging::remove_page(ctx, snapshot.as_ref(), template_folder, &page_id);
                    }
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
            } else if self.brightness.is_bound() && self.brightness.id() == param {
                self.apply_brightness(ctx);
            } else if self.new_page.is_bound() && self.new_page.id() == param {
                self.handle_new_page_request(ctx, snapshot_arc.as_ref());
            }
        }
    }

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.hardware_dirty = true;
            self.devices_dirty = true;
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

/// Returns the ordered key-shape folders under a page root, excluding the reserved
/// `pages` derived-pages container.
fn key_folders(snapshot: &ProcessTreeSnapshot, page_root: NodeId) -> Vec<NodeId> {
    snapshot
        .child_ids(page_root)
        .into_iter()
        .filter(|child| {
            snapshot.node(*child).is_some_and(|node| {
                node.param_value.is_none() && node.label != paging::PAGES_CONTAINER
            })
        })
        .collect()
}

/// Returns the parameter node at `field_index` within a key folder (declaration order:
/// color, text, pressed).
fn child_param_at(snapshot: &ProcessTreeSnapshot, key_folder: NodeId, field_index: usize) -> Option<NodeId> {
    snapshot
        .child_ids(key_folder)
        .into_iter()
        .filter(|child| snapshot.node(*child).is_some_and(|node| node.param_value.is_some()))
        .nth(field_index)
}

/// Reads the resolved feedback primitives (color + text) for one key folder.
fn read_key_visual(snapshot: &ProcessTreeSnapshot, key_folder: NodeId) -> KeyVisual {
    let color = child_param_at(snapshot, key_folder, KEY_FIELD_COLOR)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_color)
        .unwrap_or((0.0, 0.0, 0.0, 1.0));
    let text = child_param_at(snapshot, key_folder, KEY_FIELD_TEXT)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_str)
        .unwrap_or_default();
    let image = child_param_at(snapshot, key_folder, KEY_FIELD_IMAGE)
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
fn sync_enum_options(ctx: &mut ProcessCtx, param_id: NodeId, options: Vec<ParameterEnumOption>) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("Stream Deck device target is not a parameter".to_string());
        };
        let current = parameter
            .value
            .as_enum()
            .filter(|variant| !variant.trim().is_empty())
            .unwrap_or_else(|| AUTO_DEVICE_VARIANT.to_string());
        let next_value = if options.iter().any(|option| option.variant_id == current) {
            ParamValue::Enum(current)
        } else {
            ParamValue::Enum(AUTO_DEVICE_VARIANT.to_string())
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
