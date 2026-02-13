use serde::{Deserialize, Serialize};

/// RGBA color value with normalized channel components.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
}
