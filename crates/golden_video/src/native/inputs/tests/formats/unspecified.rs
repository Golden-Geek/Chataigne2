use super::support::{GpuUse, Sender, readback, wait};
use crate::native::inputs::{CopyFrame, SourcePool, SourceState};
use crate::native::{AdapterId, D3d11Control, InputLimits, SpoutProbe, device::Device, spout_layout, texture::Texture};
use crate::plan::{ExternalTexture, PlanLimits, RenderPlan, SpoutFrameCount, SpoutMetadata, SpoutSource};
use crate::runtime::DeviceGeneration;
use crate::{Extent2D, TextureId};
use std::time::Duration;
use windows::Win32::Graphics::Dxgi::Common::*;

pub(super) fn verify_replacements(adapter: AdapterId) {
    let name = format!("GoldenUnspecified-{}", uuid::Uuid::new_v4());
    let id = TextureId::new(1).unwrap();
    let maximum = Extent2D::new(2, 2).unwrap();
    let nominal = spout_layout::descriptor(maximum, 87).unwrap();
    let plan = RenderPlan {
        external_textures: vec![ExternalTexture {
            id,
            descriptor: nominal,
            source: Some(
                SpoutSource {
                    sender: name.clone(),
                    descriptor: nominal,
                    metadata: SpoutMetadata::Discover,
                    poll_interval: Duration::from_millis(16),
                    frame_count: SpoutFrameCount::Unversioned,
                }
                .into(),
            ),
        }],
        ..Default::default()
    }
    .compile_with_source_demands(
        &PlanLimits {
            max_passes: 0,
            max_external_textures: 1,
            max_outputs: 0,
            max_dependencies: 0,
            max_texture_dimension: 8,
            max_texture_bytes: 4096,
        },
        &[id],
    )
    .unwrap();
    let maximum_charge = 256;
    let mut pool = SourcePool::new(
        adapter,
        InputLimits {
            max_sources: 1,
            max_storage_bytes: maximum_charge,
        },
        &D3d11Control::default(),
        None,
        DeviceGeneration::new(uuid::Uuid::new_v4(), 0),
    );
    let budget = pool.storage_budget.clone();
    pool.ensure(&plan).unwrap();
    let demand = pool.demands(&plan);
    let device = Device::new(adapter).unwrap();
    let payload = golden_io::PayloadPool::new("unspecified-live-probe", 4, 192).unwrap();
    let mut probe = SpoutProbe::new_for_sender(&name, payload.clone()).unwrap();
    let mut previous_sequence = 0;
    let mut previous_generation = 0;
    for (phase, (format, extent)) in [
        (DXGI_FORMAT_R32G32B32A32_FLOAT, maximum),
        (DXGI_FORMAT_B8G8R8A8_UNORM, Extent2D::new(1, 1).unwrap()),
        (DXGI_FORMAT_R16G16B16A16_FLOAT, Extent2D::new(2, 1).unwrap()),
    ]
    .into_iter()
    .enumerate()
    {
        let serial = phase as u64 + 1;
        let sender = Sender::new_sized(adapter, &name, format, [0.125, 0.5, 0.75, 0.5], extent);
        sender.set_registered_format(0);
        let descriptor = spout_layout::descriptor_from_alias(extent, 0, format).unwrap();
        let expected = sender.pixels(descriptor.format.bytes_per_pixel() as usize);
        wait(
            "unspecified source worker must rebuild with exact native layout",
            || {
                pool.retire(serial - 1);
                let status = *pool.inputs[&id].shared.status.lock().unwrap();
                status.state == SourceState::Streaming
                    && status.observed_descriptor == Some(descriptor)
                    && status.pool_generation > previous_generation
            },
        );
        previous_generation = pool.inputs[&id].shared.status.lock().unwrap().pool_generation;
        let charge = InputLimits::source_storage_bytes(descriptor).unwrap();
        let remaining = (maximum_charge > charge).then(|| budget.claim(maximum_charge - charge).unwrap());
        assert!(
            budget.claim(1).is_none(),
            "actual source storage must be fully reserved"
        );
        drop(remaining);
        let capture = Texture::new_sampled(&device.device, descriptor).unwrap();
        let mut uses = GpuUse::new(&device);
        uses.submit(|| {
            wait("new worker generation must copy a fresh real frame", || {
                let copied = pool.copy(id, &device, &capture, serial, CopyFrame::default()).unwrap();
                if let Some(copied) = copied
                    && copied.sequence > previous_sequence
                {
                    previous_sequence = copied.sequence;
                    true
                } else {
                    false
                }
            })
        });
        uses.wait();
        assert_eq!(
            readback(&device, &capture, descriptor.format.bytes_per_pixel() as usize),
            expected
        );
        pool.retire(serial);
        wait(
            "retained probe must resolve and receive the current unspecified layout",
            || probe.poll(true).unwrap().first_frame,
        );
        let remaining = payload.try_reserve(192 - expected.len()).unwrap();
        assert!(
            payload.try_reserve(1).is_err(),
            "probe must reserve full actual image storage"
        );
        drop(remaining);
        if phase == 0 {
            let small = golden_io::PayloadPool::new("unspecified-float-must-not-use-byte-hint", 1, 16).unwrap();
            let mut denied = SpoutProbe::new_for_sender(&name, small.clone()).unwrap();
            assert!(
                denied.poll(true).is_err(),
                "float receiving allocation must reject a byte-sized cap"
            );
            assert!(small.try_reserve(16).is_ok());
        }
        drop(sender);
        wait(
            "sender loss must withdraw worker publication and probe readiness",
            || {
                pool.retire(serial);
                let status = *pool.inputs[&id].shared.status.lock().unwrap();
                let observation = probe.poll(true).unwrap();
                status.state == SourceState::WaitingForSender && !observation.present && !observation.first_frame
            },
        );
        assert!(
            payload.try_reserve(192).is_ok(),
            "disconnected probe storage must retire before reappearance"
        );
    }
    drop(probe);
    drop(demand);
    drop(pool);
    assert!(budget.claim(maximum_charge).is_some());
    assert!(payload.try_reserve(192).is_ok());
    eprintln!(
        "UNSPECIFIED_REPLACEMENT {}",
        serde_json::json!({
            "adapter":format!("{adapter:?}"),"registered_format":0,"native_formats":[2,87,10],
            "extents":[[2,2],[1,1],[2,1]],"phases":3,"retained_worker":true,"retained_probe":true,
            "native_bits_exact":true,"new_pool_generations":true,"stale_frames_rejected":true,
            "full_source_probe_reservations":true,"byte_sized_float_cap_rejected":true,
            "loss_withdraws_readiness":true,"sender_reappearance":true,"source_probe_storage_retired":true,
            "gpu_completion":true,
        })
    );
}
