use super::*;
use crate::plan::{SpoutFrameCount, SpoutMetadata};
use crate::{ColorEncoding, PixelFormat};
use std::time::Duration;
use windows::Win32::Graphics::Dxgi::Common::*;

#[test]
fn paired_video_admission_requires_even_width_and_defaults_to_opaque_float() {
    for native in [107, 108, 109] {
        let format = DXGI_FORMAT(native);
        for registered in [0, native as u32] {
            let actual = descriptor_from_alias(Extent2D::new(4, 3).unwrap(), registered, format).unwrap();
            assert_eq!(actual.format, PixelFormat::Rgba32Float);
            assert_eq!(actual.color_encoding, ColorEncoding::Linear);
            assert_eq!(actual.alpha_mode, AlphaMode::Opaque);
            let binding = SpoutSource {
                sender: "Paired video".into(),
                descriptor: SpoutSource::discovered_descriptor(actual.extent, PixelFormat::Bgra8Unorm),
                metadata: SpoutMetadata::Discover,
                poll_interval: Duration::from_millis(16),
                frame_count: SpoutFrameCount::Unversioned,
            };
            assert_eq!(
                observed_alias(&binding, actual.extent, registered, format),
                Some(actual)
            );
            assert!(observed_alias(&binding, Extent2D::new(6, 3).unwrap(), registered, format).is_none());
            assert!(descriptor_from_alias(Extent2D::new(3, 2).unwrap(), registered, format).is_none());
        }
        assert!(descriptor(Extent2D::new(3, 2).unwrap(), native as u32).is_none());
        assert!(descriptor_from_alias(Extent2D::new(4, 2).unwrap(), native as u32, DXGI_FORMAT_Y416).is_none());
    }
}

#[test]
fn typeless_defaults_keep_exact_native_registration_and_allow_declared_family_members() {
    let extent = Extent2D::new(4, 4).unwrap();
    for (native, format, view) in [
        (
            DXGI_FORMAT_R32G32B32A32_TYPELESS,
            PixelFormat::Rgba32Float,
            DXGI_FORMAT_R32G32B32A32_FLOAT,
        ),
        (
            DXGI_FORMAT_R16G16B16A16_TYPELESS,
            PixelFormat::Rgba16Float,
            DXGI_FORMAT_R16G16B16A16_FLOAT,
        ),
        (
            DXGI_FORMAT_R32G32_TYPELESS,
            PixelFormat::Rg32Float,
            DXGI_FORMAT_R32G32_FLOAT,
        ),
        (
            DXGI_FORMAT_R10G10B10A2_TYPELESS,
            PixelFormat::Rgb10A2Unorm,
            DXGI_FORMAT_R10G10B10A2_UNORM,
        ),
        (
            DXGI_FORMAT_R8G8B8A8_TYPELESS,
            PixelFormat::Rgba8Unorm,
            DXGI_FORMAT_R8G8B8A8_UNORM,
        ),
        (
            DXGI_FORMAT_R16G16_TYPELESS,
            PixelFormat::Rg16Float,
            DXGI_FORMAT_R16G16_FLOAT,
        ),
        (DXGI_FORMAT_R32_TYPELESS, PixelFormat::R32Float, DXGI_FORMAT_R32_FLOAT),
        (DXGI_FORMAT_R8G8_TYPELESS, PixelFormat::Rg8Unorm, DXGI_FORMAT_R8G8_UNORM),
        (DXGI_FORMAT_R16_TYPELESS, PixelFormat::R16Float, DXGI_FORMAT_R16_FLOAT),
        (DXGI_FORMAT_R8_TYPELESS, PixelFormat::R8Unorm, DXGI_FORMAT_R8_UNORM),
        (
            DXGI_FORMAT_B8G8R8A8_TYPELESS,
            PixelFormat::Bgra8Unorm,
            DXGI_FORMAT_B8G8R8A8_UNORM,
        ),
        (
            DXGI_FORMAT_B8G8R8X8_TYPELESS,
            PixelFormat::Bgrx8Unorm,
            DXGI_FORMAT_B8G8R8X8_UNORM,
        ),
    ] {
        let known = descriptor_from_alias(extent, native.0 as u32, native).unwrap();
        assert_eq!(known.format, format);
        assert_eq!(descriptor_from_alias(extent, 0, native), Some(known));
        assert!(descriptor_from_alias(extent, native.0 as u32, view).is_none());
        assert!(descriptor_from_alias(extent, view.0 as u32, native).is_none());
    }
    for (native, format) in [
        (DXGI_FORMAT_R32G32B32A32_TYPELESS, PixelFormat::Rgba32Uint),
        (DXGI_FORMAT_R32G32B32A32_TYPELESS, PixelFormat::Rgba32Sint),
        (DXGI_FORMAT_R32G32_TYPELESS, PixelFormat::Rg32Uint),
        (DXGI_FORMAT_R32G32_TYPELESS, PixelFormat::Rg32Sint),
        (DXGI_FORMAT_R10G10B10A2_TYPELESS, PixelFormat::Rgb10XrA2Unorm),
    ] {
        let binding = SpoutSource {
            sender: "Typeless".into(),
            descriptor: TextureDescriptor {
                extent,
                format,
                color_encoding: ColorEncoding::Srgb,
                alpha_mode: AlphaMode::Premultiplied,
            },
            metadata: SpoutMetadata::Declared,
            poll_interval: Duration::from_millis(16),
            frame_count: SpoutFrameCount::Unversioned,
        };
        for registration in [0, native.0 as u32] {
            assert_eq!(
                observed_alias(&binding, extent, registration, native),
                Some(binding.descriptor)
            );
            assert!(observed_alias(&binding, Extent2D::new(5, 4).unwrap(), registration, native).is_none());
            assert!(observed_alias(&binding, extent, registration, DXGI_FORMAT_R8G8B8A8_TYPELESS).is_none());
        }
    }
}

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
