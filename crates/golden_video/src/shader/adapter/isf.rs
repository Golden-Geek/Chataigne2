use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

use super::TEXTURE_NAMES;
use crate::shader::stages::{Dimension, FragmentInput, ScalarInput, Stage};
use crate::shader::{CompileError, FragmentUniform, TEMPORARY_BUFFERS, TEXTURE_INPUTS, UniformValue};

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
    fn parse(&self, inputs: &BTreeMap<&str, ScalarInput>) -> Result<Dimension, CompileError> {
        match self {
            Self::Equation(equation) => Dimension::parse(equation, inputs),
            Self::Pixels(value) => Dimension::parse(&value.to_string(), inputs),
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
    #[serde(rename = "LABEL")]
    label: Option<String>,
    #[serde(rename = "MIN")]
    min: Option<Value>,
    #[serde(rename = "MAX")]
    max: Option<Value>,
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

pub(super) struct IsfSource<'a> {
    pub body: &'a str,
    pub aliases: String,
    pub stages: Vec<Stage>,
    pub uniforms: Vec<FragmentUniform>,
}

pub(super) fn source(source: &str) -> Result<IsfSource<'_>, CompileError> {
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
    let mut scalar_inputs = BTreeMap::new();
    let mut uniforms = Vec::new();
    for input in &header.inputs {
        let name = input.name.as_str();
        if !valid_name(name) || !names.insert(name) {
            return Err(invalid("Invalid or duplicate ISF input name"));
        }
        match input.kind {
            InputType::Image => {
                let texture = images.next().ok_or_else(|| invalid("Too many ISF image inputs"))?;
                writeln!(declarations, "#define {name} {texture}").unwrap();
            }
            InputType::Event => {
                writeln!(declarations, "const bool {name} = false;").unwrap();
                scalar_inputs.insert(name, ScalarInput::Constant(0.0));
            }
            InputType::Audio | InputType::AudioFft => {
                return Err(invalid(format!("ISF input '{name}' requires a texture binding")));
            }
            _ => {
                let uniform = input.uniform()?;
                if uniform.default.scalar().is_some() {
                    scalar_inputs.insert(name, ScalarInput::Uniform(uniforms.len()));
                }
                uniforms.push(uniform);
            }
        }
    }
    let stages = if header.passes.is_empty() {
        vec![Stage::default()]
    } else {
        stages(&header.passes, &mut names, &scalar_inputs, &mut declarations)?
    };
    Ok(IsfSource {
        body,
        aliases: declarations,
        stages,
        uniforms,
    })
}

fn stages<'a>(
    passes: &'a [Pass],
    names: &mut HashSet<&'a str>,
    scalar_inputs: &BTreeMap<&str, ScalarInput>,
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
                    pass.width.as_ref().map(|size| size.parse(scalar_inputs)).transpose()?,
                    pass.height.as_ref().map(|size| size.parse(scalar_inputs)).transpose()?,
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
    name.len() <= 128
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !name.starts_with("gl_")
}

impl Input {
    fn uniform(&self) -> Result<FragmentUniform, CompileError> {
        let default = self
            .value(self.default.as_ref())
            .map_err(|_| invalid(format!("Invalid default for ISF input '{}'", self.name)))?;
        let minimum = self.min.as_ref().map(|value| self.value(Some(value))).transpose()?;
        let maximum = self.max.as_ref().map(|value| self.value(Some(value))).transpose()?;
        Ok(FragmentUniform {
            name: self.name.clone(),
            label: self.label.as_ref().filter(|label| label.len() <= 128).cloned(),
            default,
            min: minimum,
            max: maximum,
        })
    }

    fn value(&self, raw: Option<&Value>) -> Result<UniformValue, CompileError> {
        let value = match self.kind {
            InputType::Bool => {
                let boolean = match raw {
                    None => false,
                    Some(Value::Bool(boolean)) => *boolean,
                    Some(Value::Number(number)) if number.as_i64() == Some(0) => false,
                    Some(Value::Number(number)) if number.as_i64() == Some(1) => true,
                    _ => return Err(self.invalid_value()),
                };
                UniformValue::Bool(boolean)
            }
            InputType::Long => UniformValue::Int(self.decode(raw, 0_i32)?),
            InputType::Float => UniformValue::Float(self.decode(raw, 0.0_f32)?),
            InputType::Point => UniformValue::Point(self.decode(raw, [0.0_f32; 2])?),
            InputType::Color => UniformValue::Color(self.decode(raw, [0.0_f32, 0.0, 0.0, 1.0])?),
            InputType::Image | InputType::Audio | InputType::AudioFft | InputType::Event => {
                unreachable!("non-value inputs have their own bindings")
            }
        };
        value.finite().then_some(value).ok_or_else(|| self.invalid_value())
    }

    fn decode<T: DeserializeOwned>(&self, raw: Option<&Value>, fallback: T) -> Result<T, CompileError> {
        raw.map_or(Ok(fallback), |value| {
            T::deserialize(value).map_err(|_| self.invalid_value())
        })
    }

    fn invalid_value(&self) -> CompileError {
        invalid(format!("Invalid value for ISF input '{}'", self.name))
    }
}

fn invalid(message: impl Into<String>) -> CompileError {
    CompileError::Invalid(message.into())
}
