//! One-shot sender/image observation on an IO worker, independent of render demand.

use std::{ffi::CString, ptr::NonNull, time::Duration};

use golden_io::{PayloadPool, PayloadReservation};
use spout2_sys as sdk;
use windows::Win32::{Foundation::HANDLE, Graphics::Direct3D11::ID3D11Texture2D};
use windows::core::{Error, HRESULT, Result};

use super::inputs::{capture::Capture, video};
use super::{
    AdapterId,
    completion::FenceCompletion,
    device::Device,
    inputs::receiver::{Received, Receiver},
};
#[cfg(any(test, feature = "qualification"))]
use crate::PixelFormat;
#[cfg(test)]
use crate::{AlphaMode, ColorEncoding};
use crate::{
    Extent2D, TextureDescriptor,
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
    fn extent(self) -> Result<Extent2D> {
        if self.width > 16384 || self.height > 16384 {
            return Err(invalid());
        }
        Extent2D::new(self.width, self.height).map_err(|_| invalid())
    }
    fn descriptor(self) -> Result<TextureDescriptor> {
        super::spout_layout::descriptor(self.extent()?, self.format).ok_or_else(invalid)
    }
    fn reserve(self, budget: &PayloadPool) -> Result<(TextureDescriptor, PayloadReservation)> {
        let descriptor = self.descriptor()?;
        let reservation = reserve_image(
            descriptor,
            super::spout_layout::native_format(self.format).0 as u32,
            budget,
        )?;
        Ok((descriptor, reservation))
    }
}

fn reserve_image(descriptor: TextureDescriptor, native: u32, budget: &PayloadPool) -> Result<PayloadReservation> {
    let bytes = match video::Format::from_native(native) {
        Some(format) => format.image_bytes(descriptor),
        None => descriptor.byte_len(),
    }
    .map_err(|_| invalid())?;
    let bytes = usize::try_from(bytes).map_err(|_| invalid())?;
    budget.try_reserve(bytes).map_err(|_| invalid())
}

/// Worker-local one-shot receive. No window, renderer, CPU pixels or process policy is involved.
/// Share a bounded payload pool across probes; its reservation covers the owned receiving image.
/// Drop must run on the owning IO worker: it preserves native resources until GPU completion or
/// observed device removal. A timeout/cancellation never certifies native resource retirement.
pub struct SpoutProbe {
    raw: NonNull<sdk::spout_dx_t>,
    name: CString,
    adapter: Option<AdapterId>,
    budget: PayloadPool,
    session: Option<Session>,
}
impl SpoutProbe {
    /// Creates metadata observation only. Graphics storage is allocated lazily for image checks.
    pub fn new(sender: &str, adapter: AdapterId, budget: PayloadPool) -> Result<Self> {
        Self::with_adapter(sender, Some(adapter), budget)
    }

    /// Creates a renderer-independent observer. The exact sender's current shared handle selects
    /// its receiving device lazily; candidate devices allocate no receiving images or GPU work.
    /// Endpoint replacement repeats selection after the previous image has fully retired.
    pub fn new_for_sender(sender: &str, budget: PayloadPool) -> Result<Self> {
        Self::with_adapter(sender, None, budget)
    }

    fn with_adapter(sender: &str, adapter: Option<AdapterId>, budget: PayloadPool) -> Result<Self> {
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
    capture: Capture,
    completion: FenceCompletion,
    device: Device,
    generation: DeviceGeneration,
    submitted: u64,
    pending: bool,
    copied: bool,
    _reservation: PayloadReservation,
}
impl Session {
    fn new(endpoint: Endpoint, name: &CString, adapter: Option<AdapterId>, budget: &PayloadPool) -> Result<Self> {
        let extent = endpoint.extent()?;
        let known = (endpoint.format != 0).then(|| endpoint.reserve(budget)).transpose()?;
        let (bound, metadata) = known
            .as_ref()
            .map(|(descriptor, _)| (*descriptor, crate::plan::SpoutMetadata::Declared))
            .unwrap_or((
                crate::plan::SpoutSource::discovered_descriptor(extent, crate::PixelFormat::Bgra8Unorm),
                crate::plan::SpoutMetadata::Discover,
            ));
        let device = match adapter {
            Some(adapter) => Device::new(adapter)?,
            None => sender_device(endpoint)?,
        };
        let mut receiver = Receiver::new(
            &device.device,
            &crate::plan::SpoutSource {
                sender: name.to_str().map_err(|_| invalid())?.into(),
                descriptor: bound,
                metadata,
                poll_interval: Duration::from_millis(16),
                frame_count: SpoutFrameCount::Unversioned,
            },
        )?;
        let native = if known.is_some() {
            super::spout_layout::native_format(endpoint.format).0 as u32
        } else {
            if receiver.preflight()? != Some(Received::Waiting) {
                return Err(invalid());
            }
            receiver.native_storage().ok_or_else(invalid)?
        };
        let (descriptor, reservation) = match known {
            Some(admitted) => admitted,
            None => {
                // The bound above is only an extent hint. Resolve a real foreign alias before
                // charging/allocating any image; format0 never selects byte receiving storage.
                let descriptor = receiver.descriptor().ok_or_else(invalid)?;
                (descriptor, reserve_image(descriptor, native, budget)?)
            }
        };
        let completion = FenceCompletion::new(&device)?;
        let capture = Capture::new(&device, descriptor, native)?;
        Ok(Self {
            #[cfg(feature = "qualification")]
            readback: None,
            endpoint,
            receiver,
            capture,
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
        let received = self.capture.receive(&mut self.receiver);
        drop(guard);
        self.copied = received? == Received::Frame;
        Ok(false)
    }
}

fn sender_device(endpoint: Endpoint) -> Result<Device> {
    for adapter in super::adapters()?.into_iter().filter(|adapter| !adapter.software) {
        let Ok(device) = Device::new(adapter.id) else { continue };
        let mut alias = None;
        // SAFETY: open only an alias of the observed sender-owned legacy shared texture.
        // No GPU commands or receiving storage exist on an unsuccessful candidate device.
        if unsafe {
            device
                .device
                .OpenSharedResource::<ID3D11Texture2D>(HANDLE(endpoint.handle as *mut std::ffi::c_void), &mut alias)
        }
        .is_ok()
            && alias.is_some()
        {
            return Ok(device);
        }
    }
    Err(invalid())
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
