use crate::{
    ANodeInstance, ANodeTypeId, ChannelDescriptor, ChannelLayout, ManagedApplicationError, ManagedSettingClass,
    ManagedSettingPath, ManagedStateScope, PipelineCardinality, PrimitiveNodeKind, RuntimeValue, SignatureCtx,
    SocketId, StableRef, ValueLaneKey, ValueTypeId, ValueTypeRegistry, classify_managed_setting,
    configured_managed_variant, primitive_node_registry,
};

fn layout() -> ChannelLayout {
    ChannelLayout::new(vec![
        channel("left", "float"),
        channel("flag", "bool"),
        channel("right", "float"),
        channel("text", "string"),
    ])
    .unwrap()
}

#[test]
fn mapping_application_consumes_the_full_tuple_without_implicit_subset_routing() {
    let registry = primitive_node_registry();
    let value_types = ValueTypeRegistry::with_primitives();
    let ctx = SignatureCtx {
        value_types: &value_types,
        properties: None,
    };
    let mut math = instance(PrimitiveNodeKind::Math);
    math.config.set("application", RuntimeValue::String("each".into()));
    assert_eq!(
        registry.resolve_mapping_application(&math, &layout(), &ctx),
        Err(ManagedApplicationError::IncompatibleTuple)
    );
    math.config.set(
        "managed_selection",
        RuntimeValue::Array(vec![RuntimeValue::String("left".into())]),
    );
    assert_eq!(
        registry.resolve_mapping_application(&math, &layout(), &ctx),
        Err(ManagedApplicationError::ExplicitChannelRouting)
    );

    let floats = ChannelLayout::new(vec![
        channel("x", "float"),
        channel("y", "float"),
        channel("z", "float"),
    ])
    .unwrap();
    for kind in [PrimitiveNodeKind::Sum, PrimitiveNodeKind::Average] {
        let declaration = registry.get(&ANodeTypeId::new(kind.type_name())).unwrap();
        let instance = configured_managed_variant(declaration.as_ref(), 0, Some(3)).unwrap();
        assert_eq!(instance.config.get("num_inputs"), Some(&RuntimeValue::Int(3)));
        let application = registry.resolve_mapping_application(&instance, &floats, &ctx).unwrap();
        assert_eq!(application.selection.indices, vec![0, 1, 2]);
        assert_eq!(application.groups, vec![vec![0, 1, 2]]);
    }
}

fn channel(id: &str, value_type: &str) -> ChannelDescriptor {
    ChannelDescriptor::input(
        ValueLaneKey::new(id).unwrap(),
        id,
        StableRef::new(ValueTypeId::new("source"), id),
        Some(ValueTypeId::new(value_type)),
    )
}

fn instance(kind: PrimitiveNodeKind) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(kind.type_name()), kind.type_name())
}

#[test]
fn math_modes_resolve_from_configured_signature_and_mixed_layout() {
    let registry = primitive_node_registry();
    let value_types = ValueTypeRegistry::with_primitives();
    let ctx = SignatureCtx {
        value_types: &value_types,
        properties: None,
    };
    let layout = layout();
    let mut math = instance(PrimitiveNodeKind::Math);
    math.config.set("application", RuntimeValue::String("each".into()));
    let each = registry.resolve_managed_application(&math, &layout, &ctx).unwrap();
    assert_eq!(each.cardinality, PipelineCardinality::Elementwise);
    assert_eq!(each.primary_inputs, vec![SocketId::new("value1")]);
    assert_eq!(each.auxiliary_inputs, vec![SocketId::new("value2")]);
    assert_eq!(each.outputs, vec![SocketId::new("result")]);
    assert_eq!(each.selection.indices, vec![0, 2]);
    assert_eq!(each.groups, vec![vec![0], vec![2]]);
    assert_eq!(each.state_scope, ManagedStateScope::PerChannel);

    math.config.set("application", RuntimeValue::String("combine".into()));
    math.config.set("num_inputs", RuntimeValue::Int(2));
    math.config.set(
        "managed_selection",
        RuntimeValue::Array(vec![
            RuntimeValue::String("right".into()),
            RuntimeValue::String("left".into()),
        ]),
    );
    let combined = registry.resolve_managed_application(&math, &layout, &ctx).unwrap();
    assert_eq!(combined.cardinality, PipelineCardinality::Aggregate);
    assert_eq!(
        combined.primary_inputs,
        vec![SocketId::new("value1"), SocketId::new("value2")]
    );
    assert!(combined.auxiliary_inputs.is_empty());
    assert_eq!(combined.selection.indices, vec![2, 0]);
    assert_eq!(combined.groups, vec![vec![2, 0]]);
    assert_eq!(combined.state_scope, ManagedStateScope::PerGroup);
}

