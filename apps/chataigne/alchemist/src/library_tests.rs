use crate::{
    ANodeDeclaration, ANodeInstance, ANodeRoleCapability, ANodeTypeId, AutoWirePolicy, ExecutionKind,
    PipelineCardinality, PrimitiveNodeDeclaration, PrimitiveNodeKind, RuntimeValue, SignatureCtx, SurfaceItemKind,
    TypeConstraint, TypeVar, ValueTypeRegistry, primitive_node_registry,
};

fn signature(kind: PrimitiveNodeKind) -> crate::ANodeSignature {
    let declaration = PrimitiveNodeDeclaration::new(kind);
    let instance = ANodeInstance::new(declaration.type_id(), declaration.label());
    let value_types = ValueTypeRegistry::with_primitives();
    declaration.signature(
        &SignatureCtx {
            value_types: &value_types,
            properties: None,
        },
        &instance,
        &instance.type_bindings,
    )
}

#[test]
fn primitive_catalog_contains_every_declaration() {
    let registry = primitive_node_registry();
    for id in [
        "constant",
        "property",
        "math",
        "function",
        "remap",
        "clamp",
        "smooth_filter",
        "one_minus",
        "inverse",
        "negate",
        "speed",
        "counter",
        "lfo",
        "noise_generator",
        "metronome",
        "coordinate_system",
        "angle_conversion",
        "gradient_sampler",
        "convert_to_color",
        "extract_color",
        "pack_vec3",
        "concatenate",
        "convert_to_string",
        "split",
        "boolean_operation",
        "compare",
        "condition_gate",
        "trigger_on_off",
        "gate",
        "delay_one_tick",
        "debug_value",
        "debug_log",
    ] {
        assert!(registry.get(&ANodeTypeId::new(id)).is_some(), "{id}");
    }
}

#[test]
fn filter_capable_node_discovery_is_declaration_driven() {
    let registry = primitive_node_registry();
    let filter_ids = registry
        .declarations_with_role(SurfaceItemKind::Filter)
        .map(|declaration| declaration.type_id().to_string())
        .collect::<Vec<_>>();

    for id in [
        "math",
        "function",
        "remap",
        "clamp",
        "smooth_filter",
        "one_minus",
        "inverse",
        "negate",
        "speed",
        "coordinate_system",
        "angle_conversion",
        "convert_to_color",
        "extract_color",
        "pack_vec3",
        "condition_gate",
    ] {
        assert!(filter_ids.iter().any(|candidate| candidate == id), "{id}");
    }
    assert!(!filter_ids.iter().any(|candidate| candidate == "constant"));
    assert!(!filter_ids.iter().any(|candidate| candidate == "debug_log"));
}

#[test]
fn non_filter_node_has_no_filter_capability() {
    let declaration = PrimitiveNodeDeclaration::new(PrimitiveNodeKind::Constant);

    assert!(!declaration.supports_role(SurfaceItemKind::Filter));
    assert!(declaration.role_capabilities().is_empty());
}

#[test]
fn primary_socket_autowiring_is_declared_for_unary_filters() {
    let declaration = PrimitiveNodeDeclaration::new(PrimitiveNodeKind::Remap);
    let capability = declaration
        .role_capabilities()
        .into_iter()
        .find(|capability| capability.role == SurfaceItemKind::Filter)
        .expect("Remap should be filter-capable");

    assert_eq!(capability.primary_input, Some(crate::SocketId::new("value")));
    assert_eq!(capability.primary_output, Some(crate::SocketId::new("result")));
    assert_eq!(
        capability.autowire,
        AutoWirePolicy::UnaryTransform {
            input: crate::SocketId::new("value"),
            output: crate::SocketId::new("result"),
        }
    );
    assert_eq!(capability.cardinality, PipelineCardinality::Elementwise);
}

