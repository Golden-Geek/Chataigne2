use crate::{ANodeInstance, RuntimeValue};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ColorMode {
    Rgba,
    Hsva,
    Hsla,
    Cmyk,
}

impl ColorMode {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        let mode = match instance.config.get("mode") {
            Some(RuntimeValue::String(value)) => value.as_ref(),
            _ => "rgba",
        };
        match mode {
            "hsva" => Self::Hsva,
            "hsla" => Self::Hsla,
            "cmyka" | "cmyk" => Self::Cmyk,
            _ => Self::Rgba,
        }
    }
}
