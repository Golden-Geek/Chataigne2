use super::support::{GpuUse, Sender, readback, wait};
use crate::native::inputs::receiver::{Received, Receiver};
use crate::native::{AdapterId, device::Device, texture::Texture};
use crate::plan::{SpoutFrameCount, SpoutMetadata, SpoutSource};
use crate::{Extent2D, PixelFormat};
use std::time::Duration;
use windows::Win32::Graphics::Dxgi::Common::*;

/// Retain one receiver/capture while sender storage changes within one canonical descriptor.
/// This also keeps the old sender alias alive across sender destruction and fresh registration.
pub(super) fn verify_replacements(adapter: AdapterId) {
    for (format, unorm, srgb) in [
        (
            PixelFormat::Rgba8Unorm,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        ),
        (
            PixelFormat::Bgra8Unorm,
            DXGI_FORMAT_B8G8R8A8_UNORM,
            DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        ),
        (
            PixelFormat::Bgrx8Unorm,
            DXGI_FORMAT_B8G8R8X8_UNORM,
            DXGI_FORMAT_B8G8R8X8_UNORM_SRGB,
        ),
    ] {
        let name = format!("GoldenSrgbLifetime-{}", uuid::Uuid::new_v4());
        let device = Device::new(adapter).unwrap();
        let descriptor = SpoutSource::discovered_descriptor(Extent2D::new(2, 2).unwrap(), format);
        let mut receiver = Receiver::new(
            &device.device,
            &SpoutSource {
                sender: name.clone(),
                descriptor,
                metadata: SpoutMetadata::Discover,
                poll_interval: Duration::from_millis(16),
                frame_count: SpoutFrameCount::Unversioned,
            },
        )
        .unwrap();
        let capture = Texture::new(&device.device, descriptor).unwrap();
        let mut previous = None;
        for (phase, storage) in [unorm, srgb, unorm].into_iter().enumerate() {
            let sender = Sender::new(adapter, &name, storage, [0.125 + phase as f32 * 0.125, 0.5, 0.75, 0.5]);
            let expected = sender.pixels(4);
            assert_eq!(
                receiver.preflight().unwrap(),
                Some(Received::Waiting),
                "replace the native endpoint before receiving"
            );
            assert_eq!(
                receiver.descriptor(),
                Some(descriptor),
                "logical descriptor stays unchanged"
            );
            let mut uses = GpuUse::new(&device);
            uses.submit(|| {
                wait("replacement must produce a real SDK frame", || {
                    receiver.receive(&capture).unwrap() == Received::Frame
                });
            });
            uses.wait();
            let actual = readback(&device, &capture, 4);
            assert_eq!(actual, expected, "replacement bytes must match the new native storage");
            if let Some(previous) = previous.replace(actual.clone()) {
                assert_ne!(actual, previous, "replacement must not republish the prior frame");
            }
            drop(sender);
        }
        receiver.disconnect();
        assert_eq!(receiver.descriptor(), None);
        eprintln!(
            "SRGB_REPLACEMENT {}",
            serde_json::json!({
                "adapter": format!("{adapter:?}"), "format":format, "raw_formats":[unorm.0,srgb.0,unorm.0],
                "phases":3, "same_receiver":true, "same_capture":true,
                "descriptor_unchanged":true, "native_bits_exact":true, "stale_frame_rejected":true,
                "sender_drop_reappearance":true, "gpu_completion":true,
            })
        );
    }
}
