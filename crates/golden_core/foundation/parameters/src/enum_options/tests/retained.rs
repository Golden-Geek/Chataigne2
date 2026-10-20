use super::*;
use crate::NodeReference;

#[test]
fn frozen_domain_charge_includes_indexes_spare_slots_and_nested_reference_hints() {
    let mut reference = NodeReference::empty();
    reference.cached_name = Some(String::with_capacity(16_384));
    reference.relative_path_from_root = Vec::with_capacity(4);
    reference.relative_path_from_root.push(String::with_capacity(32_768));
    let mut tags = Vec::with_capacity(8);
    tags.push(String::with_capacity(65_536));
    let entry = ParameterEnumOption {
        variant_id: String::with_capacity(4096),
        value: ParamValue::Reference(reference),
        label: String::with_capacity(8192),
        tags,
        ordering: Some(3),
    };
    let mut entries = Vec::with_capacity(16);
    entries.push(entry);
    let expected = size_of::<ParameterEnumDomain>()
        + 2 * size_of::<usize>()
        + 16 * size_of::<ParameterEnumOption>()
        + 2 * size_of::<usize>()
        + 4096
        + 8192
        + 16_384
        + 4 * size_of::<String>()
        + 32_768
        + 8 * size_of::<String>()
        + 65_536;
    assert_eq!(ParameterEnumDomain::retained_bytes_for(&entries), expected);
    let domain = Arc::new(ParameterEnumDomain::new(entries));
    assert_eq!(domain.retained_bytes(), expected);
    let options = ParameterEnumOptions::from_shared(domain.clone());
    for _ in 0..128 {
        let snapshot = options.clone();
        assert_eq!(snapshot.retained_bytes(), expected);
        assert!(Arc::ptr_eq(snapshot.shared(), &domain));
    }
}

#[test]
fn semantic_equality_and_storage_charge_are_independent_of_reference_hints() {
    let first = ParameterEnumOption {
        value: ParamValue::Reference(NodeReference::empty()),
        ..option(0)
    };
    let mut second = first.clone();
    let ParamValue::Reference(reference) = &mut second.value else {
        unreachable!()
    };
    reference.cached_name = Some(String::with_capacity(8192));
    let left: ParameterEnumOptions = vec![first.clone()].into();
    let right: ParameterEnumOptions = vec![second].into();
    assert_eq!(left, right);
    assert!(right.retained_bytes() >= left.retained_bytes() + 8192);
}

#[test]
fn canonical_enum_values_share_the_variant_index_and_custom_values_keep_both_lookups() {
    let entry = |value: &str| ParameterEnumOption {
        variant_id: "aa".into(),
        value: ParamValue::Enum(value.into()),
        label: "label".into(),
        tags: vec![],
        ordering: None,
    };
    let canonical: ParameterEnumOptions = vec![entry("aa")].into();
    let custom: ParameterEnumOptions = vec![entry("bb")].into();
    assert_eq!(custom.retained_bytes(), canonical.retained_bytes() + size_of::<usize>());
    assert!(canonical.contains_variant("aa"));
    assert!(canonical.contains_value(&ParamValue::Enum("aa".into())));
    assert!(!canonical.contains_value(&ParamValue::Str("aa".into())));
    assert!(custom.contains_variant("aa"));
    assert!(custom.contains_value(&ParamValue::Enum("bb".into())));
    assert!(!custom.contains_value(&ParamValue::Enum("aa".into())));
}
