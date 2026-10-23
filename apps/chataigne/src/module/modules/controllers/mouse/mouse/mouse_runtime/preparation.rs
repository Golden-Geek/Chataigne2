use super::super::{devices::*, *};
use super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPayload, NativeCallbackPreparer};
use golden_core::parameter::ParameterEnumOptions;
use std::time::{Duration, Instant};

pub(super) const FRAME_WEIGHT: usize = 64 * 1024;

pub(in super::super) struct Catalog {
    pub(in super::super) devices: Vec<DiscoveredMouseDevice>,
    pub(in super::super) requested: String,
    pub(in super::super) selection: String,
    pub(in super::super) options: ParameterEnumOptions,
    pub(in super::super) selected: Option<DiscoveredMouseDevice>,
    pub(in super::super) capture_os_input: bool,
}
pub(in super::super) struct Frame {
    pub(in super::super) position: (i32, i32),
    pub(in super::super) delta: (i32, i32),
    pub(in super::super) buttons: [bool; 3],
    pub(in super::super) message: String,
    pub(in super::super) log: String,
    callbacks: [Vec<NativeCallbackPayload>; 2],
}
impl Frame {
    pub(in super::super) fn emit(&self, ctx: &mut ProcessCtx, module: NodeId, output_active: bool) {
        for callback in &self.callbacks[usize::from(output_active)] {
            ctx.emit_custom_event(callback.event(module));
        }
    }
}
pub(in super::super) enum Row {
    Catalog { reset: bool },
    Input(Frame),
    DeltaReset,
}
pub(in super::super) struct Prepared {
    pub(in super::super) catalog: Arc<Catalog>,
    pub(in super::super) row: Row,
}
pub(super) enum Draft {
    Catalog {
        catalog: Arc<Catalog>,
        reset: bool,
    },
    DeltaReset {
        catalog: Arc<Catalog>,
    },
    Input {
        catalog: Arc<Catalog>,
        frame: Frame,
        drafts: Vec<NativeCallbackDraft>,
    },
}
impl Draft {
    pub(super) fn weight(&self) -> usize {
        FRAME_WEIGHT
    }
    pub(super) fn try_finish(
        &mut self,
        sources: &PayloadPool,
        domains: &PayloadPool,
        callbacks: &NativeCallbackPreparer,
    ) -> Option<Prepared> {
        match self {
            Self::DeltaReset { catalog } => Some(Prepared {
                catalog: catalog.clone(),
                row: Row::DeltaReset,
            }),
            Self::Catalog { catalog, reset } => {
                // Frames keep this metadata after its first envelope has gone.
                sources.try_retain_arc(catalog.clone(), CATALOG_WEIGHT).ok()?;
                domains
                    .try_retain_arc(catalog.options.shared().clone(), catalog.options.retained_bytes())
                    .ok()?;
                Some(Prepared {
                    catalog: catalog.clone(),
                    row: Row::Catalog { reset: *reset },
                })
            }
            Self::Input { catalog, frame, drafts } => {
                let payloads = callbacks.try_retain_batch(&drafts.iter().collect::<Vec<_>>())?;
                let split = payloads.len() / 2;
                frame.callbacks = [payloads[..split].to_vec(), payloads[split..].to_vec()];
                Some(Prepared {
                    catalog: catalog.clone(),
                    row: Row::Input(Frame {
                        position: frame.position,
                        delta: frame.delta,
                        buttons: frame.buttons,
                        message: std::mem::take(&mut frame.message),
                        log: std::mem::take(&mut frame.log),
                        callbacks: std::mem::take(&mut frame.callbacks),
                    }),
                })
            }
        }
    }
}
pub(super) struct Preparer {
    catalog: Option<Arc<Catalog>>,
    position: (i32, i32),
    delta: (i32, i32),
    buttons: [bool; 3],
    delta_at: Option<Instant>,
    capture: bool,
}
impl Preparer {
    pub(super) fn new(capture: bool) -> Self {
        Self {
            catalog: None,
            position: (0, 0),
            delta: (0, 0),
            buttons: [false; 3],
            delta_at: None,
            capture,
        }
    }
    fn reset(&mut self) {
        self.position = (0, 0);
        self.delta = (0, 0);
        self.buttons = [false; 3];
        self.delta_at = None;
    }
    pub(super) fn expire_delta(&mut self, now: Instant) -> Option<Draft> {
        if !self.delta_at.is_some_and(|at| now >= at) {
            return None;
        }
        self.delta_at = None;
        self.delta = (0, 0);
        Some(Draft::DeltaReset {
            catalog: self.catalog.as_ref()?.clone(),
        })
    }
    pub(super) fn configure(&mut self, selection: &str) -> Result<Option<Draft>, String> {
        self.reset();
        let Some(devices) = self.catalog.as_ref().map(|catalog| catalog.devices.clone()) else {
            return Ok(None);
        };
        self.catalog(devices, selection, true).map(Some)
    }
    pub(super) fn event(&mut self, event: MouseRuntimeEvent, selection: &str) -> Result<Option<Draft>, String> {
        match event {
            MouseRuntimeEvent::DevicesChanged(devices) => self.catalog(devices, selection, false).map(Some),
            MouseRuntimeEvent::Input { device, event } => self.input(&device, event),
        }
    }
    fn catalog(
        &mut self,
        devices: Vec<DiscoveredMouseDevice>,
        requested: &str,
        force_reset: bool,
    ) -> Result<Draft, String> {
        let mut options = mouse_device_options(&devices);
        let selection = canonical_mouse_variant(requested, &options);
        if mouse_specific_device_selected(&selection) && !options.iter().any(|row| row.variant_id == selection) {
            options.insert(2, missing_mouse_option(&selection));
        }
        let mut options: ParameterEnumOptions = options.into();
        if options.len() > MAX_DEVICES + 3 || options.retained_bytes() > 2 * 1024 * 1024 {
            return Err("Mouse prepared choices exceed 67 rows / 2 MiB".into());
        }
        if let Some(previous) = self.catalog.as_ref().filter(|previous| previous.options == options) {
            options = previous.options.clone();
        }
        let selected = selected_mouse_device(&selection, &devices);
        let reset = force_reset
            || self
                .catalog
                .as_ref()
                .and_then(|c| c.selected.as_ref())
                .map(|d| &d.variant_id)
                != selected.as_ref().map(|d| &d.variant_id);
        if reset {
            self.reset();
        }
        let catalog = Arc::new(Catalog {
            devices,
            requested: requested.into(),
            selection,
            options,
            selected,
            capture_os_input: self.capture,
        });
        self.catalog = Some(catalog.clone());
        Ok(Draft::Catalog { catalog, reset })
    }
    fn input(&mut self, device: &str, event: MouseInputEvent) -> Result<Option<Draft>, String> {
        let Some(catalog) = self.catalog.as_ref() else {
            return Ok(None);
        };
        let Some(selected) = catalog
            .selected
            .as_ref()
            .filter(|selected| selected.variant_id == device)
        else {
            return Ok(None);
        };
        match event {
            MouseInputEvent::Moved { x, y, dx, dy } => {
                self.position = (x, y);
                self.delta = (dx, dy);
                self.delta_at = Some(Instant::now() + Duration::from_millis(8));
            }
            MouseInputEvent::ButtonChanged { button, pressed } => self.buttons[button_index(button)] = pressed,
        }
        let mut drafts = Vec::with_capacity(4);
        for output_active in [false, true] {
            let state = serde_json::json!({
                "position": { "x": self.position.0, "y": self.position.1 },
                "delta": { "x": self.delta.0, "y": self.delta.1 },
                "buttons": {
                    "left": self.buttons[0], "middle": self.buttons[1], "right": self.buttons[2],
                },
                "device": {
                    "selection": catalog.selection, "id": selected.variant_id,
                    "name": selected.label, "connectedDevices": catalog.devices.len(),
                },
                "inputActive": true, "outputActive": output_active,
            });
            match event {
                MouseInputEvent::Moved { x, y, dx, dy } => drafts.push(NativeCallbackDraft::new(
                    MOUSE_MOVED_CALLBACK,
                    vec![
                        serde_json::json!({"x":x,"y":y}),
                        serde_json::json!({"x":dx,"y":dy}),
                        state,
                    ],
                )?),
                MouseInputEvent::ButtonChanged { button, pressed } => {
                    drafts.push(NativeCallbackDraft::new(
                        MOUSE_BUTTON_CHANGED_CALLBACK,
                        vec![
                            serde_json::json!(button.as_str()),
                            serde_json::json!(pressed),
                            state.clone(),
                        ],
                    )?);
                    drafts.push(NativeCallbackDraft::new(
                        if pressed {
                            MOUSE_BUTTON_PRESSED_CALLBACK
                        } else {
                            MOUSE_BUTTON_RELEASED_CALLBACK
                        },
                        vec![serde_json::json!(button.as_str()), state],
                    )?);
                }
            }
        }
        if drafts.iter().map(NativeCallbackDraft::retained_bytes).sum::<usize>() > 1024 * 1024 {
            return Err("Mouse callback candidate batch exceeds 1 MiB".into());
        }
        let (message, log) = match event {
            MouseInputEvent::Moved { x, y, dx, dy } => (
                format!("{}: moved mouse to ({x}, {y})", selected.label),
                format!("Observed {} move to ({x}, {y}) with delta ({dx}, {dy})", selected.label),
            ),
            MouseInputEvent::ButtonChanged { button, pressed } => {
                let action = if pressed { "pressed" } else { "released" };
                (
                    format!("{}: {} button {action}", selected.label, button.label()),
                    format!(
                        "Observed {} {} mouse button {action}",
                        selected.label,
                        button.label().to_ascii_lowercase()
                    ),
                )
            }
        };
        let frame = Frame {
            position: self.position,
            delta: self.delta,
            buttons: self.buttons,
            message,
            log,
            callbacks: std::array::from_fn(|_| Vec::new()),
        };
        Ok(Some(Draft::Input {
            catalog: catalog.clone(),
            frame,
            drafts,
        }))
    }
}
pub(in super::super) fn button_index(button: MouseButtonKind) -> usize {
    match button {
        MouseButtonKind::Left => 0,
        MouseButtonKind::Middle => 1,
        MouseButtonKind::Right => 2,
    }
}

#[cfg(test)]
mod tests;
