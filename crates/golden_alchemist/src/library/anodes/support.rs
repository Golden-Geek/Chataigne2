use std::{sync::Arc, time::Duration};

use crate::{
    ANodeConfigFieldDecl, ANodeInstance, ANodeSignature, ColorValue, FormulaPropertyId, InputSocketDecl,
    OutputSocketDecl, RuntimeValue, SignatureCtx, TriggerValue, TypeBindingSource, TypeBindings, TypeConstraint,
    TypeVar, ValueTypeId,
};

pub(super) fn enum_config(id: &str, label: &str, default: &str, options: &[(&str, &str)]) -> ANodeConfigFieldDecl {
    ANodeConfigFieldDecl::new(id, label, RuntimeValue::String(Arc::from(default)))
        .with_editor("enum")
        .with_enum_options(options.iter().copied())
}

pub(super) fn smooth_method_config() -> ANodeConfigFieldDecl {
    enum_config(
        "method",
        "Method",
        "one_euro",
        &[
            ("one_euro", "One Euro"),
            ("sma", "SMA"),
            ("damping", "Damping"),
            ("savitzky_golay", "Savitzky-Golay"),
            ("median", "Median"),
        ],
    )
}

pub(super) fn string_format_config() -> ANodeConfigFieldDecl {
    enum_config(
        "format",
        "Format",
        "decimal",
        &[
            ("decimal", "Decimal"),
            ("hexadecimal", "Hexadecimal"),
            ("time", "Time"),
            ("compact", "Compact"),
        ],
    )
}

pub(super) fn lfo_shape_config() -> ANodeConfigFieldDecl {
    enum_config(
        "shape",
        "Shape",
        "sine",
        &[
            ("sine", "Sine"),
            ("triangle", "Triangle"),
            ("saw", "Saw"),
            ("square", "Square"),
            ("pulse", "Pulse"),
        ],
    )
}

pub(super) fn noise_algorithm_config() -> ANodeConfigFieldDecl {
    enum_config(
        "algorithm",
        "Algorithm",
        "random",
        &[
            ("random", "Random"),
            ("perlin", "Perlin"),
            ("simplex", "Simplex"),
            ("brownian", "Brownian"),
            ("cellular", "Cellular"),
            ("fractal", "Fractal"),
        ],
    )
}

pub(super) fn noise_config_fields(algorithm: &str) -> Vec<ANodeConfigFieldDecl> {
    let mut fields = vec![
        noise_algorithm_config(),
        ANodeConfigFieldDecl::new("seed", "Seed", RuntimeValue::Int(0)),
        ANodeConfigFieldDecl::new("scale", "Scale", RuntimeValue::Float(1.0)),
    ];
    match algorithm {
        "brownian" | "fractal" => fields.extend([
            ANodeConfigFieldDecl::new("octaves", "Octaves", RuntimeValue::Int(4)),
            ANodeConfigFieldDecl::new("persistence", "Persistence", RuntimeValue::Float(0.5)),
            ANodeConfigFieldDecl::new("lacunarity", "Lacunarity", RuntimeValue::Float(2.0)),
        ]),
        "cellular" => fields.push(ANodeConfigFieldDecl::new("jitter", "Jitter", RuntimeValue::Float(1.0))),
        _ => {}
    }
    fields
}

pub(super) fn metronome_mode_config() -> ANodeConfigFieldDecl {
    enum_config(
        "mode",
        "Mode",
        "bpm",
        &[("frequency", "Frequency"), ("bpm", "BPM"), ("time", "Time")],
    )
}

pub(super) fn color_mode_config() -> ANodeConfigFieldDecl {
    enum_config(
        "mode",
        "Mode",
        "rgba",
        &[("rgba", "RGBA"), ("hsva", "HSVA"), ("hsla", "HSLA"), ("cmyka", "CMYK")],
    )
}

pub(super) fn optional_count_config(id: &str, label: &str, default: i64) -> ANodeConfigFieldDecl {
    ANodeConfigFieldDecl::new(id, label, RuntimeValue::Int(default))
        .with_description("Disable to grow automatically as new sockets are connected.")
        .with_editor("optional_count")
}

