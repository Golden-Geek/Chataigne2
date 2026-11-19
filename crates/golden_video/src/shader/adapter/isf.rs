use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

use crate::shader::CompileError;

const MAX_INPUTS: usize = 64;

#[derive(Deserialize)]
struct Header {
    #[serde(rename = "INPUTS", default)]
    inputs: Vec<Input>,
    #[serde(rename = "PASSES", default)]
    passes: Vec<BTreeMap<String, Value>>,
    #[serde(rename = "IMPORTED", default)]
    imported: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct Input {
    #[serde(rename = "NAME")]
    name: String,
    #[serde(rename = "TYPE")]
    kind: InputType,
    #[serde(rename = "DEFAULT")]
    default: Option<Value>,
}

#[derive(Deserialize)]
enum InputType {
    #[serde(rename = "event")]
    Event,
    #[serde(rename = "bool")]
    Bool,
    #[serde(rename = "long")]
    Long,
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "point2D")]
    Point,
    #[serde(rename = "color")]
    Color,
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "audioFFT")]
    AudioFft,
}

pub(super) fn single_pass(source: &str) -> Result<(&str, String), CompileError> {
    let (json, body) = source
        .strip_prefix("/*")
        .and_then(|comment| comment.split_once("*/"))
        .ok_or_else(|| invalid("ISF source must begin with its JSON metadata comment"))?;
    let header =
        serde_json::from_str::<Header>(json).map_err(|error| invalid(format!("Invalid ISF metadata: {error}")))?;
    if header.passes.len() > 1 || header.passes.iter().any(|pass| !pass.is_empty()) {
        return Err(invalid("ISF pass targets require multipass playback"));
    }
    if !header.imported.is_empty() {
        return Err(invalid("ISF imported images require texture inputs"));
    }
    if header.inputs.len() > MAX_INPUTS {
        return Err(invalid("Too many ISF inputs"));
    }

    let mut names = HashSet::new();
    let mut declarations = String::new();
    for input in header.inputs {
        let name = input.name.as_str();
        let mut bytes = name.bytes();
        if !bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            || name.starts_with("gl_")
            || !names.insert(input.name.clone())
        {
            return Err(invalid("Invalid or duplicate ISF input name"));
        }
        let (ty, literal) = input.literal()?;
        writeln!(&mut declarations, "const {ty} {name} = {literal};").unwrap();
    }
    Ok((body, declarations))
}

impl Input {
    fn literal(&self) -> Result<(&'static str, String), CompileError> {
        Ok(match self.kind {
            InputType::Event => ("bool", "false".into()),
            InputType::Bool => {
                let default = match &self.default {
                    None => false,
                    Some(Value::Bool(boolean)) => *boolean,
                    Some(Value::Number(number)) if number.as_i64() == Some(0) => false,
                    Some(Value::Number(number)) if number.as_i64() == Some(1) => true,
                    _ => return Err(self.invalid_default()),
                };
                ("bool", default.to_string())
            }
            InputType::Long => {
                let value = self.default_value(0_i32)?;
                let literal = if value == i32::MIN {
                    "(-2147483647 - 1)".into()
                } else {
                    value.to_string()
                };
                ("int", literal)
            }
            InputType::Float => ("float", self.float(self.default_value(0.0_f32)?)?),
            InputType::Point => {
                let [x, y] = self.default_value([0.0_f32; 2])?;
                ("vec2", format!("vec2({}, {})", self.float(x)?, self.float(y)?))
            }
            InputType::Color => {
                let [r, g, b, a] = self.default_value([0.0_f32, 0.0, 0.0, 1.0])?;
                (
                    "vec4",
                    format!(
                        "vec4({}, {}, {}, {})",
                        self.float(r)?,
                        self.float(g)?,
                        self.float(b)?,
                        self.float(a)?
                    ),
                )
            }
            InputType::Image | InputType::Audio | InputType::AudioFft => {
                return Err(invalid(format!("ISF input '{}' requires a texture binding", self.name)));
            }
        })
    }

    fn default_value<T: DeserializeOwned>(&self, fallback: T) -> Result<T, CompileError> {
        self.default.as_ref().map_or(Ok(fallback), |value| {
            T::deserialize(value).map_err(|_| self.invalid_default())
        })
    }

    fn float(&self, value: f32) -> Result<String, CompileError> {
        if value.is_finite() {
            Ok(format!("{value:e}"))
        } else {
            Err(self.invalid_default())
        }
    }

    fn invalid_default(&self) -> CompileError {
        invalid(format!("Invalid default for ISF input '{}'", self.name))
    }
}

fn invalid(message: impl Into<String>) -> CompileError {
    CompileError::Invalid(message.into())
}
