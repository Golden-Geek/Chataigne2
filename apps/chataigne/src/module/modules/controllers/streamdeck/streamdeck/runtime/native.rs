use super::*;
use elgato_streamdeck::{info::Kind, list_devices, new_hidapi, StreamDeck, StreamDeckInput};
mod image;
pub(super) struct Native {
    device: Option<Connected>,
}
struct Connected {
    device: StreamDeck,
    info: DeviceInfo,
    kind: Kind,
    pressed: [bool; MAX_KEYS],
}
pub(super) fn create() -> Result<Native, String> {
    if cfg!(test) {
        return Err("Native Stream Deck endpoints are disabled in test builds".into());
    }
    Ok(Native { device: None })
}
impl Drop for Native {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}
impl Driver for Native {
    fn discover(&mut self) -> Result<Vec<DeviceInfo>, String> {
        let api = new_hidapi().map_err(|e| format!("Stream Deck discovery failed: {e}"))?;
        let mut devices = Vec::with_capacity(64);
        // The SDK's private enumeration allocation is separate from this bounded catalog.
        for (kind, serial) in list_devices(&api).into_iter().take(65) {
            if devices.len() == 64 {
                return Err("Stream Deck catalog exceeds 64 devices".into());
            }
            let info = DeviceInfo {
                serial,
                product: format!("{kind:?}"),
                key_count: usize::from(kind.key_count()),
                feedback: kind != Kind::Pedal,
            };
            info.validate()?;
            devices.push(info);
        }
        devices.sort_unstable_by(|a, b| a.serial.cmp(&b.serial));
        Ok(devices)
    }
    fn connect(&mut self, serial: &str) -> Result<DeviceInfo, String> {
        if serial.len() > MAX_SERIAL {
            return Err("Stream Deck serial exceeds 256 bytes".into());
        }
        self.disconnect()?;
        let api = new_hidapi().map_err(|e| format!("Stream Deck HID setup failed: {e}"))?;
        let kind = list_devices(&api)
            .into_iter()
            .take(64)
            .find_map(|(kind, id)| (id == serial).then_some(kind))
            .ok_or("Selected Stream Deck is not connected")?;
        let info = DeviceInfo {
            serial: serial.into(),
            product: format!("{kind:?}"),
            key_count: usize::from(kind.key_count()),
            feedback: kind != Kind::Pedal,
        };
        info.validate()?;
        // Validate identity before obtaining a handle; metadata cannot strand cleanup
        // in a temporary local after the SDK returns an owned device.
        let device =
            StreamDeck::connect(&api, kind, serial).map_err(|e| format!("Stream Deck connection failed: {e}"))?;
        self.device = Some(Connected {
            device,
            kind,
            info: info.clone(),
            pressed: [false; MAX_KEYS],
        });
        Ok(info)
    }
    fn disconnect(&mut self) -> Result<(), String> {
        if let Some(device) = self.device.take() {
            if device.info.feedback {
                device
                    .device
                    .clear_all_button_images()
                    .map_err(|e| format!("Stream Deck cleanup failed: {e}"))?;
            }
        }
        Ok(())
    }
    fn poll(&mut self) -> Result<Vec<Input>, String> {
        let Some(device) = self.device.as_mut() else {
            return Ok(Vec::new());
        };
        let report = device
            .device
            .read_input(Some(std::time::Duration::ZERO))
            .map_err(|e| format!("Stream Deck input failed: {e}"))?;
        let StreamDeckInput::ButtonStateChange(states) = report else {
            return Ok(Vec::new());
        };
        if states.len() > device.info.key_count {
            return Err("Stream Deck button report exceeds key count".into());
        }
        let mut events = Vec::with_capacity(MAX_KEYS);
        for (index, pressed) in states.into_iter().enumerate() {
            if device.pressed[index] != pressed {
                device.pressed[index] = pressed;
                events.push(if pressed { Input::Down(index) } else { Input::Up(index) });
            }
        }
        Ok(events)
    }
    fn render(&mut self, index: usize, visual: &KeyVisual) -> Result<(), RenderError> {
        let Some(device) = &self.device else {
            return Err(RenderError::Device("Stream Deck is disconnected".into()));
        };
        if !device.info.feedback {
            return Ok(());
        }
        if index >= device.info.key_count {
            return Err(RenderError::Device("Stream Deck key index is invalid".into()));
        }
        let format = device.kind.key_image_format();
        let image = image::compose(format.size, visual).map_err(RenderError::Asset)?;
        device
            .device
            .set_button_image(index as u8, image)
            .map_err(|e| RenderError::Device(format!("Stream Deck rendering failed: {e}")))
    }
    fn brightness(&mut self, percent: u8) -> Result<(), String> {
        if let Some(device) = &self.device {
            if device.info.feedback {
                device
                    .device
                    .set_brightness(percent.min(100))
                    .map_err(|e| format!("Stream Deck brightness failed: {e}"))?;
            }
        }
        Ok(())
    }
}
