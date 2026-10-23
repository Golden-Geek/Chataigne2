use super::super::{
    devices::*, GAMEPAD_AXIS_CHANGED_CALLBACK, GAMEPAD_BUTTON_CHANGED_CALLBACK, GAMEPAD_BUTTON_PRESSED_CALLBACK,
    GAMEPAD_BUTTON_RELEASED_CALLBACK, GAMEPAD_CONNECTED_CALLBACK, GAMEPAD_DISCONNECTED_CALLBACK,
};
use super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPayload, NativeCallbackPreparer};
use golden_core::parameter::ParameterEnumOptions;
pub(super) const FRAME_WEIGHT: usize = 64 * 1024;

pub(in super::super) struct Catalog {
    pub(in super::super) settings: Settings,
    pub(in super::super) selection: String,
    pub(in super::super) devices: Vec<DiscoveredGamepad>,
    pub(in super::super) selected: Option<DiscoveredGamepad>,
    pub(in super::super) options: ParameterEnumOptions,
}
pub(in super::super) struct Prepared {
    pub(in super::super) catalog: Arc<Catalog>,
    pub(in super::super) axes: [f64; 8],
    pub(in super::super) buttons: [bool; 19],
    pub(in super::super) message: Option<String>,
    pub(in super::super) log: Option<String>,
    pub(in super::super) connected_log: Option<String>,
    pub(in super::super) incoming: bool,
    pub(in super::super) catalog_change: bool,
    callbacks: Vec<NativeCallbackPayload>,
}
impl Prepared {
    pub(in super::super) fn emit(
        &self,
        ctx: &mut golden_core::process_ctx::ProcessCtx,
        node: golden_core::node::NodeId,
    ) {
        for callback in &self.callbacks {
            ctx.emit_custom_event(callback.event(node));
        }
    }
}
pub(super) struct Draft {
    prepared: Prepared,
    callback: Option<NativeCallbackDraft>,
}
impl Draft {
    pub(super) fn try_finish(
        &mut self,
        sources: &PayloadPool,
        domains: &PayloadPool,
        callbacks: &NativeCallbackPreparer,
    ) -> Option<Prepared> {
        if self.prepared.catalog_change {
            sources
                .try_retain_arc(self.prepared.catalog.clone(), CATALOG_WEIGHT)
                .ok()?;
            let options = &self.prepared.catalog.options;
            domains
                .try_retain_arc(options.shared().clone(), options.retained_bytes())
                .ok()?;
        }
        let payloads = if let Some(draft) = &self.callback {
            vec![callbacks.try_retain(draft)?]
        } else {
            Vec::new()
        };
        Some(Prepared {
            catalog: self.prepared.catalog.clone(),
            axes: self.prepared.axes,
            buttons: self.prepared.buttons,
            message: self.prepared.message.take(),
            log: self.prepared.log.take(),
            connected_log: self.prepared.connected_log.take(),
            incoming: self.prepared.incoming,
            catalog_change: self.prepared.catalog_change,
            callbacks: payloads,
        })
    }
}
pub(super) struct Preparer {
    pub(super) catalog: Option<Arc<Catalog>>,
    raw: [f64; 8],
    axes: [f64; 8],
    buttons: [bool; 19],
}
impl Preparer {
    pub(super) fn new() -> Self {
        Self {
            catalog: None,
            raw: [0.0; 8],
            axes: [0.0; 8],
            buttons: [false; 19],
        }
    }
    pub(super) fn catalog(
        &mut self,
        devices: Vec<DiscoveredGamepad>,
        settings: &Settings,
        state: Option<GamepadStateSnapshot>,
    ) -> Result<Draft, String> {
        validate_devices(&devices)?;
        let selection = settings.selection();
        let mut options = gamepad_device_options(&devices);
        if gamepad_specific_device_selected(selection) && !options.iter().any(|o| o.variant_id == selection) {
            options.insert(2, missing_gamepad_option(selection));
        }
        let mut options: ParameterEnumOptions = options.into();
        if options.len() > 67 || options.retained_bytes() > 2 * 1024 * 1024 {
            return Err("Gamepad choices exceed 67 rows / 2 MiB".into());
        }
        if let Some(previous) = &self.catalog {
            if previous.options == options {
                options = previous.options.clone();
            }
        }
        let selected = selected_gamepad(selection, &devices);
        let connected_log = selected
            .as_ref()
            .filter(|device| {
                self.catalog
                    .as_ref()
                    .and_then(|c| c.selected.as_ref())
                    .map(|d| &d.variant_id)
                    != Some(&device.variant_id)
            })
            .map(|d| format!("Connected to gamepad: {} ({})", d.label, d.details));
        let catalog = Arc::new(Catalog {
            settings: settings.clone(),
            selection: selection.into(),
            devices,
            selected,
            options,
        });
        self.catalog = Some(catalog.clone());
        self.raw = [0.0; 8];
        self.axes = [0.0; 8];
        self.buttons = [false; 19];
        if let Some(state) = state {
            validate_state(&state)?;
            if catalog
                .selected
                .as_ref()
                .is_some_and(|d| d.variant_id == state.device.variant_id)
            {
                self.raw = state.axes;
                self.buttons = state.buttons.map(|(pressed, _)| pressed);
                for index in 0..8 {
                    self.axes[index] = process_axis_value(
                        self.raw[index],
                        settings.axes[index].dead_zone,
                        settings.axes[index].offset,
                    );
                }
                self.dpad();
            }
        }
        Ok(Draft {
            prepared: Prepared {
                catalog,
                axes: self.axes,
                buttons: self.buttons,
                message: None,
                log: None,
                connected_log,
                incoming: false,
                catalog_change: true,
                callbacks: Vec::new(),
            },
            callback: None,
        })
    }
    pub(super) fn event(
        &mut self,
        event: GamepadRuntimeEvent,
        settings: &Settings,
        state: Option<GamepadStateSnapshot>,
    ) -> Result<Option<Draft>, String> {
        validate_event(&event)?;
        let mut catalog_change = false;
        let mut connected_log = None;
        if matches!(
            event,
            GamepadRuntimeEvent::Connected { .. } | GamepadRuntimeEvent::Disconnected { .. }
        ) {
            let mut devices = Vec::with_capacity(64);
            if let Some(catalog) = &self.catalog {
                devices.extend(catalog.devices.iter().cloned());
            }
            match &event {
                GamepadRuntimeEvent::Connected { device } => {
                    if !devices.iter().any(|d| d.variant_id == device.variant_id) {
                        if devices.len() == 64 {
                            return Err("Gamepad catalog exceeds 64 devices".into());
                        }
                        devices.push(device.clone());
                    }
                }
                GamepadRuntimeEvent::Disconnected { device } => devices.retain(|d| d.variant_id != device.variant_id),
                _ => {}
            }
            let draft = self.catalog(devices, settings, state)?;
            connected_log = draft.prepared.connected_log;
            catalog_change = true;
        }
        let Some(catalog) = self.catalog.clone() else {
            return Ok(None);
        };
        let device = event_device(&event);
        let selected = catalog
            .selected
            .as_ref()
            .is_some_and(|d| d.variant_id == device.variant_id);
        let arg = serde_json::json!({"id":device.variant_id,"index":device.index,"name":device.label,"details":device.details});
        let mut message = None;
        let log;
        let mut incoming = false;
        let (callback, args) = match event {
            GamepadRuntimeEvent::Connected { device } => {
                log = Some(format!("Gamepad connected: {}", device.label));
                (GAMEPAD_CONNECTED_CALLBACK, vec![arg])
            }
            GamepadRuntimeEvent::Disconnected { device } => {
                log = Some(format!("Gamepad disconnected: {}", device.label));
                (GAMEPAD_DISCONNECTED_CALLBACK, vec![arg])
            }
            GamepadRuntimeEvent::AxisChanged { axis, value, .. } if selected => {
                let index = axis_index(axis);
                self.raw[index] = value;
                self.axes[index] =
                    process_axis_value(value, settings.axes[index].dead_zone, settings.axes[index].offset);
                let processed = self.axes[index];
                incoming = true;
                message = Some(format!("{} {processed:.4}", axis.label()));
                log = Some(format!(
                    "Gamepad axis {} = {processed:.4} (raw {value:.4})",
                    axis.label()
                ));
                (
                    GAMEPAD_AXIS_CHANGED_CALLBACK,
                    vec![
                        serde_json::json!(axis.decl_id()),
                        serde_json::json!(processed),
                        serde_json::json!(value),
                        arg,
                    ],
                )
            }
            GamepadRuntimeEvent::ButtonPressed { button, .. } if selected => {
                log = self.button(button, true);
                incoming = true;
                message = Some(format!("{} pressed", button.label()));
                (
                    GAMEPAD_BUTTON_PRESSED_CALLBACK,
                    vec![serde_json::json!(button.decl_id()), serde_json::json!(1.0), arg],
                )
            }
            GamepadRuntimeEvent::ButtonReleased { button, .. } if selected => {
                log = self.button(button, false);
                incoming = true;
                message = Some(format!("{} released", button.label()));
                (
                    GAMEPAD_BUTTON_RELEASED_CALLBACK,
                    vec![serde_json::json!(button.decl_id()), serde_json::json!(0.0), arg],
                )
            }
            GamepadRuntimeEvent::ButtonChanged { button, value, .. } if selected => {
                let pressed = value > 0.5;
                log = self.button(button, pressed);
                incoming = true;
                message = Some(format!("{} {value:.4}", button.label()));
                (
                    GAMEPAD_BUTTON_CHANGED_CALLBACK,
                    vec![
                        serde_json::json!(button.decl_id()),
                        serde_json::json!(value),
                        serde_json::json!(pressed),
                        arg,
                    ],
                )
            }
            _ => return Ok(None),
        };
        let callback = Some(NativeCallbackDraft::new(callback, args)?);
        Ok(Some(Draft {
            prepared: Prepared {
                catalog,
                axes: self.axes,
                buttons: self.buttons,
                message,
                log,
                connected_log,
                incoming,
                catalog_change,
                callbacks: Vec::new(),
            },
            callback,
        }))
    }
    fn button(&mut self, button: GamepadButton, pressed: bool) -> Option<String> {
        let i = GamepadButton::ALL.iter().position(|b| *b == button).unwrap();
        let changed = self.buttons[i] != pressed;
        self.buttons[i] = pressed;
        if matches!(
            button,
            GamepadButton::DPadUp | GamepadButton::DPadDown | GamepadButton::DPadLeft | GamepadButton::DPadRight
        ) {
            self.dpad();
        }
        changed.then(|| {
            format!(
                "Button {} {}",
                button.label(),
                if pressed { "pressed" } else { "released" }
            )
        })
    }
    fn dpad(&mut self) {
        let pressed = |button| self.buttons[GamepadButton::ALL.iter().position(|b| *b == button).unwrap()];
        self.axes[6] = if pressed(GamepadButton::DPadRight) {
            1.0
        } else if pressed(GamepadButton::DPadLeft) {
            -1.0
        } else {
            0.0
        };
        self.axes[7] = if pressed(GamepadButton::DPadUp) {
            1.0
        } else if pressed(GamepadButton::DPadDown) {
            -1.0
        } else {
            0.0
        };
    }
}
pub(super) fn event_device(event: &GamepadRuntimeEvent) -> &DiscoveredGamepad {
    match event {
        GamepadRuntimeEvent::Connected { device }
        | GamepadRuntimeEvent::Disconnected { device }
        | GamepadRuntimeEvent::AxisChanged { device, .. }
        | GamepadRuntimeEvent::ButtonPressed { device, .. }
        | GamepadRuntimeEvent::ButtonReleased { device, .. }
        | GamepadRuntimeEvent::ButtonChanged { device, .. } => device,
    }
}
pub(super) fn validate_devices(devices: &Vec<DiscoveredGamepad>) -> Result<(), String> {
    if devices.capacity() > 64 {
        return Err("Gamepad catalog exceeds 64 devices".into());
    }
    for device in devices {
        validate_device(device)?;
    }
    Ok(())
}
pub(super) fn validate_device(device: &DiscoveredGamepad) -> Result<(), String> {
    if device.index > i32::MAX as usize
        || [&device.variant_id, &device.label, &device.details]
            .iter()
            .any(|s| s.capacity() > 1024)
    {
        return Err("Gamepad device exceeds identity/text bounds".into());
    }
    Ok(())
}
pub(super) fn validate_event(event: &GamepadRuntimeEvent) -> Result<(), String> {
    validate_device(event_device(event))?;
    if matches!(event,GamepadRuntimeEvent::AxisChanged{value,..}|GamepadRuntimeEvent::ButtonChanged{value,..} if !value.is_finite())
    {
        return Err("Gamepad input is not finite".into());
    }
    Ok(())
}
fn validate_state(state: &GamepadStateSnapshot) -> Result<(), String> {
    validate_device(&state.device)?;
    if !state.axes.iter().all(|v| v.is_finite()) || !state.buttons.iter().all(|(_, v)| v.is_finite()) {
        return Err("Gamepad state is not finite".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests;
