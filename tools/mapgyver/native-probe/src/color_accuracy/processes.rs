//! Finite authored-image accuracy through real child processes and the existing ingest bridge.

mod child;
mod client;
pub use child::run as child;

use super::{
    DeviceReport,
    protocol::{Report as ChildReport, Role},
};
use crate::{
    ProbeResult,
    d3d::Gpu,
    quad::Quad,
    source_worker::{self, Source},
    spout::Endpoint,
};
use mapgyver_native_probe::{
    color_reference::{Comparison, Fixture},
    presentation::PhysicalRect,
    workload::Scenario,
};
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Serialize)]
pub struct Report {
    pub passed: bool,
    evidence_kind: &'static str,
    scope: &'static str,
    scenario: Scenario,
    fixture: Fixture,
    adapter_luid: String,
    adapter_description: String,
    process_ids: [u32; 4],
    children_reaped: bool,
    source_extents: [[u32; 2]; 2],
    initial_pixel_upload_calls: u32,
    initial_pixel_upload_bytes: u64,
    full_image_readbacks: u32,
    comparison: Comparison,
    senders_unregistered: bool,
    devices: Vec<DeviceReport>,
    source_workers: Vec<source_worker::Report>,
    renderer_source_imports: Vec<source_worker::ImportReport>,
    elapsed_seconds: f64,
    unqualified: [&'static str; 4],
}

pub fn run(adapter: u32, scenario: Scenario, fixture: Fixture) -> ProbeResult<Report> {
    if !scenario.cards() {
        return Err("cross-process accuracy requires q01/q02".into());
    }
    let start = Instant::now();
    let gpu = Gpu::new(adapter)?;
    let nonce = crate::clock::now()?;
    let names = ["a", "b", "output"].map(|r| format!("MapGyver-color-processes-{}-{nonce}-{r}", std::process::id()));
    let scene = scene(adapter, scenario, fixture, &gpu, &names);
    let retirement = gpu.retire();
    let (mut children, finished, senders_unregistered) = scene?;
    let (source_workers, renderer_source_imports): (Vec<_>, Vec<_>) =
        finished.into_iter().map(|f| (f.worker, f.renderer_imports)).unzip();
    let consumer = children.pop().ok_or("consumer report absent")?;
    let comparison = consumer.comparison.ok_or("full-image comparison absent")?;
    let process_ids = [
        children[0].process_id,
        children[1].process_id,
        std::process::id(),
        consumer.process_id,
    ];
    if process_ids
        .iter()
        .enumerate()
        .any(|(i, id)| *id == 0 || process_ids[..i].contains(id))
    {
        return Err("accuracy processes are not independent".into());
    }
    let initial_pixel_upload_bytes = children.iter().map(|c| c.initial_pixel_upload_bytes).sum();
    let full_image_readbacks = consumer.full_image_readbacks;
    let mut devices = children.into_iter().map(|c| c.device).collect::<Vec<_>>();
    devices.push(DeviceReport {
        role: "renderer".into(),
        operations: gpu.allocations.operations(),
        after_fence: gpu.allocations.snapshot(),
        retirement_ms: retirement?,
    });
    devices.push(consumer.device);
    let zero = devices.iter().all(|d| {
        d.after_fence.live_textures == 0
            && d.after_fence.retiring_textures == 0
            && d.after_fence.live_bytes_by_owner == [0; 4]
            && d.after_fence.retiring_bytes_by_owner == [0; 4]
    });
    let bridges_zero = source_workers.iter().all(|s| {
        s.resources.after_fence.live_textures == 0
            && s.resources.after_fence.retiring_textures == 0
            && s.resources.after_fence.live_bytes_by_owner == [0; 4]
            && s.resources.after_fence.retiring_bytes_by_owner == [0; 4]
    });
    Ok(Report {
        passed: comparison.mismatched_pixels == 0 && zero && bridges_zero && senders_unregistered,
        evidence_kind: if fixture == Fixture::Procedural {
            "finite_procedural_card_cross_process_bridge_not_timing_qualification"
        } else {
            "finite_authored_color_alpha_cross_process_bridge_not_timing_qualification"
        },
        scope: "two independent producer processes, selected fixed fixture, existing source-worker NT-handle/keyed-mutex bridge, actual Quad renderer, independent consumer CPU reference; one consumer image readback",
        scenario,
        fixture,
        adapter_luid: gpu.adapter.luid.clone(),
        adapter_description: gpu.adapter.description.clone(),
        process_ids,
        children_reaped: true,
        source_extents: [scenario.extent(), [1280, 720]],
        initial_pixel_upload_calls: if fixture == Fixture::Authored { 2 } else { 0 },
        initial_pixel_upload_bytes,
        full_image_readbacks,
        comparison,
        senders_unregistered,
        devices,
        source_workers,
        renderer_source_imports,
        elapsed_seconds: start.elapsed().as_secs_f64(),
        unqualified: [
            if fixture == Fixture::Authored {
                "procedural card shader/frame-label generation"
            } else {
                "animated/lifecycle procedural-card accuracy beyond the fixed reference inputs"
            },
            "native preview/scanout/DPI alignment",
            "sustained cadence/latency/GPU critical path",
            "physical residency, driver-internal copies and full Q05 retirement",
        ],
    })
}

fn scene(
    adapter: u32,
    scenario: Scenario,
    fixture: Fixture,
    gpu: &Gpu,
    names: &[String; 3],
) -> ProbeResult<(Vec<ChildReport>, Vec<source_worker::Finished>, bool)> {
    let a = client::Child::spawn(adapter, scenario, fixture, Role::SourceA, &names[0], &gpu.adapter.luid)?;
    let b = client::Child::spawn(adapter, scenario, fixture, Role::SourceB, &names[1], &gpu.adapter.luid)?;
    let mut consumer = client::Child::spawn(adapter, scenario, fixture, Role::Consumer, &names[2], &gpu.adapter.luid)?;
    let until = Instant::now() + Duration::from_secs(20);
    let mut sources = [
        Source::spawn(gpu, &names[0], Instant::now()..until)?,
        Source::spawn(gpu, &names[1], Instant::now()..until)?,
    ];
    let deadline = Instant::now() + Duration::from_secs(5);
    while sources.iter().any(|s| s.frame().is_none()) {
        for source in &mut sources {
            source.poll(true)?;
        }
        if Instant::now() >= deadline {
            return Err("accuracy bridge startup timed out".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let [width, height] = scenario.extent();
    let target = gpu.texture(width, height)?;
    gpu.clear(&target, [0.05, 0.05, 0.05, 1.0])?;
    let quad = Quad::new(gpu)?;
    for (channel, source) in sources.iter().enumerate() {
        let frame = source.frame().ok_or("accuracy bridge frame absent")?;
        let expected = if channel == 0 { scenario.extent() } else { [1280, 720] };
        if frame.surface.extent != expected {
            return Err("accuracy bridge extent changed".into());
        }
        quad.draw(
            gpu,
            &target,
            frame.texture(),
            PhysicalRect {
                left: (channel as u32 * width / 2) as i32,
                top: 0,
                width: width / 2,
                height,
            },
        )?;
    }
    gpu.wait_submitted()?;
    let consumer_report = {
        let mut output = Endpoint::sender(gpu, &names[2])?;
        consumer.capture()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            output.send(&target)?;
            gpu.wait_submitted()?;
            if let Some(report) = consumer.poll()? {
                break report;
            }
            if Instant::now() >= deadline {
                return Err("accuracy consumer timed out".into());
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    };
    consumer.join()?;
    // Reader ownership is released before the producer processes stop or bridge devices drain.
    let workers = sources.into_iter().map(Source::finish).collect::<Vec<_>>();
    let children = [a.finish(), b.finish()];
    drop(quad);
    drop(target);
    let source_workers = workers.into_iter().collect::<ProbeResult<Vec<_>>>()?;
    let mut reports = children.into_iter().collect::<ProbeResult<Vec<_>>>()?;
    reports.push(consumer_report);
    let registry = Endpoint::new(gpu)?;
    let mut clean = true;
    for name in names {
        clean &= !registry.sender_exists(name)?;
    }
    Ok((reports, source_workers, clean))
}
