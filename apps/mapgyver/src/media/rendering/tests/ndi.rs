use super::*;
use golden_video::{
    AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor, image::ImageStream, plan::ExternalSource,
};

#[test]
fn ndi_reference_shares_producer_and_old_sender_observations_are_ignored() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::Ndi);
    let name = "Camera (Stage)";
    edit(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: source,
            source: ChannelSource::Ndi {
                sender: Some(name.into()),
            },
        },
    );
    let MediaEditResult::ReferenceCreated { media } = edit(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(source.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let alias = crate::media::resolve_source_node(&engine, media.0).unwrap();
    let routes = [
        ChannelRoute {
            channel: source,
            output: OutputId::new(1).unwrap(),
        },
        ChannelRoute {
            channel: alias,
            output: OutputId::new(2).unwrap(),
        },
    ];
    let capture = capture_channels(&engine, &[source, alias], 32).unwrap();
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(7, 3).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        alpha_mode: AlphaMode::Opaque,
        color_encoding: ColorEncoding::Srgb,
    };
    let stream = ImageStream::new(descriptor);
    let probe = renderer();
    let mut observations = probe.source_observations();
    close(probe);
    let plan = |observations: &SourceObservations| {
        super::super::plan::build(
            &capture,
            super::super::plan::Targets {
                routes: &routes,
                screens: &[],
                preview: None,
            },
            super::super::plan::Previous {
                channels: &HashMap::new(),
                screens: &HashMap::new(),
                preview_images: &Default::default(),
            },
            &mut 0,
            config().plan_limits,
            super::super::plan::Sources {
                observations,
                pictures: None,
                #[cfg(all(windows, feature = "native-video"))]
                shaders: None,
                #[cfg(feature = "video-files")]
                videos: None,
                calibration: &mut Default::default(),
            },
        )
        .unwrap()
        .submission
        .plan
    };
    observations.ndi.insert(
        source,
        super::super::ndi::Observation {
            sender: "Camera (Previous)".into(),
            stream: stream.clone(),
        },
    );
    assert!(plan(&observations).external_textures.is_empty());
    observations.ndi.insert(
        source,
        super::super::ndi::Observation {
            sender: name.into(),
            stream: stream.clone(),
        },
    );
    let ready = plan(&observations);
    let [input] = ready.external_textures.as_slice() else {
        panic!("expected one shared NDI input")
    };
    assert!(matches!(input.source.as_ref(), Some(ExternalSource::Frames(value)) if value.id() == stream.id()));
    assert_eq!(ready.outputs[0].source, ready.outputs[1].source);
    assert_eq!(ready.passes[0].descriptor.extent, descriptor.extent);
}
