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
            if format == PixelFormat::Bgrx8Unorm {
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
}
