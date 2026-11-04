use windows::Win32::Graphics::Direct3D11::{D3D11_FENCE_FLAG_NONE, ID3D11Device5, ID3D11DeviceContext4, ID3D11Fence};
use windows::core::{Error, HRESULT, Interface, Result};

use super::super::device::Device;
use crate::runtime::{CompletionToken, DeviceGeneration};

/// Copy-only source contexts need an explicit timeline signal. GPU timing queries must
/// never be necessary to make a production source's completion advance promptly.
pub(in crate::native) struct FenceCompletion {
    fence: ID3D11Fence,
    context: ID3D11DeviceContext4,
    pending: [Option<u64>; 2],
    failure: Option<HRESULT>,
}

pub(in crate::native) struct Submission<'a> {
    completion: &'a mut FenceCompletion,
    device: &'a Device,
    token: CompletionToken,
}

impl Drop for Submission<'_> {
    fn drop(&mut self) {
        let slot = self
            .completion
            .pending
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("preflighted source completion slot");
        *slot = Some(self.token.serial);
        // SAFETY: this worker owns the immediate context and fence. Signal follows all
        // source copies; Flush submits the signal before the worker waits or retires.
        unsafe {
            if let Err(error) = self
                .completion
                .context
                .Signal(&self.completion.fence, self.token.serial)
            {
                self.completion.failure = Some(error.code());
            }
            self.device.context.Flush();
        }
    }
}

impl FenceCompletion {
    pub(in crate::native) fn new(device: &Device) -> Result<Self> {
        let native: ID3D11Device5 = device.device.cast()?;
        let context = device.context.cast()?;
        let mut fence = None;
        // SAFETY: cold creation on the selected source device; no shared/native handle escapes.
        unsafe {
            native.CreateFence(0, D3D11_FENCE_FLAG_NONE, &mut fence)?;
        }
        Ok(Self {
            fence: fence.ok_or_else(|| Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?,
            context,
            pending: [None, None],
            failure: None,
        })
    }

    pub(in crate::native) fn begin<'a>(&'a mut self, device: &'a Device, token: CompletionToken) -> Submission<'a> {
        assert!(self.pending.iter().any(Option::is_none), "source completion capacity");
        Submission {
            completion: self,
            device,
            token,
        }
    }

    pub(in crate::native) fn completed(
        &mut self,
        device: &Device,
        generation: DeviceGeneration,
    ) -> Result<CompletionToken> {
        if let Some(error) = self.failure {
            return Err(Error::from_hresult(error));
        }
        // SAFETY: a read-only, nonblocking observation of this worker's timeline.
        let serial = unsafe { self.fence.GetCompletedValue() };
        if serial == u64::MAX {
            unsafe {
                device.device.GetDeviceRemovedReason()?;
            }
            return Err(Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)));
        }
        for slot in &mut self.pending {
            if slot.is_some_and(|value| value <= serial) {
                *slot = None;
            }
        }
        Ok(CompletionToken {
            device: generation,
            serial,
        })
    }
}
