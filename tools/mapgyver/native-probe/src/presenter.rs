use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows::Win32::Foundation::{DXGI_STATUS_OCCLUDED, HANDLE, HWND, RECT, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::core::Interface;

use crate::cadence::Submission;
use crate::{ProbeResult, d3d::Gpu};
use mapgyver_native_probe::allocation::{Lease, Owner, bgra_bytes};
use serde::Serialize;
mod admission;
mod trace;
pub use admission::Report as AdmissionReport;
use admission::{Admission, MAXIMUM_FRAME_LATENCY};
pub use trace::{Report as TraceReport, Role};

pub enum Target {
    Ready(ID3D11Texture2D),
    Busy,
    Unavailable,
}

pub struct Presenter {
    // Caller-owned handle closes before the swapchain COM reference.
    readiness: OwnedHandle,
    chain: IDXGISwapChain2,
    trace: trace::Identity,
    admission: Admission,
    window: HWND,
    size: [u32; 2],
    buffers: Lease,
}

#[derive(Serialize)]
pub struct DisplayStatistics {
    pub present_count: u32,
    pub present_refresh_count: u32,
    pub sync_refresh_count: u32,
    pub sync_qpc_time: i64,
}

#[derive(Serialize)]
pub struct Statistics {
    pub maximum_frame_latency: Result<u32, String>,
    pub admission: AdmissionReport,
    pub last_present_count: Result<u32, String>,
    pub frame: Result<DisplayStatistics, String>,
}

impl Presenter {
    /// Host keeps both native HWNDs alive until this worker drops its presenters.
    pub fn new(gpu: &Gpu, window: usize, role: Role) -> ProbeResult<Self> {
        let begin_qpc = crate::clock::now()?;
        let window = HWND(window as *mut _);
        // SAFETY: HWND belongs to this probe and remains valid through worker completion.
        let chain = unsafe {
            let factory: IDXGIFactory2 = CreateDXGIFactory1()?;
            factory.CreateSwapChainForHwnd(
                &gpu.device,
                window,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: 1,
                    Height: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    Scaling: DXGI_SCALING_STRETCH,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                    AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                    Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
                    ..Default::default()
                },
                None,
                None,
            )?
        };
        let chain = chain.cast::<IDXGISwapChain2>()?;
        let trace = trace::Identity::new(&chain, role, begin_qpc)?;
        // SAFETY: these settings affect only this newly created probe swapchain. The caller
        // owns the returned handle, per DXGI; it is never serialized or sent to the WebView.
        let readiness = unsafe {
            chain.SetMaximumFrameLatency(MAXIMUM_FRAME_LATENCY)?;
            if chain.GetMaximumFrameLatency()? != MAXIMUM_FRAME_LATENCY {
                return Err("DXGI did not retain the requested frame latency".into());
            }
            let handle = chain.GetFrameLatencyWaitableObject();
            if handle.is_invalid() {
                return Err("DXGI frame latency handle absent".into());
            }
            OwnedHandle::from_raw_handle(handle.0)
        };
        Ok(Self {
            readiness,
            chain,
            trace,
            admission: Admission::default(),
            window,
            size: [1, 1],
            buffers: gpu.allocations.allocate(Owner::Swapchain, bgra_bytes(1, 1, 2)?, 2)?,
        })
    }

    pub fn target(&mut self, gpu: &Gpu) -> ProbeResult<Target> {
        let mut rect = RECT::default();
        // SAFETY: host retains the HWND; only query our window, never modify another app.
        unsafe { GetClientRect(self.window, &mut rect)? };
        let size = [rect.right.max(0) as u32, rect.bottom.max(0) as u32];
        if size.contains(&0) {
            return Ok(Target::Unavailable);
        }
        if size != self.size {
            // SAFETY: no back-buffer views survive a frame; Quad clears its context bindings.
            unsafe {
                self.chain.ResizeBuffers(
                    2,
                    size[0],
                    size[1],
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
                )?
            };
            self.size = size;
            self.buffers = gpu
                .allocations
                .allocate(Owner::Swapchain, bgra_bytes(size[0], size[1], 2)?, 2)?;
        }
        let handle = HANDLE(self.readiness.as_raw_handle());
        if !self.admission.ready(|| -> ProbeResult<bool> {
            // SAFETY: owned event handle, zero timeout. A busy editor never blocks the
            // shared render worker or the independent output/Spout paths.
            match unsafe { WaitForSingleObject(handle, 0) } {
                WAIT_OBJECT_0 => Ok(true),
                WAIT_TIMEOUT => Ok(false),
                result if result == windows::Win32::Foundation::WAIT_FAILED => {
                    Err(windows::core::Error::from_win32().into())
                }
                _ => Err("unexpected DXGI readiness wait result".into()),
            }
        })? {
            return Ok(Target::Busy);
        }
        // SAFETY: chain is live and buffer zero is owned until this frame ends.
        Ok(Target::Ready(unsafe { self.chain.GetBuffer(0)? }))
    }

    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    pub fn statistics(&self) -> Statistics {
        // SAFETY: non-mutating counters on our live flip-model chain. No DwmFlush or wait.
        unsafe {
            let mut s = DXGI_FRAME_STATISTICS::default();
            Statistics {
                maximum_frame_latency: self
                    .chain
                    .GetMaximumFrameLatency()
                    .map_err(|e| format!("0x{:08x}", e.code().0 as u32)),
                admission: self.admission.report(),
                last_present_count: self
                    .chain
                    .GetLastPresentCount()
                    .map_err(|e| format!("0x{:08x}", e.code().0 as u32)),
                frame: self
                    .chain
                    .GetFrameStatistics(&mut s)
                    .map(|()| DisplayStatistics {
                        present_count: s.PresentCount,
                        present_refresh_count: s.PresentRefreshCount,
                        sync_refresh_count: s.SyncRefreshCount,
                        sync_qpc_time: s.SyncQPCTime,
                    })
                    .map_err(|e| format!("0x{:08x}", e.code().0 as u32)),
            }
        }
    }

    pub fn admission(&self) -> AdmissionReport {
        self.admission.report()
    }

    /// Call after the last Present, before dropping this presenter. The CLI report owns only
    /// ephemeral identity strings; ETW equality must still be verified against the raw trace.
    pub fn trace(&self) -> ProbeResult<TraceReport> {
        self.trace.finish()
    }

    pub fn present(&mut self) -> ProbeResult<Submission> {
        // SAFETY: nonblocking submission. A slow/occluded editor cannot wait on output cadence.
        let result = unsafe { self.chain.Present(0, DXGI_PRESENT_DO_NOT_WAIT) };
        if result == DXGI_ERROR_WAS_STILL_DRAWING {
            self.admission.presented(Submission::Busy);
            return Ok(Submission::Busy);
        }
        if result == DXGI_STATUS_OCCLUDED {
            self.admission.presented(Submission::Occluded);
            return Ok(Submission::Occluded);
        }
        result.ok()?;
        self.admission.presented(Submission::Accepted);
        Ok(Submission::Accepted)
    }
}
