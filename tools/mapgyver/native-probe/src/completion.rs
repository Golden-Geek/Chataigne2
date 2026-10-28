//! Borrowed D3D11 timeline completion. Runtime polls never wait; bounded waits are teardown-only.

use crate::{ProbeResult, d3d::Gpu};
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Direct3D11::{D3D11_FENCE_FLAG_NONE, ID3D11Device5, ID3D11DeviceContext4, ID3D11Fence};
use windows::core::Interface;

pub struct Completion<'gpu> {
    gpu: &'gpu Gpu,
    context: ID3D11DeviceContext4,
    fence: ID3D11Fence,
    submitted: u64,
}

impl<'gpu> Completion<'gpu> {
    pub fn new(gpu: &'gpu Gpu) -> ProbeResult<Self> {
        let device = gpu.device.cast::<ID3D11Device5>()?;
        let context = gpu.context.cast::<ID3D11DeviceContext4>()?;
        let mut fence = None;
        // SAFETY: checked modern interfaces, initialized output; owned through windows-rs.
        unsafe {
            device.CreateFence(0, D3D11_FENCE_FLAG_NONE, &mut fence)?;
        }
        Ok(Self {
            gpu,
            context,
            fence: fence.ok_or("D3D11 returned no completion fence")?,
            submitted: 0,
        })
    }

    pub fn submit(&mut self) -> ProbeResult<u64> {
        let value = self.signal()?;
        // SAFETY: submit this context's preceding commands and signal before CPU pacing.
        unsafe { self.context.Flush() };
        Ok(value)
    }

    /// Enqueue completion; the submission owner must flush before CPU pacing.
    pub fn signal(&mut self) -> ProbeResult<u64> {
        self.submitted = self.submitted.checked_add(1).ok_or("completion timeline exhausted")?;
        // SAFETY: the immediate context and fence belong to the borrowed device.
        unsafe {
            self.context.Signal(&self.fence, self.submitted)?;
        }
        Ok(self.submitted)
    }

    pub fn complete(&self, value: u64) -> ProbeResult<bool> {
        // SAFETY: read the owned fence's CPU-visible progress; no GPU wait or implicit flush.
        let completed = unsafe { self.fence.GetCompletedValue() };
        if completed == u64::MAX {
            return Err("D3D11 completion fence reported device removal".into());
        }
        Ok(completed >= value)
    }

    pub fn wait(&self, value: u64) -> ProbeResult<f64> {
        let start = Instant::now();
        while !self.complete(value)? {
            if start.elapsed() >= Duration::from_secs(2) {
                // SAFETY: diagnostic query only; never reset the desktop driver.
                unsafe {
                    self.gpu.device.GetDeviceRemovedReason()?;
                }
                return Err("GPU completion exceeded two-second worker deadline".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }
}
