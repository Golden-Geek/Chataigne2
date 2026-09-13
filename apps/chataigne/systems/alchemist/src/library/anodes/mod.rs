use std::sync::Arc;

use crate::{
    ANodeConfigFieldDecl, ANodeDeclaration, ANodeInstance, ANodeRegistry, ANodeRoleCapability, ANodeSignature,
    ANodeTypeId, AutoWirePolicy, CompiledNodeOperation, Diagnostic, ExecutionKind, InputSocketDecl, ManagedUiMode,
    NodeStateLayout, OutputSocketDecl, PipelineCardinality, RegistryError, ResolvedANodeSignature, RuntimeValue,
    SignatureCtx, StableRef, SurfaceItemKind, TriggerValue, TypeBindings, TypeConstraint, ValueTypeId,
};

mod angle_conversion;
mod boolean_operation;
mod color_mode;
mod compare;
mod concatenate;
mod condition_gate;
mod config_fields;
mod constant;
mod convert_compound;
mod convert_scalar;
mod convert_to_color;
mod convert_to_string;
mod convert_tuple;
mod coordinate_system;
mod counter;
mod curve_remap;
mod debug_log;
mod debug_value;
mod delay_one_tick;
mod extract_color;
mod extract_vec2;
mod extract_vec3;
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
mod pack_vec2;
mod pack_vec3;
mod property;
mod remap;
mod signature;
mod smooth_filter;
mod speed;
mod split;
mod support;
mod threshold;
mod timed_delay;
mod trigger_on_off;

use support::{
    color_mode_config, compare_signature, config_bool, config_float, config_int, config_string, constant_signature,
    convert_to_color_signature, enum_config, exact, extract_color_signature, float_signature, function_signature,
    generic_numbered_numeric_signature, generic_numeric_signature, input_count, lfo_shape_config,
    metronome_mode_config, noise_config_fields, numbered_inputs, optional_count_config, passthrough_signature,
    property_signature, smooth_method_config, string_format_config, value_type_config_field,
    value_type_config_field_with_constraint,
};

pub use curve_remap::curve_config_value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveNodeKind {
    Constant,
    Property,
    Math,
    Sum,
    Average,
    Product,
    Minimum,
    Maximum,
    Difference,
    Distance,
    Function,
    Remap,
    CurveRemap,
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
    ConvertCompound,
    ConvertToInt,
    ConvertToFloat,
    ConvertToBool,
    ConvertTuple,
    ExtractColor,
    ExtractVec2,
    ExtractVec3,
    PackVec2,
    PackVec3,
    Concatenate,
    ConvertToString,
    Split,
    BooleanOperation,
    Compare,
    Threshold,
    ConditionGate,
    TriggerOnOff,
    Gate,
    DelayOneTick,
    TimedDelay,
    DebugValue,
    DebugLog,
}

impl PrimitiveNodeKind {
    const ALL: [Self; 50] = [
        Self::Constant,
        Self::Property,
        Self::Math,
        Self::Sum,
        Self::Average,
        Self::Product,
        Self::Minimum,
        Self::Maximum,
        Self::Difference,
        Self::Distance,
        Self::Function,
        Self::Remap,
        Self::CurveRemap,
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
        Self::ConvertCompound,
        Self::ConvertToInt,
        Self::ConvertToFloat,
        Self::ConvertToBool,
        Self::ConvertTuple,
        Self::ExtractColor,
        Self::ExtractVec2,
        Self::ExtractVec3,
        Self::PackVec2,
        Self::PackVec3,
        Self::Concatenate,
        Self::ConvertToString,
        Self::Split,
        Self::BooleanOperation,
        Self::Compare,
        Self::Threshold,
        Self::ConditionGate,
        Self::TriggerOnOff,
        Self::Gate,
        Self::DelayOneTick,
        Self::TimedDelay,
        Self::DebugValue,
        Self::DebugLog,
    ];

