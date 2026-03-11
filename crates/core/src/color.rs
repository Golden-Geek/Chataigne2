use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// RGBA color value with normalized channel components.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Color {
    /// Red component in the `[0.0, 1.0]` range.
    r: f64,
    /// Green component in the `[0.0, 1.0]` range.
    g: f64,
    /// Blue component in the `[0.0, 1.0]` range.
    b: f64,
    /// Alpha component in the `[0.0, 1.0]` range.
    a: f64,
}

impl Color {
    /// Creates a color from RGBA components.
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    /// Returns the red component.
    pub fn r(&self) -> f64 {
        self.r
    }

    /// Returns the green component.
    pub fn g(&self) -> f64 {
        self.g
    }

    /// Returns the blue component.
    pub fn b(&self) -> f64 {
        self.b
    }

    /// Returns the alpha component.
    pub fn a(&self) -> f64 {
        self.a
    }
}

impl From<(f64, f64, f64, f64)> for Color {
    fn from(value: (f64, f64, f64, f64)) -> Self {
        Self::new(value.0, value.1, value.2, value.3)
    }
}

impl From<Color> for (f64, f64, f64, f64) {
    fn from(value: Color) -> Self {
        (value.r, value.g, value.b, value.a)
    }
}
