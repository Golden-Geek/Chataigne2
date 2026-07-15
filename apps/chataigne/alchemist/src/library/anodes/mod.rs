use std::sync::Arc;

use crate::{
    ANodeConfigFieldDecl, ANodeDeclaration, ANodeInstance, ANodeRegistry, ANodeRoleCapability, ANodeSignature,
    ANodeTypeId, AutoWirePolicy, CompiledNodeOperation, Diagnostic, ExecutionKind, InputSocketDecl, ManagedUiMode,
    OutputSocketDecl, PipelineCardinality, RegistryError, ResolvedANodeSignature, RuntimeValue, SignatureCtx,
    StableRef, SurfaceItemKind, TriggerValue, TypeBindings, TypeConstraint, ValueTypeId,
};

mod angle_conversion;
mod boolean_operation;
mod color_mode;
mod compare;
mod concatenate;
mod condition_gate;
mod constant;
mod convert_to_color;
mod convert_to_string;
mod coordinate_system;
mod counter;
mod debug_log;
mod debug_value;
mod delay_one_tick;
mod extract_color;
mod function;
mod gate;
mod gradient_sampler;
mod inverse;
mod lfo;
mod math;
mod metronome;
mod negate;
mod noise_generator;
mod one_minus;
mod pack_vec3;
mod property;
mod remap;
mod smooth_filter;
mod speed;
mod split;
mod support;
mod trigger_on_off;

use support::{
    color_mode_config, compare_signature, config_bool, config_float, config_int, config_string, constant_signature,
    convert_to_color_signature, enum_config, exact, extract_color_signature, float_signature, function_signature,
    generic_numbered_numeric_signature, generic_numeric_signature, input_count, lfo_shape_config,
    metronome_mode_config, noise_config_fields, numbered_inputs, optional_count_config, passthrough_signature,
    property_signature, smooth_method_config, string_format_config, value_type_config_field,
    value_type_config_field_with_constraint,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveNodeKind {
    Constant,
    Property,
    Math,
    Function,
    Remap,
    Clamp,
    SmoothFilter,
    OneMinus,
    Inverse,
    Negate,
    Speed,
    Counter,
    Lfo,
    NoiseGenerator,
    Metronome,
    CoordinateSystem,
    AngleConversion,
    GradientSampler,
    ConvertToColor,
    ExtractColor,
    PackVec3,
    Concatenate,
    ConvertToString,
    Split,
    BooleanOperation,
    Compare,
    ConditionGate,
    TriggerOnOff,
    Gate,
    DelayOneTick,
    DebugValue,
    DebugLog,
}

impl PrimitiveNodeKind {
    const ALL: [Self; 32] = [
        Self::Constant,
        Self::Property,
        Self::Math,
        Self::Function,
        Self::Remap,
        Self::Clamp,
        Self::SmoothFilter,
        Self::OneMinus,
        Self::Inverse,
        Self::Negate,
        Self::Speed,
        Self::Counter,
        Self::Lfo,
        Self::NoiseGenerator,
        Self::Metronome,
        Self::CoordinateSystem,
        Self::AngleConversion,
        Self::GradientSampler,
        Self::ConvertToColor,
        Self::ExtractColor,
        Self::PackVec3,
        Self::Concatenate,
        Self::ConvertToString,
        Self::Split,
        Self::BooleanOperation,
        Self::Compare,
        Self::ConditionGate,
        Self::TriggerOnOff,
        Self::Gate,
        Self::DelayOneTick,
        Self::DebugValue,
        Self::DebugLog,
    ];

    #[must_use]
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::Property => "property",
            Self::Math => "math",
            Self::Function => "function",
            Self::Remap => "remap",
            Self::Clamp => "clamp",
            Self::SmoothFilter => "smooth_filter",
            Self::OneMinus => "one_minus",
            Self::Inverse => "inverse",
            Self::Negate => "negate",
            Self::Speed => "speed",
            Self::Counter => "counter",
            Self::Lfo => "lfo",
            Self::NoiseGenerator => "noise_generator",
            Self::Metronome => "metronome",
            Self::CoordinateSystem => "coordinate_system",
            Self::AngleConversion => "angle_conversion",
            Self::GradientSampler => "gradient_sampler",
            Self::ConvertToColor => "convert_to_color",
            Self::ExtractColor => "extract_color",
            Self::PackVec3 => "pack_vec3",
            Self::Concatenate => "concatenate",
            Self::ConvertToString => "convert_to_string",
            Self::Split => "split",
            Self::BooleanOperation => "boolean_operation",
            Self::Compare => "compare",
            Self::ConditionGate => "condition_gate",
            Self::TriggerOnOff => "trigger_on_off",
            Self::Gate => "gate",
            Self::DelayOneTick => "delay_one_tick",
            Self::DebugValue => "debug_value",
            Self::DebugLog => "debug_log",
        }
    }
}