pub(super) fn exact(id: &str) -> TypeConstraint {
    TypeConstraint::Exact(ValueTypeId::new(id))
}

pub(super) fn constant_signature(instance: &ANodeInstance) -> ANodeSignature {
    let value_type = instance
        .config
        .get("value")
        .map_or_else(|| ValueTypeId::new("float"), RuntimeValue::value_type);
    ANodeSignature {
        inputs: Vec::new(),
        outputs: vec![OutputSocketDecl::new(
            "value",
            "Value",
            TypeConstraint::Exact(value_type),
        )],
        ..ANodeSignature::default()
    }
}

pub(super) fn property_signature(ctx: &SignatureCtx<'_>, instance: &ANodeInstance) -> ANodeSignature {
    let value_type = property_id_from_config(instance)
        .and_then(|id| ctx.properties.and_then(|schema| schema.get(&id)))
        .map_or_else(
            || ValueTypeId::new("unit"),
            |declaration| declaration.value_type.clone(),
        );
    ANodeSignature {
        inputs: Vec::new(),
        outputs: vec![OutputSocketDecl::new(
            "value",
            "Value",
            TypeConstraint::Exact(value_type),
        )],
        ..ANodeSignature::default()
    }
}

pub(super) fn property_id_from_config(instance: &ANodeInstance) -> Option<FormulaPropertyId> {
    let RuntimeValue::Ref(value) = instance.config.get("property_id")? else {
        return None;
    };
    (!value.stable_id.is_empty()).then(|| FormulaPropertyId::new(value.stable_id.as_ref()))
}

pub(super) fn function_signature(instance: &ANodeInstance) -> ANodeSignature {
    let inputs = if super::function::FunctionKind::from_config(instance) == super::function::FunctionKind::Atan2 {
        vec![
            InputSocketDecl::new("y", "Y", exact("float")),
            InputSocketDecl::new("x", "X", exact("float")),
        ]
    } else {
        vec![InputSocketDecl::new("value", "Value", exact("float"))]
    };
    ANodeSignature {
        inputs,
        outputs: vec![OutputSocketDecl::new("result", "Result", exact("float"))],
        ..ANodeSignature::default()
    }
}

pub(super) fn generic_numeric_signature(inputs: &[&str], output: &str) -> ANodeSignature {
    let variable = TypeVar::new("TNumeric");
    let mut default_bindings = TypeBindings::default();
    let mut generic_constraints = indexmap::IndexMap::new();
    default_bindings.insert(variable.clone(), ValueTypeId::new("float"), TypeBindingSource::Default);
    generic_constraints.insert(variable.clone(), TypeConstraint::NumericLike);
    ANodeSignature {
        inputs: inputs
            .iter()
            .map(|id| InputSocketDecl::new(*id, title(id), TypeConstraint::Generic(variable.clone())))
            .collect(),
        outputs: vec![OutputSocketDecl::new(
            output,
            title(output),
            TypeConstraint::Generic(variable),
        )],
        default_bindings,
        generic_constraints,
    }
}

pub(super) fn generic_numbered_numeric_signature(
    prefix: &str,
    label: &str,
    count: usize,
    output: &str,
) -> ANodeSignature {
    let variable = TypeVar::new("TNumeric");
    let mut default_bindings = TypeBindings::default();
    let mut generic_constraints = indexmap::IndexMap::new();
    default_bindings.insert(variable.clone(), ValueTypeId::new("float"), TypeBindingSource::Default);
    generic_constraints.insert(variable.clone(), TypeConstraint::NumericLike);
    ANodeSignature {
        inputs: (1..=count)
            .map(|index| {
                InputSocketDecl::new(
                    format!("{prefix}{index}"),
                    format!("{label} {index}"),
                    TypeConstraint::Generic(variable.clone()),
                )
            })
            .collect(),
        outputs: vec![OutputSocketDecl::new(
            output,
            title(output),
            TypeConstraint::Generic(variable),
        )],
        default_bindings,
        generic_constraints,
    }
}

