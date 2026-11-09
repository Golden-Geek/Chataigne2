use super::*;
use golden_video::decode::DecodeError;

#[test]
fn native_open_falls_back_only_for_an_explicit_unsupported_capability() {
    for error in [DecodeError::UnsupportedCodec, DecodeError::UnsupportedHardware] {
        assert!(matches!(NativeFailure::opening(error), NativeFailure::Fallback(_)));
    }
    for error in [
        DecodeError::Hardware("adapter reset while opening".into()),
        DecodeError::LimitExceeded,
        DecodeError::NotAFile,
        DecodeError::ReopenRequired,
    ] {
        assert!(matches!(NativeFailure::opening(error), NativeFailure::Retry(_)));
    }
}

#[test]
fn runtime_device_reset_and_capacity_recreate_the_native_path() {
    let reset = windows::core::Error::from_hresult(windows::core::HRESULT(0x887A0007u32 as i32));
    for error in [
        GpuFrameError::Native(reset),
        GpuFrameError::Capacity,
        GpuFrameError::Layout,
        GpuFrameError::Decode(DecodeError::ReopenRequired),
    ] {
        let failure: Failure = error.into();
        let expected = failure.0.clone();
        let NativeFailure::Retry(actual) = NativeFailure::playback(failure, false) else {
            panic!("runtime failure downgraded a supported native decoder");
        };
        assert_eq!(actual.0, expected);
    }
}

#[test]
fn unsupported_conversion_can_fall_back_but_hap_keeps_compressed_storage() {
    assert!(matches!(
        NativeFailure::playback(GpuFrameError::Unsupported.into(), false),
        NativeFailure::Fallback(_)
    ));
    assert!(matches!(
        NativeFailure::playback(GpuFrameError::Unsupported.into(), true),
        NativeFailure::Retry(_)
    ));
}
