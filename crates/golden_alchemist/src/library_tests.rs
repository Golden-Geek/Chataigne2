use crate::{
    ANodeDeclaration, ANodeInstance, ANodeTypeId, ExecutionKind, PrimitiveNodeDeclaration, PrimitiveNodeKind,
    RuntimeValue, SignatureCtx, TypeConstraint, TypeVar, ValueTypeRegistry, primitive_node_registry,
};

fn signature(kind: PrimitiveNodeKind) -> crate::ANodeSignature {
    let declaration = PrimitiveNodeDeclaration::new(kind);
    let instance = ANodeInstance::new(declaration.type_id(), declaration.label());
    let value_types = ValueTypeRegistry::with_primitives();
    declaration.signature(
        &SignatureCtx {
            value_types: &value_types,
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
        "add",
        "compare",
        "bool_and",
        "bool_or",
        "bool_not",
        "edge",
        "gate",
        "map_range",
        "clamp",
        "delay_one_tick",
        "debug_log",
    ] {
        assert!(registry.get(&ANodeTypeId::new(id)).is_some(), "{id}");
    }
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
fn add_signature_shares_named_generic_and_defaults_to_float() {
    let signature = signature(PrimitiveNodeKind::Add);
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

signature_test!(compare_signature_is_declared, Compare, 2, 1);
signature_test!(bool_and_signature_is_declared, BoolAnd, 2, 1);
signature_test!(bool_or_signature_is_declared, BoolOr, 2, 1);
signature_test!(bool_not_signature_is_declared, BoolNot, 1, 1);
signature_test!(edge_signature_is_declared, Edge, 1, 1);
signature_test!(gate_signature_is_declared, Gate, 2, 1);
signature_test!(map_range_signature_is_declared, MapRange, 5, 1);
signature_test!(clamp_signature_is_declared, Clamp, 3, 1);
signature_test!(delay_signature_is_declared, DelayOneTick, 1, 1);
signature_test!(debug_log_signature_is_declared, DebugLog, 1, 0);

#[test]
fn stateful_and_effect_nodes_are_explicit() {
    assert_eq!(
        PrimitiveNodeDeclaration::new(PrimitiveNodeKind::Edge).execution_kind(),
        ExecutionKind::Stateful
    );
    assert_eq!(
        PrimitiveNodeDeclaration::new(PrimitiveNodeKind::DebugLog).execution_kind(),
        ExecutionKind::EffectEmitter
    );
    assert!(PrimitiveNodeDeclaration::new(PrimitiveNodeKind::DelayOneTick).breaks_dependency_cycle());
}
