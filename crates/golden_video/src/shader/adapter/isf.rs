use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

use super::TEXTURE_NAMES;
use crate::shader::stages::{Dimension, FragmentInput, Stage};
use crate::shader::{CompileError, TEMPORARY_BUFFERS, TEXTURE_INPUTS};

const MAX_INPUTS: usize = 64;
const MAX_PASSES: usize = 8;

#[derive(Deserialize)]
struct Header {
    #[serde(rename = "INPUTS", default)]
    inputs: Vec<Input>,
    #[serde(rename = "PASSES", default)]
    passes: Vec<Pass>,
    #[serde(rename = "IMPORTED", default)]
    imported: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct Pass {
    #[serde(rename = "TARGET")]
    target: Option<String>,
    #[serde(rename = "WIDTH")]
    width: Option<Size>,
    #[serde(rename = "HEIGHT")]
    height: Option<Size>,
    #[serde(rename = "FLOAT")]
    float: Option<Flag>,
    #[serde(rename = "PERSISTENT")]
    persistent: Option<Flag>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Size {
    Equation(String),
    Pixels(f64),
}

impl Size {
    fn parse(&self, defaults: &BTreeMap<&str, f64>) -> Result<Dimension, CompileError> {
        match self {
            Self::Equation(equation) => Dimension::parse(equation, defaults),
            Self::Pixels(value) => Dimension::parse(&value.to_string(), defaults),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Flag {
    Bool(bool),
    Number(f64),
}

impl Flag {
    fn enabled(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            Self::Number(value) => *value > 0.0,
        }
    }
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

pub(super) fn source(source: &str) -> Result<(&str, String, Vec<Stage>), CompileError> {
    let (json, body) = source
        .strip_prefix("/*")
        .and_then(|comment| comment.split_once("*/"))
        .ok_or_else(|| invalid("ISF source must begin with its JSON metadata comment"))?;
    let header =
        serde_json::from_str::<Header>(json).map_err(|error| invalid(format!("Invalid ISF metadata: {error}")))?;
    if header.passes.len() > MAX_PASSES {
        return Err(invalid("Too many ISF passes"));
    }
    if !header.imported.is_empty() {
        return Err(invalid("ISF imported images require texture inputs"));
    }
    if header.inputs.len() > MAX_INPUTS {
        return Err(invalid("Too many ISF inputs"));
    }

    let mut names = HashSet::new();
    let mut images = TEXTURE_NAMES[..TEXTURE_INPUTS].iter();
    let mut declarations = String::new();
    let mut defaults = BTreeMap::new();
    for input in &header.inputs {
        let name = input.name.as_str();
        if !valid_name(name) || !names.insert(name) {
            return Err(invalid("Invalid or duplicate ISF input name"));
        }
        if matches!(input.kind, InputType::Image) {
            let texture = images.next().ok_or_else(|| invalid("Too many ISF image inputs"))?;
            writeln!(declarations, "#define {name} {texture}").unwrap();
        } else {
            let (ty, literal) = input.literal()?;
            writeln!(declarations, "const {ty} {name} = {literal};").unwrap();
            if let Some(default) = input.scalar_default() {
                defaults.insert(name, default);
            }
        }
    }
    let stages = if header.passes.is_empty() {
        vec![Stage::default()]
    } else {
        stages(&header.passes, &mut names, &defaults, &mut declarations)?
    };
    Ok((body, declarations, stages))
}

fn stages<'a>(
    passes: &'a [Pass],
    names: &mut HashSet<&'a str>,
    defaults: &BTreeMap<&str, f64>,
    declarations: &mut String,
) -> Result<Vec<Stage>, CompileError> {
    let mut slots = HashMap::new();
    let mut last_writer = [None; TEMPORARY_BUFFERS];
    let mut persistent = [false; TEMPORARY_BUFFERS];
    for (index, pass) in passes.iter().enumerate() {
        let persists = pass.persistent.as_ref().is_some_and(Flag::enabled);
        let Some(name) = pass.target.as_deref() else {
            if persists {
                return Err(invalid("A persistent ISF pass requires a TARGET"));
            }
            if index + 1 < passes.len() {
                return Err(invalid("Each intermediate ISF pass requires a TARGET"));
            }
            continue;
        };
        let slot = match slots.get(name) {
            Some(&slot) => slot,
            None => {
                if !valid_name(name) || !names.insert(name) {
                    return Err(invalid("Invalid or conflicting ISF pass target name"));
                }
                let slot = slots.len();
                let texture = TEXTURE_NAMES
                    .get(TEXTURE_INPUTS + slot)
                    .ok_or_else(|| invalid("Too many ISF temporary buffers"))?;
                writeln!(declarations, "#define {name} {texture}").unwrap();
                slots.insert(name, slot);
                slot
            }
        };
        last_writer[slot] = Some(index as u8);
        persistent[slot] |= persists;
    }
    let mut previous: [_; TEMPORARY_BUFFERS] = std::array::from_fn(|slot| {
        last_writer[slot]
            .filter(|_| persistent[slot])
            .map(FragmentInput::PreviousFrame)
    });
    passes
        .iter()
        .enumerate()
        .map(|(index, pass)| {
            let mut stage = Stage {
                size: [
                    pass.width.as_ref().map(|size| size.parse(defaults)).transpose()?,
                    pass.height.as_ref().map(|size| size.parse(defaults)).transpose()?,
                ],
                float: pass.float.as_ref().is_some_and(Flag::enabled),
                ..Stage::default()
            };
            stage.inputs[TEXTURE_INPUTS..].copy_from_slice(&previous);
            if let Some(name) = pass.target.as_deref() {
                previous[slots[name]] = Some(FragmentInput::EarlierPass(index as u8));
            }
            Ok(stage)
        })
        .collect()
}

fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !name.starts_with("gl_")
}

impl Input {
    fn scalar_default(&self) -> Option<f64> {
        match self.kind {
            InputType::Event => Some(0.0),
            InputType::Bool => Some(self.default.as_ref().map_or(0.0, |value| match value {
                Value::Bool(value) => f64::from(u8::from(*value)),
                _ => value.as_f64().unwrap_or(0.0),
            })),
            InputType::Long | InputType::Float => Some(self.default.as_ref().and_then(Value::as_f64).unwrap_or(0.0)),
            _ => None,
        }
    }

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
