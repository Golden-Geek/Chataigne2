use crate::{AudioDeviceMatch, AudioDeviceSelection, AudioDirection, match_device_selection};

use super::support::{device, fingerprint};

#[test]
fn stable_id_survives_rename_and_reenumeration() {
    let original = device(
        "stable-a",
        "Old Name",
        true,
        fingerprint("Interface", 0, 2),
        0,
        2,
        false,
        false,
    );
    let selection = AudioDeviceSelection::from_descriptor(&original);
    let unrelated = device("other", "Other", true, fingerprint("Other", 0, 2), 0, 2, false, false);
    let mut renamed = original;
    renamed.label = "New Name".to_owned();

    assert_eq!(
        match_device_selection(&selection, AudioDirection::Output, &[unrelated, renamed.clone()]),
        AudioDeviceMatch::Matched(Box::new(renamed))
    );
}

#[test]
fn fallback_fingerprint_matches_uniquely_and_reports_ambiguity() {
    let shared_fingerprint = fingerprint("No Serial Interface", 0, 2);
    let original = device(
        "ephemeral-1",
        "Interface",
        false,
        shared_fingerprint.clone(),
        0,
        2,
        false,
        false,
    );
    let selection = AudioDeviceSelection::from_descriptor(&original);
    let replacement = device(
        "ephemeral-2",
        "Renamed",
        false,
        shared_fingerprint.clone(),
        0,
        2,
        false,
        false,
    );
    assert!(matches!(
        match_device_selection(
            &selection,
            AudioDirection::Output,
            std::slice::from_ref(&replacement)
        ),
        AudioDeviceMatch::Matched(found) if found.target == replacement.target
    ));

    let duplicate = device(
        "ephemeral-3",
        "Duplicate",
        false,
        shared_fingerprint,
        0,
        2,
        false,
        false,
    );
    assert!(matches!(
        match_device_selection(
            &selection,
            AudioDirection::Output,
            &[replacement, duplicate]
        ),
        AudioDeviceMatch::Ambiguous(candidates) if candidates.len() == 2
    ));
}
