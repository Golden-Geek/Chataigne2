//! Extract authored uniforms from active top-level GLSL declarations.

use std::{borrow::Cow, collections::HashMap, ops::Range};

use pp_rs::{
    pp::Preprocessor,
    token::{Punct, Token, TokenValue},
};

use super::super::{CompileError, FragmentFormat, FragmentUniform, MAX_SOURCE_BYTES, UniformValue};

mod values;

const MAX_UNIFORMS: usize = 64;
const MAX_TOKENS: usize = 2 * MAX_SOURCE_BYTES;

pub(super) struct Declarations {
    ranges: Vec<Range<usize>>,
    pub uniforms: Vec<FragmentUniform>,
}

impl Declarations {
    pub fn body<'a>(&self, source: &'a str) -> Cow<'a, str> {
        if self.ranges.is_empty() {
            return Cow::Borrowed(source);
        }
        let mut bytes = source.as_bytes().to_vec();
        for range in &self.ranges {
            for byte in &mut bytes[range.clone()] {
                if !matches!(*byte, b'\n' | b'\r') {
                    *byte = b' ';
                }
            }
        }
        Cow::Owned(String::from_utf8(bytes).expect("source ranges start and end at ASCII token boundaries"))
    }
}

pub(super) fn parse(format: FragmentFormat, source: &str) -> Result<Declarations, CompileError> {
    let mut declarations = Declarations {
        ranges: Vec::new(),
        uniforms: Vec::new(),
    };
    // A procedural shader with no explicit storage qualifiers needs no declaration pass.
    if !source.contains("uniform") {
        return Ok(declarations);
    }
    let mut seen = HashMap::new();
    let mut depth = 0usize;
    let mut statement = Vec::new();
    for (index, token) in Preprocessor::new(source).enumerate() {
        if index == MAX_TOKENS {
            return Err(invalid("GLSL uniform preprocessing exceeds the token limit"));
        }
        let token = token.map_err(|(error, location)| {
            invalid(format!(
                "GLSL preprocessing failed at byte {}: {error:?}",
                location.start
            ))
        })?;
        match token.value {
            TokenValue::Punct(Punct::LeftBrace) => {
                depth += 1;
                statement.clear();
            }
            TokenValue::Punct(Punct::RightBrace) => {
                depth = depth.saturating_sub(1);
            }
            TokenValue::Punct(Punct::Semicolon) if depth == 0 => {
                if let Some((range, uniforms)) = declaration(format, source, &statement, &token)? {
                    declarations.ranges.push(range);
                    for uniform in uniforms {
                        if seen.contains_key(&uniform.name) {
                            return Err(invalid(format!("Duplicate uniform '{}'", uniform.name)));
                        }
                        insert_uniform(&mut seen, &mut declarations.uniforms, &uniform)?;
                    }
                }
                statement.clear();
            }
            TokenValue::Version(_) | TokenValue::Extension(_) | TokenValue::Pragma(_) => {}
            _ if depth == 0 => statement.push(token),
            _ => {}
        }
    }
    Ok(declarations)
}

type UniformStatement = (Range<usize>, Vec<FragmentUniform>);

