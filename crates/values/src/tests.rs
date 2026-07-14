use std::{sync::Arc, time::Duration};

use super::{ColorValue, TriggerValue, Value, ValueComponent, ValueTypeId};

#[test]
fn canonical_conversions_cover_current_scalar_and_vector_behavior() {
    assert_eq!(
        Value::Int(7).convert_to(&ValueTypeId::new("float")).unwrap(),
        Value::Float(7.0)
    );
    assert_eq!(
        Value::String(Arc::from("2,3,4"))
            .convert_to(&ValueTypeId::new("vec3"))
            .unwrap(),
        Value::Vec3([2.0, 3.0, 4.0])
    );
    assert_eq!(
        Value::Duration(Duration::from_millis(250))
            .convert_to(&ValueTypeId::new("float"))
            .unwrap(),
        Value::Float(0.25)
    );
}

#[test]
fn component_updates_preserve_unmodified_channels() {
    let color = Value::Color(ColorValue {
        red: 0.1,
        green: 0.2,
        blue: 0.3,
        alpha: 0.4,
    });
    assert_eq!(
        color.with_component(ValueComponent::G, &Value::Float(0.8)).unwrap(),
        Value::Color(ColorValue {
            red: 0.1,
            green: 0.8,
            blue: 0.3,
            alpha: 0.4,
        })
    );
}

#[test]
fn trigger_edges_remain_explicit_data() {
    assert_eq!(
        Value::Trigger(TriggerValue::fired(9, 11)),
        Value::Trigger(TriggerValue {
            fired: true,
            edge_id: 9,
            logical_tick: 11,
        })
    );
}
