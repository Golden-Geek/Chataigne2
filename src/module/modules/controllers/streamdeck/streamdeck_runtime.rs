//! Device abstraction for the Stream Deck module.
//!
//! The module talks to hardware through the [`StreamDeckDevice`] trait so the paging
//! and viewport logic is fully testable without a physical device. Two implementations
//! exist:
//!
//! * [`SimulatedStreamDeck`] — always available; records rendered visuals and lets tests
//!   inject button presses.
//! * `HidStreamDeck` — real Elgato hardware via the `elgato-streamdeck` crate, compiled
//!   only with the `streamdeck-hid` cargo feature (keeps the default build free of the
//!   native `hidapi`/`image` dependencies).

/// A normalized button transition produced by polling a device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StreamDeckInputEvent {
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
pub(crate) struct DiscoveredStreamDeck {
    pub serial: String,
    pub product: String,
    pub key_count: usize,
}

/// Hardware-facing contract. Implementations must be `Send` so the module can own one.
pub(crate) trait StreamDeckDevice: Send {
    /// Number of physical keys exposed by this device.
    fn key_count(&self) -> usize;
    /// Stable serial used to re-select the device.
    fn serial(&self) -> &str;
    /// Human-readable product name.
    fn product(&self) -> &str;
    /// Non-blocking poll returning button transitions since the last call.
    fn poll_input(&mut self) -> Vec<StreamDeckInputEvent>;
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

/// Lists devices currently visible to the host (empty unless `streamdeck-hid` is enabled).
pub(crate) fn discover_devices() -> Vec<DiscoveredStreamDeck> {
    #[cfg(feature = "streamdeck-hid")]
    {
        hid::discover()
    }
    #[cfg(not(feature = "streamdeck-hid"))]
    {
        Vec::new()
    }
}

/// Connects to a device by serial, returning a boxed driver.
pub(crate) fn connect(serial: &str) -> Result<Box<dyn StreamDeckDevice>, String> {
    #[cfg(feature = "streamdeck-hid")]
    {
        hid::connect(serial).map(|device| Box::new(device) as Box<dyn StreamDeckDevice>)
    }
    #[cfg(not(feature = "streamdeck-hid"))]
    {
        let _ = serial;
        Err("Stream Deck hardware support is not compiled in (rebuild with `--features streamdeck-hid`).".to_string())
    }
}

// ---------------------------------------------------------------------------
// Simulated device (always available — powers the test suite and headless use).
// ---------------------------------------------------------------------------

/// In-memory device that records what was rendered and replays injected presses.
pub(crate) struct SimulatedStreamDeck {
    serial: String,
    product: String,
    pressed: Vec<bool>,
    queued: Vec<StreamDeckInputEvent>,
    rendered: Vec<KeyVisual>,
    brightness: u8,
}

impl SimulatedStreamDeck {
    pub(crate) fn new(serial: impl Into<String>, key_count: usize) -> Self {
        Self {
            serial: serial.into(),
            product: "Simulated Stream Deck".to_string(),
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
            self.queued.push(StreamDeckInputEvent::ButtonDown(index));
        }
    }

