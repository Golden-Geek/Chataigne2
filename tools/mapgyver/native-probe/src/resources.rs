//! Sparse worker-side OS observations. DXGI figures are process usage, not texture accounting.

use crate::presenter::{Presenter, Statistics};
use crate::{ProbeResult, d3d::Gpu};
use mapgyver_native_probe::allocation;
use mapgyver_native_probe::resource_log::MemoryInfo;
use serde::Serialize;
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

#[derive(Serialize)]
pub struct ProcessSnapshot {
    pub process_id: u32,
    pub private_bytes: usize,
    pub working_set_bytes: usize,
    pub handle_count: u32,
}

impl ProcessSnapshot {
    pub fn current() -> ProbeResult<Self> {
        // SAFETY: the current-process pseudo handle is borrowed, never closed.
        Self::read(std::process::id(), unsafe { GetCurrentProcess() })
    }

    pub fn child(child: &Child) -> ProbeResult<Self> {
        Self::read(child.id(), HANDLE(child.as_raw_handle()))
    }

    fn read(process_id: u32, handle: HANDLE) -> ProbeResult<Self> {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
        counters.cb = std::mem::size_of_val(&counters) as u32;
        let mut handles = 0;
        // SAFETY: valid borrowed process handle and correctly sized writable stack outputs.
        unsafe {
            GetProcessMemoryInfo(
                handle,
                (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(),
                counters.cb,
            )?;
            GetProcessHandleCount(handle, &mut handles)?;
        }
        Ok(Self {
            process_id,
            private_bytes: counters.PrivateUsage,
            working_set_bytes: counters.WorkingSetSize,
            handle_count: handles,
        })
    }
}

#[derive(Serialize)]
pub struct ResourceSample {
    pub elapsed_seconds: f64,
    pub in_sample_window: bool,
    pub stage: Stage,
    pub renderer: ProcessSnapshot,
    pub sources: [ProcessSnapshot; 2],
    pub observer: ProcessSnapshot,
    pub renderer_gpu_local_nonlocal: [Option<MemoryInfo>; 2],
    pub renderer_allocations: allocation::Snapshot,
    pub presentation_editor_output: [Statistics; 2],
}

#[derive(Clone, Copy, Serialize)]
pub struct Stage {
    pub generation: u32,
    pub settling: bool,
    pub gpu_pipeline: mapgyver_native_probe::gpu_admission::Report,
}

#[derive(Serialize)]
pub struct SceneCheckpoint {
    pub elapsed_seconds: f64,
    pub process: ProcessSnapshot,
    pub allocations: allocation::Snapshot,
    pub gpu_local_nonlocal: [Option<MemoryInfo>; 2],
}

impl SceneCheckpoint {
    pub fn read(elapsed_seconds: f64, gpu: &Gpu) -> ProbeResult<Self> {
        Ok(Self {
            elapsed_seconds,
            process: ProcessSnapshot::current()?,
            allocations: gpu.allocations.snapshot(),
            gpu_local_nonlocal: gpu.memory()?,
        })
    }
}

impl ResourceSample {
    pub fn read(
        elapsed_seconds: f64,
        in_sample_window: bool,
        gpu: &Gpu,
        sources: [&Child; 2],
        observer: &Child,
        presenters: [&Presenter; 2],
        stage: Stage,
    ) -> ProbeResult<Self> {
        Ok(Self {
            elapsed_seconds,
            in_sample_window,
            stage,
            renderer: ProcessSnapshot::current()?,
            sources: [ProcessSnapshot::child(sources[0])?, ProcessSnapshot::child(sources[1])?],
            observer: ProcessSnapshot::child(observer)?,
            renderer_gpu_local_nonlocal: gpu.memory()?,
            renderer_allocations: gpu.allocations.snapshot(),
            presentation_editor_output: [presenters[0].statistics(), presenters[1].statistics()],
        })
    }
}

#[derive(Serialize)]
pub struct ResourceReport {
    pub logical_texture_owners: [&'static str; 4],
    pub before_gpu: ProcessSnapshot,
    pub after_gpu_release: Option<ProcessSnapshot>,
    pub samples: Vec<ResourceSample>,
    pub exclusions: [&'static str; 6],
}

impl ResourceReport {
    pub fn new() -> ProbeResult<Self> {
        Ok(Self {
            logical_texture_owners: allocation::OWNERS,
            before_gpu: ProcessSnapshot::current()?,
            after_gpu_release: None,
            samples: Vec::new(),
            exclusions: [
                "WebView2 subprocess memory and handles",
                "Per-texture residency and physical allocation lifetimes",
                "COM release does not prove driver allocation retirement",
                "Logical BGRA bytes exclude alignment, compression and driver-private allocations",
                "Context fences do not measure the DXGI/DWM display queue or imported source allocation ownership",
                "Raw DXGI statistics do not prove scanout; multi-monitor reliability is unqualified and DwmFlush is not used",
            ],
        })
    }
}
