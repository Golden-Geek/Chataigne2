use super::*;

#[test]
fn frame_validation_preserves_dmx_boundaries() {
    assert!(DmxFrame::new(0, vec![]).is_err());
    assert!(DmxFrame::new(1, vec![0; DMX_SLOT_COUNT]).is_ok());
    assert!(DmxFrame::new(SACN_MAX_UNIVERSE, vec![]).is_ok());
    assert!(DmxFrame::new(SACN_MAX_UNIVERSE + 1, vec![]).is_err());
    assert!(DmxFrame::new(1, vec![0; DMX_SLOT_COUNT + 1]).is_err());
}

#[test]
fn channel_updates_are_one_based_and_expand_to_a_complete_universe() {
    let mut frame = DmxFrame::new(1, Vec::new()).unwrap();
    frame.set_channel(1, 12).unwrap();
    frame.set_channel(512, 34).unwrap();

    assert_eq!(frame.slots.len(), DMX_SLOT_COUNT);
    assert_eq!(frame.slots[0], 12);
    assert_eq!(frame.slots[511], 34);
    assert!(frame.set_channel(0, 1).is_err());
    assert!(frame.set_channel(513, 1).is_err());
}

#[test]
fn json_frames_reject_oversized_channel_values() {
    assert_eq!(parse_slots_json("[0, 127, 255]").unwrap(), vec![0, 127, 255]);
    assert!(parse_slots_json("[256]").is_err());
    assert!(parse_slots_json("not-json").is_err());
}
