use super::*;
use std::mem::size_of;

#[test]
fn retained_storage_counts_spare_string_capacity_and_scalar_inline_size() {
    for value in [
        ParamValue::Int(0),
        ParamValue::Trigger(),
        ParamValue::Vec3(0.0, 0.0, 0.0),
    ] {
        assert_eq!(value.retained_bytes(), size_of::<ParamValue>());
    }
    for value in [ParamValue::Str, ParamValue::File, ParamValue::Enum] {
        let mut text = String::with_capacity(1024);
        text.push('x');
        let expected = size_of::<ParamValue>() + text.capacity();
        assert_eq!(value(text).retained_bytes(), expected);
    }
}

#[test]
fn retained_reference_storage_includes_spare_path_slots_segments_and_cached_name() {
    let mut reference = NodeReference {
        relative_path_from_root: Vec::with_capacity(64),
        ..Default::default()
    };
    let mut segment = String::with_capacity(128);
    segment.push('a');
    let mut name = String::with_capacity(256);
    name.push('b');
    let expected = size_of::<ParamValue>()
        + reference.relative_path_from_root.capacity() * size_of::<String>()
        + segment.capacity()
        + name.capacity();
    reference.relative_path_from_root.push(segment);
    reference.cached_name = Some(name);
    assert_eq!(ParamValue::Reference(reference).retained_bytes(), expected);
}
