use super::{Properties, statistics, validate_name};

const NAME: &str = "MapGyver-M01-Present-0123456789abcdef0123456789abcdef";

#[test]
fn only_the_exact_owned_prefix_and_lowercase_uuid_hex_are_admitted() {
    assert!(validate_name(NAME).is_ok());
    for invalid in [
        "PresentMon",
        "MapGyver-M01-Present-",
        "MapGyver-M01-Present-0123456789abcdef0123456789abcde",
        "MapGyver-M01-Present-0123456789abcdef0123456789abcdef0",
        "MapGyver-M01-Present-0123456789ABCDEF0123456789ABCDEF",
        "MapGyver-M01-Present-0123456789abcdef0123456789abcdeg",
        "mapgyver-m01-present-0123456789abcdef0123456789abcdef",
        " MapGyver-M01-Present-0123456789abcdef0123456789abcdef",
        "MapGyver-M01-Present-0123456789abcdef0123456789abcdef\0",
    ] {
        assert!(validate_name(invalid).is_err(), "unexpectedly admitted {invalid:?}");
    }
}

#[test]
fn failed_api_output_is_unavailable_even_when_zero_or_partially_written() {
    let mut properties = Properties::new();
    assert!(statistics(NAME, 5, &properties).unwrap().is_none());
    properties.header.EventsLost = 17;
    properties.header.RealTimeBuffersLost = 19;
    assert!(statistics(NAME, 4201, &properties).unwrap().is_none());
}

#[test]
fn success_requires_the_returned_identity_and_preserves_separate_loss_counts() {
    let mut properties = Properties::new();
    assert!(statistics(NAME, 0, &properties).is_err());
    for (output, input) in properties.logger_name.iter_mut().zip(NAME.encode_utf16()) {
        *output = input;
    }
    properties.header.EventsLost = 7;
    properties.header.LogBuffersLost = 11;
    properties.header.RealTimeBuffersLost = 13;
    let result = statistics(NAME, 0, &properties).unwrap().unwrap();
    assert_eq!(result.returned_session_name, NAME);
    assert_eq!(result.events_lost, 7);
    assert_eq!(result.log_buffers_lost, 11);
    assert_eq!(result.realtime_buffers_lost, 13);
    properties.logger_name[0] = 0xd800;
    assert!(statistics(NAME, 0, &properties).is_err());
}
