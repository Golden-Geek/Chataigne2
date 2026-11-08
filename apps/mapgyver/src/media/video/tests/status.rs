use super::*;
use crate::media::protocol::{AssetLocator, ChannelId, ChannelReference, MediaId};
use crate::media::video::protocol::*;
use golden_core::node::NodeUuid;

fn fixture() -> (VideoControl, ChannelReference, AssetLocator) {
    let reference = ChannelReference {
        media: MediaId(NodeUuid(uuid::Uuid::new_v4())),
        channel: ChannelId(NodeUuid(uuid::Uuid::new_v4())),
    };
    let locator = AssetLocator::ProjectRelative {
        path: "clip.mp4".to_owned().try_into().unwrap(),
    };
    (VideoControl::default(), reference, locator)
}
#[test]
fn unconfigured_and_unsupported_builds_do_not_claim_decoded_readiness_or_retry() {
    let (control, reference, file) = fixture();
    assert_eq!(
        control.source_status(reference, &None).unwrap().state,
        VideoSourceState::Unconfigured
    );
    #[cfg(not(feature = "video-files"))]
    {
        let status = control.source_status(reference, &Some(file)).unwrap();
        assert_eq!(status.state, VideoSourceState::Unavailable);
        assert_eq!(status.error.unwrap().kind, VideoSourceErrorKind::Unsupported);
        assert!(!status.retrying);
    }
    #[cfg(feature = "video-files")]
    assert_eq!(
        control.source_status(reference, &Some(file)).unwrap().state,
        VideoSourceState::Loading
    );
}

#[cfg(feature = "video-files")]
#[test]
fn source_diagnostics_distinguish_metadata_pixels_recovery_and_fence_retired_file_and_decoder_owners() {
    use golden_video::{
        AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor,
        image::{ImageData, ImageStream},
    };
    let (control, reference, file) = fixture();
    let binding = uuid::Uuid::new_v4();
    control
        .bind_file(reference.channel, binding, Some(file.clone()))
        .unwrap();
    let generation = control.metadata_files().unwrap()[0].1;
    control
        .metadata_duration(reference.channel, generation, Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        control.source_status(reference, &Some(file.clone())).unwrap().state,
        VideoSourceState::Prepared
    );
    let decoder = uuid::Uuid::new_v4();
    control.claim_channel(reference.channel, decoder, file.clone()).unwrap();
    assert_eq!(
        control.source_status(reference, &Some(file.clone())).unwrap().state,
        VideoSourceState::Loading
    );
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(1, 1).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    };
    let stream = ImageStream::new(descriptor);
    control
        .publish_frame(
            reference.channel,
            decoder,
            control.sample().unwrap(),
            &stream,
            ImageData::new(descriptor, vec![255; 4]).unwrap(),
        )
        .unwrap();
    assert_eq!(
        control.source_status(reference, &Some(file.clone())).unwrap().state,
        VideoSourceState::Ready
    );
    let problem = VideoSourceError::new(VideoSourceErrorKind::Unsupported, "é".repeat(1000));
    assert_eq!(problem.message.chars().count(), 256);
    control
        .source_failure(reference.channel, decoder, problem.clone())
        .unwrap();
    let status = control.source_status(reference, &Some(file.clone())).unwrap();
    assert_eq!(status.state, VideoSourceState::Recovering);
    assert!(status.retrying);
    assert_eq!(status.error, Some(problem.clone()));
    let replacement = uuid::Uuid::new_v4();
    control
        .claim_channel(reference.channel, replacement, file.clone())
        .unwrap();
    control
        .source_failure(reference.channel, decoder, problem.clone())
        .unwrap();
    assert!(
        control
            .source_status(reference, &Some(file.clone()))
            .unwrap()
            .error
            .is_none()
    );
    control.bind_file(reference.channel, binding, None).unwrap();
    control
        .metadata_failure(reference.channel, generation, problem)
        .unwrap();
    assert_eq!(
        control.source_status(reference, &None).unwrap().state,
        VideoSourceState::Unconfigured
    );
}