    /// Injects a physical button release (test helper).
    pub(crate) fn release(&mut self, index: usize) {
        if index < self.pressed.len() && self.pressed[index] {
            self.pressed[index] = false;
            self.queued.push(StreamDeckInputEvent::ButtonUp(index));
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

impl StreamDeckDevice for SimulatedStreamDeck {
    fn key_count(&self) -> usize {
        self.pressed.len()
    }

    fn serial(&self) -> &str {
        &self.serial
    }

    fn product(&self) -> &str {
        &self.product
    }

    fn poll_input(&mut self) -> Vec<StreamDeckInputEvent> {
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

// ---------------------------------------------------------------------------
// Real HID device (elgato-streamdeck) — gated behind the `streamdeck-hid` feature.
// ---------------------------------------------------------------------------

#[cfg(feature = "streamdeck-hid")]
mod hid {
    use super::{DiscoveredStreamDeck, KeyVisual, StreamDeckDevice, StreamDeckInputEvent};
    use elgato_streamdeck::info::Kind;
    use elgato_streamdeck::{list_devices, new_hidapi, StreamDeck, StreamDeckInput};
    use std::time::Duration;

    pub(super) fn discover() -> Vec<DiscoveredStreamDeck> {
        let Ok(hidapi) = new_hidapi() else {
            return Vec::new();
        };
        list_devices(&hidapi)
            .into_iter()
            .filter_map(|(kind, serial)| {
                StreamDeck::connect(&hidapi, kind, &serial).ok().map(|device| {
                    let product = device.product().unwrap_or_else(|_| kind_label(kind));
                    DiscoveredStreamDeck {
                        serial,
                        product,
                        key_count: kind.key_count() as usize,
                    }
                })
            })
            .collect()
    }

    pub(super) fn connect(serial: &str) -> Result<HidStreamDeck, String> {
        let hidapi = new_hidapi().map_err(|err| format!("failed to open HID backend: {err}"))?;
        let kind = list_devices(&hidapi)
            .into_iter()
            .find(|(_, candidate)| candidate == serial)
            .map(|(kind, _)| kind)
            .ok_or_else(|| format!("Stream Deck '{serial}' is not connected"))?;
        let device = StreamDeck::connect(&hidapi, kind, serial)
            .map_err(|err| format!("failed to connect to Stream Deck '{serial}': {err}"))?;
        let key_count = kind.key_count() as usize;
        let product = device.product().unwrap_or_else(|_| kind_label(kind));
        let _ = device.reset();
        Ok(HidStreamDeck {
            device,
            kind,
            key_count,
            serial: serial.to_string(),
            product,
            pressed: vec![false; key_count],
        })
    }

    fn kind_label(kind: Kind) -> String {
        format!("{kind:?}")
    }

    pub(super) struct HidStreamDeck {
        device: StreamDeck,
        kind: Kind,
        key_count: usize,
        serial: String,
        product: String,
        pressed: Vec<bool>,
    }

    impl StreamDeckDevice for HidStreamDeck {
        fn key_count(&self) -> usize {
            self.key_count
        }

        fn serial(&self) -> &str {
            &self.serial
        }

        fn product(&self) -> &str {
            &self.product
        }

        fn poll_input(&mut self) -> Vec<StreamDeckInputEvent> {
            // Zero timeout => non-blocking read so the engine tick is never stalled.
            let input = match self.device.read_input(Some(Duration::ZERO)) {
                Ok(input) => input,
                Err(_) => return Vec::new(),
            };
            let StreamDeckInput::ButtonStateChange(states) = input else {
                return Vec::new();
            };
            let mut events = Vec::new();
            for (index, pressed) in states.into_iter().enumerate() {
                let was = self.pressed.get(index).copied().unwrap_or(false);
                if pressed != was {
                    if index < self.pressed.len() {
                        self.pressed[index] = pressed;
                    }
                    events.push(if pressed {
                        StreamDeckInputEvent::ButtonDown(index)
                    } else {
                        StreamDeckInputEvent::ButtonUp(index)
                    });
                }
            }
            events
        }

        fn render_key(&mut self, index: usize, visual: &KeyVisual) -> Result<(), String> {
            let image = compose_key_image(self.kind, visual);
            self.device
                .set_button_image(index as u8, image)
                .map_err(|err| format!("failed to render Stream Deck key {index}: {err}"))
            // NOTE: caption text rendering requires a bundled font; tracked as a follow-up.
            // The simulated device records `visual.text`, so paging/viewport tests cover it.
        }

        fn set_brightness(&mut self, percent: u8) -> Result<(), String> {
            self.device
                .set_brightness(percent.min(100))
                .map_err(|err| format!("failed to set Stream Deck brightness: {err}"))
        }

        fn clear(&mut self) -> Result<(), String> {
            self.device
                .clear_all_button_images()
                .map_err(|err| format!("failed to clear Stream Deck: {err}"))
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    /// Builds the final key image: a solid background of `visual.color` with the (optional)
    /// image composited over it. Transparent image pixels let the key color show through.
    fn compose_key_image(kind: Kind, visual: &KeyVisual) -> image::DynamicImage {
        use image::GenericImageView;

        let (width, height) = kind.key_image_format().size;
        let (width, height) = (width as u32, height as u32);
        let to_u8 = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let bg = image::Rgba([to_u8(visual.color.0), to_u8(visual.color.1), to_u8(visual.color.2), 255]);
        let mut canvas = image::RgbaImage::from_pixel(width, height, bg);

        if !visual.image.trim().is_empty() {
            if let Ok(overlay) = image::open(visual.image.trim()) {
                let overlay = overlay.resize_exact(width, height, image::imageops::FilterType::Triangle);
                for (x, y, pixel) in overlay.pixels() {
                    let alpha = pixel[3] as f64 / 255.0;
                    if alpha <= 0.0 {
                        continue; // fully transparent => keep key background color
                    }
                    let base = canvas.get_pixel(x, y);
                    let blend = |fg: u8, bg: u8| ((fg as f64 * alpha) + (bg as f64 * (1.0 - alpha))).round() as u8;
                    canvas.put_pixel(
                        x,
                        y,
                        image::Rgba([
                            blend(pixel[0], base[0]),
                            blend(pixel[1], base[1]),
                            blend(pixel[2], base[2]),
                            255,
                        ]),
                    );
                }
            }
        }

        image::DynamicImage::ImageRgba8(canvas).to_rgb8().into()
    }
}
