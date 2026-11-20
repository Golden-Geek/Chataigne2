use pp_rs::{
    pp::Preprocessor,
    token::{Punct, Token, TokenValue},
};

use super::{CompileError, FragmentUniform, UniformValue, comma_groups, ident, invalid, punct};

pub(super) fn zero(ty: &str) -> Option<UniformValue> {
    Some(match ty {
        "float" => UniformValue::Float(0.0),
        "int" => UniformValue::Int(0),
        "bool" => UniformValue::Bool(false),
        "vec2" => UniformValue::Point([0.0; 2]),
        "vec3" => UniformValue::Vector3([0.0; 3]),
        "vec4" => UniformValue::Color([0.0; 4]),
        _ => return None,
    })
}

pub(super) fn literal(ty: &str, zero: UniformValue, tokens: &[Token]) -> Option<UniformValue> {
    match zero {
        UniformValue::Float(_) => number(tokens)
            .map(|n| UniformValue::Float(n as f32))
            .filter(|v| v.finite()),
        UniformValue::Int(_) => {
            let number = number(tokens)?;
            (number.fract() == 0.0 && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&number))
                .then_some(UniformValue::Int(number as i32))
        }
        UniformValue::Bool(_) => match tokens {
            [word] if ident(word) == Some("true") => Some(UniformValue::Bool(true)),
            [word] if ident(word) == Some("false") => Some(UniformValue::Bool(false)),
            _ => None,
        },
        UniformValue::Point(_) => vector(ty, tokens).map(UniformValue::Point),
        UniformValue::Vector3(_) => vector(ty, tokens).map(UniformValue::Vector3),
        UniformValue::Color(_) => vector(ty, tokens).map(UniformValue::Color),
    }
}

fn number(tokens: &[Token]) -> Option<f64> {
    let (sign, token) = match tokens {
        [sign, token] if punct(sign, Punct::Minus) => (-1.0, token),
        [sign, token] if punct(sign, Punct::Plus) => (1.0, token),
        [token] => (1.0, token),
        _ => return None,
    };
    Some(
        sign * match &token.value {
            TokenValue::Integer(number) => number.value as f64,
            TokenValue::Float(number) => f64::from(number.value),
            _ => return None,
        },
    )
}

fn vector<const N: usize>(ty: &str, tokens: &[Token]) -> Option<[f32; N]> {
    let (constructor, tokens) = match tokens {
        [name, open, fields @ .., close]
            if ident(name) == Some(ty) && punct(open, Punct::LeftParen) && punct(close, Punct::RightParen) =>
        {
            (true, fields)
        }
        _ => (false, tokens),
    };
    let groups = comma_groups(tokens);
    let mut values = [0.0; N];
    if constructor && groups.len() == 1 {
        values.fill(number(groups[0])? as f32);
    } else if groups.len() == N {
        for (out, tokens) in values.iter_mut().zip(groups) {
            *out = number(tokens)? as f32;
        }
    } else {
        return None;
    }
    values.into_iter().all(f32::is_finite).then_some(values)
}

pub(super) fn annotation(
    source: &str,
    after: usize,
    ty: &str,
    uniform: &mut FragmentUniform,
) -> Result<(), CompileError> {
    let trailing = source[after..].lines().next().unwrap_or("").trim_start();
    let Some(comment) = trailing.strip_prefix("//").map(str::trim) else {
        return Ok(());
    };
    let (default, bounds) = comment.split_once('[').map_or((comment, None), |(default, bounds)| {
        (default.trim_end().trim_end_matches(',').trim_end(), Some(bounds))
    });
    let Some(value) = tokens(default).and_then(|tokens| literal(ty, uniform.default, &tokens)) else {
        return Ok(()); // Plain explanatory comments do not declare a slider.
    };
    uniform.default = value;
    let Some(bounds) = bounds else {
        return Ok(());
    };
    let error = || invalid(format!("Invalid annotated range for uniform '{}'", uniform.name));
    let bounds = bounds.strip_suffix(']').and_then(tokens).ok_or_else(error)?;
    let groups = comma_groups(&bounds);
    let [min, max] = groups.as_slice() else {
        return Err(error());
    };
    let min = number(min).filter(|number| number.is_finite()).ok_or_else(error)?;
    let max = number(max).filter(|number| number.is_finite()).ok_or_else(error)?;
    if min > max {
        return Err(error());
    }
    uniform.min = Some(bound(value, min).ok_or_else(error)?);
    uniform.max = Some(bound(value, max).ok_or_else(error)?);
    Ok(())
}

fn tokens(source: &str) -> Option<Vec<Token>> {
    Preprocessor::new(source).collect::<Result<_, _>>().ok()
}

fn bound(prototype: UniformValue, value: f64) -> Option<UniformValue> {
    let float = value as f32;
    let bound = match prototype {
        UniformValue::Float(_) => UniformValue::Float(float),
        UniformValue::Int(_)
            if value.fract() == 0.0 && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&value) =>
        {
            UniformValue::Int(value as i32)
        }
        UniformValue::Point(_) => UniformValue::Point([float; 2]),
        UniformValue::Vector3(_) => UniformValue::Vector3([float; 3]),
        UniformValue::Color(_) => UniformValue::Color([float; 4]),
        _ => return None,
    };
    bound.finite().then_some(bound)
}
