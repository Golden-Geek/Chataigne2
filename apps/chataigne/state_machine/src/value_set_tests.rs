use chataigne_alchemist::{StableRef, ValueTypeId};
use golden_values::Value as RuntimeValue;

use crate::{ValueLaneKey, ValueSet, ValueSetEntry, value_set::VALUE_SET_TYPE};

#[test]
fn value_set_constructs_with_stable_lane_keys() {
    let source = StableRef::new(ValueTypeId::new("chataigne.module_endpoint"), "module/fader");
    let entry = ValueSetEntry::new(
        ValueLaneKey::new("module:fader").unwrap(),
        "Fader",
        RuntimeValue::Float(0.75),
    )
    .with_source(source.clone());
    let value_set = ValueSet::with_entries(42, vec![entry]);

    assert_eq!(value_set.logical_tick, 42);
    assert_eq!(value_set.entries[0].key.as_str(), "module:fader");
    assert_eq!(value_set.entries[0].source.as_ref(), Some(&source));
}

#[test]
fn value_set_rejects_empty_lane_keys() {
    assert!(ValueLaneKey::new("  ").is_err());
}

#[test]
fn value_set_roundtrips_through_runtime_extension_payload() {
    let value_set = ValueSet::with_entries(
        7,
        vec![ValueSetEntry::new(
            ValueLaneKey::new("input:x").unwrap(),
            "X",
            RuntimeValue::Vec2([1.0, 2.0]),
        )],
    );

    let runtime_value = value_set.to_runtime_value().unwrap();

    assert_eq!(runtime_value.value_type(), ValueTypeId::new(VALUE_SET_TYPE));
    assert_eq!(ValueSet::from_runtime_value(&runtime_value).unwrap(), value_set);
}

#[test]
fn old_parameter_array_runtime_type_is_not_accepted_as_valueset() {
    let old_value = RuntimeValue::Extension(chataigne_alchemist::ExtensionValue::new(
        ValueTypeId::new("chataigne.param_array"),
        [],
    ));

    assert!(ValueSet::from_runtime_value(&old_value).is_err());
}