pub(super) fn compare_signature() -> ANodeSignature {
    let variable = TypeVar::new("TValue");
    let mut default_bindings = TypeBindings::default();
    let mut generic_constraints = indexmap::IndexMap::new();
    default_bindings.insert(variable.clone(), ValueTypeId::new("float"), TypeBindingSource::Default);
    generic_constraints.insert(variable.clone(), TypeConstraint::Primitive);
    ANodeSignature {
        inputs: vec![
            InputSocketDecl::new("left", "Left", TypeConstraint::Generic(variable.clone())),
            InputSocketDecl::new("right", "Right", TypeConstraint::Generic(variable.clone())),
        ],
        outputs: vec![OutputSocketDecl::new("result", "Result", exact("bool"))],
        default_bindings,
        generic_constraints,
    }
}

pub(super) fn color_channel_specs(mode: super::color_mode::ColorMode) -> [(&'static str, &'static str, f64); 4] {
    match mode {
        super::color_mode::ColorMode::Rgba => [("r", "R", 0.0), ("g", "G", 0.0), ("b", "B", 0.0), ("a", "A", 1.0)],
        super::color_mode::ColorMode::Hsva => [("h", "H", 0.0), ("s", "S", 0.0), ("v", "V", 0.0), ("a", "A", 1.0)],
        super::color_mode::ColorMode::Hsla => [("h", "H", 0.0), ("s", "S", 0.0), ("l", "L", 0.0), ("a", "A", 1.0)],
        super::color_mode::ColorMode::Cmyk => [("c", "C", 0.0), ("m", "M", 0.0), ("y", "Y", 0.0), ("k", "K", 0.0)],
    }
}

pub(super) fn convert_to_color_signature(instance: &ANodeInstance) -> ANodeSignature {
    let mode = super::color_mode::ColorMode::from_config(instance);
    ANodeSignature {
        inputs: color_channel_specs(mode)
            .into_iter()
            .map(|(id, label, default)| {
                InputSocketDecl::new(id, label, exact("float")).with_default(RuntimeValue::Float(default))
            })
            .collect(),
        outputs: vec![OutputSocketDecl::new("color", "Color", exact("color"))],
        ..ANodeSignature::default()
    }
}

pub(super) fn extract_color_signature(instance: &ANodeInstance) -> ANodeSignature {
    let mode = super::color_mode::ColorMode::from_config(instance);
    ANodeSignature {
        inputs: vec![InputSocketDecl::new("color", "Color", exact("color"))],
        outputs: color_channel_specs(mode)
            .into_iter()
            .map(|(id, label, _)| OutputSocketDecl::new(id, label, exact("float")))
            .collect(),
        ..ANodeSignature::default()
    }
}

pub(super) fn float_signature(inputs: &[&str], output: &str) -> ANodeSignature {
    ANodeSignature {
        inputs: inputs
            .iter()
            .map(|id| InputSocketDecl::new(*id, title(id), exact("float")))
            .collect(),
        outputs: vec![OutputSocketDecl::new(output, title(output), exact("float"))],
        ..ANodeSignature::default()
    }
}

pub(super) fn numbered_inputs(
    prefix: &str,
    label: &str,
    count: usize,
    constraint: TypeConstraint,
) -> Vec<InputSocketDecl> {
    (1..=count)
        .map(|index| {
            InputSocketDecl::new(
                format!("{prefix}{index}"),
                format!("{label} {index}"),
                constraint.clone(),
            )
        })
        .collect()
}

pub(super) fn passthrough_signature() -> ANodeSignature {
    let variable = TypeVar::new("TValue");
    let mut default_bindings = TypeBindings::default();
    default_bindings.insert(variable.clone(), ValueTypeId::new("float"), TypeBindingSource::Default);
    ANodeSignature {
        inputs: vec![InputSocketDecl::new(
            "value",
            "Value",
            TypeConstraint::Generic(variable.clone()),
        )],
        outputs: vec![OutputSocketDecl::new(
            "value",
            "Value",
            TypeConstraint::Generic(variable),
        )],
        default_bindings,
        ..ANodeSignature::default()
    }
}

pub(super) fn value_type_config_field(variable: &str) -> ANodeConfigFieldDecl {
    value_type_config_field_with_constraint(variable, TypeConstraint::NumericLike)
}

