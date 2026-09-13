use std::sync::Arc;

use chataigne_alchemist::{ChannelDescriptor, ChannelLayout, StableRef, ValueLaneKey, ValueTypeId};
use golden_values::Value as RuntimeValue;

use crate::{ChannelFrame, ChannelFrameError, ChannelValidity};

fn layout() -> Arc<ChannelLayout> {
    Arc::new(
        ChannelLayout::new(vec![ChannelDescriptor::input(
            ValueLaneKey::new("input").unwrap(),
            "Input",
            StableRef::new(ValueTypeId::new("source"), "source"),
            Some(ValueTypeId::new("float")),
        )])
        .unwrap(),
    )
}

#[test]
fn frames_validate_values_and_keep_delivery_separate_from_change() {
    let layout = layout();
    let mut frame = ChannelFrame::new(layout.clone());
    frame.begin_tick(1);
    frame
        .set(0, Some(RuntimeValue::Float(0.5)), ChannelValidity::Valid, true)
        .unwrap();
    assert!(frame.slots()[0].changed);
    assert!(frame.slots()[0].deliver);
    let revision = frame.value_revision();
    frame.begin_tick(2);
    frame
        .set(0, Some(RuntimeValue::Float(0.5)), ChannelValidity::Valid, false)
        .unwrap();
    assert!(!frame.slots()[0].changed);
    assert!(!frame.slots()[0].deliver);
    assert_eq!(frame.value_revision(), revision);
    assert!(Arc::ptr_eq(frame.layout(), &layout));
    assert_eq!(
        frame.set(0, Some(RuntimeValue::Bool(true)), ChannelValidity::Valid, true),
        Err(ChannelFrameError::TypeMismatch {
            channel: ValueLaneKey::new("input").unwrap(),
            expected: ValueTypeId::new("float"),
            actual: ValueTypeId::new("bool"),
        })
    );
}

#[test]
fn frame_reorder_and_rename_preserve_state_by_identity() {
    let initial = Arc::new(
        ChannelLayout::new(vec![
            ChannelDescriptor::input(
                ValueLaneKey::new("a").unwrap(),
                "A",
                StableRef::new(ValueTypeId::new("source"), "a"),
                Some("float".into()),
            ),
            ChannelDescriptor::input(
                ValueLaneKey::new("b").unwrap(),
                "B",
                StableRef::new(ValueTypeId::new("source"), "b"),
                Some("bool".into()),
            ),
        ])
        .unwrap(),
    );
    let mut frame = ChannelFrame::new(initial.clone());
    frame
        .set(0, Some(RuntimeValue::Float(3.0)), ChannelValidity::Valid, true)
        .unwrap();
    frame
        .set(1, Some(RuntimeValue::Bool(true)), ChannelValidity::Valid, true)
        .unwrap();
    let mut reordered = initial.channels().to_vec();
    reordered.reverse();
    reordered[1].label = "Renamed".into();
    let updated = Arc::new(initial.reconcile(reordered).unwrap());
    let frame = frame.with_layout(updated);
    assert_eq!(frame.slots()[0].value, Some(RuntimeValue::Bool(true)));
    assert_eq!(frame.slots()[1].value, Some(RuntimeValue::Float(3.0)));
}