pub struct PrimitiveNodeDeclaration {
    kind: PrimitiveNodeKind,
}

impl PrimitiveNodeDeclaration {
    #[must_use]
    pub const fn new(kind: PrimitiveNodeKind) -> Self {
        Self { kind }
    }

    #[must_use]
    pub const fn kind(&self) -> PrimitiveNodeKind {
        self.kind
    }
}

impl ANodeDeclaration for PrimitiveNodeDeclaration {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new(self.kind.type_name())
    }

    fn label(&self) -> &'static str {
        match self.kind {
            PrimitiveNodeKind::Constant => "Constant",
            PrimitiveNodeKind::Property => "Property",
            PrimitiveNodeKind::Math => "Math",
            PrimitiveNodeKind::Function => "Function",
            PrimitiveNodeKind::Remap => "Remap",
            PrimitiveNodeKind::Clamp => "Clamp",
            PrimitiveNodeKind::SmoothFilter => "Smooth Filter",
            PrimitiveNodeKind::OneMinus => "One Minus",
            PrimitiveNodeKind::Inverse => "Inverse",
            PrimitiveNodeKind::Negate => "Negate",
            PrimitiveNodeKind::Speed => "Speed",
            PrimitiveNodeKind::Counter => "Counter",
            PrimitiveNodeKind::Lfo => "LFO",
            PrimitiveNodeKind::NoiseGenerator => "Noise Generator",
            PrimitiveNodeKind::Metronome => "Metronome",
            PrimitiveNodeKind::CoordinateSystem => "Coordinate System",
            PrimitiveNodeKind::AngleConversion => "Degrees/Radians",
            PrimitiveNodeKind::GradientSampler => "Gradient Sampler",
            PrimitiveNodeKind::ConvertToColor => "Convert To Color",
            PrimitiveNodeKind::ExtractColor => "Extract Color",
            PrimitiveNodeKind::PackVec3 => "Pack Vec3",
            PrimitiveNodeKind::Concatenate => "Concatenate",
            PrimitiveNodeKind::ConvertToString => "Convert To String",
            PrimitiveNodeKind::Split => "Split",
            PrimitiveNodeKind::BooleanOperation => "Boolean Operation",
            PrimitiveNodeKind::Compare => "Compare",
            PrimitiveNodeKind::ConditionGate => "Condition Gate",
            PrimitiveNodeKind::TriggerOnOff => "Trigger On/Off",
            PrimitiveNodeKind::Gate => "Gate",
            PrimitiveNodeKind::DelayOneTick => "Delay One Tick",
            PrimitiveNodeKind::DebugValue => "Debug Value",
            PrimitiveNodeKind::DebugLog => "Debug Log",
        }
    }

    fn category(&self) -> &'static str {
        match self.kind {
            PrimitiveNodeKind::Constant
            | PrimitiveNodeKind::Property
            | PrimitiveNodeKind::Lfo
            | PrimitiveNodeKind::NoiseGenerator
            | PrimitiveNodeKind::Metronome => "Values",
            PrimitiveNodeKind::Math
            | PrimitiveNodeKind::Function
            | PrimitiveNodeKind::Remap
            | PrimitiveNodeKind::Clamp
            | PrimitiveNodeKind::SmoothFilter
            | PrimitiveNodeKind::OneMinus
            | PrimitiveNodeKind::Inverse
            | PrimitiveNodeKind::Negate
            | PrimitiveNodeKind::Speed => "Number",
            PrimitiveNodeKind::Counter => "Number",
            PrimitiveNodeKind::CoordinateSystem | PrimitiveNodeKind::AngleConversion => "Geometry",
            PrimitiveNodeKind::GradientSampler
            | PrimitiveNodeKind::ConvertToColor
            | PrimitiveNodeKind::ExtractColor => "Color",
            PrimitiveNodeKind::PackVec3 => "Geometry",
            PrimitiveNodeKind::Concatenate | PrimitiveNodeKind::ConvertToString | PrimitiveNodeKind::Split => "String",
            PrimitiveNodeKind::BooleanOperation | PrimitiveNodeKind::Compare => "Logic",
            PrimitiveNodeKind::ConditionGate => "Flow",
            PrimitiveNodeKind::TriggerOnOff | PrimitiveNodeKind::Gate | PrimitiveNodeKind::DelayOneTick => "Flow",
            PrimitiveNodeKind::DebugValue | PrimitiveNodeKind::DebugLog => "Debug",
        }
    }

    fn execution_kind(&self) -> ExecutionKind {
        match self.kind {
            PrimitiveNodeKind::SmoothFilter
            | PrimitiveNodeKind::Speed
            | PrimitiveNodeKind::Counter
            | PrimitiveNodeKind::Lfo
            | PrimitiveNodeKind::NoiseGenerator
            | PrimitiveNodeKind::Metronome
            | PrimitiveNodeKind::ConditionGate
            | PrimitiveNodeKind::TriggerOnOff
            | PrimitiveNodeKind::DelayOneTick => ExecutionKind::Stateful,
            PrimitiveNodeKind::DebugLog => ExecutionKind::EffectEmitter,
            _ => ExecutionKind::Pure,
        }
    }

    fn default_process_on_input_change_only(&self) -> bool {
        !matches!(
            self.kind,
            PrimitiveNodeKind::SmoothFilter
                | PrimitiveNodeKind::Speed
                | PrimitiveNodeKind::Lfo
                | PrimitiveNodeKind::NoiseGenerator
                | PrimitiveNodeKind::Metronome
                | PrimitiveNodeKind::DelayOneTick
        )
    }

    fn process_on_input_change_only(&self, instance: &ANodeInstance) -> bool {
        if matches!(self.kind, PrimitiveNodeKind::Counter) {
            true
        } else {
            match instance.config.get(crate::PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG) {
                Some(RuntimeValue::Bool(value)) => *value,
                _ => self.default_process_on_input_change_only(),
            }
        }
    }

    fn process_on_input_change_only_configurable(&self) -> bool {
        !matches!(self.kind, PrimitiveNodeKind::Counter)
    }

    fn breaks_dependency_cycle(&self) -> bool {
        self.kind == PrimitiveNodeKind::DelayOneTick
    }

    fn config_fields(&self) -> Vec<ANodeConfigFieldDecl> {
        match self.kind {
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
            PrimitiveNodeKind::ConditionGate => vec![
                enum_config(
                    "mode",
                    "Mode",
                    "pass_when_true",
                    &[
                        ("pass_when_true", "Pass When True"),
                        ("pass_when_false", "Pass When False"),
                        ("hold_last", "Hold Last"),
                        ("output_default", "Output Default"),
                        ("block_trigger", "Block Trigger"),
                    ],
                ),
                enum_config(
                    "gate_application",
                    "Application",
                    "whole",
                    &[("whole", "Whole"), ("per_lane", "Per Lane")],
                ),
            ],
            PrimitiveNodeKind::TriggerOnOff => vec![
                ANodeConfigFieldDecl::new("toggle", "Toggle", RuntimeValue::Bool(false))
                    .with_description("Alternate On and Off triggers on rising input edges."),
            ],
            _ => Vec::new(),
        }
    }

    fn config_fields_for(&self, instance: &ANodeInstance) -> Vec<ANodeConfigFieldDecl> {
        match self.kind {
            PrimitiveNodeKind::SmoothFilter => {
                let mut fields = vec![smooth_method_config()];
                fields.extend(match config_string(instance, "method", "one_euro").as_str() {
                    "sma" | "savitzky_golay" | "median" => vec![
                        ANodeConfigFieldDecl::new("window", "Window", RuntimeValue::Int(5))
                            .with_description("Number of samples retained by the filter."),
                    ],
                    "damping" => vec![
                        ANodeConfigFieldDecl::new("mass", "Mass", RuntimeValue::Float(1.0)),
                        ANodeConfigFieldDecl::new("friction", "Friction", RuntimeValue::Float(8.0)),
                    ],
                    _ => vec![
                        ANodeConfigFieldDecl::new("min_cutoff", "Min Cutoff", RuntimeValue::Float(1.0)),
                        ANodeConfigFieldDecl::new("beta", "Beta", RuntimeValue::Float(0.0)),
                    ],
                });
                fields
            }
            PrimitiveNodeKind::ConvertToString => {
                let mut fields = vec![string_format_config()];
                match config_string(instance, "format", "decimal").as_str() {
                    "decimal" | "time" => {
                        fields.push(ANodeConfigFieldDecl::new("decimals", "Decimals", RuntimeValue::Int(3)))
                    }
                    _ => {}
                }
                fields
            }
            PrimitiveNodeKind::NoiseGenerator => noise_config_fields(&config_string(instance, "algorithm", "random")),
            _ => self.config_fields(),
        }
    }

    fn role_capabilities(&self) -> Vec<ANodeRoleCapability> {
        match self.kind {
            PrimitiveNodeKind::Math => vec![filter_capability(
                None,
                Some("result"),
                AutoWirePolicy::None,
                PipelineCardinality::Aggregate,
            )],
            PrimitiveNodeKind::Function => vec![unary_filter_capability(
                "value",
                "result",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::Remap => vec![unary_filter_capability(
                "value",
                "result",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::Clamp => vec![unary_filter_capability(
                "value",
                "result",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::SmoothFilter => vec![unary_filter_capability(
                "value",
                "result",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::OneMinus
            | PrimitiveNodeKind::Inverse
            | PrimitiveNodeKind::Negate
            | PrimitiveNodeKind::Speed
            | PrimitiveNodeKind::AngleConversion
            | PrimitiveNodeKind::CoordinateSystem => vec![unary_filter_capability(
                "value",
                "result",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::ConvertToColor => vec![filter_capability(
                None,
                Some("color"),
                AutoWirePolicy::None,
                PipelineCardinality::Reshape,
            )],
            PrimitiveNodeKind::ExtractColor => vec![filter_capability(
                Some("color"),
                None,
                AutoWirePolicy::None,
                PipelineCardinality::Reshape,
            )],
            PrimitiveNodeKind::PackVec3 => vec![filter_capability(
                None,
                Some("value"),
                AutoWirePolicy::None,
                PipelineCardinality::Reshape,
            )],
            PrimitiveNodeKind::ConditionGate => vec![filter_capability(
                Some("value"),
                Some("value"),
                AutoWirePolicy::Gate {
                    input: socket("value"),
                    condition: socket("condition"),
                    output: socket("value"),
                },
                PipelineCardinality::WholeSet,
            )],
            _ => Vec::new(),
        }
    }

    fn signature(&self, ctx: &SignatureCtx<'_>, instance: &ANodeInstance, _bindings: &TypeBindings) -> ANodeSignature {
        match self.kind {
            PrimitiveNodeKind::Constant => constant_signature(instance),
            PrimitiveNodeKind::Property => property_signature(ctx, instance),
            PrimitiveNodeKind::Math => {
                generic_numbered_numeric_signature("value", "Value", input_count(instance, 2), "result")
            }
            PrimitiveNodeKind::Function => function_signature(instance),
            PrimitiveNodeKind::Remap => float_signature(&["value", "in_min", "in_max", "out_min", "out_max"], "result"),
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
            PrimitiveNodeKind::ExtractColor => extract_color_signature(instance),
            PrimitiveNodeKind::PackVec3 => ANodeSignature {
                inputs: vec![
                    InputSocketDecl::new("x", "X", exact("float")),
                    InputSocketDecl::new("y", "Y", exact("float")),
                    InputSocketDecl::new("z", "Z", exact("float")),
                ],
                outputs: vec![OutputSocketDecl::new("value", "Value", exact("vec3"))],
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
            PrimitiveNodeKind::DelayOneTick => passthrough_signature(),
            PrimitiveNodeKind::DebugValue => passthrough_signature(),
            PrimitiveNodeKind::DebugLog => ANodeSignature {
                inputs: vec![InputSocketDecl::new("value", "Value", TypeConstraint::Any)],
                outputs: Vec::new(),
                ..ANodeSignature::default()
            },
        }
    }

    fn compile_operation(
        &self,
        instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, Diagnostic> {
        Ok(match self.kind {
            PrimitiveNodeKind::Constant => constant::operation(instance),
            PrimitiveNodeKind::Property => property::operation(instance)?,
            PrimitiveNodeKind::Math => CompiledNodeOperation::Custom(Arc::new(math::MathEval {
                operator: math::MathOperator::from_config(instance),
            })),
            PrimitiveNodeKind::Function => CompiledNodeOperation::Custom(Arc::new(function::FunctionEval {
                function: function::FunctionKind::from_config(instance),
            })),
            PrimitiveNodeKind::Remap => CompiledNodeOperation::Custom(Arc::new(remap::RemapEval)),
            PrimitiveNodeKind::Clamp => CompiledNodeOperation::Clamp,
            PrimitiveNodeKind::SmoothFilter => {
                CompiledNodeOperation::Custom(Arc::new(smooth_filter::SmoothFilterEval::from_config(instance)))
            }
            PrimitiveNodeKind::OneMinus => CompiledNodeOperation::Custom(Arc::new(one_minus::OneMinusEval)),
            PrimitiveNodeKind::Inverse => CompiledNodeOperation::Custom(Arc::new(inverse::InverseEval)),
            PrimitiveNodeKind::Negate => CompiledNodeOperation::Custom(Arc::new(negate::NegateEval)),
            PrimitiveNodeKind::Speed => CompiledNodeOperation::Custom(Arc::new(speed::SpeedEval {
                window_seconds: config_float(instance, "window_seconds", 0.1).max(0.0),
            })),
            PrimitiveNodeKind::Counter => CompiledNodeOperation::Custom(Arc::new(counter::CounterEval)),
            PrimitiveNodeKind::Lfo => CompiledNodeOperation::Custom(Arc::new(lfo::LfoEval {
                shape: lfo::LfoShape::from_config(instance),
                frequency: config_float(instance, "frequency", 1.0),
                update_rate: config_float(instance, "update_rate", 60.0),
                minimum: config_float(instance, "minimum", 0.0),
                maximum: config_float(instance, "maximum", 1.0),
            })),
            PrimitiveNodeKind::NoiseGenerator => {
                CompiledNodeOperation::Custom(Arc::new(noise_generator::NoiseGeneratorEval {
                    algorithm: noise_generator::NoiseAlgorithm::from_config(instance),
                    scale: config_float(instance, "scale", 1.0).max(0.0001),
                    seed: config_int(instance, "seed", 0),
                    octaves: config_int(instance, "octaves", 4).clamp(1, 12) as usize,
                    persistence: config_float(instance, "persistence", 0.5).clamp(0.0, 1.0),
                    lacunarity: config_float(instance, "lacunarity", 2.0).max(0.0001),
                    jitter: config_float(instance, "jitter", 1.0).clamp(0.0, 1.0),
                }))
            }
            PrimitiveNodeKind::Metronome => CompiledNodeOperation::Custom(Arc::new(metronome::MetronomeEval {
                mode: metronome::MetronomeMode::from_config(instance),
                value: config_float(instance, "value", 120.0),
                on_ratio: config_float(instance, "on_ratio", 0.5).clamp(0.0, 1.0),
                randomness: config_float(instance, "randomness", 0.0).clamp(0.0, 1.0),
            })),
            PrimitiveNodeKind::CoordinateSystem => {
                CompiledNodeOperation::Custom(Arc::new(coordinate_system::CoordinateSystemEval {
                    mode: coordinate_system::CoordinateMode::from_config(instance),
                }))
            }
            PrimitiveNodeKind::AngleConversion => {
                CompiledNodeOperation::Custom(Arc::new(angle_conversion::AngleConversionEval {
                    mode: angle_conversion::AngleMode::from_config(instance),
                }))
            }
            PrimitiveNodeKind::GradientSampler => {
                CompiledNodeOperation::Custom(Arc::new(gradient_sampler::GradientSamplerEval {
                    stops: gradient_sampler::stops_from_config(instance),
                }))
            }
            PrimitiveNodeKind::ConvertToColor => {
                CompiledNodeOperation::Custom(Arc::new(convert_to_color::ConvertToColorEval {
                    mode: color_mode::ColorMode::from_config(instance),
                }))
            }
            PrimitiveNodeKind::ExtractColor => {
                CompiledNodeOperation::Custom(Arc::new(extract_color::ExtractColorEval {
                    mode: color_mode::ColorMode::from_config(instance),
                }))
            }
            PrimitiveNodeKind::PackVec3 => CompiledNodeOperation::Custom(Arc::new(pack_vec3::PackVec3Eval)),
            PrimitiveNodeKind::Concatenate => CompiledNodeOperation::Custom(Arc::new(concatenate::ConcatenateEval {
                prefix: config_string(instance, "prefix", ""),
                suffix: config_string(instance, "suffix", ""),
                separator: config_string(instance, "separator", ""),
            })),
            PrimitiveNodeKind::ConvertToString => {
                CompiledNodeOperation::Custom(Arc::new(convert_to_string::ConvertToStringEval {
                    format: convert_to_string::StringFormat::from_config(instance),
                    decimals: config_int(instance, "decimals", 3).clamp(0, 12) as usize,
                }))
            }
            PrimitiveNodeKind::Split => CompiledNodeOperation::Custom(Arc::new(split::SplitEval {
                separator: config_string(instance, "separator", ","),
                trim: config_bool(instance, "trim", true),
                omit_empty: config_bool(instance, "omit_empty", false),
            })),
            PrimitiveNodeKind::BooleanOperation => {
                CompiledNodeOperation::Custom(Arc::new(boolean_operation::BooleanOperationEval {
                    operator: boolean_operation::BooleanOperator::from_config(instance),
                }))
            }
            PrimitiveNodeKind::Compare => CompiledNodeOperation::Custom(Arc::new(compare::CompareEval {
                comparator: compare::Comparator::from_config(instance),
            })),
            PrimitiveNodeKind::ConditionGate => {
                CompiledNodeOperation::Custom(Arc::new(condition_gate::ConditionGateEval::from_config(instance)))
            }
            PrimitiveNodeKind::TriggerOnOff => {
                CompiledNodeOperation::Custom(Arc::new(trigger_on_off::TriggerOnOffEval {
                    toggle: config_bool(instance, "toggle", false),
                }))
            }
            PrimitiveNodeKind::Gate => gate::operation(),
            PrimitiveNodeKind::DelayOneTick => delay_one_tick::operation(),
            PrimitiveNodeKind::DebugValue => CompiledNodeOperation::Custom(Arc::new(debug_value::DebugValueEval)),
            PrimitiveNodeKind::DebugLog => debug_log::operation(),
        })
    }
}

fn socket(id: &str) -> crate::SocketId {
    crate::SocketId::new(id)
}

fn filter_capability(
    primary_input: Option<&str>,
    primary_output: Option<&str>,
    autowire: AutoWirePolicy,
    cardinality: PipelineCardinality,
) -> ANodeRoleCapability {
    ANodeRoleCapability {
        role: SurfaceItemKind::Filter,
        primary_input: primary_input.map(socket),
        primary_output: primary_output.map(socket),
        autowire,
        cardinality,
        ui_mode: ManagedUiMode::CompactRow,
    }
}

fn unary_filter_capability(input: &str, output: &str, cardinality: PipelineCardinality) -> ANodeRoleCapability {
    filter_capability(
        Some(input),
        Some(output),
        AutoWirePolicy::UnaryTransform {
            input: socket(input),
            output: socket(output),
        },
        cardinality,
    )
}

pub fn register_primitive_nodes(registry: &mut ANodeRegistry) -> Result<(), RegistryError> {
    for kind in PrimitiveNodeKind::ALL {
        registry.register(PrimitiveNodeDeclaration::new(kind))?;
    }
    Ok(())
}

#[must_use]
pub fn primitive_node_registry() -> ANodeRegistry {
    let mut registry = ANodeRegistry::default();
    register_primitive_nodes(&mut registry).expect("primitive ANode IDs must be unique");
    registry
}
