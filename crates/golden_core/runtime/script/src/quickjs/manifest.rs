use super::*;

pub(super) fn parse_manifest_from_json(
    payload: &JsonValue,
    export_names: Vec<String>,
) -> Result<ScriptManifest, ScriptRuntimeError> {
    let Some(root) = payload.as_object() else {
        return Err(ScriptRuntimeError::InvalidManifest(
            "manifest JSON root must be an object".to_string(),
        ));
    };

    let api_version = json_object_get(root, &["apiVersion"])
        .and_then(JsonValue::as_u64)
        .unwrap_or(1) as u32;
    if api_version == 0 {
        return Err(ScriptRuntimeError::InvalidManifest(
            "apiVersion must be >= 1".to_string(),
        ));
    }

    let update_rate_hz = json_object_get(root, &["updateRateHz"])
        .and_then(JsonValue::as_u64)
        .map(|value| value as u32);
    let parameters = parse_parameter_specs_json(json_object_get(root, &["parameters"]))?;
    let subscriptions = parse_subscription_specs_json(json_object_get(root, &["subscriptions"]))?;
    let exports = export_names
        .into_iter()
        .map(|name| ScriptExportSpec {
            name,
            signature: ScriptFnSignature::default(),
        })
        .collect();

    Ok(ScriptManifest {
        api_version,
        update_rate_hz,
        parameters,
        subscriptions,
        exports,
    })
}

pub(super) fn json_object_get<'a>(
    object: &'a serde_json::Map<String, JsonValue>,
    keys: &[&str],
) -> Option<&'a JsonValue> {
    for key in keys {
        if let Some(value) = object.get(*key) {
            return Some(value);
        }
    }
    None
}

pub(super) fn parse_subscription_specs_json(
    value: Option<&JsonValue>,
) -> Result<Vec<ScriptSubscriptionSpec>, ScriptRuntimeError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };

    let Some(items) = value.as_array() else {
        return Err(ScriptRuntimeError::InvalidManifest(
            "subscriptions must be an array".to_string(),
        ));
    };

    let mut specs = Vec::new();
    for item in items {
        let Some(entry) = item.as_object() else {
            return Err(ScriptRuntimeError::InvalidManifest(
                "subscription entry must be an object".to_string(),
            ));
        };

        let selector_raw = entry.get("node").and_then(JsonValue::as_str).ok_or_else(|| {
            ScriptRuntimeError::InvalidManifest("subscription entry must define string field 'node'".to_string())
        })?;
        let max_depth = json_object_get(entry, &["maxDepth"])
            .and_then(JsonValue::as_u64)
            .unwrap_or(0) as u32;
        let selector = if selector_raw == "@host" {
            ScriptNodeSelector::HostPath(String::new())
        } else if selector_raw == "@root" {
            ScriptNodeSelector::RootPath(String::new())
        } else if let Some(path) = selector_raw.strip_prefix("@host/") {
            ScriptNodeSelector::HostPath(path.to_string())
        } else if let Some(path) = selector_raw.strip_prefix("@root/") {
            ScriptNodeSelector::RootPath(path.to_string())
        } else {
            ScriptNodeSelector::Path(selector_raw.to_string())
        };

        specs.push(ScriptSubscriptionSpec {
            node: selector,
            max_depth,
        });
    }

    Ok(specs)
}

