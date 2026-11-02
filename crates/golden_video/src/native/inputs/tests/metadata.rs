use super::*;
use crate::plan::{SpoutFrameCount, SpoutMetadata, SpoutSource};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT,
};

#[test]
fn exact_native_metadata_admission_discovers_byte_storage_and_rejects_unsupported_or_oversized_sources() {
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
    ] {
        let actual = receiver::layout(&binding, 3840, 2160, storage.0 as u32).unwrap();
        assert_eq!(actual.extent, Extent2D::new(3840, 2160).unwrap());
        assert_eq!(actual.format, format);
        assert_eq!(actual.color_encoding, ColorEncoding::Srgb);
        assert_eq!(actual.alpha_mode, AlphaMode::Straight);
    }
    for (width, height, storage) in [
        (0, 1, DXGI_FORMAT_B8G8R8A8_UNORM),
        (4097, 1, DXGI_FORMAT_B8G8R8A8_UNORM),
        (1, 4097, DXGI_FORMAT_B8G8R8A8_UNORM),
        (1, 1, DXGI_FORMAT_R16G16B16A16_FLOAT),
    ] {
        assert!(receiver::layout(&binding, width, height, storage.0 as u32).is_none());
    }
}
