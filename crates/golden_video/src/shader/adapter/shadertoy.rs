use std::collections::HashMap;

use serde::Deserialize;

use crate::shader::stages::{FragmentInput, Stage};
use crate::shader::{CompileError, TEXTURE_INPUTS};

const MAX_PASSES: usize = 6;

#[derive(Deserialize)]
#[serde(untagged)]
enum Document {
    Wrapped {
        #[serde(rename = "Shader")]
        shader: Shader,
    },
    Direct(Shader),
    List([Shader; 1]),
}

#[derive(Deserialize)]
struct Shader {
    renderpass: Vec<Pass>,
}

#[derive(Deserialize)]
struct Pass {
    #[serde(default)]
    name: String,
    #[serde(rename = "type")]
    kind: String,
    code: String,
    #[serde(default)]
    inputs: Vec<Input>,
    #[serde(default)]
    outputs: Vec<Output>,
}

#[derive(Deserialize)]
struct Input {
    #[serde(alias = "type")]
    ctype: String,
    channel: u8,
    id: Option<Id>,
}

#[derive(Deserialize)]
struct Output {
    #[serde(default)]
    channel: u8,
    id: Id,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Id {
    Number(u64),
    String(String),
}

impl Id {
    fn key(&self) -> String {
        match self {
            Self::Number(number) => number.to_string(),
            Self::String(value) => value.clone(),
        }
    }
}

pub(in crate::shader) fn parse(source: &str) -> Result<Vec<(String, Stage)>, CompileError> {
    let document = serde_json::from_str::<Document>(source)
        .map_err(|error| invalid(format!("Invalid ShaderToy JSON: {error}")))?;
    let shader = match document {
        Document::Wrapped { shader } | Document::Direct(shader) | Document::List([shader]) => shader,
    };
    if shader.renderpass.len() > MAX_PASSES {
        return Err(invalid("Too many ShaderToy passes"));
    }
    let mut common = None;
    let mut image = None;
    let mut buffers = [const { None }; TEXTURE_INPUTS];
    for pass in shader.renderpass {
        let destination = match pass.kind.as_str() {
            "common" => &mut common,
            "image" => &mut image,
            "buffer" => &mut buffers[buffer_index(&pass.name)?],
            kind => return Err(invalid(format!("Unsupported ShaderToy pass type '{kind}'"))),
        };
        if destination.replace(pass).is_some() {
            return Err(invalid("Duplicate ShaderToy pass"));
        }
    }
    let image = image.ok_or_else(|| invalid("ShaderToy JSON requires an Image pass"))?;
    let mut buffer_ids = HashMap::new();
    let mut draws = Vec::with_capacity(MAX_PASSES - 1);
    for pass in buffers.into_iter().flatten() {
        let [output] = pass.outputs.as_slice() else {
            return Err(invalid("A ShaderToy Buffer pass requires one output"));
        };
        let id = output.id.key();
        if id.is_empty() || output.channel != 0 || buffer_ids.insert(id, draws.len() as u8).is_some() {
            return Err(invalid("Invalid or duplicate ShaderToy buffer output"));
        }
        draws.push(pass);
    }
    draws.push(image);
    let common = common.as_ref().map_or("", |pass| pass.code.as_str());
    draws
        .into_iter()
        .enumerate()
        .map(|(index, pass)| {
            let mut stage = Stage {
                float: pass.kind == "buffer",
                ..Stage::default()
            };
            let mut seen = [false; TEXTURE_INPUTS];
            for input in pass.inputs {
                let channel = usize::from(input.channel);
                if channel >= TEXTURE_INPUTS || std::mem::replace(&mut seen[channel], true) {
                    return Err(invalid("Invalid or duplicate ShaderToy input channel"));
                }
                stage.inputs[channel] = Some(match input.ctype.as_str() {
                    "buffer" => {
                        let writer = input
                            .id
                            .as_ref()
                            .and_then(|id| buffer_ids.get(&id.key()))
                            .copied()
                            .ok_or_else(|| invalid("ShaderToy input refers to a missing Buffer pass"))?;
                        if usize::from(writer) < index {
                            FragmentInput::EarlierPass(writer)
                        } else {
                            FragmentInput::PreviousFrame(writer)
                        }
                    }
                    "texture" | "video" | "webcam" => FragmentInput::Media(input.channel),
                    kind => return Err(invalid(format!("Unsupported ShaderToy input type '{kind}'"))),
                });
            }
            Ok((format!("{common}\n{}", pass.code), stage))
        })
        .collect()
}

fn buffer_index(name: &str) -> Result<usize, CompileError> {
    ["Buffer A", "Buffer B", "Buffer C", "Buffer D"]
        .iter()
        .position(|candidate| *candidate == name)
        .ok_or_else(|| invalid("A ShaderToy buffer must be named Buffer A, B, C or D"))
}

fn invalid(message: impl Into<String>) -> CompileError {
    CompileError::Invalid(message.into())
}