pub(super) fn value_type_config_field_with_constraint(
    variable: &str,
    constraint: TypeConstraint,
) -> ANodeConfigFieldDecl {
    ANodeConfigFieldDecl::new("value_type", "Value Type", RuntimeValue::String("float".into()))
        .with_description("Optional fixed value type for this node. Disable to infer from inputs.")
        .with_editor("value_type")
        .with_type_variable(variable)
        .with_type_options(match constraint {
            TypeConstraint::NumericLike => vec![
                ValueTypeId::new("int"),
                ValueTypeId::new("float"),
                ValueTypeId::new("vec2"),
                ValueTypeId::new("vec3"),
                ValueTypeId::new("color"),
            ],
            TypeConstraint::Primitive => vec![
                ValueTypeId::new("unit"),
                ValueTypeId::new("bool"),
                ValueTypeId::new("trigger"),
                ValueTypeId::new("int"),
                ValueTypeId::new("float"),
                ValueTypeId::new("string"),
                ValueTypeId::new("vec2"),
                ValueTypeId::new("vec3"),
                ValueTypeId::new("color"),
                ValueTypeId::new("duration"),
                ValueTypeId::new("value_array"),
            ],
            _ => Vec::new(),
        })
}

pub(super) fn title(id: &str) -> String {
    id.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn config_string(instance: &ANodeInstance, field: &str, fallback: &str) -> String {
    match instance.config.get(field) {
        Some(RuntimeValue::String(value)) => value.to_string(),
        _ => fallback.to_owned(),
    }
}

pub(super) fn config_int(instance: &ANodeInstance, field: &str, fallback: i64) -> i64 {
    match instance.config.get(field) {
        Some(RuntimeValue::Int(value)) => *value,
        Some(RuntimeValue::Float(value)) => *value as i64,
        _ => fallback,
    }
}

pub(super) fn config_float(instance: &ANodeInstance, field: &str, fallback: f64) -> f64 {
    match instance.config.get(field) {
        Some(RuntimeValue::Float(value)) => *value,
        Some(RuntimeValue::Int(value)) => *value as f64,
        _ => fallback,
    }
}

pub(super) fn config_bool(instance: &ANodeInstance, field: &str, fallback: bool) -> bool {
    match instance.config.get(field) {
        Some(RuntimeValue::Bool(value)) => *value,
        _ => fallback,
    }
}

pub(super) fn input_count(instance: &ANodeInstance, fallback: usize) -> usize {
    config_int(instance, "num_inputs", fallback as i64).clamp(1, 64) as usize
}

pub(super) fn require_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[&RuntimeValue; N], String> {
    inputs
        .iter()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| format!("node expects {N} input(s)"))
}

pub(super) fn bool_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[bool; N], String> {
    require_inputs::<N>(inputs)?
        .map(runtime_bool)
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "invalid boolean input count".into())
}

pub(super) fn float_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[f64; N], String> {
    require_inputs::<N>(inputs)?
        .map(|value| Ok(value_to_f64(value)))
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "invalid numeric input count".into())
}

pub(super) fn runtime_bool(value: &RuntimeValue) -> Result<bool, String> {
    match value {
        RuntimeValue::Bool(value) => Ok(*value),
        RuntimeValue::Trigger(value) => Ok(value.fired),
        _ => Err("node expects boolean inputs".into()),
    }
}

pub(super) fn trigger_fired(value: &RuntimeValue) -> Result<bool, String> {
    match value {
        RuntimeValue::Trigger(value) => Ok(value.fired),
        RuntimeValue::Bool(value) => Ok(*value),
        _ => Err("node expects trigger inputs".into()),
    }
}

pub(super) fn hash_u64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub(super) fn hash_noise(seed: i64, index: i64) -> f64 {
    let combined = (seed as u64)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(index as u64);
    let mantissa = hash_u64(combined) >> 11;
    let unit = mantissa as f64 * (1.0 / ((1_u64 << 53) as f64));
    unit * 2.0 - 1.0
}

pub(super) fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

