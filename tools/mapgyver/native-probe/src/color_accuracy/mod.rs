//! Finite image-transfer oracle. Never enabled by the scene runner or a timing workload.

pub mod processes;
mod protocol;
mod readback;

use std::time::{Duration, Instant};

use mapgyver_native_probe::{
    allocation::{Operations, Snapshot},
    color_reference::{self, Comparison, Image},
    presentation::PhysicalRect,
    workload::Scenario,
};
use serde::{Deserialize, Serialize};

use crate::{
    ProbeResult,
    d3d::Gpu,
    quad::Quad,
    spout::{Endpoint, ReceivedFrame},
    texture::Texture,
};

#[derive(Serialize, Deserialize)]
pub struct DeviceReport {
    role: String,
    operations: Operations,
    after_fence: Snapshot,
    retirement_ms: f64,
}

#[derive(Serialize)]
pub struct Report {
    pub passed: bool,
    evidence_kind: &'static str,
    scope: &'static str,
    scenario: Scenario,
    adapter_luid: String,
    adapter_description: String,
    source_extents: [[u32; 2]; 2],
    initial_pixel_upload_calls: u32,
    initial_pixel_upload_bytes: u64,
    full_image_readbacks: u32,
    comparison: Comparison,
    senders_unregistered: bool,
    devices: Vec<DeviceReport>,
    elapsed_seconds: f64,
    unqualified: [&'static str; 5],
}

pub fn run(adapter: u32, scenario: Scenario) -> ProbeResult<Report> {
    if !scenario.cards() {
        return Err("color accuracy requires q01 or q02".into());
    }
    let start = Instant::now();
    let gpus = [
        Gpu::new(adapter)?,
        Gpu::new(adapter)?,
        Gpu::new(adapter)?,
        Gpu::new(adapter)?,
    ];
    if gpus.iter().any(|g| g.adapter.luid != gpus[0].adapter.luid) {
        return Err("accuracy devices disagree on adapter".into());
    }
    let images = [Image::card(scenario.extent(), 0)?, Image::card([1280, 720], 1)?];
    let nonce = crate::clock::now()?;
    let names = ["a", "b", "output"].map(|role| format!("MapGyver-color-{}-{nonce}-{role}", std::process::id()));
    let scene = run_scene(&gpus, &images, &names, scenario.extent());
    // Every endpoint/texture has left scope, including on error. Drain every context even if
    // an earlier retirement fails; a diagnostic error cannot skip another device's cleanup.
    let retirements = gpus.iter().map(Gpu::retire).collect::<Vec<_>>();
    let (comparison, senders_unregistered) = scene?;
    let mut devices = Vec::with_capacity(4);
    for ((gpu, retirement), role) in gpus
        .iter()
        .zip(retirements)
        .zip(["source_a", "source_b", "renderer", "consumer"])
    {
        devices.push(DeviceReport {
            role: role.to_owned(),
            operations: gpu.allocations.operations(),
            after_fence: gpu.allocations.snapshot(),
            retirement_ms: retirement?,
        });
    }
    let zero = devices.iter().all(|d| {
        d.after_fence.live_textures == 0
            && d.after_fence.retiring_textures == 0
            && d.after_fence.live_bytes_by_owner == [0; 4]
            && d.after_fence.retiring_bytes_by_owner == [0; 4]
    });
    Ok(Report {
        passed: comparison.mismatched_pixels == 0 && senders_unregistered && zero,
        evidence_kind: "finite_authored_color_alpha_gpu_transfer_not_timing_qualification",
        scope: "two one-time CPU-authored BGRA uploads; four independent D3D11 devices in one process; GPU Spout inputs, actual Quad composite, GPU Spout output, one full-image consumer readback",
        scenario,
        adapter_luid: gpus[0].adapter.luid.clone(),
        adapter_description: gpus[0].adapter.description.clone(),
        source_extents: images.each_ref().map(Image::extent),
        initial_pixel_upload_calls: 2,
        initial_pixel_upload_bytes: images.iter().map(|i| i.pixels().len() as u64 * 4).sum(),
        full_image_readbacks: 1,
        comparison,
        senders_unregistered,
        devices,
        elapsed_seconds: start.elapsed().as_secs_f64(),
        unqualified: [
            "procedural card shader/frame-label generation",
            "native preview/scanout/DPI alignment",
            "sustained cadence/latency/GPU critical path",
            "cross-process full-image accuracy and source-worker bridge",
            "physical residency, driver-internal copies and full Q05 retirement",
        ],
    })
}

fn run_scene(
    gpus: &[Gpu; 4],
    images: &[Image; 2],
    names: &[String; 3],
    extent: [u32; 2],
) -> ProbeResult<(Comparison, bool)> {
    let comparison = {
        let textures = [upload(&gpus[0], &images[0])?, upload(&gpus[1], &images[1])?];
        let mut sources = [
            Endpoint::sender(&gpus[0], &names[0])?,
            Endpoint::sender(&gpus[1], &names[1])?,
        ];
        let mut inputs = [
            Endpoint::receiver(&gpus[2], &names[0])?,
            Endpoint::receiver(&gpus[2], &names[1])?,
        ];
        let a = exchange(&gpus[0], &mut sources[0], &textures[0], &mut inputs[0])?;
        let b = exchange(&gpus[1], &mut sources[1], &textures[1], &mut inputs[1])?;
        for (frame, image) in [(&a, &images[0]), (&b, &images[1])] {
            if [frame.width, frame.height] != image.extent() {
                return Err("accuracy source extent changed".into());
            }
        }
        let target = gpus[2].texture(extent[0], extent[1])?;
        gpus[2].clear(&target, [0.05, 0.05, 0.05, 1.0])?;
        let quad = Quad::new(&gpus[2])?;
        for (channel, frame) in [&a, &b].into_iter().enumerate() {
            quad.draw(
                &gpus[2],
                &target,
                &frame.texture,
                PhysicalRect {
                    left: (channel as u32 * (extent[0] / 2)) as i32,
                    top: 0,
                    width: extent[0] / 2,
                    height: extent[1],
                },
            )?;
        }
        let mut output = Endpoint::sender(&gpus[2], &names[2])?;
        let mut consumer = Endpoint::receiver(&gpus[3], &names[2])?;
        let frame = exchange(&gpus[2], &mut output, &target, &mut consumer)?;
        if [frame.width, frame.height] != extent {
            return Err("accuracy output extent changed".into());
        }
        let pixels = readback::image(&gpus[3], &frame.texture, extent)?;
        color_reference::compare(images, extent, &pixels)?
    };
    let registry = Endpoint::new(&gpus[2])?;
    let mut unregistered = true;
    for name in names {
        unregistered &= !registry.sender_exists(name)?;
    }
    Ok((comparison, unregistered))
}

pub(super) fn upload(gpu: &Gpu, image: &Image) -> ProbeResult<Texture> {
    let [width, height] = image.extent();
    let texture = gpu.texture(width, height)?;
    // SAFETY: one exact, bounded BGRA row-major upload, retained through this synchronous
    // call. Reported separately as initial pixel upload, never as a uniform/renderer upload.
    unsafe {
        gpu.context
            .UpdateSubresource(&*texture, 0, None, image.pixels().as_ptr().cast(), width * 4, 0);
    }
    gpu.wait_submitted()?;
    Ok(texture)
}

fn exchange(
    source_gpu: &Gpu,
    sender: &mut Endpoint<'_>,
    texture: &Texture,
    receiver: &mut Endpoint<'_>,
) -> ProbeResult<ReceivedFrame> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // Metadata discovery may consume the first SDK notification. Republishing the same
        // immutable GPU texture is finite startup recovery; it never uploads pixels again.
        sender.send(texture)?;
        source_gpu.wait_submitted()?;
        if let Some(frame) = receiver.receive()? {
            return Ok(frame);
        }
        if Instant::now() >= deadline {
            return Err("static accuracy endpoint discovery timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
