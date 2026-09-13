use super::*;

pub(super) fn for_kind(kind: PrimitiveNodeKind) -> Vec<ANodeConfigFieldDecl> {
    match kind {
        PrimitiveNodeKind::Constant => vec![
            ANodeConfigFieldDecl::new("value", "Value", RuntimeValue::Float(0.0))
                .with_description("The constant value emitted by this node.")
                .with_editor("runtime_value"),
        ],
        PrimitiveNodeKind::Property => vec![
            ANodeConfigFieldDecl::new(
                "property_id",
                "Property",
                RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), "")),
            )
            .with_description("Referenced Formula property."),
        ],
        PrimitiveNodeKind::Math => vec![
            enum_config(
                "application",
                "Apply",
                "combine",
                &[("each", "Apply to each"), ("combine", "Combine inputs")],
            ),
            enum_config(
                "operator",
                "Operator",
                "add",
                &[
                    ("add", "Add"),
                    ("subtract", "Subtract"),
                    ("multiply", "Multiply"),
                    ("divide", "Divide"),
                    ("modulo", "Modulo"),
                ],
            ),
            optional_count_config("num_inputs", "Num Inputs", 2),
            value_type_config_field("TNumeric"),
        ],
        PrimitiveNodeKind::Sum
        | PrimitiveNodeKind::Product
        | PrimitiveNodeKind::Minimum
        | PrimitiveNodeKind::Maximum
        | PrimitiveNodeKind::Difference => vec![
            optional_count_config("num_inputs", "Num Inputs", 2),
            value_type_config_field("TNumeric"),
        ],
        PrimitiveNodeKind::Average => vec![optional_count_config("num_inputs", "Num Inputs", 2)],
        PrimitiveNodeKind::ConvertTuple => vec![
            enum_config(
                "target",
                "Target",
                "float",
                &[
                    ("float", "Float"),
                    ("int", "Integer"),
                    ("bool", "Boolean"),
                    ("string", "String"),
                ],
            ),
            optional_count_config("num_inputs", "Num Inputs", 2),
        ],
        PrimitiveNodeKind::ConvertCompound => vec![enum_config(
            "target",
            "Target",
            "vec3",
            &[("vec2", "Vec2"), ("vec3", "Vec3"), ("color", "Color")],
        )],
        PrimitiveNodeKind::Function => vec![enum_config(
            "function",
            "Function",
            "sqrt",
            &[
                ("sqrt", "Sqrt"),
                ("log", "Log"),
                ("log10", "Log10"),
                ("exp", "Exp"),
                ("abs", "Abs"),
                ("floor", "Floor"),
                ("ceil", "Ceil"),
                ("round", "Round"),
                ("sin", "Sin"),
                ("cos", "Cos"),
                ("tan", "Tan"),
                ("asin", "Asin"),
                ("acos", "Acos"),
                ("atan", "Atan"),
                ("atan2", "Atan2"),
            ],
        )],
        PrimitiveNodeKind::SmoothFilter => vec![smooth_method_config()],
        PrimitiveNodeKind::CurveRemap => vec![
            ANodeConfigFieldDecl::new("curve", "Curve", curve_remap::default_curve_config())
                .with_editor("curve")
                .with_update_class(crate::ManagedSettingClass::Resource)
                .with_description("Golden curve keys and easings used to remap the input."),
        ],
        PrimitiveNodeKind::OneMinus | PrimitiveNodeKind::Inverse | PrimitiveNodeKind::Negate => {
            vec![value_type_config_field("TNumeric")]
        }
        PrimitiveNodeKind::Speed => vec![
            ANodeConfigFieldDecl::new("window_seconds", "Window", RuntimeValue::Float(0.1))
                .with_description("Smoothing window in seconds for the speed estimate."),
        ],
        PrimitiveNodeKind::Lfo => vec![
            lfo_shape_config(),
            ANodeConfigFieldDecl::new("frequency", "Frequency", RuntimeValue::Float(1.0)),
            ANodeConfigFieldDecl::new("update_rate", "Update Rate", RuntimeValue::Float(60.0)),
            ANodeConfigFieldDecl::new("minimum", "Minimum", RuntimeValue::Float(0.0)),
            ANodeConfigFieldDecl::new("maximum", "Maximum", RuntimeValue::Float(1.0)),
        ],
        PrimitiveNodeKind::NoiseGenerator => noise_config_fields("random"),
        PrimitiveNodeKind::Metronome => vec![
            metronome_mode_config(),
            ANodeConfigFieldDecl::new("value", "Value", RuntimeValue::Float(120.0)),
            ANodeConfigFieldDecl::new("on_ratio", "On Ratio", RuntimeValue::Float(0.5)),
            ANodeConfigFieldDecl::new("randomness", "Randomness", RuntimeValue::Float(0.0)),
        ],
        PrimitiveNodeKind::CoordinateSystem => vec![enum_config(
            "mode",
            "Mode",
            "cartesian_to_polar",
            &[
                ("cartesian_to_polar", "Cartesian To Polar"),
                ("polar_to_cartesian", "Polar To Cartesian"),
            ],
        )],
        PrimitiveNodeKind::AngleConversion => vec![enum_config(
            "mode",
            "Mode",
            "degrees_to_radians",
            &[
                ("degrees_to_radians", "Degrees To Radians"),
                ("radians_to_degrees", "Radians To Degrees"),
            ],
        )],
        PrimitiveNodeKind::GradientSampler => vec![
            ANodeConfigFieldDecl::new("gradient", "Gradient", gradient_sampler::default_gradient_config())
                .with_editor("gradient")
                .with_update_class(crate::ManagedSettingClass::Resource)
                .with_description("Color stops (position, color, interpolation) edited with the gradient editor."),
        ],
        PrimitiveNodeKind::ConvertToColor | PrimitiveNodeKind::ExtractColor => {
            vec![color_mode_config()]
        }
        PrimitiveNodeKind::Concatenate => vec![
            optional_count_config("num_inputs", "Num Inputs", 2),
            ANodeConfigFieldDecl::new("prefix", "Prefix", RuntimeValue::String(Arc::from(""))),
            ANodeConfigFieldDecl::new("suffix", "Suffix", RuntimeValue::String(Arc::from(""))),
            ANodeConfigFieldDecl::new("separator", "Separator", RuntimeValue::String(Arc::from(""))),
        ],
        PrimitiveNodeKind::ConvertToString => vec![string_format_config()],
        PrimitiveNodeKind::Split => vec![
            ANodeConfigFieldDecl::new("separator", "Separator", RuntimeValue::String(Arc::from(","))),
            ANodeConfigFieldDecl::new("trim", "Trim", RuntimeValue::Bool(true)),
            ANodeConfigFieldDecl::new("omit_empty", "Omit Empty", RuntimeValue::Bool(false)),
        ],
        PrimitiveNodeKind::BooleanOperation => vec![enum_config(
            "operator",
            "Operator",
            "and",
            &[("and", "AND"), ("or", "OR"), ("xor", "XOR")],
        )],
        PrimitiveNodeKind::Compare => vec![
            enum_config(
                "comparator",
                "Comparator",
                "equal",
                &[
                    ("equal", "Equal"),
                    ("not_equal", "Not Equal"),
                    ("greater", "Greater"),
                    ("greater_or_equal", "Greater Or Equal"),
                    ("less", "Less"),
                    ("less_or_equal", "Less Or Equal"),
                    ("longer", "Longer"),
                    ("shorter", "Shorter"),
                    ("contains", "Contains"),
                    ("brighter", "Brighter"),
                    ("darker", "Darker"),
                ],
            ),
            value_type_config_field_with_constraint("TValue", TypeConstraint::Primitive),
        ],
        PrimitiveNodeKind::Threshold => vec![enum_config(
            "direction",
            "Direction",
            "above",
            &[("above", "Above"), ("below", "Below")],
        )],
        PrimitiveNodeKind::ConditionGate => vec![
            enum_config(
                "mode",
                "Mode",
                "pass_when_true",
                &[
                    ("pass_when_true", "Pass When True"),
                    ("pass_when_false", "Pass When False"),
                    ("hold_last", "Hold Last"),
                    ("hold_last_with_default", "Hold Last, Then Default"),
                    ("output_default", "Output Default"),
                    ("output_default_when_false", "Output Default When False"),
                    ("block_trigger", "Block Trigger"),
                    ("block_trigger_with_default", "Block Trigger With Default"),
                ],
            ),
            enum_config("gate_application", "Application", "whole", &[("whole", "Whole")]),
        ],
        PrimitiveNodeKind::TriggerOnOff => vec![
            ANodeConfigFieldDecl::new("toggle", "Toggle", RuntimeValue::Bool(false))
                .with_description("Alternate On and Off triggers on rising input edges."),
        ],
        PrimitiveNodeKind::TimedDelay => vec![
            ANodeConfigFieldDecl::new("seconds", "Delay (seconds)", RuntimeValue::Float(0.1)),
            ANodeConfigFieldDecl::new("capacity", "Queue capacity", RuntimeValue::Int(64)),
        ],
        _ => Vec::new(),
    }
}