pub(super) fn lerp(left: f64, right: f64, amount: f64) -> f64 {
    left + (right - left) * amount
}

pub(super) fn gradient_noise_1d(seed: i64, x: f64) -> f64 {
    let base = x.floor() as i64;
    let local = x - base as f64;
    let weight = smoothstep(local);
    let left = hash_noise(seed, base) * local;
    let right = hash_noise(seed, base + 1) * (local - 1.0);
    (lerp(left, right, weight) * 2.0).clamp(-1.0, 1.0)
}

pub(super) fn fractal_noise_1d(seed: i64, x: f64, octaves: usize, persistence: f64, lacunarity: f64) -> f64 {
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut sum = 0.0;
    let mut amplitude_sum = 0.0;
    for octave in 0..octaves {
        sum += gradient_noise_1d(seed + octave as i64 * 101, x * frequency) * amplitude;
        amplitude_sum += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    if amplitude_sum <= f64::EPSILON {
        0.0
    } else {
        (sum / amplitude_sum).clamp(-1.0, 1.0)
    }
}

pub(super) fn cellular_noise_1d(seed: i64, x: f64, jitter: f64) -> f64 {
    let base = x.floor() as i64;
    let mut nearest = f64::INFINITY;
    for cell in (base - 1)..=(base + 1) {
        let random = hash_noise(seed, cell) * 0.5 + 0.5;
        let feature = cell as f64 + random * jitter;
        nearest = nearest.min((x - feature).abs());
    }
    (1.0 - nearest.clamp(0.0, 1.0) * 2.0).clamp(-1.0, 1.0)
}

pub(super) fn randomized_period(period: f64, randomness: f64, seed: i64) -> f64 {
    let factor = 1.0 + hash_noise(seed, 0) * randomness;
    (period * factor.max(0.001)).max(0.001)
}

pub(super) fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

pub(super) fn hue_sector(hue_degrees: f64, chroma: f64) -> (f64, f64, f64) {
    let hue = hue_degrees.rem_euclid(360.0) / 60.0;
    let x = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    match hue.floor() as i32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    }
}

pub(super) fn hsva_to_rgba(hue: f64, saturation: f64, value: f64, alpha: f64) -> ColorValue {
    let saturation = clamp01(saturation);
    let value = clamp01(value);
    let chroma = value * saturation;
    let (red, green, blue) = hue_sector(hue, chroma);
    let m = value - chroma;
    ColorValue {
        red: (red + m) as f32,
        green: (green + m) as f32,
        blue: (blue + m) as f32,
        alpha: clamp01(alpha) as f32,
    }
}

pub(super) fn hsla_to_rgba(hue: f64, saturation: f64, lightness: f64, alpha: f64) -> ColorValue {
    let saturation = clamp01(saturation);
    let lightness = clamp01(lightness);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let (red, green, blue) = hue_sector(hue, chroma);
    let m = lightness - chroma * 0.5;
    ColorValue {
        red: (red + m) as f32,
        green: (green + m) as f32,
        blue: (blue + m) as f32,
        alpha: clamp01(alpha) as f32,
    }
}

pub(super) fn cmyk_to_rgba(cyan: f64, magenta: f64, yellow: f64, key: f64) -> ColorValue {
    let cyan = clamp01(cyan);
    let magenta = clamp01(magenta);
    let yellow = clamp01(yellow);
    let key = clamp01(key);
    ColorValue {
        red: ((1.0 - cyan) * (1.0 - key)) as f32,
        green: ((1.0 - magenta) * (1.0 - key)) as f32,
        blue: ((1.0 - yellow) * (1.0 - key)) as f32,
        alpha: 1.0,
    }
}

pub(super) fn color_hue(red: f64, green: f64, blue: f64, max: f64, delta: f64) -> f64 {
    if delta <= f64::EPSILON {
        0.0
    } else if (max - red).abs() <= f64::EPSILON {
        60.0 * ((green - blue) / delta).rem_euclid(6.0)
    } else if (max - green).abs() <= f64::EPSILON {
        60.0 * (((blue - red) / delta) + 2.0)
    } else {
        60.0 * (((red - green) / delta) + 4.0)
    }
}

