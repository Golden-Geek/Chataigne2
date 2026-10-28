use super::{
    CAPACITY, Report, Shared,
    surface::{Publication, Slot},
};
use crate::{ProbeResult, child_resources::Recorder, d3d::Gpu, frame_admission::FrameAdmission, spout::Endpoint};
use mapgyver_native_probe::{
    allocation::{Work, bgra_bytes},
    operation_timing::Stamp,
    statistics::Series,
};
use std::sync::{Arc, atomic::Ordering};
use std::time::{Duration, Instant};

pub(super) fn run(
    adapter: u32,
    luid: &str,
    name: &str,
    sample: std::ops::Range<Instant>,
    shared: &Arc<Shared>,
) -> ProbeResult<Report> {
    let gpu = Gpu::new(adapter)?;
    if gpu.adapter.luid != luid {
        return Err("source worker adapter changed".into());
    }
    let mut receiver = Endpoint::receiver(&gpu, name)?;
    let mut resources = Recorder::new(&gpu)?;
    let mut admission = FrameAdmission::new(&gpu)?;
    let mut timing = crate::timing::Timing::traced(&gpu)?;
    let mut slots: Vec<Slot> = Vec::with_capacity(CAPACITY);
    let mut receive_time = Series::default();
    let mut calls = 0;
    let mut publications = 0;
    let mut busy = 0;
    let mut overwritten = 0;
    let mut next = Instant::now();
    while !shared.stop.load(Ordering::Acquire) {
        let now = Instant::now();
        if now >= sample.end {
            // Await the renderer's reader release, without extending the measurement window.
            pace(&mut next);
            continue;
        }
        let collecting = sample.contains(&now);
        let Some(reservation) = admission.reserve(collecting)? else {
            resources.tick(&gpu)?;
            pace(&mut next);
            continue;
        };
        let start = Instant::now();
        timing.begin(
            &gpu,
            start,
            collecting,
            Stamp {
                frame: calls,
                qpc: crate::clock::now()?,
            },
        )?;
        let receive_start = Instant::now();
        let frame = receiver.receive()?;
        receive_time.push(receive_start.elapsed().as_secs_f64() * 1000.0);
        calls += 1;
        if !receiver.connected() {
            shared
                .latest
                .lock()
                .map_err(|_| "source publication poisoned")?
                .publication = None;
        }
        if let Some(frame) = frame {
            let extent = [frame.width, frame.height];
            let mut selected = None;
            for (index, slot) in slots.iter().enumerate() {
                if slot.acquire_free()? {
                    selected = Some(index);
                    break;
                }
            }
            if selected.is_none() && slots.len() < CAPACITY {
                let slot = Slot::new(&gpu, extent)?;
                if !slot.acquire_free()? {
                    return Err("fresh bridge slot not available".into());
                }
                selected = Some(slots.len());
                slots.push(slot);
            }
            if let Some(index) = selected {
                if slots[index].surface.extent != extent {
                    // The old free slot is acquired on this context and has no readers.
                    // Release its key before replacing its complete allocation/NT handle.
                    unsafe {
                        slots[index].mutex.ReleaseSync(0)?;
                    }
                    slots[index] = Slot::new(&gpu, extent)?;
                    if !slots[index].acquire_free()? {
                        return Err("resized bridge slot not available".into());
                    }
                }
                let slot = &slots[index];
                gpu.allocations
                    .work(Work::GpuCopy(bgra_bytes(extent[0], extent[1], 1)?));
                // SAFETY: both textures belong to this source device, match descriptors and
                // the producer owns this slot's keyed mutex. No renderer context is touched.
                unsafe {
                    gpu.context.CopyResource(&*slot.texture, &*frame.texture);
                    gpu.context.Flush();
                    slot.mutex.ReleaseSync(1)?;
                }
                let mut latest = shared.latest.lock().map_err(|_| "source publication poisoned")?;
                overwritten += u64::from(latest.publication.is_some());
                latest.publication = Some(Publication {
                    surface: slot.surface.clone(),
                    batch: calls - 1,
                });
                publications += 1;
            } else {
                busy += 1;
            }
        }
        resources.tick(&gpu)?;
        timing.end_gpu(&gpu);
        admission.submit(reservation)?;
        timing.end_cpu(start);
        pace(&mut next);
    }
    shared
        .latest
        .lock()
        .map_err(|_| "source publication poisoned")?
        .publication = None;
    let peak = slots.len();
    if slots.iter().any(|slot| Arc::strong_count(&slot.surface) != 1) {
        return Err("source worker still has a bridge reader at shutdown".into());
    }
    let before = gpu.allocations.snapshot();
    let timing = timing.finish(&gpu)?;
    drop(slots);
    drop(receiver);
    let resources = resources.finish(&gpu, before)?;
    let gpu_admission = admission.finish_after_fence()?;
    Ok(Report {
        adapter_luid: gpu.adapter.luid.clone(),
        maximum_surfaces: CAPACITY,
        peak_surfaces: peak,
        receive_calls: calls,
        publications,
        pool_busy: busy,
        replaced_publications: overwritten,
        truncated_receive_samples: receive_time.truncated,
        receive_wall_time_all_windows: receive_time.distribution(),
        gpu_admission,
        timing,
        resources,
    })
}

fn pace(next: &mut Instant) {
    *next += Duration::from_nanos(16_666_667);
    if let Some(delay) = next.checked_duration_since(Instant::now()) {
        std::thread::sleep(delay);
    } else {
        *next = Instant::now();
    }
}
