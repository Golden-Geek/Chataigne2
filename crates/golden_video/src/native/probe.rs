//! One-shot sender/image observation on an IO worker, independent of render demand.

use std::{ffi::CString, ptr::NonNull, time::Duration};

use golden_io::{PayloadPool, PayloadReservation};
use spout2_sys as sdk;
use windows::core::{Error, HRESULT, Result};

use super::{
    AdapterId,
    completion::EventCompletion,
    device::Device,
    inputs::receiver::{Received, Receiver},
    texture::Texture,
};
use crate::{
    AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor,
    plan::SpoutFrameCount,
    runtime::{CompletionToken, DeviceGeneration},
};

#[cfg(feature = "qualification")]
mod pixels;
#[cfg(feature = "qualification")]
pub use pixels::ProbePixels;

/// Actual endpoint observations; registration alone never proves an image was received.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpoutProbeSnapshot {
    /// Exact sender has nonempty registered storage.
    pub present: bool,
    /// A successful SDK image copy completed on this probe's device.
    pub first_frame: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Endpoint {
    handle: usize,
    width: u32,
    height: u32,
    format: u32,
}
impl Endpoint {
    fn descriptor(self) -> Result<TextureDescriptor> {
        if self.width > 16384 || self.height > 16384 {
            return Err(invalid());
        }
        Ok(TextureDescriptor {
            extent: Extent2D::new(self.width, self.height).map_err(|_| invalid())?,
            format: match self.format {
                28 => PixelFormat::Rgba8Unorm,
                87 => PixelFormat::Bgra8Unorm,
                _ => return Err(invalid()),
            },
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        })
    }
    fn reserve(self, budget: &PayloadPool) -> Result<(TextureDescriptor, PayloadReservation)> {
        let descriptor = self.descriptor()?;
        let bytes = usize::try_from(u64::from(self.width) * u64::from(self.height) * 4).map_err(|_| invalid())?;
        let reservation = budget.try_reserve(bytes).map_err(|_| invalid())?;
        Ok((descriptor, reservation))
    }
}

/// Worker-local one-shot receive. No window, renderer, CPU pixels or process policy is involved.
/// Share a bounded payload pool across probes; its reservation covers the owned receiving image.
/// Drop must run on the owning IO worker: it preserves native resources until GPU completion or
/// observed device removal. A timeout/cancellation never certifies native resource retirement.
pub struct SpoutProbe {
    raw: NonNull<sdk::spout_dx_t>,
    name: CString,
    adapter: AdapterId,
    budget: PayloadPool,
    session: Option<Session>,
}
impl SpoutProbe {
    /// Creates metadata observation only. Graphics storage is allocated lazily for image checks.
    pub fn new(sender: &str, adapter: AdapterId, budget: PayloadPool) -> Result<Self> {
        if sender.is_empty() || sender.len() > 255 {
            return Err(invalid());
        }
        let name = CString::new(sender).map_err(|_| invalid())?;
        // SAFETY: uniquely owned SDK instance, used and destroyed on this caller's IO worker.
        let raw = NonNull::new(unsafe { sdk::spout_dx_create() }).ok_or_else(invalid)?;
        Ok(Self {
            raw,
            name,
            adapter,
            budget,
            session: None,
        })
    }

    fn endpoint(&self) -> Option<Endpoint> {
        let (mut width, mut height, mut format) = (0, 0, 0);
        let mut handle = std::ptr::null_mut();
        // SAFETY: fixed exact selector and initialized metadata outputs; no GPU work.
        let found = unsafe {
            sdk::spout_dx_get_sender_info(
                self.raw.as_ptr(),
                self.name.as_ptr(),
                &mut width,
                &mut height,
                &mut handle,
                &mut format,
            ) != 0
        };
        (found && !handle.is_null() && width != 0 && height != 0).then_some(Endpoint {
            handle: handle as usize,
            width,
            height,
            format,
        })
    }

