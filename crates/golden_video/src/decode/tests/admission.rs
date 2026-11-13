use super::*;

fn actual_limit(decoder: &Decoder) -> i64 {
    // SAFETY: immutable metadata on the live uniquely owned decoder between calls.
    unsafe { (*decoder.decoder.as_ref().unwrap().as_ptr()).max_pixels }
}

#[test]
fn tightening_is_atomic_and_cannot_expand_or_undercut_the_initial_stream() {
    let mut decoder = Decoder::open(&fixture("h264.mp4"), limits()).unwrap();
    let pixels = u64::from(decoder.info().width) * u64::from(decoder.info().height);
    let original = actual_limit(&decoder);
    assert!(matches!(
        decoder.tighten_pixel_limit(0),
        Err(DecodeError::InvalidLimits)
    ));
    assert!(matches!(
        decoder.tighten_pixel_limit(limits().max_pixels + 1),
        Err(DecodeError::InvalidLimits)
    ));
    assert!(matches!(
        decoder.tighten_pixel_limit(pixels - 1),
        Err(DecodeError::LimitExceeded)
    ));
    assert_eq!(actual_limit(&decoder), original);
    decoder.tighten_pixel_limit(pixels).unwrap();
    let allocation = actual_limit(&decoder);
    assert!(allocation >= pixels as i64 && allocation <= original);
    assert_eq!(decoder.limits.max_pixels, pixels);
    assert!(matches!(
        decoder.tighten_pixel_limit(pixels + 1),
        Err(DecodeError::InvalidLimits)
    ));
    assert_eq!(actual_limit(&decoder), allocation);
    let first = decoder.next_frame().unwrap().unwrap();
    assert_eq!(first.software_image().unwrap().width(), decoder.info().width);
}

#[test]
fn tightened_guard_survives_eof_codec_replacement_and_retained_frame_observers() {
    for name in ["h264.mp4", "hevc.mp4", "prores.mov", "prores-odd.mov"] {
        let mut decoder = Decoder::open(&fixture(name), limits()).unwrap();
        let pixels = u64::from(decoder.info().width) * u64::from(decoder.info().height);
        decoder.tighten_pixel_limit(pixels).unwrap();
        let allocation = actual_limit(&decoder);
        let retained = decoder.next_frame().unwrap().unwrap();
        let layout = retained.software_layout().unwrap();
        let mut count = 1;
        while decoder.next_frame().unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 6);
        let old = unsafe { decoder.decoder.as_ref().unwrap().as_ptr() };
        decoder.seek_before(decoder.info().start.unwrap_or(0)).unwrap();
        assert_ne!(unsafe { decoder.decoder.as_ref().unwrap().as_ptr() }, old);
        assert_eq!(actual_limit(&decoder), allocation);
        assert_eq!(decoder.limits.max_pixels, pixels);
        assert_eq!(retained.software_layout(), Some(layout));
        assert_eq!(decoder.next_frame().unwrap().unwrap().timestamp(), retained.timestamp());
    }
}
