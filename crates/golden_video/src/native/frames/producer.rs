//! Worker-local file conversion, with completion before immutable shared publication.
use super::{Fault, GpuFrameBudget, GpuFrameStream, GpuImage};
use crate::native::{
    AllocationKind,
    completion::FenceCompletion,
    device::Device,
    inputs::{
        surface::Slot,
        video::{Decoder, Format, Image},
    },
};
use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{
    AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor,
    decode::{D3d11Frame, D3d11Storage, DecodeError, HapFrame},
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
    core::{Error, HRESULT, Interface},
};
mod diagnostics;
mod software;
mod submission;

/// Source-local conversion/admission failure, separate from renderer state.
#[derive(Debug, thiserror::Error)]
pub enum GpuFrameError {
    /// Frame interpretation or native storage is outside the implemented SDR contract.
    #[error("unsupported Video GPU color interpretation or storage")]
    Unsupported,
    /// A fixed source contract changed and requires a new admitted producer.
    #[error("Video GPU source contract changed")]
    Layout,
    /// Shared conversion/bridge storage is still retained at capacity.
    #[error("Video GPU bridge storage capacity retained")]
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
    token: CompletionToken,
    software_upload: Option<crate::native::SoftwareUploadTrace>,
    traced: bool,
    _input: Input,
}
pub(super) enum Input {
    Software {
        _image: Arc<crate::image::ImageData>,
    },
    Hap {
        _frame: HapFrame,
    },
    Decoded {
        _frame: D3d11Frame,
    },
    #[cfg(test)]
    Texture {
        _texture: ID3D11Texture2D,
    },
}
enum Conversion {
    Software {
        sequence: u64,
        clock: Option<crate::native::timing::Clock>,
    },
    Video {
        interpretation: u32,
        storage: D3d11Storage,
        input: Image,
        shader: Decoder,
    },
    Hap(super::hap::HapImage),
}
/// Fixed three-slot GPU conversion/upload owner. No readback or growing frame queue.
/// Call submit/poll and drop only on the file worker; Drop drains real GPU completion.
pub struct GpuFramePublisher {
    stream: GpuFrameStream,
    fault: Arc<Fault>,
    conversion: Conversion,
    pending: Option<Pending>,
    slots: Vec<Slot>,
    completion: FenceCompletion,
    generation: DeviceGeneration,
    serial: u64,
    diagnostics: Option<diagnostics::ProducerDiagnostics>,
    device: Device,
    _lease: Arc<dyn Send + Sync>,
    uploads: Option<GpuFrameBudget>,
}
impl GpuFramePublisher {
    /// Select timestamp-based presentation before sharing or publishing the source stream.
    pub fn set_presentation_clock(&mut self, clock: Arc<dyn super::GpuFrameClock>) -> Result<(), GpuFrameError> {
        if Arc::strong_count(&self.stream.state) != 1
            || self.stream.state.lock().map_err(|_| GpuFrameError::Layout)?.sequence != 0
        {
            return Err(GpuFrameError::Layout);
        }
        self.stream.clock = Some(clock);
        Ok(())
    }

