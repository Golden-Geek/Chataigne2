use super::support::{GpuUse, Sender, readback, wait};
use crate::native::inputs::receiver::{Received, Receiver};
use crate::native::{AdapterId, device::Device, spout_layout, texture::Texture};
use crate::plan::{SpoutFrameCount, SpoutMetadata, SpoutSource};
use crate::{AlphaMode, Extent2D};
use std::time::Duration;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

pub(super) fn verify_replacements(adapter: AdapterId) {
    let name = format!("GoldenLegacyLifetime-{}", uuid::Uuid::new_v4());
    let device = Device::new(adapter).unwrap();
    let extent = Extent2D::new(2, 2).unwrap();
    let mut receiver = Receiver::new(
        &device.device,
        &SpoutSource {
            sender: name.clone(),
            descriptor: spout_layout::descriptor(extent, 21).unwrap(),
            metadata: SpoutMetadata::Discover,
            poll_interval: Duration::from_millis(16),
            frame_count: SpoutFrameCount::Unversioned,
        },
    )
    .unwrap();
    let mut previous = None;
    for (phase, registered) in [21, 22, 21].into_iter().enumerate() {
        let sender = Sender::new(
            adapter,
            &name,
            DXGI_FORMAT_B8G8R8A8_UNORM,
            [0.125 + phase as f32 * 0.125, 0.5, 0.75, 0.5],
        );
        sender.set_registered_format(registered);
        let expected = sender.pixels(4);
        let descriptor = spout_layout::descriptor(extent, registered).unwrap();
        assert_eq!(
            descriptor.alpha_mode,
            if registered == 22 {
                AlphaMode::Opaque
            } else {
                AlphaMode::Straight
            }
        );
        assert_eq!(receiver.preflight().unwrap(), Some(Received::Waiting));
        assert_eq!(receiver.descriptor(), Some(descriptor));
        let capture = Texture::new(&device.device, descriptor).unwrap();
        let mut uses = GpuUse::new(&device);
        uses.submit(|| {
            wait("legacy replacement must copy a real SDK image", || {
                receiver.receive(&capture).unwrap() == Received::Frame
            })
        });
        uses.wait();
        let actual = readback(&device, &capture, 4);
        assert_eq!(actual, expected);
        if let Some(previous) = previous.replace(actual.clone()) {
            assert_ne!(actual, previous);
        }
        drop(sender);
    }
    receiver.disconnect();
    assert_eq!(receiver.descriptor(), None);
    let invalid_sender = Sender::new(
        adapter,
        &name,
        windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R32G32B32A32_FLOAT,
        [0.125, 1.25, -0.25, 0.5],
    );
    invalid_sender.set_registered_format(21);
    assert_eq!(
        receiver.preflight().unwrap(),
        Some(Received::Mismatch),
        "legacy metadata cannot authenticate a foreign float alias"
    );
    assert_eq!(receiver.descriptor(), None);
    let budget = golden_io::PayloadPool::new("invalid-legacy-alias", 2, 32).unwrap();
    let mut probe = crate::native::SpoutProbe::new_for_sender(&name, budget.clone()).unwrap();
    for _ in 0..3 {
        let observed = probe.poll(true).unwrap();
        assert!(
            observed.present && !observed.first_frame,
            "registration alone is not Ready"
        );
    }
    drop(probe);
    assert!(budget.try_reserve(32).is_ok());
    eprintln!(
        "LEGACY_REPLACEMENT {}",
        serde_json::json!({
            "adapter":format!("{adapter:?}"), "registered_formats":[21,22,21], "native_format":87,
            "phases":3, "same_receiver":true, "native_bits_exact":true,
            "straight_opaque_straight":true, "stale_frame_rejected":true,
            "sender_drop_reappearance":true, "gpu_completion":true,
        "wrong_native_alias_rejected":true, "invalid_alias_never_ready":true,
            "scope":"Actual SDK legacy metadata over independent D3D11 BGRA storage; no D3D9 producer-device claim.",
        })
    );
}
