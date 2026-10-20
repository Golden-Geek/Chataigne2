use super::*;
use crate::{ParamValue, ParameterConstraints};

mod membership;
mod retained;

fn option(index: usize) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: format!("variant-{index}"),
        value: ParamValue::Enum(format!("variant-{index}")),
        label: format!("Device {index}"),
        tags: vec!["device".into()],
        ordering: Some(index as i32),
    }
}

#[test]
fn large_constraint_clones_share_the_original_option_payload() {
    let constraints = ParameterConstraints {
        enum_options: (0..32_768).map(option).collect::<Vec<_>>().into(),
        ..Default::default()
    };
    let snapshots = (0..128).map(|_| constraints.clone()).collect::<Vec<_>>();
    for snapshot in &snapshots {
        assert!(snapshot.enum_options.same_storage(&constraints.enum_options));
        assert_eq!(
            snapshot.enum_options[0].label.as_ptr(),
            constraints.enum_options[0].label.as_ptr()
        );
    }
    let mut edited = constraints.enum_options.as_slice().to_vec();
    edited[0].label = "Changed".into();
    let edited = ParameterEnumOptions::from(edited);
    assert!(!edited.same_storage(&constraints.enum_options));
    assert_eq!(snapshots[0].enum_options[0].label, "Device 0");
}

#[test]
fn enum_option_wire_shape_stays_an_array_and_empty_constraints_stay_sparse() {
    let original = vec![option(2), option(1)];
    let shared = ParameterEnumOptions::from(original.clone());
    let json = serde_json::to_value(&shared).unwrap();
    assert_eq!(json, serde_json::to_value(original).unwrap());
    let loaded: ParameterEnumOptions = serde_json::from_value(json).unwrap();
    assert_eq!(shared, loaded);
    assert!(!shared.same_storage(&loaded));
    let constraints = serde_json::to_value(ParameterConstraints::default()).unwrap();
    assert!(constraints.get("enum_options").is_none());
    let loaded: ParameterConstraints =
        serde_json::from_value(serde_json::json!({"enum_options": [option(3)]})).unwrap();
    assert_eq!(loaded.enum_options[0].variant_id, "variant-3");
}

#[test]
fn empty_defaults_share_storage_but_explicit_spare_capacity_is_preserved() {
    let left = ParameterEnumOptions::default();
    let right = ParameterEnumOptions::from(Vec::new());
    assert!(left.same_storage(&right));
    let spare = ParameterEnumOptions::from(Vec::with_capacity(64));
    assert!(spare.is_empty());
    assert!(spare.capacity() >= 64);
    assert!(!spare.same_storage(&left));
    let external = Arc::new(ParameterEnumDomain::new(vec![option(1)]));
    let shared = ParameterEnumOptions::from_shared(external.clone());
    assert!(Arc::ptr_eq(shared.shared(), &external));
}