    /// Admit conversion/bridge storage on the decoded frame's exact device.
    /// The decoder's pool reservation already covers the retained input frame.
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
        let lease = budget
            .reserve(bytes, AllocationKind::HardwareProducer)
            .ok_or(GpuFrameError::Capacity)?;
        let input = Image::new(&device, descriptor, format)?;
        let shader = Decoder::with_interpretation(&device.device, format, descriptor, interpretation)?;
        Self::assemble(
            device,
            descriptor,
            Conversion::Video {
                interpretation,
                storage,
                input,
                shader,
            },
            lease,
            wake,
            budget.upload_accounting(),
        )
    }
    /// Upload/sample reference-decoded HAP blocks on one exact adapter. No CPU pixel decode.
    pub fn new_hap(
        frame: &HapFrame,
        adapter: crate::native::AdapterId,
        budget: &GpuFrameBudget,
        wake: Option<golden_io::WorkerNotifier>,
    ) -> Result<Self, GpuFrameError> {
        let descriptor = TextureDescriptor {
            extent: frame.extent(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        };
        let bytes = descriptor
            .byte_len()
            .ok()
            .and_then(|rgba| rgba.checked_mul(3))
            .and_then(|rgba| rgba.checked_add(frame.blocks().len() as u64))
            .and_then(|bytes| bytes.checked_add(16))
            .ok_or(GpuFrameError::Capacity)?;
        let lease = budget
            .reserve(bytes, AllocationKind::HapProducer)
            .ok_or(GpuFrameError::Capacity)?;
        let device = Device::new(adapter)?;
        let input = super::hap::HapImage::new(&device, frame)?;
        Self::assemble(
            device,
            descriptor,
            Conversion::Hap(input),
            lease,
            wake,
            budget.upload_accounting(),
        )
    }
    fn assemble(
        device: Device,
        descriptor: TextureDescriptor,
        conversion: Conversion,
        lease: Arc<dyn Send + Sync>,
        wake: Option<golden_io::WorkerNotifier>,
        uploads: Option<GpuFrameBudget>,
    ) -> Result<Self, GpuFrameError> {
        let slots = (0..3)
            .map(|index| Slot::new_leased(&device, descriptor, index, Some(lease.clone())))
            .collect::<windows::core::Result<Vec<_>>>()?;
        let completion = FenceCompletion::new(&device)?;
        Ok(Self {
            stream: GpuFrameStream::new(descriptor, device.adapter),
            fault: Arc::new(Fault {
                code: AtomicI32::new(0),
                wake,
            }),
            conversion,
            pending: None,
            slots,
            completion,
            generation: DeviceGeneration::new(uuid::Uuid::new_v4(), 1),
            serial: 0,
            diagnostics: None,
            device,
            _lease: lease,
            uploads,
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
        let Conversion::Video {
            storage,
            interpretation: flags,
            ..
        } = &self.conversion
        else {
            return Err(GpuFrameError::Layout);
        };
        // Same adapter is insufficient for an immediate-context copy. A reopened decoder
        // on another device needs a new publisher, even if its LUID/format are unchanged.
        let owner = unsafe { frame.texture.GetDevice()? };
        if owner.as_raw() != self.device.device.as_raw() {
            return Err(GpuFrameError::Layout);
        }
        if frame.adapter() != self.device.adapter
            || frame.extent() != self.stream.descriptor().extent
            || frame.storage() != *storage
            || interpretation(frame)? != *flags
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
        if !matches!(self.conversion, Conversion::Video { .. }) {
            return Err(GpuFrameError::Layout);
        }
        let Some(index) = self.begin(input)? else {
            return Ok(false);
        };
        let Conversion::Video { input, shader, .. } = &self.conversion else {
            unreachable!()
        };
        let token = self.pending.as_ref().unwrap().token;
        let _cpu = self
            .diagnostics
            .as_ref()
            .map(|diagnostics| diagnostics.cpu_scope(token));
        let _mark = self.completion.begin(&self.device, token);
        let _gpu = self
            .diagnostics
            .as_mut()
            .map(|diagnostics| diagnostics.gpu.begin(&self.device.context, token));
        let extent = self.stream.descriptor().extent;
        unsafe {
            self.device.context.CopySubresourceRegion(
                &input.texture,
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
        shader.draw(&self.device.context, input, &self.slots[index].texture);
        Ok(true)
    }
    /// At most one HAP upload/conversion; held GPU consumers keep the previous image.
    pub fn submit_hap(&mut self, frame: &HapFrame) -> Result<bool, GpuFrameError> {
        let Conversion::Hap(input) = &self.conversion else {
            return Err(GpuFrameError::Layout);
        };
        if input.format != frame.format() || frame.extent() != self.stream.descriptor().extent {
            return Err(GpuFrameError::Layout);
        }
        let Some(index) = self.begin(Input::Hap { _frame: frame.clone() })? else {
            return Ok(false);
        };
        let Conversion::Hap(input) = &self.conversion else {
            unreachable!()
        };
        let token = self.pending.as_ref().unwrap().token;
        let _cpu = self
            .diagnostics
            .as_ref()
            .map(|diagnostics| diagnostics.cpu_scope(token));
        let _mark = self.completion.begin(&self.device, token);
        let _gpu = self
            .diagnostics
            .as_mut()
            .map(|diagnostics| diagnostics.gpu.begin(&self.device.context, token));
        input.upload_draw(&self.device, frame, &self.slots[index].texture)?;
        if let Some(uploads) = &self.uploads {
            uploads.record_upload(super::budget::UploadKind::Hap, frame.blocks().len() as u64);
        }
        Ok(true)
    }
    fn begin(&mut self, input: Input) -> Result<Option<usize>, GpuFrameError> {
        self.healthy()?;
        if self.pending.is_some() {
            return Ok(None);
        }
        if let Some(diagnostics) = &mut self.diagnostics {
            diagnostics.poll(&self.device.context);
        }
        let serial = self.serial.checked_add(1).ok_or(GpuFrameError::Capacity)?;
        let mut free = None;
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.acquire_free()? {
                free = Some(index);
                break;
            }
        }
        let Some(index) = free else {
            return Ok(None);
        };
        self.pending = Some(Pending {
            slot: index,
            token: CompletionToken {
                device: self.generation,
                serial,
            },
            software_upload: None,
            traced: false,
            _input: input,
        });
        self.serial = serial;
        Ok(Some(index))
    }
    fn completed(&mut self) -> windows::core::Result<bool> {
        let completed = self.completion.completed(&self.device, self.generation)?;
        if let Some(diagnostics) = &mut self.diagnostics {
            diagnostics.completed = completed;
            diagnostics.poll(&self.device.context);
        }
        Ok(self
            .pending
            .as_ref()
            .is_some_and(|pending| completed.serial >= pending.token.serial))
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
            software_upload: pending.software_upload,
            submission: self.diagnostics.as_ref().map(|_| Arc::new(pending.token)),
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
        if let Some(diagnostics) = &mut self.diagnostics {
            diagnostics.finish(&self.device.context);
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