    /// Poll on the owning IO worker. Busy/unchanged/registration cannot satisfy an image check.
    /// Reconnects and storage changes withdraw the prior observation before another receive.
    pub fn poll(&mut self, require_frame: bool) -> Result<SpoutProbeSnapshot> {
        let endpoint = self.endpoint();
        let mut result = SpoutProbeSnapshot {
            present: endpoint.is_some(),
            first_frame: false,
        };
        if !require_frame {
            return Ok(result);
        }
        if let Some(session) = &mut self.session
            && (Some(session.endpoint) != endpoint || session.device.removed())
        {
            if !session.drained()? {
                return Ok(result);
            }
            self.session.take();
        }
        let Some(endpoint) = endpoint else { return Ok(result) };
        if self.session.is_none() {
            self.session = Some(Session::new(endpoint, &self.name, self.adapter, &self.budget)?);
        }
        result.first_frame = self.session.as_mut().unwrap().poll()? && self.endpoint() == Some(endpoint);
        Ok(result)
    }

    /// Nonblocking completion check before releasing a probe while another owner is still live.
    /// External readiness evidence may arrive while this probe's own copy remains in flight.
    pub fn is_idle(&mut self) -> Result<bool> {
        self.session.as_mut().map_or(Ok(true), Session::drained)
    }
}
impl Drop for SpoutProbe {
    fn drop(&mut self) {
        self.session.take();
        // SAFETY: metadata instance has no submitted GPU work and has exactly one owner.
        unsafe { sdk::spout_dx_destroy(self.raw.as_ptr()) };
    }
}

struct Session {
    #[cfg(feature = "qualification")]
    readback: Option<pixels::Readback>,
    endpoint: Endpoint,
    receiver: Receiver,
    texture: Texture,
    completion: EventCompletion,
    device: Device,
    generation: DeviceGeneration,
    submitted: u64,
    pending: bool,
    copied: bool,
    _reservation: PayloadReservation,
}
impl Session {
    fn new(endpoint: Endpoint, name: &CString, adapter: AdapterId, budget: &PayloadPool) -> Result<Self> {
        let (descriptor, reservation) = endpoint.reserve(budget)?;
        let device = Device::new(adapter)?;
        let receiver = Receiver::new(
            &device.device,
            name.to_str().map_err(|_| invalid())?,
            descriptor,
            SpoutFrameCount::Unversioned,
        )?;
        let completion = EventCompletion::new(&device)?;
        let texture = Texture::new(&device.device, descriptor)?;
        Ok(Self {
            #[cfg(feature = "qualification")]
            readback: None,
            endpoint,
            receiver,
            texture,
            completion,
            device,
            generation: DeviceGeneration::new(uuid::Uuid::new_v4(), 1),
            submitted: 0,
            pending: false,
            copied: false,
            _reservation: reservation,
        })
    }
    fn drained(&mut self) -> Result<bool> {
        if self.device.removed() {
            return Ok(true);
        }
        Ok(self.completion.completed(&self.device, self.generation)?.serial >= self.submitted)
    }
    fn poll(&mut self) -> Result<bool> {
        if self.copied && !self.pending {
            return Ok(!self.device.removed());
        }
        if self.pending {
            if !self.drained()? {
                return Ok(false);
            }
            self.pending = false;
            if self.copied && !self.device.removed() {
                return Ok(true);
            }
        }
        if self.receiver.preflight()?.is_some() {
            return Ok(false);
        }
        self.submitted = self.submitted.checked_add(1).ok_or_else(invalid)?;
        self.pending = true;
        self.copied = false;
        // The guard fences ClearRenderTargetView and any SDK commands, including error paths.
        let guard = self.completion.begin(
            &self.device,
            CompletionToken {
                device: self.generation,
                serial: self.submitted,
            },
        );
        let received = self.receiver.receive(&self.texture);
        drop(guard);
        self.copied = received? == Received::Frame;
        Ok(false)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        while !self.device.removed() && !matches!(self.drained(), Ok(true)) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
fn invalid() -> Error {
    Error::new(
        HRESULT(0x8007_0057_u32 as i32),
        "Spout readiness endpoint, format or bounded image capacity is unavailable",
    )
}

#[cfg(test)]
mod tests;
