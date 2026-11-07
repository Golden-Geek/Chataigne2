use super::*;
use crate::media::resolution::*;
use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

fn observed(width: u32, height: u32) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, height).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    }
}

#[test]
fn automatic_and_fixed_extents_keep_availability_separate() {
    let solid = ChannelDefinition::for_media(MediaKind::SolidColor);
    assert_eq!(
        resolve(&solid, SourceObservation::Unavailable).unwrap(),
        ChannelResolution {
            extent: Some(default_canvas()),
            availability: ChannelAvailability::Ready
        }
    );
    let mut spout = ChannelDefinition::for_media(MediaKind::SharedTexture);
    assert_eq!(
        resolve(&spout, SourceObservation::Unavailable).unwrap().availability,
        ChannelAvailability::Unconfigured
    );
    let ChannelSource::Spout { sender } = &mut spout.source else {
        panic!()
    };
    *sender = Some("Input".into());
    let mut actual = observed(640, 480);
    assert_eq!(
        resolve(&spout, SourceObservation::Available(actual)).unwrap().extent,
        Some(actual.extent)
    );
    assert_eq!(resolve(&spout, SourceObservation::Unavailable).unwrap().extent, None);
    spout.size = SizePolicy::Fixed {
        extent: Extent2D::new(128, 256).unwrap(),
    };
    assert_eq!(
        resolve(&spout, SourceObservation::Unavailable).unwrap(),
        ChannelResolution {
            extent: Some(Extent2D::new(128, 256).unwrap()),
            availability: ChannelAvailability::Unavailable
        }
    );
    actual.alpha_mode = AlphaMode::Opaque;
    assert_eq!(
        resolve(&spout, SourceObservation::Available(actual))
            .unwrap()
            .availability,
        ChannelAvailability::Ready
    );
}

#[test]
fn shared_texture_sizing_accepts_native_color_contracts_and_keeps_dimension_bounds() {
    let mut source = ChannelDefinition::for_media(MediaKind::SharedTexture);
    source.source = ChannelSource::Spout {
        sender: Some("Native format input".into()),
    };
    for (format, color_encoding, alpha_mode) in [
        (PixelFormat::Rgba32Float, ColorEncoding::Linear, AlphaMode::Straight),
        (PixelFormat::Rgba16Float, ColorEncoding::Linear, AlphaMode::Straight),
        (PixelFormat::Rgba32Uint, ColorEncoding::Linear, AlphaMode::Straight),
        (PixelFormat::Rgba32Sint, ColorEncoding::Linear, AlphaMode::Straight),
        (PixelFormat::R8Unorm, ColorEncoding::Linear, AlphaMode::Opaque),
        (PixelFormat::Bgrx8Unorm, ColorEncoding::Srgb, AlphaMode::Opaque),
        (PixelFormat::Rgba32Float, ColorEncoding::Linear, AlphaMode::Opaque),
        (PixelFormat::Rgba8Unorm, ColorEncoding::Srgb, AlphaMode::Premultiplied),
    ] {
        let actual = TextureDescriptor {
            extent: Extent2D::new(64, 48).unwrap(),
            format,
            color_encoding,
            alpha_mode,
        };
        for size in [
            SizePolicy::Automatic,
            SizePolicy::Fixed {
                extent: Extent2D::new(128, 96).unwrap(),
            },
        ] {
            source.size = size;
            let resolved = resolve(&source, SourceObservation::Available(actual)).unwrap();
            assert_eq!(resolved.availability, ChannelAvailability::Ready);
            assert_eq!(
                resolved.extent,
                Some(match size {
                    SizePolicy::Automatic => actual.extent,
                    SizePolicy::Fixed { extent } => extent,
                })
            );
            assert_eq!(
                resolve(
                    &source,
                    SourceObservation::Available(TextureDescriptor {
                        extent: Extent2D::new(MAX_CHANNEL_DIMENSION + 1, 48).unwrap(),
                        ..actual
                    })
                )
                .unwrap()
                .availability,
                ChannelAvailability::Incompatible
            );
        }
    }
}

#[test]
fn invalid_extents_selectors_colors_and_oversized_observations_do_not_resolve() {
    assert!(serde_json::from_value::<Extent2D>(serde_json::json!({"width":0,"height":10})).is_err());
    let mut source = ChannelDefinition::for_media(MediaKind::SolidColor);
    source.source = ChannelSource::SolidColor {
        rgba: [f32::NAN, 0.0, 0.0, 1.0],
    };
    assert!(source.validate().is_err());
    source.source = ChannelSource::SolidColor {
        rgba: [0.0, 0.0, 0.0, 2.0],
    };
    assert!(source.validate().is_err());
    source = ChannelDefinition::for_media(MediaKind::SharedTexture);
    let ChannelSource::Spout { sender } = &mut source.source else {
        panic!()
    };
    *sender = Some("Input".into());
    let actual = observed(16385, 2160);
    assert_eq!(
        resolve(&source, SourceObservation::Available(actual))
            .unwrap()
            .availability,
        ChannelAvailability::Incompatible
    );
    for selector in ["".into(), "bad\0sender".into(), "x".repeat(256)] {
        let ChannelSource::Spout { sender, .. } = &mut source.source else {
            panic!()
        };
        *sender = Some(selector);
        assert!(source.validate().is_err());
    }
    source.size = SizePolicy::Fixed {
        extent: Extent2D::new(16385, 1).unwrap(),
    };
    assert!(source.validate().is_err());
}