pub(super) fn parse_parameter_specs_json(
    value: Option<&JsonValue>,
) -> Result<Vec<ScriptParameterSpec>, ScriptRuntimeError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };

    let Some(parameters) = value.as_object() else {
        return Err(ScriptRuntimeError::InvalidManifest(
            "parameters must be an object map".to_string(),
        ));
    };

    let mut specs = Vec::new();
    for (name, raw_entry) in parameters {
        let Some(entry) = raw_entry.as_object() else {
            return Err(ScriptRuntimeError::InvalidManifest(format!(
                "parameter '{name}' must be an object"
            )));
        };

        let value_type_label = entry.get("type").and_then(JsonValue::as_str).unwrap_or("float");
        let value_type = ScriptValueType::from_manifest_label(value_type_label).ok_or_else(|| {
            ScriptRuntimeError::InvalidManifest(format!("parameter '{name}' has unsupported type '{value_type_label}'"))
        })?;

        let default_value = match entry.get("default") {
            Some(raw) => parameter_default_from_json_value(value_type, raw)?,
            None => default_param_value(value_type),
        };

        let mut constraints = ParameterConstraints {
            step: json_object_get(entry, &["step"]).and_then(JsonValue::as_f64),
            step_base: json_object_get(entry, &["stepBase"]).and_then(JsonValue::as_f64),
            ..ParameterConstraints::default()
        };
        if let Some(policy_label) = entry.get("policy").and_then(JsonValue::as_str) {
            constraints.policy = match policy_label.trim().to_ascii_lowercase().as_str() {
                "clampadapt" | "clamp_adapt" | "clamp-adapt" => ParameterConstraintPolicy::ClampAdapt,
                "reject" => ParameterConstraintPolicy::Reject,
                _ => {
                    return Err(ScriptRuntimeError::InvalidManifest(format!(
                        "parameter '{name}' has unsupported policy '{policy_label}'"
                    )));
                }
            };
        }

        constraints.range = parse_range_constraint_json(value_type, entry.get("min"), entry.get("max"))?;

        if let Some(enum_options) = json_object_get(entry, &["enumOptions"]) {
            let Some(options) = enum_options.as_array() else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{name}' enum_options must be an array"
                )));
            };

            let mut mapped = Vec::new();
            for option in options {
                let Some(variant_id) = option.as_str() else {
                    return Err(ScriptRuntimeError::InvalidManifest(format!(
                        "parameter '{name}' enum_options entries must be strings"
                    )));
                };
                mapped.push(ParameterEnumOption {
                    variant_id: variant_id.to_string(),
                    value: ParamValue::Enum(variant_id.to_string()),
                    label: variant_id.to_string(),
                    tags: Vec::new(),
                    ordering: None,
                });
            }
            constraints.enum_options = mapped;
        }
        constraints.file = parse_file_constraints_json(entry, name)?;

        let ui_hints = ParameterUiHints {
            widget: entry.get("widget").and_then(JsonValue::as_str).map(ToString::to_string),
            unit: entry.get("unit").and_then(JsonValue::as_str).map(ToString::to_string),
        };

        let decl_id = json_object_get(entry, &["declId"])
            .and_then(JsonValue::as_str)
            .unwrap_or(name);
        let label = entry.get("label").and_then(JsonValue::as_str).map(ToString::to_string);
        let read_only = json_object_get(entry, &["readOnly"])
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);

        specs.push(ScriptParameterSpec {
            name: name.to_string(),
            decl_id: DeclId(decl_id.to_string()),
            label,
            value_type,
            default_value,
            read_only,
            constraints,
            ui_hints,
        });
    }

    specs.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(specs)
}

pub(super) fn parse_file_constraints_json(
    entry: &serde_json::Map<String, JsonValue>,
    param_name: &str,
) -> Result<FileConstraints, ScriptRuntimeError> {
    let mut constraints = FileConstraints::default();

    if let Some(allowed_types) = json_object_get(entry, &["allowedTypes"]) {
        let Some(values) = allowed_types.as_array() else {
            return Err(ScriptRuntimeError::InvalidManifest(format!(
                "parameter '{param_name}' allowed_types must be an array"
            )));
        };
        let mut parsed = Vec::new();
        for value in values {
            let Some(value) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' allowed_types entries must be strings"
                )));
            };
            let Some(group) = FileTypeGroup::from_label(value) else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' has unknown file group '{value}' in allowed_types"
                )));
            };
            parsed.push(group);
        }
        constraints.allowed_types = parsed;
    }

    if let Some(allowed_extensions) = json_object_get(entry, &["allowedExtensions"]) {
        let Some(values) = allowed_extensions.as_array() else {
            return Err(ScriptRuntimeError::InvalidManifest(format!(
                "parameter '{param_name}' allowed_extensions must be an array"
            )));
        };
        let mut parsed = Vec::new();
        for value in values {
            let Some(value) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' allowed_extensions entries must be strings"
                )));
            };
            let Some(extension) = FileConstraints::normalize_extension_label(value) else {
                return Err(ScriptRuntimeError::InvalidManifest(format!(
                    "parameter '{param_name}' has invalid extension '{value}' in allowed_extensions"
                )));
            };
            parsed.push(extension);
        }
        constraints.allowed_extensions = parsed;
    }

    Ok(constraints)
}