#[test]
fn explicit_incompatible_selection_and_invalid_mode_are_not_advertised() {
    let registry = primitive_node_registry();
    let value_types = ValueTypeRegistry::with_primitives();
    let ctx = SignatureCtx {
        value_types: &value_types,
        properties: None,
    };
    let layout = layout();
    let mut math = instance(PrimitiveNodeKind::Math);
    math.config.set("application", RuntimeValue::String("each".into()));
    math.config.set(
        "managed_selection",
        RuntimeValue::Array(vec![RuntimeValue::String("flag".into())]),
    );
    assert!(matches!(
        registry.resolve_managed_application(&math, &layout, &ctx),
        Err(ManagedApplicationError::Layout(
            crate::ChannelLayoutError::IncompatibleChannel(_)
        ))
    ));
    math.config
        .set("application", RuntimeValue::String("unsupported".into()));
    assert_eq!(
        registry.resolve_managed_application(&math, &layout, &ctx),
        Err(ManagedApplicationError::NotFilterCapable)
    );
    math.config.set("application", RuntimeValue::String("each".into()));
    math.config.set("managed_selection", RuntimeValue::String("all".into()));
    math.input_defaults
        .insert(SocketId::new("value2"), RuntimeValue::Bool(true));
    assert_eq!(
        registry.resolve_managed_application(&math, &layout, &ctx),
        Err(ManagedApplicationError::IncompatibleAuxiliaryInput {
            socket: SocketId::new("value2"),
            actual: ValueTypeId::new("bool"),
        })
    );
    math.input_defaults.insert(
        SocketId::new("value2"),
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("bool"), "module/flag")),
    );
    assert_eq!(
        registry.resolve_managed_application(&math, &layout, &ctx),
        Err(ManagedApplicationError::IncompatibleAuxiliaryInput {
            socket: SocketId::new("value2"),
            actual: ValueTypeId::new("bool"),
        })
    );
}

#[test]
fn multiple_outputs_and_empty_compatible_selection_are_explicit() {
    let registry = primitive_node_registry();
    let value_types = ValueTypeRegistry::with_primitives();
    let ctx = SignatureCtx {
        value_types: &value_types,
        properties: None,
    };
    let extract = instance(PrimitiveNodeKind::ExtractColor);
    let color_layout = ChannelLayout::new(vec![channel("color", "color")]).unwrap();
    let resolved = registry
        .resolve_managed_application(&extract, &color_layout, &ctx)
        .unwrap();
    assert!(resolved.outputs.len() >= 3);
    assert_eq!(resolved.primary_inputs, vec![SocketId::new("color")]);
    assert_eq!(resolved.groups, vec![vec![0]]);
    assert_eq!(resolved.state_scope, ManagedStateScope::PerChannel);
    let empty = registry.resolve_managed_application(&extract, &layout(), &ctx).unwrap();
    assert!(empty.selection.no_compatible_channels);
    assert!(empty.groups.is_empty());
}

#[test]
fn signature_arity_and_declared_groups_are_validated_before_palette_use() {
    let registry = primitive_node_registry();
    let value_types = ValueTypeRegistry::with_primitives();
    let ctx = SignatureCtx {
        value_types: &value_types,
        properties: None,
    };
    let mut math = instance(PrimitiveNodeKind::Math);
    math.config.set("application", RuntimeValue::String("combine".into()));
    math.config.set("num_inputs", RuntimeValue::Int(2));
    let three = ChannelLayout::new(vec![
        channel("a", "float"),
        channel("b", "float"),
        channel("c", "float"),
    ])
    .unwrap();
    assert_eq!(
        registry.resolve_managed_application(&math, &three, &ctx),
        Err(ManagedApplicationError::GroupArityMismatch { expected: 2, actual: 3 })
    );
    math.config.set(
        "managed_selection",
        RuntimeValue::Array(vec![RuntimeValue::String("c".into()), RuntimeValue::String("a".into())]),
    );
    math.config.set(
        "managed_groups",
        RuntimeValue::Array(vec![RuntimeValue::Array(vec![
            RuntimeValue::String("c".into()),
            RuntimeValue::String("a".into()),
        ])]),
    );
    let resolved = registry.resolve_managed_application(&math, &three, &ctx).unwrap();
    assert_eq!(resolved.groups, vec![vec![2, 0]]);
}

#[test]
fn setting_classification_tracks_executable_dependencies() {
    let registry = primitive_node_registry();
    let value_types = ValueTypeRegistry::with_primitives();
    let ctx = SignatureCtx {
        value_types: &value_types,
        properties: None,
    };
    let math = instance(PrimitiveNodeKind::Math);
    let declaration = registry.get(&math.type_id).unwrap();
    assert_eq!(
        classify_managed_setting(
            declaration.as_ref(),
            &math,
            &ctx,
            ManagedSettingPath::Input(&SocketId::new("value2"))
        )
        .unwrap(),
        ManagedSettingClass::RuntimeValue
    );
    assert_eq!(
        classify_managed_setting(
            declaration.as_ref(),
            &math,
            &ctx,
            ManagedSettingPath::Config("application")
        )
        .unwrap(),
        ManagedSettingClass::Structural
    );
    assert_eq!(
        classify_managed_setting(declaration.as_ref(), &math, &ctx, ManagedSettingPath::Presentation).unwrap(),
        ManagedSettingClass::Presentation
    );
    let gradient = instance(PrimitiveNodeKind::GradientSampler);
    let declaration = registry.get(&gradient.type_id).unwrap();
    assert_eq!(
        classify_managed_setting(
            declaration.as_ref(),
            &gradient,
            &ctx,
            ManagedSettingPath::Config("gradient")
        )
        .unwrap(),
        ManagedSettingClass::Resource
    );
}