fn declaration(
    format: FragmentFormat,
    source: &str,
    tokens: &[Token],
    end: &Token,
) -> Result<Option<UniformStatement>, CompileError> {
    let Some(storage) = tokens.iter().position(|token| ident(token) == Some("uniform")) else {
        return Ok(None);
    };
    let first = &tokens[0];
    let keyword = &tokens[storage];
    let start = first.location.start as usize;
    let stop = end.location.end as usize;
    // Macro-generated declarations do not have a contiguous authored span to remove.
    let keyword_position = keyword.location.start as usize;
    if start > keyword_position
        || keyword.location.end > end.location.start
        || source.get(keyword_position..keyword.location.end as usize) != Some("uniform")
        || source[..start]
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .trim_start()
            .starts_with('#')
        || source.get(end.location.start as usize..stop) != Some(";")
        || (storage != 0 && (ident(first) != Some("layout") || !punct(&tokens[storage - 1], Punct::RightParen)))
    {
        return Ok(None);
    }
    let mut suffix = &tokens[storage + 1..];
    if suffix
        .first()
        .and_then(ident)
        .is_some_and(|word| matches!(word, "lowp" | "mediump" | "highp"))
    {
        suffix = &suffix[1..];
    }
    let Some((ty, names)) = suffix.split_first().and_then(|(ty, names)| Some((ident(ty)?, names))) else {
        return Ok(None);
    };
    let groups = comma_groups(names);
    let mut uniforms = Vec::new();
    for group in &groups {
        let Some((name, rest)) = group.split_first().and_then(|(name, rest)| Some((ident(name)?, rest))) else {
            return Ok(None);
        };
        if let Some((host_type, array_len)) = host_uniform(format, name) {
            if ty != host_type || !has_extent(rest, array_len) {
                return Err(invalid(format!("Invalid declaration of host uniform '{name}'")));
            }
            continue;
        }
        let Some(zero) = values::zero(ty) else {
            // Preserve unsupported resource declarations for the compiler's ordinary validation.
            return Ok(None);
        };
        if !name.is_ascii() || name.len() > 128 || name.starts_with("gl_") {
            return Err(invalid(format!("Invalid GLSL uniform name '{name}'")));
        }
        let mut uniform = FragmentUniform {
            name: name.into(),
            label: None,
            default: zero,
            min: None,
            max: None,
        };
        match rest {
            [] => {}
            [equal, initializer @ ..] if punct(equal, Punct::Equal) => {
                uniform.default = values::literal(ty, zero, initializer)
                    .ok_or_else(|| invalid(format!("Unsupported initializer for uniform '{name}'")))?;
            }
            _ => return Ok(None),
        }
        if groups.len() == 1 {
            values::annotation(source, stop, ty, &mut uniform)?;
        }
        uniforms.push(uniform);
    }
    Ok(Some((start..stop, uniforms)))
}

fn host_uniform(format: FragmentFormat, name: &str) -> Option<(&'static str, Option<u64>)> {
    match (format, name) {
        (_, "iResolution") => Some(("vec3", None)),
        (_, "iTime") | (FragmentFormat::ShaderToy, "iTimeDelta" | "iFrameRate") => Some(("float", None)),
        (FragmentFormat::ShaderToy, "iFrame") => Some(("int", None)),
        (FragmentFormat::ShaderToy, "iChannelResolution") => Some(("vec3", Some(4))),
        (_, "iChannel0" | "iChannel1" | "iChannel2" | "iChannel3") => Some(("sampler2D", None)),
        _ => None,
    }
}

fn has_extent(tokens: &[Token], extent: Option<u64>) -> bool {
    match (tokens, extent) {
        ([], None) => true,
        ([open, size, close], Some(extent)) => {
            punct(open, Punct::LeftBracket)
                && punct(close, Punct::RightBracket)
                && matches!(&size.value, TokenValue::Integer(size) if size.value == extent)
        }
        _ => false,
    }
}

pub(super) fn merge_uniforms<'a>(
    fields: impl IntoIterator<Item = &'a FragmentUniform>,
) -> Result<Vec<FragmentUniform>, CompileError> {
    let mut seen = HashMap::new();
    let mut uniforms = Vec::new();
    for field in fields {
        insert_uniform(&mut seen, &mut uniforms, field)?;
    }
    Ok(uniforms)
}

fn insert_uniform(
    seen: &mut HashMap<String, usize>,
    uniforms: &mut Vec<FragmentUniform>,
    field: &FragmentUniform,
) -> Result<(), CompileError> {
    if let Some(&index) = seen.get(&field.name) {
        if &uniforms[index] != field {
            return Err(invalid(format!(
                "Uniform '{}' has conflicting ShaderToy declarations",
                field.name
            )));
        }
        return Ok(());
    }
    if uniforms.len() == MAX_UNIFORMS {
        return Err(invalid("Too many shader uniforms"));
    }
    seen.insert(field.name.clone(), uniforms.len());
    uniforms.push(field.clone());
    Ok(())
}

fn comma_groups(tokens: &[Token]) -> Vec<&[Token]> {
    let mut depth = 0usize;
    tokens
        .split(|token| match token.value {
            TokenValue::Punct(Punct::LeftParen) => {
                depth += 1;
                false
            }
            TokenValue::Punct(Punct::RightParen) => {
                depth = depth.saturating_sub(1);
                false
            }
            TokenValue::Punct(Punct::Comma) => depth == 0,
            _ => false,
        })
        .collect()
}

fn ident(token: &Token) -> Option<&str> {
    match &token.value {
        TokenValue::Ident(word) => Some(word),
        _ => None,
    }
}

fn punct(token: &Token, mark: Punct) -> bool {
    token.value == TokenValue::Punct(mark)
}

fn invalid(message: impl Into<String>) -> CompileError {
    CompileError::Invalid(message.into())
}