#[test]
fn condition_gate_declares_filter_gate_capability() {
    let declaration = PrimitiveNodeDeclaration::new(PrimitiveNodeKind::ConditionGate);
    let capability = declaration
        .role_capabilities()
        .into_iter()
        .find(|capability| capability.role == SurfaceItemKind::Filter)
        .expect("ConditionGate should be filter-capable");

    assert_eq!(capability.primary_input, Some(crate::SocketId::new("value")));
    assert_eq!(capability.primary_output, Some(crate::SocketId::new("value")));
    assert_eq!(
        capability.autowire,
        AutoWirePolicy::Gate {
            input: crate::SocketId::new("value"),
            condition: crate::SocketId::new("condition"),
            output: crate::SocketId::new("value"),
        }
    );
    assert_eq!(capability.cardinality, PipelineCardinality::WholeSet);
}

#[test]
fn capability_metadata_roundtrips_through_json() {
    let capability = PrimitiveNodeDeclaration::new(PrimitiveNodeKind::ConvertToColor)
        .role_capabilities()
        .into_iter()
        .next()
        .expect("Convert To Color should expose a reshape capability");

    let encoded = serde_json::to_string(&capability).unwrap();
    let decoded: ANodeRoleCapability = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, capability);
    assert_eq!(decoded.cardinality, PipelineCardinality::Reshape);
}

#[test]
fn constant_signature_uses_configured_value_type() {
    let declaration = PrimitiveNodeDeclaration::new(PrimitiveNodeKind::Constant);
    let mut instance = ANodeInstance::new(declaration.type_id(), declaration.label());
    instance.config.set("value", RuntimeValue::Vec3([1.0, 2.0, 3.0]));
    let value_types = ValueTypeRegistry::with_primitives();
    let signature = declaration.signature(
        &SignatureCtx {
            value_types: &value_types,
            properties: None,
        },
        &instance,
        &instance.type_bindings,
    );

    assert_eq!(
        signature.outputs[0].constraint,
        TypeConstraint::Exact(crate::ValueTypeId::new("vec3"))
    );
}

#[test]
fn math_signature_shares_named_generic_and_defaults_to_float() {
    let signature = signature(PrimitiveNodeKind::Math);
    let expected = TypeConstraint::Generic(TypeVar::new("TNumeric"));

    assert_eq!(signature.inputs[0].constraint, expected);
    assert_eq!(signature.inputs[1].constraint, expected);
    assert_eq!(signature.outputs[0].constraint, expected);
    assert_eq!(
        signature
            .default_bindings
            .get(&TypeVar::new("TNumeric"))
            .unwrap()
            .value_type,
        crate::ValueTypeId::new("float")
    );
}

