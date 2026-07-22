use std::sync::Arc;

use crate::{
    ConversionKind, ExtensionValue, FacetId, RuntimeValue, ValueStorageKind, ValueTypeDescriptor, ValueTypeId,
    ValueTypeRegistry,
};

#[test]
fn primitive_value_types_are_registered() {
    let registry = ValueTypeRegistry::with_primitives();

    for id in [
        "unit", "bool", "trigger", "int", "float", "string", "vec2", "vec3", "color", "duration",
    ] {
        assert!(registry.contains(&ValueTypeId::new(id)), "{id} is missing");
    }
    assert!(registry.can_convert_automatically(&ValueTypeId::new("int"), &ValueTypeId::new("float")));
    assert!(registry.can_convert_automatically(&ValueTypeId::new("float"), &ValueTypeId::new("int")));
    assert!(registry.can_convert_automatically(&ValueTypeId::new("string"), &ValueTypeId::new("bool")));
}

#[test]
fn custom_extension_value_type_can_be_registered() {
    let custom_id = ValueTypeId::new("example.packet");
    let mut registry = ValueTypeRegistry::with_primitives();

    registry
        .register(ValueTypeDescriptor::new(
            custom_id.clone(),
            "Packet",
            ValueStorageKind::Extension,
            {
                let custom_id = custom_id.clone();
                move || RuntimeValue::Extension(ExtensionValue::new(custom_id.clone(), Arc::<[u8]>::from([])))
            },
        ))
        .unwrap();

    let descriptor = registry.get(&custom_id).unwrap();
    assert_eq!((descriptor.default_value)().value_type(), custom_id);
}

#[test]
fn facet_compatibility_is_descriptor_driven() {
    let command_target = FacetId::new("command_target");
    let module_id = ValueTypeId::new("example.module");
    let mut registry = ValueTypeRegistry::with_primitives();
    registry
        .register(
            ValueTypeDescriptor::new(module_id.clone(), "Module", ValueStorageKind::StableRef, {
                let module_id = module_id.clone();
                move || RuntimeValue::Ref(crate::StableRef::new(module_id.clone(), "default"))
            })
            .with_facets([command_target.clone()])
            .with_conversion(ValueTypeId::new("string"), ConversionKind::Lossy),
        )
        .unwrap();

    assert!(registry.supports_facet(&module_id, &command_target));
    assert!(registry.can_convert_automatically(&module_id, &ValueTypeId::new("string")));
}

#[test]
fn trigger_is_distinct_from_boolean() {
    assert_ne!(
        RuntimeValue::Bool(true).value_type(),
        RuntimeValue::Trigger(crate::TriggerValue::fired(7, 42)).value_type()
    );
}
