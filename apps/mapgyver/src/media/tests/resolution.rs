use super::*;
use crate::media::resolution::*;
use golden_video::{AlphaMode, Extent2D};

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
    let ChannelSource::Spout { sender, maximum } = &mut spout.source else {
        panic!()
    };
    *sender = Some("Input".into());
    let mut actual = *maximum;
    actual.extent = Extent2D::new(640, 480).unwrap();
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
        ChannelAvailability::Incompatible
    );
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
    let ChannelSource::Spout { sender, maximum } = &mut source.source else {
        panic!()
    };
    *sender = Some("Input".into());
    let mut actual = *maximum;
    actual.extent = Extent2D::new(3840, 2160).unwrap();
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
