use super::super::{selection::*, *};
use super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPayload, NativeCallbackPreparer};
use golden_core::parameter::ParameterEnumOptions;
use std::collections::BTreeSet;

pub(super) const FRAME_WEIGHT: usize = 64 * 1024;

pub(in super::super) struct Catalog {
    pub(in super::super) devices: Vec<DiscoveredKeyboardDevice>,
    pub(in super::super) requested: String,
    pub(in super::super) selection: String,
    pub(in super::super) options: ParameterEnumOptions,
    pub(in super::super) selected: Option<DiscoveredKeyboardDevice>,
}
pub(in super::super) struct Frame {
    pub(in super::super) key: KeyboardKey,
    pub(in super::super) pressed: bool,
    pub(in super::super) modifiers: [bool; 9],
    pub(in super::super) held: String,
    pub(in super::super) count: usize,
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
                frame.callbacks = [payloads[..2].to_vec(), payloads[2..].to_vec()];
                Some(Prepared {
                    catalog: catalog.clone(),
                    row: Row::Input(Frame {
                        key: frame.key,
                        pressed: frame.pressed,
                        modifiers: frame.modifiers,
                        held: std::mem::take(&mut frame.held),
                        count: frame.count,
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
    pressed: BTreeSet<KeyboardKey>,
}
impl Preparer {
    pub(super) fn new() -> Self {
        Self {
            catalog: None,
            pressed: BTreeSet::new(),
        }
    }
    pub(super) fn configure(&mut self, selection: &str) -> Result<Option<Draft>, String> {
        self.pressed.clear();
        let Some(devices) = self.catalog.as_ref().map(|catalog| catalog.devices.clone()) else {
            return Ok(None);
        };
        self.catalog(devices, selection, true).map(Some)
    }
    pub(super) fn event(&mut self, event: KeyboardRuntimeEvent, selection: &str) -> Result<Option<Draft>, String> {
        match event {
            KeyboardRuntimeEvent::DevicesChanged(devices) => self.catalog(devices, selection, false).map(Some),
            KeyboardRuntimeEvent::Input { device, event } => self.input(&device, event),
        }
    }
    fn catalog(
        &mut self,
        devices: Vec<DiscoveredKeyboardDevice>,
        requested: &str,
        force_reset: bool,
    ) -> Result<Draft, String> {
        let mut options = keyboard_device_options(&devices);
        let selection = canonical_keyboard_variant(requested, &options);
        if keyboard_specific_device_selected(&selection) && !options.iter().any(|row| row.variant_id == selection) {
            options.insert(2, missing_keyboard_option(&selection));
        }
        let mut options: ParameterEnumOptions = options.into();
        if options.len() > MAX_DEVICES + 3 || options.retained_bytes() > 2 * 1024 * 1024 {
            return Err("Keyboard prepared choices exceed 67 rows / 2 MiB".into());
        }
        if let Some(previous) = self.catalog.as_ref().filter(|previous| previous.options == options) {
            options = previous.options.clone();
        }
        let selected = selected_keyboard_device(&selection, &devices);
        let reset = force_reset
            || self
                .catalog
                .as_ref()
                .and_then(|c| c.selected.as_ref())
                .map(|d| &d.variant_id)
                != selected.as_ref().map(|d| &d.variant_id);
        if reset {
            self.pressed.clear();
        }
        let catalog = Arc::new(Catalog {
            devices,
            requested: requested.into(),
            selection,
            options,
            selected,
        });
        self.catalog = Some(catalog.clone());
        Ok(Draft::Catalog { catalog, reset })
    }
    fn input(&mut self, device: &str, event: KeyboardInputEvent) -> Result<Option<Draft>, String> {
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
        let KeyboardInputEvent::KeyChanged { key, pressed } = event;
        if pressed {
            self.pressed.insert(key);
        } else {
            self.pressed.remove(&key);
        }
        let modifiers = modifier_keys().map(|key| self.pressed.contains(&key));
        let held = self.pressed.iter().map(|key| key.as_str()).collect::<Vec<_>>();
        let mut drafts = Vec::with_capacity(4);
        for output_active in [false, true] {
            let state = serde_json::json!({
                "heldKeys": held,
                "modifiers": {
                    "leftShift": modifiers[0], "rightShift": modifiers[1],
                    "leftControl": modifiers[2], "rightControl": modifiers[3],
                    "leftAlt": modifiers[4], "rightAlt": modifiers[5],
                    "leftMeta": modifiers[6], "rightMeta": modifiers[7], "capsLock": modifiers[8],
                },
                "device": { "selection": catalog.selection, "id": selected.variant_id,
                    "name": selected.label, "connectedDevices": catalog.devices.len() },
                "lastKey": key.as_str(), "lastPressed": pressed,
                "inputActive": true, "outputActive": output_active,
            });
            drafts.push(NativeCallbackDraft::new(
                KEYBOARD_KEY_CHANGED_CALLBACK,
                vec![
                    serde_json::json!(key.as_str()),
                    serde_json::json!(pressed),
                    state.clone(),
                ],
            )?);
            drafts.push(NativeCallbackDraft::new(
                if pressed {
                    KEYBOARD_KEY_PRESSED_CALLBACK
                } else {
                    KEYBOARD_KEY_RELEASED_CALLBACK
                },
                vec![serde_json::json!(key.as_str()), state],
            )?);
        }
        if drafts.iter().map(NativeCallbackDraft::retained_bytes).sum::<usize>() > 1024 * 1024 {
            return Err("Keyboard callback candidate batch exceeds 1 MiB".into());
        }
        let action = if pressed { "pressed" } else { "released" };
        let frame = Frame {
            key,
            pressed,
            modifiers,
            held: held.join(", "),
            count: held.len(),
            message: format!("{}: {} key {action}", selected.label, key.label()),
            log: format!(
                "Observed {} {} key {action}",
                selected.label,
                key.label().to_ascii_lowercase()
            ),
            callbacks: std::array::from_fn(|_| Vec::new()),
        };
        Ok(Some(Draft::Input {
            catalog: catalog.clone(),
            frame,
            drafts,
        }))
    }
}
pub(in super::super) fn modifier_keys() -> [KeyboardKey; 9] {
    [
        KeyboardKey::LeftShift,
        KeyboardKey::RightShift,
        KeyboardKey::LeftControl,
        KeyboardKey::RightControl,
        KeyboardKey::LeftAlt,
        KeyboardKey::RightAlt,
        KeyboardKey::LeftMeta,
        KeyboardKey::RightMeta,
        KeyboardKey::CapsLock,
    ]
}

#[cfg(test)]
mod tests;
