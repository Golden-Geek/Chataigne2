//! Headless isolation of native ownership, including a fixed ten-second idle drain.

use serde::Serialize;
use std::time::{Duration, Instant};
use windows::core::{IUnknown, Interface};

use crate::{ProbeResult, d3d::Gpu, resources::ProcessSnapshot, spout::Endpoint, timing::Timing};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Case {
    Device,
    DeviceNoMemory,
    Inventory,
    DeviceEmpty,
    ContextFirst,
    FinalFlush,
    Thread,
    TextureOnly,
    ClearOnly,
    QueryOnly,
    Fence,
    Spout,
    Timing,
}

impl std::str::FromStr for Case {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "device" => Ok(Self::Device),
            "device_no_memory" => Ok(Self::DeviceNoMemory),
            "inventory" => Ok(Self::Inventory),
            "device_empty" => Ok(Self::DeviceEmpty),
            "context_first" => Ok(Self::ContextFirst),
            "final_flush" => Ok(Self::FinalFlush),
            "thread" => Ok(Self::Thread),
            "texture_only" => Ok(Self::TextureOnly),
            "clear_only" => Ok(Self::ClearOnly),
            "query_only" => Ok(Self::QueryOnly),
            "fence" => Ok(Self::Fence),
            "spout" => Ok(Self::Spout),
            "timing" => Ok(Self::Timing),
            _ => Err("unknown retirement case; see the native-probe README".into()),
        }
    }
}

#[derive(Serialize)]
pub struct Cycle {
    pub cycle: u32,
    pub elapsed_ms: f64,
    pub retirement_fence_ms: f64,
    pub device_context_reference_observations: [u32; 2],
    pub after_release: ProcessSnapshot,
    pub final_device_reference_observation: Option<u32>,
}

#[derive(Serialize)]
pub struct DrainSample {
    pub elapsed_seconds: f64,
    pub process: ProcessSnapshot,
}

#[derive(Serialize)]
pub struct Report {
    pub evidence_kind: &'static str,
    pub case: Case,
    pub adapter_luid: String,
    pub before: ProcessSnapshot,
    pub cycles: Vec<Cycle>,
    pub idle_drain: Vec<DrainSample>,
    pub reference_count_scope: &'static str,
}

// Counts include this temporary QueryInterface reference. This is a diagnostic observation,
// never a lifetime/safety decision or an attempt to release another owner's reference.
fn observed_refs(value: &impl Interface) -> ProbeResult<u32> {
    let unknown = value.cast::<IUnknown>()?;
    // SAFETY: balanced AddRef/Release on our live interface; the Rust-owned reference stays live.
    Ok(unsafe {
        (unknown.vtable().AddRef)(unknown.as_raw());
        (unknown.vtable().Release)(unknown.as_raw())
    })
}

pub fn run(adapter: u32, case: Case, count: u32) -> ProbeResult<Report> {
    if !(1..=1000).contains(&count) {
        return Err("retirement count must be 1..=1000".into());
    }
    let before = ProcessSnapshot::current()?;
    let mut cycles = Vec::with_capacity(count as usize);
    let mut adapter_luid = None;
    for cycle in 0..count {
        let start = Instant::now();
        let (retirement_fence_ms, refs, luid, final_device_reference_observation) = scope(adapter, case, cycle)?;
        if adapter_luid.as_ref().is_some_and(|expected| expected != &luid) {
            return Err("adapter changed during ownership isolation".into());
        }
        adapter_luid = Some(luid);
        cycles.push(Cycle {
            cycle,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
            retirement_fence_ms,
            device_context_reference_observations: refs,
            after_release: ProcessSnapshot::current()?,
            final_device_reference_observation,
        });
    }
    let start = Instant::now();
    let mut idle_drain = Vec::with_capacity(11);
    for second in 0..=10 {
        let deadline = start + Duration::from_secs(second);
        if let Some(delay) = deadline.checked_duration_since(Instant::now()) {
            std::thread::sleep(delay);
        }
        idle_drain.push(DrainSample {
            elapsed_seconds: start.elapsed().as_secs_f64(),
            process: ProcessSnapshot::current()?,
        });
    }
    Ok(Report {
        evidence_kind: "native_headless_ownership_isolation_not_full_scene_qualification",
        case,
        adapter_luid: adapter_luid.ok_or("no device scope executed")?,
        before,
        cycles,
        idle_drain,
        reference_count_scope: "Each count includes one temporary IUnknown observation reference; diagnostic only",
    })
}