pub(super) fn parse_range_constraint_json(
    value_type: ScriptValueType,
    min: Option<&JsonValue>,
    max: Option<&JsonValue>,
) -> Result<Option<RangeConstraint>, ScriptRuntimeError> {
    match value_type {
        ScriptValueType::Int
        | ScriptValueType::Float
        | ScriptValueType::Enum
        | ScriptValueType::Bool
        | ScriptValueType::Str
        | ScriptValueType::File
        | ScriptValueType::Trigger
        | ScriptValueType::Reference
        | ScriptValueType::CssValue => {
            let min = min.and_then(json_as_f64);
            let max = max.and_then(json_as_f64);
            Ok(RangeConstraint::uniform(min, max))
        }
        ScriptValueType::Vec2 | ScriptValueType::Vec3 | ScriptValueType::Color => {
            let min = min.and_then(json_as_f64_vec);
            let max = max.and_then(json_as_f64_vec);
            Ok(RangeConstraint::components(min, max))
        }
    }
}

pub(super) fn parameter_default_from_json_value(
    value_type: ScriptValueType,
    value: &JsonValue,
) -> Result<ParamValue, ScriptRuntimeError> {
    let parsed = match value_type {
        ScriptValueType::Trigger => ParamValue::Trigger(),
        ScriptValueType::Int => {
            let Some(raw) = value.as_i64().or_else(|| value.as_f64().map(|value| value as i64)) else {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "expected numeric default for int parameter".to_string(),
                ));
            };
            ParamValue::Int(
                i32::try_from(raw).map_err(|_| {
                    ScriptRuntimeError::InvalidManifest(format!("int default {raw} is outside i32 range"))
                })?,
            )
        }
        ScriptValueType::Float => {
            let Some(raw) = value.as_f64().or_else(|| value.as_i64().map(|value| value as f64)) else {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "expected numeric default for float parameter".to_string(),
                ));
            };
            ParamValue::Float(raw)
        }
        ScriptValueType::Str => {
            let Some(raw) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "expected string default for str parameter".to_string(),
                ));
            };
            ParamValue::Str(raw.to_string())
        }
        ScriptValueType::File => {
            let Some(raw) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "expected string default for file parameter".to_string(),
                ));
            };
            ParamValue::File(raw.to_string())
        }
        ScriptValueType::Enum => {
            let Some(raw) = value.as_str() else {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "expected string default for enum parameter".to_string(),
                ));
            };
            ParamValue::Enum(raw.to_string())
        }
        ScriptValueType::Bool => {
            let Some(raw) = value.as_bool() else {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "expected boolean default for bool parameter".to_string(),
                ));
            };
            ParamValue::Bool(raw)
        }
        ScriptValueType::CssValue => {
            let parsed = if let Some(raw) = value.as_str() {
                CssValue::parse_with_default_unit(raw, Some(CssUnit::Rem))
                    .ok_or_else(|| ScriptRuntimeError::InvalidManifest(format!("invalid css_value default '{raw}'")))?
            } else if let Some(raw) = value.as_f64().or_else(|| value.as_i64().map(|raw| raw as f64)) {
                CssValue::new(raw, CssUnit::Rem)
            } else {
                serde_json::from_value::<CssValue>(value.clone()).map_err(|error| {
                    ScriptRuntimeError::InvalidManifest(format!(
                        "expected css_value default as string, number, or object: {error}"
                    ))
                })?
            };
            ParamValue::CssValue(parsed)
        }
        ScriptValueType::Vec2 => {
            let Some(raw) = value.as_array() else {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "expected [x,y] array default for vec2 parameter".to_string(),
                ));
            };
            if raw.len() != 2 {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "vec2 default must have exactly 2 components".to_string(),
                ));
            }
            ParamValue::Vec2(
                json_as_f64(&raw[0])
                    .ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec2[0] must be numeric".to_string()))?,
                json_as_f64(&raw[1])
                    .ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec2[1] must be numeric".to_string()))?,
            )
        }
        ScriptValueType::Vec3 => {
            let Some(raw) = value.as_array() else {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "expected [x,y,z] array default for vec3 parameter".to_string(),
                ));
            };
            if raw.len() != 3 {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "vec3 default must have exactly 3 components".to_string(),
                ));
            }
            ParamValue::Vec3(
                json_as_f64(&raw[0])
                    .ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec3[0] must be numeric".to_string()))?,
                json_as_f64(&raw[1])
                    .ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec3[1] must be numeric".to_string()))?,
                json_as_f64(&raw[2])
                    .ok_or_else(|| ScriptRuntimeError::InvalidManifest("vec3[2] must be numeric".to_string()))?,
            )
        }
        ScriptValueType::Color => {
            let Some(raw) = value.as_array() else {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "expected [r,g,b,a] array default for color parameter".to_string(),
                ));
            };
            if raw.len() < 3 || raw.len() > 4 {
                return Err(ScriptRuntimeError::InvalidManifest(
                    "color default must have 3 or 4 components".to_string(),
                ));
            }
            let r = json_as_f64(&raw[0])
                .ok_or_else(|| ScriptRuntimeError::InvalidManifest("color[0] must be numeric".to_string()))?;
            let g = json_as_f64(&raw[1])
                .ok_or_else(|| ScriptRuntimeError::InvalidManifest("color[1] must be numeric".to_string()))?;
            let b = json_as_f64(&raw[2])
                .ok_or_else(|| ScriptRuntimeError::InvalidManifest("color[2] must be numeric".to_string()))?;
            let a = raw.get(3).and_then(json_as_f64).unwrap_or(1.0);
            ParamValue::Color(r, g, b, a)
        }
        ScriptValueType::Reference => ParamValue::Reference(Default::default()),
    };
    Ok(parsed)
}

