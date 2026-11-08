//! Worker-local file conversion, with completion before immutable shared publication.
use super::{Fault, GpuFrameBudget, GpuFrameStream, GpuImage};
use crate::native::{
    device::Device,
    inputs::{
        budget::Lease,
        surface::Slot,
        video::{Decoder, Format, Image},
    },
};
use crate::{
    AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor,
    decode::{D3d11Frame, D3d11Storage, DecodeError},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};
use windows::{
    Win32::Graphics::Direct3D11::*,
    core::{BOOL, Error, HRESULT, Interface},
};

/// Source-local conversion/admission failure, separate from renderer state.
#[derive(Debug, thiserror::Error)]
pub enum GpuFrameError {
    /// Frame interpretation or native storage is outside the implemented SDR contract.
    #[error("unsupported hardware Video color interpretation or storage")]
    Unsupported,
    /// A fixed source contract changed and requires a new admitted producer.
    #[error("hardware Video source contract changed")]
    Layout,
    /// Shared conversion/bridge storage is still retained at capacity.
    #[error("hardware Video bridge storage capacity retained")]
    Capacity,
    /// Actual decoder-frame retention failed.
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// Exact native allocation/device/ownership failure.
    #[error(transparent)]
    Native(#[from] Error),
}
struct Pending {
    slot: usize,
    _input: Input,
}
pub(super) enum Input {
    Decoded {
        _frame: D3d11Frame,
    },
    #[cfg(test)]
    Texture {
        _texture: ID3D11Texture2D,
    },
}
/// Fixed three-slot GPU converter. No pixel transfer/readback or growing frame queue.
/// Call submit/poll and drop only on the file worker; Drop drains real GPU completion.
pub struct GpuFramePublisher {
    stream: GpuFrameStream,
    fault: Arc<Fault>,
    interpretation: u32,
    storage: D3d11Storage,
    pending: Option<Pending>,
    slots: Vec<Slot>,
    input: Image,
    shader: Decoder,
    query: ID3D11Query,
    device: Device,
    _lease: Arc<Lease>,
}
impl GpuFramePublisher {
    /// Admit conversion/bridge storage on the decoded frame's exact device.
    /// The decoder's separate pool budget must already cover the retained input frame.
    pub fn new(
        frame: &D3d11Frame,
        budget: &GpuFrameBudget,
        wake: Option<golden_io::WorkerNotifier>,
    ) -> Result<Self, GpuFrameError> {
        let interpretation = interpretation(frame)?;
        let device = Device::from_existing(unsafe { frame.texture.GetDevice()? }, frame.adapter())?;
        Self::prepare(device, frame.extent(), frame.storage(), interpretation, budget, wake)
    }
    pub(super) fn prepare(
        device: Device,
        extent: crate::Extent2D,
        storage: D3d11Storage,
        interpretation: u32,
        budget: &GpuFrameBudget,
        wake: Option<golden_io::WorkerNotifier>,
    ) -> Result<Self, GpuFrameError> {
        let format = format(storage);
        let descriptor = TextureDescriptor {
            extent,
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        };
        let bytes = format
            .image_bytes(descriptor)
            .ok()
            .and_then(|raw| {
                descriptor
                    .byte_len()
                    .ok()?
                    .checked_mul(3)?
                    .checked_add(raw)?
                    .checked_add(16)
            })
            .ok_or(GpuFrameError::Capacity)?;
        let lease = Arc::new(budget.0.claim(bytes).ok_or(GpuFrameError::Capacity)?);
        let input = Image::new(&device, descriptor, format)?;
        let shader = Decoder::with_interpretation(&device.device, format, descriptor, interpretation)?;
        let slots = (0..3)
            .map(|index| Slot::new_leased(&device, descriptor, index, Some(lease.clone())))
            .collect::<windows::core::Result<Vec<_>>>()?;
        let mut query = None;
        unsafe {
            device.device.CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: D3D11_QUERY_EVENT,
                    MiscFlags: 0,
                },
                Some(&mut query),
            )?
        };
        Ok(Self {
            stream: GpuFrameStream::new(descriptor, device.adapter),
            fault: Arc::new(Fault {
                code: AtomicI32::new(0),
                wake,
            }),
            interpretation,
            storage,
            pending: None,
            slots,
            input,
            shader,
            query: query.ok_or_else(missing)?,
            device,
            _lease: lease,
        })
    }
    /// Stable immutable source binding, shared by every render consumer/generation.
    pub fn stream(&self) -> GpuFrameStream {
        self.stream.clone()
    }
    /// Whether completion polling is needed before this producer can accept another frame.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    fn healthy(&self) -> Result<(), GpuFrameError> {
        let fault = self.fault.code.load(Ordering::Acquire);
        if fault != 0 {
            return Err(Error::from_hresult(HRESULT(fault)).into());
        }
        unsafe { self.device.device.GetDeviceRemovedReason()? };
        Ok(())
    }
    /// Submit one copy/conversion. False means busy/retained slots; keep the previous image.
    pub fn submit(&mut self, frame: &D3d11Frame) -> Result<bool, GpuFrameError> {
        self.healthy()?;
        // Same adapter is insufficient for an immediate-context copy. A reopened decoder
        // on another device needs a new publisher, even if its LUID/format are unchanged.
        let owner = unsafe { frame.texture.GetDevice()? };
        if owner.as_raw() != self.device.device.as_raw() {
            return Err(GpuFrameError::Layout);
        }
        if frame.adapter() != self.device.adapter
            || frame.extent() != self.stream.descriptor().extent
            || frame.storage() != self.storage
            || interpretation(frame)? != self.interpretation
        {
            return Err(GpuFrameError::Layout);
        }
        self.submit_owned(
            &frame.texture,
            frame.slice,
            Input::Decoded {
                _frame: frame.retain()?,
            },
        )
    }
    pub(super) fn submit_owned(
        &mut self,
        texture: &ID3D11Texture2D,
        slice: u32,
        input: Input,
    ) -> Result<bool, GpuFrameError> {
        self.healthy()?;
        if self.pending.is_some() {
            return Ok(false);
        }
        let mut free = None;
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.acquire_free()? {
                free = Some(index);
                break;
            }
        }
        let Some(index) = free else {
            return Ok(false);
        };
        self.pending = Some(Pending {
            slot: index,
            _input: input,
        });
        // Mark even if a draw panics after queuing work. Pending pins all source storage.
        let _mark = Mark {
            device: &self.device,
            query: &self.query,
        };
        let extent = self.stream.descriptor().extent;
        unsafe {
            self.device.context.CopySubresourceRegion(
                &self.input.texture,
                0,
                0,
                0,
                0,
                texture,
                slice,
                Some(&D3D11_BOX {
                    left: 0,
                    top: 0,
                    front: 0,
                    right: extent.width(),
                    bottom: extent.height(),
                    back: 1,
                }),
            )
        };
        self.shader
            .draw(&self.device.context, &self.input, &self.slots[index].texture);
        Ok(true)
    }
    fn completed(&self) -> windows::core::Result<bool> {
        let mut done = BOOL(0);
        unsafe {
            self.device.context.GetData(
                &self.query,
                Some((&mut done as *mut BOOL).cast()),
                size_of::<BOOL>() as u32,
                D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
            )?
        };
        Ok(done.as_bool())
    }
    /// Observe completion without waiting. Caller fences its media epoch before publishing.
    pub fn poll(&mut self) -> Result<Option<GpuImage>, GpuFrameError> {
        self.healthy()?;
        if self.pending.is_none() || !self.completed()? {
            return Ok(None);
        }
        let pending = self.pending.take().unwrap();
        let slot = &self.slots[pending.slot];
        unsafe { slot.mutex.ReleaseSync(1)? };
        Ok(Some(GpuImage {
            surface: slot.surface.clone(),
            adapter: self.device.adapter,
            fault: self.fault.clone(),
        }))
    }
}
impl Drop for GpuFramePublisher {
    fn drop(&mut self) {
        let _ = self.stream.publish(None);
        while self.pending.is_some() {
            if self.device.removed() || self.completed().is_ok_and(|done| done) {
                if !self.device.removed()
                    && let Some(pending) = self.pending.take()
                {
                    let _ = unsafe { self.slots[pending.slot].mutex.ReleaseSync(0) };
                }
                break;
            }
            // A wait/error never fabricates completion or frees an in-flight pool slot.
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
struct Mark<'a> {
    device: &'a Device,
    query: &'a ID3D11Query,
}
impl Drop for Mark<'_> {
    fn drop(&mut self) {
        unsafe {
            self.device.context.End(self.query);
            self.device.context.Flush();
        }
    }
}
fn format(storage: D3d11Storage) -> Format {
    match storage {
        D3d11Storage::Nv12 => Format::Nv12,
        D3d11Storage::P010 => Format::P010,
    }
}
fn interpretation(frame: &D3d11Frame) -> Result<u32, GpuFrameError> {
    use ffmpeg_next::color::{Range, Space, TransferCharacteristic as Transfer};
    if !matches!(frame.color_space(), Space::BT709 | Space::Unspecified) {
        return Err(GpuFrameError::Unsupported);
    }
    let transfer = match frame.color_transfer() {
        Transfer::BT709 | Transfer::SMPTE170M | Transfer::Unspecified => 0,
        Transfer::IEC61966_2_1 => 2,
        Transfer::Linear => 4,
        _ => return Err(GpuFrameError::Unsupported),
    };
    Ok(transfer | u32::from(frame.color_range() == Range::JPEG))
}
fn missing() -> Error {
    Error::from_hresult(HRESULT(0x8000_4005_u32 as i32))
}
