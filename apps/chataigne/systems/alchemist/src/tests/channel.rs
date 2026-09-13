use crate::{
    ChannelDescriptor, ChannelGroups, ChannelLayout, ChannelLayoutError, ChannelMetadata, ChannelSelection,
    ManagedItemId, SocketId, StableRef, ValueComponent, ValueLaneKey, ValueTypeId,
};

fn channel(id: &str, value_type: &str) -> ChannelDescriptor {
    ChannelDescriptor::input(
        ValueLaneKey::new(id).unwrap(),
        id,
        StableRef::new(ValueTypeId::new("source"), id),
        Some(ValueTypeId::new(value_type)),
    )
}

#[test]
fn mixed_layout_keeps_compounds_and_stable_authored_identities() {
    let first = ManagedItemId::new();
    let second = ManagedItemId::new();
    let source = StableRef::new(ValueTypeId::new("endpoint"), "same/source");
    let layout = ChannelLayout::new(vec![
        ChannelDescriptor::input(
            ValueLaneKey::input(first),
            "First",
            source.clone(),
            Some("float".into()),
        ),
        ChannelDescriptor::input(ValueLaneKey::input(second), "Second", source, Some("float".into())),
        channel("vector", "vec3"),
        channel("flag", "bool"),
        channel("text", "string"),
    ])
    .unwrap();
    assert_ne!(layout.channels()[0].id, layout.channels()[1].id);
    assert_eq!(layout.channels().len(), 5);
    assert_eq!(layout.channels()[2].value_type, Some("vec3".into()));
    assert_eq!(
        ChannelSelection::AllCompatible
            .resolve(&layout, |kind| kind.as_str() == "float")
            .unwrap()
            .indices,
        vec![0, 1]
    );
    let empty = ChannelSelection::AllCompatible
        .resolve(&layout, |kind| kind.as_str() == "color")
        .unwrap();
    assert!(empty.indices.is_empty());
    assert!(empty.no_compatible_channels);
    assert_eq!(
        ChannelSelection::Explicit(vec![layout.channels()[2].id.clone()])
            .resolve(&layout, |kind| kind.as_str() == "float")
            .unwrap_err(),
        ChannelLayoutError::IncompatibleChannel(layout.channels()[2].id.clone())
    );
}

#[test]
fn layout_revisions_distinguish_metadata_from_structure_and_values() {
    let initial = ChannelLayout::new(vec![channel("a", "float"), channel("b", "bool")]).unwrap();
    let mut relabeled = initial.channels().to_vec();
    relabeled[0].label = "Renamed source".into();
    relabeled[0].metadata = ChannelMetadata {
        minimum: Some(-1.0),
        maximum: Some(1.0),
        unit: Some("V".into()),
    };
    let metadata = initial.reconcile(relabeled).unwrap();
    assert_eq!(metadata.structural_revision(), initial.structural_revision());
    assert_eq!(metadata.presentation_revision(), initial.presentation_revision() + 1);
    let same = metadata.reconcile(metadata.channels().to_vec()).unwrap();
    assert_eq!(same.presentation_revision(), metadata.presentation_revision());
    let reordered = metadata
        .reorder(&[ValueLaneKey::new("b").unwrap(), ValueLaneKey::new("a").unwrap()])
        .unwrap();
    assert_eq!(reordered.structural_revision(), metadata.structural_revision() + 1);
    assert_eq!(reordered.channels()[1].metadata.unit.as_deref(), Some("V"));
}

#[test]
fn missing_selection_and_duplicate_identity_fail_without_retargeting() {
    let layout = ChannelLayout::new(vec![channel("a", "float")]).unwrap();
    assert_eq!(
        ChannelSelection::Explicit(vec![ValueLaneKey::new("removed").unwrap()])
            .resolve(&layout, |_| true)
            .unwrap_err(),
        ChannelLayoutError::MissingChannel(ValueLaneKey::new("removed").unwrap())
    );
    assert_eq!(
        ChannelLayout::new(vec![channel("a", "float"), channel("a", "bool")]).unwrap_err(),
        ChannelLayoutError::DuplicateIdentity(ValueLaneKey::new("a").unwrap())
    );
}

#[test]
fn ordered_group_replacement_and_extraction_have_declared_identities() {
    let layout = ChannelLayout::new(vec![
        channel("a", "float"),
        channel("b", "bool"),
        channel("c", "float"),
        channel("d", "float"),
    ])
    .unwrap();
    let groups = ChannelGroups(vec![vec![
        ValueLaneKey::new("d").unwrap(),
        ValueLaneKey::new("a").unwrap(),
    ]]);
    assert_eq!(
        groups.resolve(&layout, |kind| kind.as_str() == "float").unwrap(),
        vec![vec![3, 0]]
    );
    let item = ManagedItemId::new();
    let port = SocketId::new("result");
    let result_id = ValueLaneKey::output(item, &port);
    let replacement = ChannelDescriptor::derived(
        result_id.clone(),
        "Result",
        "float".into(),
        item,
        port.clone(),
        groups.0[0].clone(),
    );
    let replaced = layout.replace_selected(&[3, 0], vec![replacement]).unwrap();
    assert_eq!(
        replaced
            .channels()
            .iter()
            .map(|channel| channel.id.as_str())
            .collect::<Vec<_>>(),
        vec![result_id.as_str(), "b", "c"]
    );
    let vector = ValueLaneKey::new("vector").unwrap();
    assert_ne!(vector.extracted(ValueComponent::X), vector.extracted(ValueComponent::Y));
    assert_eq!(
        vector.extracted(ValueComponent::X),
        ValueLaneKey::new("vector").unwrap().extracted(ValueComponent::X)
    );
    assert_ne!(ValueLaneKey::duplicate(item, &port), ValueLaneKey::output(item, &port));
}

#[test]
fn projection_layouts_compose_before_values_exist() {
    let layout = ChannelLayout::new(vec![
        channel("left", "float"),
        channel("flag", "bool"),
        channel("right", "float"),
    ])
    .unwrap();
    let pack = ManagedItemId::new();
    let packed = layout
        .project_group(
            &[ValueLaneKey::new("right").unwrap(), ValueLaneKey::new("left").unwrap()],
            pack,
            SocketId::new("value"),
            ValueTypeId::new("vec2"),
            "Packed",
        )
        .unwrap();
    assert_eq!(packed.channels()[0].value_type, Some(ValueTypeId::new("vec2")));
    assert_eq!(packed.channels()[1].id.as_str(), "flag");
    let extracted = packed
        .project_extract(
            &[packed.channels()[0].id.clone()],
            &[ValueComponent::X, ValueComponent::Y],
            ManagedItemId::new(),
        )
        .unwrap();
    assert_eq!(extracted.channels().len(), 3);
    assert_eq!(extracted.channels()[2].id.as_str(), "flag");
    let duplicate = extracted
        .project_duplicate(&extracted.channels()[1].id, ManagedItemId::new(), SocketId::new("copy"))
        .unwrap();
    assert_eq!(duplicate.channels().len(), 4);
    assert_eq!(duplicate.channels()[1].id, extracted.channels()[1].id);
    assert_ne!(duplicate.channels()[2].id, extracted.channels()[1].id);
}
