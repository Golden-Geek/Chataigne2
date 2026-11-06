use super::*;
use crate::plan::{SpoutFrameCount, SpoutMetadata};
use crate::{ColorEncoding, PixelFormat};
use std::time::Duration;
use windows::Win32::Graphics::Dxgi::Common::*;

#[test]
fn xr_registration_remains_exact_despite_copy_compatible_unorm_storage() {
    let extent = Extent2D::new(4, 4).unwrap();
    let xr = DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM;
    let unorm = DXGI_FORMAT_R10G10B10A2_UNORM;
    let actual = descriptor_from_alias(extent, 89, xr).unwrap();
    assert_eq!(actual.format, PixelFormat::Rgb10XrA2Unorm);
    assert_eq!(super::super::texture::format(actual.format), unorm);
    assert_eq!(descriptor_from_alias(extent, 0, xr), Some(actual));
    assert!(descriptor_from_alias(extent, 24, xr).is_none());
    assert!(descriptor_from_alias(extent, 89, unorm).is_none());
    let declared = TextureDescriptor {
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Premultiplied,
        ..actual
    };
    let binding = SpoutSource {
        sender: "XR".into(),
        descriptor: declared,
        metadata: SpoutMetadata::Declared,
        poll_interval: Duration::from_millis(16),
        frame_count: SpoutFrameCount::Unversioned,
    };
    assert_eq!(observed_alias(&binding, extent, 89, xr), Some(declared));
}

#[test]
fn unspecified_registration_requires_a_real_supported_native_layout() {
    let extent = Extent2D::new(4, 4).unwrap();
    assert!(descriptor(extent, 0).is_none());
    let actual = descriptor_from_alias(extent, 0, DXGI_FORMAT_R32G32B32A32_FLOAT).unwrap();
    assert_eq!(actual.format, PixelFormat::Rgba32Float);
    assert_eq!(actual.color_encoding, ColorEncoding::Linear);
    assert_eq!(actual.alpha_mode, AlphaMode::Straight);
    assert_eq!(actual.byte_len().unwrap(), 256);
    assert!(
        descriptor_from_alias(extent, 0, DXGI_FORMAT(21)).is_none(),
        "native DXGI21 is not legacy ARGB metadata"
    );
    assert!(
        descriptor_from_alias(extent, 29, DXGI_FORMAT_R8G8B8A8_UNORM).is_none(),
        "nonzero native registration is exact"
    );
    assert_eq!(
        descriptor_from_alias(extent, 0, DXGI_FORMAT_B8G8R8X8_UNORM)
            .unwrap()
            .alpha_mode,
        AlphaMode::Opaque
    );
    assert_eq!(
        descriptor_from_alias(extent, 0, DXGI_FORMAT_B8G8R8A8_UNORM)
            .unwrap()
            .alpha_mode,
        AlphaMode::Straight
    );
    assert_eq!(
        descriptor_from_alias(extent, 22, DXGI_FORMAT_B8G8R8A8_UNORM)
            .unwrap()
            .alpha_mode,
        AlphaMode::Opaque
    );
}

#[test]
fn resolved_unspecified_layout_keeps_declared_interpretation_and_rejects_bounds_or_format_changes() {
    let extent = Extent2D::new(4, 4).unwrap();
    let mut binding = SpoutSource {
        sender: "Unspecified".into(),
        descriptor: TextureDescriptor {
            extent,
            format: PixelFormat::Rgba32Float,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Premultiplied,
        },
        metadata: SpoutMetadata::Declared,
        poll_interval: Duration::from_millis(16),
        frame_count: SpoutFrameCount::Unversioned,
    };
    assert_eq!(
        observed_alias(&binding, extent, 0, DXGI_FORMAT_R32G32B32A32_FLOAT),
        Some(binding.descriptor)
    );
    assert!(observed_alias(&binding, extent, 0, DXGI_FORMAT_B8G8R8A8_UNORM).is_none());
    assert!(
        observed_alias(
            &binding,
            Extent2D::new(5, 4).unwrap(),
            0,
            DXGI_FORMAT_R32G32B32A32_FLOAT
        )
        .is_none()
    );
    binding.metadata = SpoutMetadata::Discover;
    let actual = observed_alias(&binding, extent, 0, DXGI_FORMAT_R32G32B32A32_FLOAT).unwrap();
    assert_eq!(
        (actual.color_encoding, actual.alpha_mode),
        (ColorEncoding::Linear, AlphaMode::Straight)
    );
}
