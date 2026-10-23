use super::*;
use gilrs::{EventType, Gamepad, GamepadId, Gilrs, GilrsBuilder};
pub(super) struct Native {
    gilrs: Gilrs,
    seed: bool,
}
pub(super) fn create() -> Result<Native, String> {
    if cfg!(test) {
        return Err("Native Gamepad endpoints are disabled in test builds".into());
    }
    match GilrsBuilder::new().with_force_feedback(false).build() {
        Ok(gilrs) => Ok(Native { gilrs, seed: true }),
        Err(gilrs::Error::NotImplemented(_)) => Err("gamepad input is not supported on this platform".into()),
        Err(error) => Err(format!("failed to initialize gamepad input: {error}")),
    }
}
impl Native {
    fn catalog(&self) -> Result<Vec<DiscoveredGamepad>, String> {
        let mut devices = Vec::with_capacity(64);
        for (id, pad) in self.gilrs.gamepads().take(65) {
            if devices.len() == 64 {
                return Err("Gamepad catalog exceeds 64 devices".into());
            }
            devices.push(device(id, &pad)?);
        }
        Ok(devices)
    }
}
impl Provider for Native {
    fn poll(&mut self) -> Result<Vec<Record>, String> {
        if self.seed {
            self.seed = false;
            return Ok(vec![Record::Catalog(self.catalog()?)]);
        }
        let mut rows = Vec::with_capacity(16);
        // A connection edge can produce a catalog and its ordered callback.
        for _ in 0..8 {
            let Some(event) = self.gilrs.next_event() else {
                break;
            };
            let device = device(event.id, &self.gilrs.gamepad(event.id))?;
            let event = match event.event {
                EventType::Connected => {
                    rows.push(Record::Catalog(self.catalog()?));
                    Some(GamepadRuntimeEvent::Connected { device })
                }
                EventType::Disconnected => {
                    rows.push(Record::Catalog(self.catalog()?));
                    Some(GamepadRuntimeEvent::Disconnected { device })
                }
                EventType::AxisChanged(axis, value, _) => {
                    GamepadAxis::from_gilrs(axis).map(|axis| GamepadRuntimeEvent::AxisChanged {
                        device,
                        axis,
                        value: f64::from(value),
                    })
                }
                EventType::ButtonPressed(button, _) => GamepadButton::from_gilrs(button)
                    .map(|button| GamepadRuntimeEvent::ButtonPressed { device, button }),
                EventType::ButtonReleased(button, _) => GamepadButton::from_gilrs(button)
                    .map(|button| GamepadRuntimeEvent::ButtonReleased { device, button }),
                EventType::ButtonChanged(button, value, _) => {
                    GamepadButton::from_gilrs(button).map(|button| GamepadRuntimeEvent::ButtonChanged {
                        device,
                        button,
                        value: f64::from(value),
                    })
                }
                _ => None,
            };
            if let Some(event) = event {
                rows.push(Record::Event(event));
            }
        }
        self.gilrs.inc();
        Ok(rows)
    }
    fn state(&mut self, selection: &str) -> Result<Option<GamepadStateSnapshot>, String> {
        for (id, pad) in self.gilrs.gamepads().take(64) {
            let device = device(id, &pad)?;
            if !selection_matches_gamepad(selection, &device) {
                continue;
            }
            return Ok(Some(GamepadStateSnapshot {
                device,
                axes: std::array::from_fn(|i| f64::from(pad.value(GamepadAxis::ALL[i].to_gilrs()))),
                buttons: std::array::from_fn(|i| {
                    let button = GamepadButton::ALL[i].to_gilrs();
                    let pressed = pad.is_pressed(button);
                    let value = pad
                        .button_data(button)
                        .map(|data| f64::from(data.value()))
                        .unwrap_or(if pressed { 1.0 } else { 0.0 });
                    (pressed, value)
                }),
            }));
        }
        Ok(None)
    }
}
fn device(id: GamepadId, pad: &Gamepad<'_>) -> Result<DiscoveredGamepad, String> {
    let index = usize::from(id);
    let name = pad.name();
    if name.len() > 992 || index > i32::MAX as usize {
        return Err("Gamepad identity exceeds supported bounds".into());
    }
    let label = name.to_string();
    let vendor = pad
        .vendor_id()
        .map(|v| format!("{v:04x}"))
        .unwrap_or_else(|| "unknown".into());
    let product = pad
        .product_id()
        .map(|v| format!("{v:04x}"))
        .unwrap_or_else(|| "unknown".into());
    Ok(DiscoveredGamepad {
        index,
        variant_id: format!("gamepad:{index}|{label}").into_boxed_str().into_string(),
        label,
        details: format!("index={index}, vid={vendor}, pid={product}"),
    })
}
