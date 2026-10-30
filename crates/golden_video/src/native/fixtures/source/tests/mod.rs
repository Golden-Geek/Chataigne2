use super::{FixtureSource, FixtureSourceConfig, INVALID_ARGUMENT};
use crate::native::AdapterId;
use crate::native::timing::DiagnosticsConfig;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};
use std::num::NonZeroUsize;

#[test]
fn source_rejects_excessive_and_infinite_waits_before_opening_a_device() {
    for send_access_timeout_ms in [3, 67, u32::MAX] {
        let config = FixtureSourceConfig {
            adapter: AdapterId { high: 0, low: 0 },
            descriptor: TextureDescriptor {
                extent: Extent2D::new(64, 64).unwrap(),
                format: PixelFormat::Bgra8Unorm,
                color_encoding: ColorEncoding::Srgb,
                alpha_mode: AlphaMode::Straight,
            },
            source_id: 1,
            instance: uuid::Uuid::new_v4(),
            sender_name: "invalid-wait-must-never-register".into(),
            send_access_timeout_ms,
            diagnostics: DiagnosticsConfig {
                capacity: NonZeroUsize::new(64).unwrap(),
            },
        };
        assert_eq!(FixtureSource::new(config).err().unwrap().code(), INVALID_ARGUMENT);
    }
}