#[test]
fn forceable_math_value_type_options_derive_from_signature_constraint() {
    let value_types = ValueTypeRegistry::with_primitives();
    for kind in [
        PrimitiveNodeKind::Math,
        PrimitiveNodeKind::OneMinus,
        PrimitiveNodeKind::Inverse,
        PrimitiveNodeKind::Negate,
    ] {
        let declaration = PrimitiveNodeDeclaration::new(kind);
        let fields = declaration.config_fields();
        let field = fields
            .iter()
            .find(|field| field.id.as_str() == "value_type")
            .expect("forceable math node should declare Value Type");
        let options = field
            .resolved_type_options(&signature(kind), &value_types)
            .into_iter()
            .map(|value_type| value_type.to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            options,
            ["int", "float", "vec2", "vec3", "color"]
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn passthrough_delay_infers_type_without_forced_value_type_field() {
    assert!(
        PrimitiveNodeDeclaration::new(PrimitiveNodeKind::DelayOneTick)
            .config_fields()
            .is_empty()
    );
}

macro_rules! signature_test {
    ($name:ident, $kind:ident, $inputs:expr, $outputs:expr) => {
        #[test]
        fn $name() {
            let signature = signature(PrimitiveNodeKind::$kind);
            assert_eq!(signature.inputs.len(), $inputs);
            assert_eq!(signature.outputs.len(), $outputs);
        }
    };
}

signature_test!(math_signature_is_declared, Math, 2, 1);
signature_test!(function_signature_is_declared, Function, 1, 1);
signature_test!(remap_signature_is_declared, Remap, 5, 1);
signature_test!(clamp_signature_is_declared, Clamp, 3, 1);
signature_test!(smooth_filter_signature_is_declared, SmoothFilter, 1, 1);
signature_test!(one_minus_signature_is_declared, OneMinus, 1, 1);
signature_test!(inverse_signature_is_declared, Inverse, 1, 1);
signature_test!(negate_signature_is_declared, Negate, 1, 1);
signature_test!(speed_signature_is_declared, Speed, 1, 1);
signature_test!(counter_signature_is_declared, Counter, 3, 1);
signature_test!(lfo_signature_is_declared, Lfo, 0, 1);
signature_test!(noise_signature_is_declared, NoiseGenerator, 1, 1);
signature_test!(metronome_signature_is_declared, Metronome, 0, 2);
signature_test!(coordinate_system_signature_is_declared, CoordinateSystem, 1, 1);
signature_test!(angle_conversion_signature_is_declared, AngleConversion, 1, 1);
signature_test!(gradient_sampler_signature_is_declared, GradientSampler, 1, 1);
signature_test!(convert_to_color_signature_is_declared, ConvertToColor, 4, 1);
signature_test!(extract_color_signature_is_declared, ExtractColor, 1, 4);
signature_test!(pack_vec3_signature_is_declared, PackVec3, 3, 1);
signature_test!(concatenate_signature_is_declared, Concatenate, 2, 1);
signature_test!(convert_to_string_signature_is_declared, ConvertToString, 1, 1);
signature_test!(split_signature_is_declared, Split, 1, 1);
signature_test!(boolean_operation_signature_is_declared, BooleanOperation, 2, 1);
signature_test!(compare_signature_is_declared, Compare, 2, 1);
signature_test!(trigger_on_off_signature_is_declared, TriggerOnOff, 1, 2);
signature_test!(gate_signature_is_declared, Gate, 2, 1);
signature_test!(delay_signature_is_declared, DelayOneTick, 1, 1);
signature_test!(debug_value_signature_is_declared, DebugValue, 1, 1);
signature_test!(debug_log_signature_is_declared, DebugLog, 1, 0);

#[test]
fn metronome_signature_declares_tick_and_on_outputs() {
    let signature = signature(PrimitiveNodeKind::Metronome);

    assert!(signature.inputs.is_empty());
    assert_eq!(signature.outputs[0].id.as_str(), "tick");
    assert_eq!(signature.outputs[0].label, "Tick");
    assert_eq!(signature.outputs[1].id.as_str(), "on");
    assert_eq!(signature.outputs[1].label, "On");
}

#[test]
fn stateful_and_effect_nodes_are_explicit() {
    for kind in [
        PrimitiveNodeKind::SmoothFilter,
        PrimitiveNodeKind::Speed,
        PrimitiveNodeKind::Counter,
        PrimitiveNodeKind::Lfo,
        PrimitiveNodeKind::NoiseGenerator,
        PrimitiveNodeKind::Metronome,
        PrimitiveNodeKind::TriggerOnOff,
        PrimitiveNodeKind::DelayOneTick,
    ] {
        assert_eq!(
            PrimitiveNodeDeclaration::new(kind).execution_kind(),
            ExecutionKind::Stateful
        );
    }
    assert_eq!(
        PrimitiveNodeDeclaration::new(PrimitiveNodeKind::DebugLog).execution_kind(),
        ExecutionKind::EffectEmitter
    );
    assert!(PrimitiveNodeDeclaration::new(PrimitiveNodeKind::DelayOneTick).breaks_dependency_cycle());
}
