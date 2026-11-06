use super::*;
use crate::plan::{SpoutFrameCount, SpoutMetadata, SpoutSource};
use windows::Win32::Graphics::Dxgi::Common::*;

#[test]
fn exact_native_metadata_admission_discovers_byte_and_float_storage_without_changing_precision() {
    let binding = SpoutSource {
        sender: "Exact source".into(),
        metadata: SpoutMetadata::Discover,
        descriptor: TextureDescriptor {
            extent: Extent2D::new(4096, 4096).unwrap(),
            format: PixelFormat::Bgra8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        },
        poll_interval: Duration::from_millis(16),
        frame_count: SpoutFrameCount::Unversioned,
    };
    for (storage, format) in [
        (DXGI_FORMAT_B8G8R8A8_UNORM, PixelFormat::Bgra8Unorm),
        (DXGI_FORMAT_R8G8B8A8_UNORM, PixelFormat::Rgba8Unorm),
        (DXGI_FORMAT_R16G16B16A16_FLOAT, PixelFormat::Rgba16Float),
        (DXGI_FORMAT_R32G32B32A32_FLOAT, PixelFormat::Rgba32Float),
        (DXGI_FORMAT_R16G16B16A16_UNORM, PixelFormat::Rgba16Unorm),
        (DXGI_FORMAT_R16G16B16A16_SNORM, PixelFormat::Rgba16Snorm),
        (DXGI_FORMAT_R8G8B8A8_SNORM, PixelFormat::Rgba8Snorm),
        (DXGI_FORMAT_R10G10B10A2_UNORM, PixelFormat::Rgb10A2Unorm),
        (DXGI_FORMAT_B8G8R8X8_UNORM, PixelFormat::Bgrx8Unorm),
        (DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, PixelFormat::Rgba8Unorm),
        (DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, PixelFormat::Bgra8Unorm),
        (DXGI_FORMAT_B8G8R8X8_UNORM_SRGB, PixelFormat::Bgrx8Unorm),
        (DXGI_FORMAT_R8_UNORM, PixelFormat::R8Unorm),
        (DXGI_FORMAT_R8_SNORM, PixelFormat::R8Snorm),
        (DXGI_FORMAT_R8G8_UNORM, PixelFormat::Rg8Unorm),
        (DXGI_FORMAT_R8G8_SNORM, PixelFormat::Rg8Snorm),
        (DXGI_FORMAT_R16_UNORM, PixelFormat::R16Unorm),
        (DXGI_FORMAT_R16_SNORM, PixelFormat::R16Snorm),
        (DXGI_FORMAT_R16G16_UNORM, PixelFormat::Rg16Unorm),
        (DXGI_FORMAT_R16G16_SNORM, PixelFormat::Rg16Snorm),
        (DXGI_FORMAT_R16_FLOAT, PixelFormat::R16Float),
        (DXGI_FORMAT_R16G16_FLOAT, PixelFormat::Rg16Float),
        (DXGI_FORMAT_R32_FLOAT, PixelFormat::R32Float),
        (DXGI_FORMAT_R11G11B10_FLOAT, PixelFormat::Rg11B10Float),
        (DXGI_FORMAT_A8_UNORM, PixelFormat::A8Unorm),
        (DXGI_FORMAT_R32G32_FLOAT, PixelFormat::Rg32Float),
        (DXGI_FORMAT_B5G6R5_UNORM, PixelFormat::Bgr565Unorm),
        (DXGI_FORMAT_B5G5R5A1_UNORM, PixelFormat::Bgra5551Unorm),
    ] {
        let actual = receiver::layout(&binding, 3840, 2160, storage.0 as u32).unwrap();
        assert_eq!(actual.extent, Extent2D::new(3840, 2160).unwrap());
        assert_eq!(actual.format, format);
        assert_eq!(
            actual.color_encoding,
            match format {
                PixelFormat::Rgba8Unorm | PixelFormat::Bgra8Unorm | PixelFormat::Bgrx8Unorm => ColorEncoding::Srgb,
                _ => ColorEncoding::Linear,
            }
        );
        assert_eq!(
            actual.alpha_mode,
            if !format.has_alpha() {
                AlphaMode::Opaque
            } else {
                AlphaMode::Straight
            }
        );
    }
    for (width, height, storage) in [
        (0, 1, DXGI_FORMAT_B8G8R8A8_UNORM),
        (4097, 1, DXGI_FORMAT_B8G8R8A8_UNORM),
        (1, 4097, DXGI_FORMAT_B8G8R8A8_UNORM),
        (1, 1, DXGI_FORMAT_UNKNOWN),
    ] {
        assert!(receiver::layout(&binding, width, height, storage.0 as u32).is_none());
    }
    for (registered, alpha) in [(21, AlphaMode::Straight), (22, AlphaMode::Opaque)] {
        let actual = receiver::layout(&binding, 3840, 2160, registered).unwrap();
        assert_eq!(actual.format, PixelFormat::Bgra8Unorm);
        assert_eq!(actual.color_encoding, ColorEncoding::Srgb);
        assert_eq!(actual.alpha_mode, alpha);
        assert_eq!(
            crate::native::spout_layout::native_format(registered),
            DXGI_FORMAT_B8G8R8A8_UNORM
        );
        let mut declared = binding.clone();
        declared.metadata = SpoutMetadata::Declared;
        assert_eq!(
            receiver::layout(&declared, 3840, 2160, registered).unwrap().alpha_mode,
            AlphaMode::Straight
        );
    }
}
