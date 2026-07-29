use std::time::Duration;

use crate::SampleRate;

use super::super::frame_at_or_after;

#[test]
fn start_offsets_select_the_first_frame_at_or_after_the_requested_time() {
    let sample_rate = SampleRate::new(48_000).unwrap();

    assert_eq!(frame_at_or_after(Duration::ZERO, sample_rate).unwrap(), 0);
    assert_eq!(frame_at_or_after(Duration::from_millis(1), sample_rate).unwrap(), 48,);
    assert_eq!(frame_at_or_after(Duration::from_nanos(1), sample_rate).unwrap(), 1);
    assert_eq!(frame_at_or_after(Duration::from_nanos(20_834), sample_rate).unwrap(), 2,);
}