    #[must_use]
    #[cfg(test)]
    pub(crate) const fn all() -> &'static [Self] {
        &Self::ALL
    }

    #[must_use]
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::Property => "property",
            Self::Math => "math",
            Self::Sum => "sum",
            Self::Average => "average",
            Self::Product => "product",
            Self::Minimum => "minimum",
            Self::Maximum => "maximum",
            Self::Difference => "difference",
            Self::Distance => "distance",
            Self::Function => "function",
            Self::Remap => "remap",
            Self::CurveRemap => "curve_remap",
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
            Self::ConvertCompound => "convert_compound",
            Self::ConvertToInt => "convert_to_int",
            Self::ConvertToFloat => "convert_to_float",
            Self::ConvertToBool => "convert_to_bool",
            Self::ConvertTuple => "convert_tuple",
            Self::ExtractColor => "extract_color",
            Self::ExtractVec2 => "extract_vec2",
            Self::ExtractVec3 => "extract_vec3",
            Self::PackVec2 => "pack_vec2",
            Self::PackVec3 => "pack_vec3",
            Self::Concatenate => "concatenate",
            Self::ConvertToString => "convert_to_string",
            Self::Split => "split",
            Self::BooleanOperation => "boolean_operation",
            Self::Compare => "compare",
            Self::Threshold => "threshold",
            Self::ConditionGate => "condition_gate",
            Self::TriggerOnOff => "trigger_on_off",
            Self::Gate => "gate",
            Self::DelayOneTick => "delay_one_tick",
            Self::TimedDelay => "timed_delay",
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
            PrimitiveNodeKind::Sum => "Sum",
            PrimitiveNodeKind::Average => "Average",
            PrimitiveNodeKind::Product => "Product",
            PrimitiveNodeKind::Minimum => "Minimum",
            PrimitiveNodeKind::Maximum => "Maximum",
            PrimitiveNodeKind::Difference => "Difference",
            PrimitiveNodeKind::Distance => "Distance",
            PrimitiveNodeKind::Function => "Function",
            PrimitiveNodeKind::Remap => "Remap",
            PrimitiveNodeKind::CurveRemap => "Curve Remap",
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
            PrimitiveNodeKind::ConvertCompound => "Convert Compound",
            PrimitiveNodeKind::ConvertToInt => "Convert To Integer",
            PrimitiveNodeKind::ConvertToFloat => "Convert To Float",
            PrimitiveNodeKind::ConvertToBool => "Convert To Boolean",
            PrimitiveNodeKind::ConvertTuple => "Convert Tuple",
            PrimitiveNodeKind::ExtractColor => "Extract Color",
            PrimitiveNodeKind::ExtractVec2 => "Extract Vec2",
            PrimitiveNodeKind::ExtractVec3 => "Extract Vec3",
            PrimitiveNodeKind::PackVec2 => "Pack Vec2",
            PrimitiveNodeKind::PackVec3 => "Pack Vec3",
            PrimitiveNodeKind::Concatenate => "Concatenate",
            PrimitiveNodeKind::ConvertToString => "Convert To String",
            PrimitiveNodeKind::Split => "Split",
            PrimitiveNodeKind::BooleanOperation => "Boolean Operation",
            PrimitiveNodeKind::Compare => "Compare",
            PrimitiveNodeKind::Threshold => "Threshold",
            PrimitiveNodeKind::ConditionGate => "Condition Gate",
            PrimitiveNodeKind::TriggerOnOff => "Trigger On/Off",
            PrimitiveNodeKind::Gate => "Gate",
            PrimitiveNodeKind::DelayOneTick => "Delay One Tick",
            PrimitiveNodeKind::TimedDelay => "Timed Delay",
            PrimitiveNodeKind::DebugValue => "Debug Value",
            PrimitiveNodeKind::DebugLog => "Debug Log",
        }
    }

    fn category(&self) -> &'static str {
        match self.kind {
            PrimitiveNodeKind::Constant
            | PrimitiveNodeKind::Property
            | PrimitiveNodeKind::ConvertCompound
            | PrimitiveNodeKind::ConvertToInt
            | PrimitiveNodeKind::ConvertToFloat
            | PrimitiveNodeKind::ConvertToBool
            | PrimitiveNodeKind::ConvertTuple
            | PrimitiveNodeKind::Lfo
            | PrimitiveNodeKind::NoiseGenerator
            | PrimitiveNodeKind::Metronome => "Values",
            PrimitiveNodeKind::Math
            | PrimitiveNodeKind::Sum
            | PrimitiveNodeKind::Average
            | PrimitiveNodeKind::Product
            | PrimitiveNodeKind::Minimum
            | PrimitiveNodeKind::Maximum
            | PrimitiveNodeKind::Difference
            | PrimitiveNodeKind::Distance
            | PrimitiveNodeKind::Function
            | PrimitiveNodeKind::Remap
            | PrimitiveNodeKind::CurveRemap
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
            PrimitiveNodeKind::ExtractVec2
            | PrimitiveNodeKind::ExtractVec3
            | PrimitiveNodeKind::PackVec2
            | PrimitiveNodeKind::PackVec3 => "Geometry",
            PrimitiveNodeKind::Concatenate | PrimitiveNodeKind::ConvertToString | PrimitiveNodeKind::Split => "String",
            PrimitiveNodeKind::BooleanOperation | PrimitiveNodeKind::Compare | PrimitiveNodeKind::Threshold => "Logic",
            PrimitiveNodeKind::ConditionGate => "Flow",
            PrimitiveNodeKind::TriggerOnOff
            | PrimitiveNodeKind::Gate
            | PrimitiveNodeKind::DelayOneTick
            | PrimitiveNodeKind::TimedDelay => "Flow",
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
            | PrimitiveNodeKind::Threshold
            | PrimitiveNodeKind::TriggerOnOff
            | PrimitiveNodeKind::DelayOneTick => ExecutionKind::Stateful,
            PrimitiveNodeKind::TimedDelay => ExecutionKind::Stateful,
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
                | PrimitiveNodeKind::TimedDelay
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

    fn state_layout(&self, _instance: &ANodeInstance, _resolved: &ResolvedANodeSignature) -> NodeStateLayout {
        match self.kind {
            PrimitiveNodeKind::DelayOneTick => NodeStateLayout::RuntimeValues(2),
            PrimitiveNodeKind::TimedDelay => NodeStateLayout::RuntimeValues(2),
            _ if self.execution_kind() == ExecutionKind::Stateful => NodeStateLayout::RuntimeValues(1),
            _ => NodeStateLayout::Stateless,
        }
    }

    fn config_fields(&self) -> Vec<ANodeConfigFieldDecl> {
        config_fields::for_kind(self.kind)
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
            PrimitiveNodeKind::Math
            | PrimitiveNodeKind::Sum
            | PrimitiveNodeKind::Average
            | PrimitiveNodeKind::Product
            | PrimitiveNodeKind::Minimum
            | PrimitiveNodeKind::Maximum
            | PrimitiveNodeKind::Difference
            | PrimitiveNodeKind::Distance => vec![filter_capability(
                None,
                Some("result"),
                AutoWirePolicy::None,
                PipelineCardinality::Aggregate,
            )],
            PrimitiveNodeKind::ConvertTuple => vec![filter_capability(
                None,
                None,
                AutoWirePolicy::None,
                PipelineCardinality::Aggregate,
            )],
            PrimitiveNodeKind::Function => vec![unary_filter_capability(
                "value",
                "result",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::Remap | PrimitiveNodeKind::CurveRemap => vec![unary_filter_capability(
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
            | PrimitiveNodeKind::CoordinateSystem
            | PrimitiveNodeKind::DelayOneTick => vec![unary_filter_capability(
                "value",
                if self.kind == PrimitiveNodeKind::DelayOneTick {
                    "value"
                } else {
                    "result"
                },
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::TimedDelay => vec![unary_filter_capability(
                "value",
                "value",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::GradientSampler => vec![unary_filter_capability(
                "position",
                "color",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::Threshold => vec![unary_filter_capability(
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
            PrimitiveNodeKind::ConvertCompound => vec![unary_filter_capability(
                "value",
                "result",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::ConvertToInt
            | PrimitiveNodeKind::ConvertToFloat
            | PrimitiveNodeKind::ConvertToBool
            | PrimitiveNodeKind::ConvertToString => vec![unary_filter_capability(
                "value",
                "result",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::ExtractColor => vec![filter_capability(
                Some("color"),
                None,
                AutoWirePolicy::None,
                PipelineCardinality::Reshape,
            )],
            PrimitiveNodeKind::ExtractVec2 => vec![filter_capability(
                Some("value"),
                None,
                AutoWirePolicy::None,
                PipelineCardinality::Reshape,
            )],
            PrimitiveNodeKind::ExtractVec3 => vec![filter_capability(
                Some("value"),
                None,
                AutoWirePolicy::None,
                PipelineCardinality::Reshape,
            )],
            PrimitiveNodeKind::PackVec2 | PrimitiveNodeKind::PackVec3 => vec![filter_capability(
                None,
                Some("value"),
                AutoWirePolicy::None,
                PipelineCardinality::Reshape,
            )],
            PrimitiveNodeKind::Concatenate | PrimitiveNodeKind::BooleanOperation | PrimitiveNodeKind::Compare => {
                vec![filter_capability(
                    None,
                    Some("result"),
                    AutoWirePolicy::None,
                    PipelineCardinality::Aggregate,
                )]
            }
            PrimitiveNodeKind::Split => vec![unary_filter_capability(
                "value",
                "values",
                PipelineCardinality::Elementwise,
            )],
            PrimitiveNodeKind::Gate => vec![unary_filter_capability(
                "trigger",
                "trigger",
                PipelineCardinality::Elementwise,
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

    fn role_capabilities_for(&self, instance: &ANodeInstance) -> Vec<ANodeRoleCapability> {
        if self.kind != PrimitiveNodeKind::Math {
            return self.role_capabilities();
        }
        match config_string(instance, "application", "combine").as_str() {
            "each" => vec![unary_filter_capability(
                "value1",
                "result",
                PipelineCardinality::Elementwise,
            )],
            "combine" => self.role_capabilities(),
            _ => Vec::new(),
        }
    }

    fn managed_application_variants(&self) -> Vec<ANodeInstance> {
        let default = ANodeInstance::new(self.type_id(), self.label());
        if self.kind == PrimitiveNodeKind::ConvertTuple {
            return [
                ("float", "Convert Tuple to Float"),
                ("int", "Convert Tuple to Integer"),
                ("bool", "Convert Tuple to Boolean"),
                ("string", "Convert Tuple to String"),
            ]
            .into_iter()
            .map(|(target, label)| {
                let mut variant = default.clone();
                variant.label = label.into();
                variant.config.set("target", RuntimeValue::String(target.into()));
                variant
            })
            .collect();
        }
        if self.kind == PrimitiveNodeKind::ConvertCompound {
            return [
                ("vec2", "Convert to Vec2"),
                ("vec3", "Convert to Vec3"),
                ("color", "Convert to Color"),
            ]
            .into_iter()
            .map(|(target, label)| {
                let mut variant = default.clone();
                variant.label = label.into();
                variant.config.set("target", RuntimeValue::String(target.into()));
                variant
            })
            .collect();
        }
        if self.kind == PrimitiveNodeKind::Threshold {
            return [("above", "Above Threshold"), ("below", "Below Threshold")]
                .into_iter()
                .map(|(direction, label)| {
                    let mut variant = default.clone();
                    variant.label = label.into();
                    variant.config.set("direction", RuntimeValue::String(direction.into()));
                    variant
                })
                .collect();
        }
        if self.kind != PrimitiveNodeKind::Math {
            return self
                .supports_role(SurfaceItemKind::Filter)
                .then_some(default)
                .into_iter()
                .collect();
        }
        let mut combine = default.clone();
        combine.label = "Math (Combine)".into();
        let mut each = default;
        each.label = "Math (Each)".into();
        each.config.set("application", RuntimeValue::String("each".into()));
        vec![combine, each]
    }

    fn signature(&self, ctx: &SignatureCtx<'_>, instance: &ANodeInstance, _bindings: &TypeBindings) -> ANodeSignature {
        signature::for_kind(self.kind, ctx, instance)
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
            PrimitiveNodeKind::Sum => CompiledNodeOperation::Custom(Arc::new(math::ReductionEval {
                mode: math::ReductionMode::Sum,
            })),
            PrimitiveNodeKind::Average => CompiledNodeOperation::Custom(Arc::new(math::ReductionEval {
                mode: math::ReductionMode::Average,
            })),
            PrimitiveNodeKind::Product => CompiledNodeOperation::Custom(Arc::new(math::ReductionEval {
                mode: math::ReductionMode::Product,
            })),
            PrimitiveNodeKind::Minimum => CompiledNodeOperation::Custom(Arc::new(math::ReductionEval {
                mode: math::ReductionMode::Minimum,
            })),
            PrimitiveNodeKind::Maximum => CompiledNodeOperation::Custom(Arc::new(math::ReductionEval {
                mode: math::ReductionMode::Maximum,
            })),
            PrimitiveNodeKind::Difference => CompiledNodeOperation::Custom(Arc::new(math::ReductionEval {
                mode: math::ReductionMode::Difference,
            })),
            PrimitiveNodeKind::Distance => CompiledNodeOperation::Custom(Arc::new(math::ReductionEval {
                mode: math::ReductionMode::Distance,
            })),
            PrimitiveNodeKind::Function => CompiledNodeOperation::Custom(Arc::new(function::FunctionEval {
                function: function::FunctionKind::from_config(instance),
            })),
            PrimitiveNodeKind::Remap => CompiledNodeOperation::Custom(Arc::new(remap::RemapEval)),
            PrimitiveNodeKind::CurveRemap => CompiledNodeOperation::Custom(Arc::new(
                curve_remap::CurveRemapEval::from_config(instance).map_err(|error| {
                    Diagnostic::error(
                        "invalid_curve_resource",
                        error,
                        crate::DiagnosticOrigin::Node(instance.id),
                    )
                })?,
            )),
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
                    stops: gradient_sampler::stops_from_config(instance).map_err(|error| {
                        Diagnostic::error(
                            "invalid_gradient_resource",
                            error,
                            crate::DiagnosticOrigin::Node(instance.id),
                        )
                    })?,
                }))
            }
            PrimitiveNodeKind::ConvertToColor => {
                CompiledNodeOperation::Custom(Arc::new(convert_to_color::ConvertToColorEval {
                    mode: color_mode::ColorMode::from_config(instance),
                }))
            }
            PrimitiveNodeKind::ConvertCompound => {
                CompiledNodeOperation::Custom(Arc::new(convert_compound::ConvertCompoundEval {
                    target: convert_compound::CompoundTarget::from_config(instance),
                }))
            }
            PrimitiveNodeKind::ConvertToInt => {
                CompiledNodeOperation::Custom(Arc::new(convert_scalar::ConvertScalarEval {
                    target: convert_scalar::ScalarTarget::Int,
                }))
            }
            PrimitiveNodeKind::ConvertToFloat => {
                CompiledNodeOperation::Custom(Arc::new(convert_scalar::ConvertScalarEval {
                    target: convert_scalar::ScalarTarget::Float,
                }))
            }
            PrimitiveNodeKind::ConvertToBool => {
                CompiledNodeOperation::Custom(Arc::new(convert_scalar::ConvertScalarEval {
                    target: convert_scalar::ScalarTarget::Bool,
                }))
            }
            PrimitiveNodeKind::ConvertTuple => {
                CompiledNodeOperation::Custom(Arc::new(convert_tuple::ConvertTupleEval {
                    target: convert_scalar::ScalarTarget::from_config(instance),
                }))
            }
            PrimitiveNodeKind::ExtractColor => {
                CompiledNodeOperation::Custom(Arc::new(extract_color::ExtractColorEval {
                    mode: color_mode::ColorMode::from_config(instance),
                }))
            }
            PrimitiveNodeKind::ExtractVec2 => CompiledNodeOperation::Custom(Arc::new(extract_vec2::ExtractVec2Eval)),
            PrimitiveNodeKind::ExtractVec3 => CompiledNodeOperation::Custom(Arc::new(extract_vec3::ExtractVec3Eval)),
            PrimitiveNodeKind::PackVec2 => CompiledNodeOperation::Custom(Arc::new(pack_vec2::PackVec2Eval)),
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
            PrimitiveNodeKind::Threshold => {
                CompiledNodeOperation::Custom(Arc::new(threshold::ThresholdEval::from_config(instance)))
            }
            PrimitiveNodeKind::ConditionGate => CompiledNodeOperation::Custom(Arc::new(
                condition_gate::ConditionGateEval::from_config(instance).map_err(|error| {
                    Diagnostic::error(
                        "invalid_condition_gate_config",
                        error,
                        crate::DiagnosticOrigin::Node(instance.id),
                    )
                })?,
            )),
            PrimitiveNodeKind::TriggerOnOff => {
                CompiledNodeOperation::Custom(Arc::new(trigger_on_off::TriggerOnOffEval {
                    toggle: config_bool(instance, "toggle", false),
                }))
            }
            PrimitiveNodeKind::Gate => gate::operation(),
            PrimitiveNodeKind::DelayOneTick => delay_one_tick::operation(),
            PrimitiveNodeKind::TimedDelay => {
                CompiledNodeOperation::Custom(Arc::new(timed_delay::TimedDelayEval::from_config(instance).map_err(
                    |error| Diagnostic::error("invalid_timed_delay", error, crate::DiagnosticOrigin::Node(instance.id)),
                )?))
            }
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
