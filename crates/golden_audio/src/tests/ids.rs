use std::str::FromStr;

use uuid::Uuid;

use crate::{AudioChannelId, AudioDeviceId, BackendId, CommandSequence, PlaybackId};

#[test]
fn uuid_ids_preserve_equality_ordering_formatting_and_serde() {
    let first_uuid = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let second_uuid = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
    let first = AudioChannelId::from_uuid(first_uuid);
    let same = AudioChannelId::from_str(first.to_string().as_str()).unwrap();
    let second = AudioChannelId::from_uuid(second_uuid);

    assert_eq!(first, same);
    assert!(first < second);
    assert_eq!(first.to_string(), first_uuid.to_string());

    let json = serde_json::to_string(&first).unwrap();
    assert_eq!(serde_json::from_str::<AudioChannelId>(&json).unwrap(), first);
}

#[test]
fn string_ids_are_trimmed_nonempty_and_round_trip() {
    let backend = BackendId::new("wasapi").unwrap();
    let device = AudioDeviceId::new("endpoint:{device-guid}").unwrap();
    let playback = PlaybackId::new("intro").unwrap();

    assert!(BackendId::new("").is_err());
    assert!(AudioDeviceId::new(" device ").is_err());
    assert_eq!(backend.to_string(), "wasapi");
    assert_eq!(device.as_str(), "endpoint:{device-guid}");

    let json = serde_json::to_string(&playback).unwrap();
    assert_eq!(serde_json::from_str::<PlaybackId>(&json).unwrap(), playback);
}

#[test]
fn command_sequences_are_nonzero_and_monotonic() {
    assert!(CommandSequence::new(0).is_err());
    assert_eq!(CommandSequence::FIRST.get(), 1);
    assert_eq!(CommandSequence::FIRST.next().get(), 2);
}