fn scope(adapter: u32, case: Case, cycle: u32) -> ProbeResult<(f64, [u32; 2], String, Option<u32>)> {
    if matches!(case, Case::Thread) {
        return Ok(std::thread::Builder::new()
            .name("isolated-d3d-device".into())
            .spawn(move || scope(adapter, Case::Device, cycle).map_err(|error| error.to_string()))?
            .join()
            .map_err(|_| "device isolation worker panicked")??);
    }
    if matches!(case, Case::Inventory) {
        return Ok((0.0, [0; 2], crate::d3d::adapter_info(adapter)?.luid, None));
    }
    let gpu = Gpu::with_memory_observation(adapter, !matches!(case, Case::DeviceNoMemory))?;
    if matches!(case, Case::QueryOnly) {
        // SAFETY: retain the original event-query diagnostic, independent of runtime fences.
        unsafe {
            gpu.context.ClearState();
        }
        return Ok((
            gpu.wait_submitted_event()?,
            [observed_refs(&gpu.device)?, observed_refs(&gpu.context)?],
            gpu.adapter.luid.clone(),
            None,
        ));
    }
    if matches!(case, Case::DeviceEmpty) {
        return Ok((
            0.0,
            [observed_refs(&gpu.device)?, observed_refs(&gpu.context)?],
            gpu.adapter.luid.clone(),
            None,
        ));
    }
    let texture = gpu.texture(3840, 2160)?;
    if !matches!(case, Case::TextureOnly) {
        gpu.clear(&texture, [0.1, 0.2, 0.3, 1.0])?;
    }
    if matches!(case, Case::TextureOnly | Case::ClearOnly | Case::Fence) {
        let wait = if matches!(case, Case::Fence) {
            let mut fence = crate::completion::Completion::new(&gpu)?;
            let value = fence.submit()?;
            fence.wait(value)?
        } else {
            0.0
        };
        drop(texture);
        // SAFETY: release retained state and flush destruction; these controls do not poll a query.
        unsafe {
            gpu.context.ClearState();
            gpu.context.Flush();
        }
        return Ok((
            wait,
            [observed_refs(&gpu.device)?, observed_refs(&gpu.context)?],
            gpu.adapter.luid.clone(),
            None,
        ));
    }
    match case {
        Case::Device | Case::DeviceNoMemory | Case::ContextFirst | Case::FinalFlush => {}
        Case::Thread => unreachable!("thread case executed in a new worker"),
        Case::TextureOnly | Case::ClearOnly | Case::QueryOnly | Case::Fence => {
            unreachable!("control returned before extra work")
        }
        Case::DeviceEmpty => unreachable!("empty device returned before allocating textures"),
        Case::Inventory => unreachable!("inventory returned before creating a device"),
        Case::Spout => {
            let name = format!("MapGyver-retirement-{}-{cycle}", std::process::id());
            let mut sender = Endpoint::sender(&gpu, &name)?;
            if sender.name() != name {
                return Err("isolation sender collision".into());
            }
            let mut receivers = [Endpoint::receiver(&gpu, &name)?, Endpoint::receiver(&gpu, &name)?];
            for _ in 0..3 {
                sender.send(&texture)?;
                for receiver in &mut receivers {
                    drop(receiver.receive()?);
                }
            }
            drop(receivers);
            drop(sender);
            let directory = Endpoint::new(&gpu)?;
            if directory.sender_exists(&name)? {
                return Err("isolation sender survived retirement".into());
            }
        }
        Case::Timing => {
            let mut timing = Timing::new(&gpu)?;
            for frame in 0..8 {
                let start = Instant::now();
                timing.begin(
                    &gpu,
                    start,
                    true,
                    mapgyver_native_probe::operation_timing::Stamp {
                        frame,
                        qpc: crate::clock::now()?,
                    },
                )?;
                gpu.clear(&texture, [0.2, 0.3, 0.4, 1.0])?;
                timing.end_gpu(&gpu);
                // This diagnostic-only case has no runtime frame-admission owner.
                unsafe {
                    gpu.context.Flush();
                }
                timing.end_cpu(start);
            }
            timing.finish(&gpu)?;
        }
    }
    drop(texture);
    let retirement = gpu.retire()?;
    let refs = [observed_refs(&gpu.device)?, observed_refs(&gpu.context)?];
    if matches!(case, Case::FinalFlush) {
        // SAFETY: the fence query is now released; flush its deferred destruction separately.
        unsafe {
            gpu.context.Flush();
        }
    }
    if matches!(case, Case::ContextFirst) {
        let Gpu {
            device,
            context,
            adapter,
            ..
        } = gpu;
        let unknown = device.cast::<IUnknown>()?;
        drop(context);
        drop(device);
        let release = unknown.vtable().Release;
        let raw = unknown.into_raw();
        // SAFETY: consume exactly the owned observation reference. No Rust wrapper retains it.
        let remaining = unsafe { release(raw) };
        return Ok((retirement, refs, adapter.luid, Some(remaining)));
    }
    Ok((retirement, refs, gpu.adapter.luid.clone(), None))
}