pub(super) fn json_as_f64(value: &JsonValue) -> Option<f64> {
    value.as_f64().or_else(|| value.as_i64().map(|value| value as f64))
}

pub(super) fn json_as_f64_vec(value: &JsonValue) -> Option<Vec<f64>> {
    let values = value.as_array()?;
    let mut out = Vec::with_capacity(values.len());
    for item in values {
        out.push(json_as_f64(item)?);
    }
    Some(out)
}

pub(super) fn default_param_value(value_type: ScriptValueType) -> ParamValue {
    match value_type {
        ScriptValueType::Trigger => ParamValue::Trigger(),
        ScriptValueType::Int => ParamValue::Int(0),
        ScriptValueType::Float => ParamValue::Float(0.0),
        ScriptValueType::Str => ParamValue::Str(String::new()),
        ScriptValueType::File => ParamValue::File(String::new()),
        ScriptValueType::Enum => ParamValue::Enum(String::new()),
        ScriptValueType::Bool => ParamValue::Bool(false),
        ScriptValueType::CssValue => ParamValue::CssValue(CssValue::default()),
        ScriptValueType::Vec2 => ParamValue::Vec2(0.0, 0.0),
        ScriptValueType::Vec3 => ParamValue::Vec3(0.0, 0.0, 0.0),
        ScriptValueType::Color => ParamValue::Color(0.0, 0.0, 0.0, 1.0),
        ScriptValueType::Reference => ParamValue::Reference(Default::default()),
    }
}
