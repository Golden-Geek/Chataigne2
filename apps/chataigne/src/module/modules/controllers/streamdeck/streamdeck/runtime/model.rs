pub(in super::super) const MAX_KEYS: usize = 64;
pub(in super::super) const MAX_VISUALS: usize = 32;
pub(in super::super) const MAX_SERIAL: usize = 256;
pub(in super::super) const MAX_TEXT: usize = 1024;
pub(in super::super) const MAX_IMAGE_PATH: usize = 4096;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum Input {
    Down(usize),
    Up(usize),
}
impl Input {
    pub(in super::super) fn parts(self) -> (usize, bool) {
        match self {
            Self::Down(index) => (index, true),
            Self::Up(index) => (index, false),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(in super::super) struct KeyVisual {
    pub(in super::super) color: (f64, f64, f64, f64),
    pub(in super::super) text: String,
    pub(in super::super) image: String,
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
impl KeyVisual {
    pub(in super::super) fn validate(&self) -> Result<(), String> {
        if self.text.capacity() > MAX_TEXT
            || self.image.capacity() > MAX_IMAGE_PATH
            || ![self.color.0, self.color.1, self.color.2, self.color.3]
                .iter()
                .all(|v| v.is_finite())
        {
            return Err("Stream Deck visual exceeds text/path bounds or has nonfinite color".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct DeviceInfo {
    pub(in super::super) serial: String,
    pub(in super::super) product: String,
    pub(in super::super) key_count: usize,
    pub(in super::super) feedback: bool,
}
impl DeviceInfo {
    pub(in super::super) fn validate(&self) -> Result<(), String> {
        if self.serial.is_empty()
            || self.serial.capacity() > MAX_SERIAL
            || self.product.capacity() > MAX_SERIAL
            || !(1..=MAX_KEYS).contains(&self.key_count)
        {
            return Err("Stream Deck identity or key count exceeds supported bounds".into());
        }
        Ok(())
    }
}
#[derive(Debug)]
pub(in super::super) enum RenderError {
    Asset(String),
    Device(String),
}
pub(in super::super) trait Driver: 'static {
    fn discover(&mut self) -> Result<Vec<DeviceInfo>, String>;
    fn connect(&mut self, serial: &str) -> Result<DeviceInfo, String>;
    fn disconnect(&mut self) -> Result<(), String>;
    fn poll(&mut self) -> Result<Vec<Input>, String>;
    fn render(&mut self, index: usize, visual: &KeyVisual) -> Result<(), RenderError>;
    fn brightness(&mut self, percent: u8) -> Result<(), String>;
}