pub(super) fn rgba_to_hsva(color: ColorValue) -> [f64; 4] {
    let red = f64::from(color.red);
    let green = f64::from(color.green);
    let blue = f64::from(color.blue);
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let saturation = if max <= f64::EPSILON { 0.0 } else { delta / max };
    [
        color_hue(red, green, blue, max, delta),
        saturation,
        max,
        f64::from(color.alpha),
    ]
}

pub(super) fn rgba_to_hsla(color: ColorValue) -> [f64; 4] {
    let red = f64::from(color.red);
    let green = f64::from(color.green);
    let blue = f64::from(color.blue);
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let lightness = (max + min) * 0.5;
    let saturation = if delta <= f64::EPSILON {
        0.0
    } else {
        delta / (1.0 - (2.0 * lightness - 1.0).abs())
    };
    [
        color_hue(red, green, blue, max, delta),
        saturation,
        lightness,
        f64::from(color.alpha),
    ]
}

pub(super) fn rgba_to_cmyk(color: ColorValue) -> [f64; 4] {
    let red = f64::from(color.red);
    let green = f64::from(color.green);
    let blue = f64::from(color.blue);
    let key = 1.0 - red.max(green).max(blue);
    if key >= 1.0 - f64::EPSILON {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let denominator = 1.0 - key;
    [
        (1.0 - red - key) / denominator,
        (1.0 - green - key) / denominator,
        (1.0 - blue - key) / denominator,
        key,
    ]
}

pub(super) fn numeric_map(value: &RuntimeValue, mapper: impl Fn(f64) -> f64) -> RuntimeValue {
    numeric_map_checked(value, |value| Ok(mapper(value))).expect("infallible numeric map")
}

pub(super) fn numeric_map_checked(
    value: &RuntimeValue,
    mapper: impl Fn(f64) -> Result<f64, String>,
) -> Result<RuntimeValue, String> {
    Ok(match value {
        RuntimeValue::Int(value) => RuntimeValue::Int(mapper(*value as f64)? as i64),
        RuntimeValue::Float(value) => RuntimeValue::Float(mapper(*value)?),
        RuntimeValue::Vec2(value) => RuntimeValue::Vec2([mapper(value[0])?, mapper(value[1])?]),
        RuntimeValue::Vec3(value) => RuntimeValue::Vec3([mapper(value[0])?, mapper(value[1])?, mapper(value[2])?]),
        RuntimeValue::Color(value) => RuntimeValue::Color(ColorValue {
            red: mapper(f64::from(value.red))? as f32,
            green: mapper(f64::from(value.green))? as f32,
            blue: mapper(f64::from(value.blue))? as f32,
            alpha: mapper(f64::from(value.alpha))? as f32,
        }),
        _ => RuntimeValue::Float(mapper(value_to_f64(value))?),
    })
}

pub(super) fn value_to_f64(value: &RuntimeValue) -> f64 {
    match value {
        RuntimeValue::Unit => 0.0,
        RuntimeValue::Bool(value) => f64::from(*value),
        RuntimeValue::Trigger(value) => f64::from(value.fired),
        RuntimeValue::Int(value) => *value as f64,
        RuntimeValue::Float(value) => *value,
        RuntimeValue::String(value) => value.trim().parse::<f64>().unwrap_or(0.0),
        RuntimeValue::Vec2(value) => value[0],
        RuntimeValue::Vec3(value) => value[0],
        RuntimeValue::Color(value) => f64::from(value.red),
        RuntimeValue::Duration(value) => value.as_secs_f64(),
        RuntimeValue::Array(value) => value.first().map_or(0.0, value_to_f64),
        RuntimeValue::Ref(_) | RuntimeValue::Extension(_) => 0.0,
    }
}

pub(super) fn value_to_i64(value: &RuntimeValue) -> i64 {
    let value = value_to_f64(value);
    if value.is_finite() { value as i64 } else { 0 }
}

pub(super) fn decimal_string(value: &RuntimeValue, decimals: usize) -> String {
    match value {
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::Float(value) => format!("{value:.decimals$}"),
        _ => format_runtime_value(value, decimals),
    }
}

pub(super) fn format_runtime_value(value: &RuntimeValue, decimals: usize) -> String {
    match value {
        RuntimeValue::Unit => String::new(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Trigger(value) => value.fired.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::Float(value) => format!("{value:.decimals$}"),
        RuntimeValue::String(value) => value.to_string(),
        RuntimeValue::Vec2(value) => format!(
            "[{},{}]",
            format_float(value[0], decimals),
            format_float(value[1], decimals)
        ),
        RuntimeValue::Vec3(value) => format!(
            "[{},{},{}]",
            format_float(value[0], decimals),
            format_float(value[1], decimals),
            format_float(value[2], decimals)
        ),
        RuntimeValue::Color(value) => format!(
            "[{},{},{},{}]",
            format_float(f64::from(value.red), decimals),
            format_float(f64::from(value.green), decimals),
            format_float(f64::from(value.blue), decimals),
            format_float(f64::from(value.alpha), decimals)
        ),
        RuntimeValue::Duration(value) => time_string(value.as_secs_f64(), decimals),
        RuntimeValue::Array(values) => values
            .iter()
            .map(|value| format_runtime_value(value, decimals))
            .collect::<Vec<_>>()
            .join(","),
        RuntimeValue::Ref(value) => value.stable_id.to_string(),
        RuntimeValue::Extension(value) => value.payload.iter().map(|byte| format!("{byte:02x}")).collect(),
    }
}

pub(super) fn format_float(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}")
}

pub(super) fn time_string(seconds: f64, decimals: usize) -> String {
    let safe_seconds = seconds.max(0.0);
    let hours = (safe_seconds / 3600.0).floor() as u64;
    let minutes = ((safe_seconds % 3600.0) / 60.0).floor() as u64;
    let seconds = safe_seconds % 60.0;
    if decimals == 0 {
        format!("{hours:02}:{minutes:02}:{:02}", seconds.round() as u64)
    } else {
        let width = 3 + decimals;
        format!("{hours:02}:{minutes:02}:{seconds:0width$.decimals$}")
    }
}

pub(super) fn brightness(value: &RuntimeValue) -> f64 {
    match value {
        RuntimeValue::Color(value) => {
            0.2126 * f64::from(value.red) + 0.7152 * f64::from(value.green) + 0.0722 * f64::from(value.blue)
        }
        _ => value_to_f64(value).abs(),
    }
}

pub(super) fn delta_seconds(duration: Duration) -> f64 {
    duration.as_secs_f64().max(1.0 / 120.0)
}

pub(super) fn smoothing_alpha(cutoff: f64, dt: f64) -> f64 {
    let tau = 1.0 / (2.0 * std::f64::consts::PI * cutoff.max(0.0001));
    (dt / (tau + dt)).clamp(0.0, 1.0)
}

pub(super) fn state_values(state: Option<&RuntimeValue>, minimum_len: usize) -> Vec<f64> {
    let mut values = match state {
        Some(RuntimeValue::Array(values)) => values.iter().map(value_to_f64).collect(),
        Some(RuntimeValue::Vec2(value)) => value.to_vec(),
        Some(RuntimeValue::Vec3(value)) => value.to_vec(),
        Some(RuntimeValue::Float(value)) => vec![*value],
        Some(RuntimeValue::Int(value)) => vec![*value as f64],
        _ => Vec::new(),
    };
    values.resize(minimum_len, 0.0);
    values
}

pub(super) fn set_state_values(state: Option<&mut RuntimeValue>, values: Vec<f64>) {
    if let Some(state) = state {
        *state = RuntimeValue::Array(values.into_iter().map(RuntimeValue::Float).collect());
    }
}

pub(super) fn trim_history(history: &mut Vec<f64>, window: usize) {
    if history.len() > window {
        let remove_count = history.len() - window;
        history.drain(0..remove_count);
    }
}

pub(super) fn trigger(fired: bool, edge_id: u64, logical_tick: u64) -> TriggerValue {
    if fired {
        TriggerValue::fired(edge_id, logical_tick)
    } else {
        TriggerValue::default()
    }
}
