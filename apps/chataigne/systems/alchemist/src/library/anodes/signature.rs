use super::*;

pub(super) fn for_kind(kind: PrimitiveNodeKind, ctx: &SignatureCtx<'_>, instance: &ANodeInstance) -> ANodeSignature {
    match kind {
        PrimitiveNodeKind::Constant => constant_signature(instance),
        PrimitiveNodeKind::Property => property_signature(ctx, instance),
        PrimitiveNodeKind::Math => {
            generic_numbered_numeric_signature("value", "Value", input_count(instance, 2), "result")
        }
        PrimitiveNodeKind::Sum
        | PrimitiveNodeKind::Product
        | PrimitiveNodeKind::Minimum
        | PrimitiveNodeKind::Maximum
        | PrimitiveNodeKind::Difference => {
            generic_numbered_numeric_signature("value", "Value", input_count(instance, 2), "result")
        }
        PrimitiveNodeKind::Average => ANodeSignature {
            inputs: numbered_inputs("value", "Value", input_count(instance, 2), exact("float")),
            outputs: vec![OutputSocketDecl::new("result", "Result", exact("float"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::ConvertTuple => {
            let count = input_count(instance, 2);
            let target = convert_scalar::ScalarTarget::from_config(instance);
            ANodeSignature {
                inputs: numbered_inputs(
                    "value",
                    "Value",
                    count,
                    TypeConstraint::OneOf(vec![exact("int"), exact("float"), exact("bool"), exact("string")]),
                ),
                outputs: (1..=count)
                    .map(|index| {
                        OutputSocketDecl::new(
                            format!("result{index}"),
                            format!("Result {index}"),
                            exact(target.type_name()),
                        )
                    })
                    .collect(),
                ..ANodeSignature::default()
            }
        }
        PrimitiveNodeKind::Distance => float_signature(&["value1", "value2"], "result"),
        PrimitiveNodeKind::Function => function_signature(instance),
        PrimitiveNodeKind::Remap => {
            let mut signature = float_signature(&["value", "in_min", "in_max", "out_min", "out_max"], "result");
            for input in &mut signature.inputs {
                if matches!(input.id.as_str(), "in_max" | "out_max") {
                    input.default_value = Some(RuntimeValue::Float(1.0));
                }
            }
            signature
        }
        PrimitiveNodeKind::CurveRemap => float_signature(&["value"], "result"),
        PrimitiveNodeKind::Clamp => generic_numeric_signature(&["value", "minimum", "maximum"], "result"),
        PrimitiveNodeKind::SmoothFilter | PrimitiveNodeKind::Speed => float_signature(&["value"], "result"),
        PrimitiveNodeKind::OneMinus | PrimitiveNodeKind::Inverse | PrimitiveNodeKind::Negate => {
            generic_numeric_signature(&["value"], "result")
        }
        PrimitiveNodeKind::Counter => ANodeSignature {
            inputs: vec![
                InputSocketDecl::new("add", "Add", exact("trigger"))
                    .with_default(RuntimeValue::Trigger(TriggerValue::default())),
                InputSocketDecl::new("amount", "Amount", exact("float")).with_default(RuntimeValue::Float(1.0)),
                InputSocketDecl::new("reset", "Reset", exact("trigger"))
                    .with_default(RuntimeValue::Trigger(TriggerValue::default())),
            ],
            outputs: vec![OutputSocketDecl::new("count", "Count", exact("float"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::Lfo => ANodeSignature {
            inputs: Vec::new(),
            outputs: vec![OutputSocketDecl::new("value", "Value", exact("float"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::NoiseGenerator => ANodeSignature {
            inputs: vec![
                InputSocketDecl::new("position", "Position", exact("float")).with_default(RuntimeValue::Float(0.0)),
            ],
            outputs: vec![OutputSocketDecl::new("value", "Value", exact("float"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::Metronome => ANodeSignature {
            outputs: vec![
                OutputSocketDecl::new("tick", "Tick", exact("trigger")),
                OutputSocketDecl::new("on", "On", exact("bool")),
            ],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::CoordinateSystem => ANodeSignature {
            inputs: vec![InputSocketDecl::new("value", "Value", exact("vec2"))],
            outputs: vec![OutputSocketDecl::new("result", "Result", exact("vec2"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::AngleConversion => float_signature(&["value"], "result"),
        PrimitiveNodeKind::GradientSampler => ANodeSignature {
            inputs: vec![
                InputSocketDecl::new("position", "Position", exact("float")).with_default(RuntimeValue::Float(0.0)),
            ],
            outputs: vec![OutputSocketDecl::new("color", "Color", exact("color"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::ConvertToColor => convert_to_color_signature(instance),
        PrimitiveNodeKind::ConvertCompound => ANodeSignature {
            inputs: vec![InputSocketDecl::new(
                "value",
                "Value",
                TypeConstraint::OneOf(vec![exact("vec2"), exact("vec3"), exact("color")]),
            )],
            outputs: vec![OutputSocketDecl::new(
                "result",
                "Result",
                exact(convert_compound::CompoundTarget::from_config(instance).type_name()),
            )],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::ConvertToInt | PrimitiveNodeKind::ConvertToFloat | PrimitiveNodeKind::ConvertToBool => {
            ANodeSignature {
                inputs: vec![InputSocketDecl::new(
                    "value",
                    "Value",
                    TypeConstraint::OneOf(vec![exact("int"), exact("float"), exact("bool"), exact("string")]),
                )],
                outputs: vec![OutputSocketDecl::new(
                    "result",
                    "Result",
                    exact(match kind {
                        PrimitiveNodeKind::ConvertToInt => "int",
                        PrimitiveNodeKind::ConvertToFloat => "float",
                        PrimitiveNodeKind::ConvertToBool => "bool",
                        _ => unreachable!(),
                    }),
                )],
                ..ANodeSignature::default()
            }
        }
        PrimitiveNodeKind::ExtractColor => extract_color_signature(instance),
        PrimitiveNodeKind::ExtractVec2 => ANodeSignature {
            inputs: vec![InputSocketDecl::new("value", "Value", exact("vec2"))],
            outputs: vec![
                OutputSocketDecl::new("x", "X", exact("float")),
                OutputSocketDecl::new("y", "Y", exact("float")),
            ],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::ExtractVec3 => ANodeSignature {
            inputs: vec![InputSocketDecl::new("value", "Value", exact("vec3"))],
            outputs: vec![
                OutputSocketDecl::new("x", "X", exact("float")),
                OutputSocketDecl::new("y", "Y", exact("float")),
                OutputSocketDecl::new("z", "Z", exact("float")),
            ],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::PackVec3 => ANodeSignature {
            inputs: vec![
                InputSocketDecl::new("x", "X", exact("float")),
                InputSocketDecl::new("y", "Y", exact("float")),
                InputSocketDecl::new("z", "Z", exact("float")),
            ],
            outputs: vec![OutputSocketDecl::new("value", "Value", exact("vec3"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::PackVec2 => ANodeSignature {
            inputs: vec![
                InputSocketDecl::new("x", "X", exact("float")),
                InputSocketDecl::new("y", "Y", exact("float")),
            ],
            outputs: vec![OutputSocketDecl::new("value", "Value", exact("vec2"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::Concatenate => ANodeSignature {
            inputs: numbered_inputs("part", "Part", input_count(instance, 2), exact("string")),
            outputs: vec![OutputSocketDecl::new("result", "Result", exact("string"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::ConvertToString => ANodeSignature {
            inputs: vec![InputSocketDecl::new("value", "Value", TypeConstraint::Any)],
            outputs: vec![OutputSocketDecl::new("result", "Result", exact("string"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::Split => ANodeSignature {
            inputs: vec![InputSocketDecl::new("value", "Value", exact("string"))],
            outputs: vec![OutputSocketDecl::new("values", "Values", exact("value_array"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::BooleanOperation => ANodeSignature {
            inputs: vec![
                InputSocketDecl::new("a", "A", exact("bool")),
                InputSocketDecl::new("b", "B", exact("bool")),
            ],
            outputs: vec![OutputSocketDecl::new("result", "Result", exact("bool"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::Compare => compare_signature(),
        PrimitiveNodeKind::Threshold => ANodeSignature {
            inputs: vec![
                InputSocketDecl::new("value", "Value", exact("float")),
                InputSocketDecl::new("threshold", "Threshold", exact("float")).with_default(RuntimeValue::Float(0.5)),
                InputSocketDecl::new("hysteresis", "Hysteresis", exact("float")).with_default(RuntimeValue::Float(0.0)),
            ],
            outputs: vec![OutputSocketDecl::new("result", "Result", exact("bool"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::ConditionGate => condition_gate::signature(),
        PrimitiveNodeKind::TriggerOnOff => ANodeSignature {
            inputs: vec![InputSocketDecl::new("value", "Value", exact("bool"))],
            outputs: vec![
                OutputSocketDecl::new("on", "On", exact("trigger")),
                OutputSocketDecl::new("off", "Off", exact("trigger")),
            ],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::Gate => ANodeSignature {
            inputs: vec![
                InputSocketDecl::new("trigger", "Trigger", exact("trigger")),
                InputSocketDecl::new("open", "Open", exact("bool")).with_default(RuntimeValue::Bool(true)),
            ],
            outputs: vec![OutputSocketDecl::new("trigger", "Trigger", exact("trigger"))],
            ..ANodeSignature::default()
        },
        PrimitiveNodeKind::DelayOneTick | PrimitiveNodeKind::TimedDelay => passthrough_signature(),
        PrimitiveNodeKind::DebugValue => passthrough_signature(),
        PrimitiveNodeKind::DebugLog => ANodeSignature {
            inputs: vec![InputSocketDecl::new("value", "Value", TypeConstraint::Any)],
            outputs: Vec::new(),
            ..ANodeSignature::default()
        },
    }
}
