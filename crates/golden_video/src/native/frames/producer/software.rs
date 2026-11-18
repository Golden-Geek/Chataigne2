//! Software RGBA upload on its source worker, before immutable shared GPU publication.
use super::*;
use crate::image::ImageData;
use crate::native::{CpuInterval, SoftwareUploadTrace, texture::Texture, timing::Clock};

impl GpuFramePublisher {
    /// Admit three RGBA8 slots on one exact adapter before native device creation.
    /// Encoded RGB/alpha bytes are preserved; render sampling applies their declared contract.
    pub fn new_software(
        descriptor: TextureDescriptor,
        adapter: crate::native::AdapterId,
        budget: &GpuFrameBudget,
        wake: Option<golden_io::WorkerNotifier>,
    ) -> Result<Self, GpuFrameError> {
        if descriptor.format != PixelFormat::Rgba8Unorm {
            return Err(GpuFrameError::Unsupported);
        }
        let bytes = descriptor
            .byte_len()
            .ok()
            .and_then(|bytes| bytes.checked_mul(3))
            .ok_or(GpuFrameError::Capacity)?;
        let lease = budget
            .reserve(bytes, AllocationKind::SoftwareProducer)
            .ok_or(GpuFrameError::Capacity)?;
        Self::assemble(
            Device::new(adapter)?,
            descriptor,
            Conversion::Software {
                sequence: 0,
                clock: None,
            },
            lease,
            wake,
            budget,
        )
    }

    /// Enable source API timestamps for qualification. No QPC calls occur while disabled.
    pub fn record_software_upload_timing(&mut self) {
        self.stream.timing = Some(crate::native::timing::Clock::new());
        if let Conversion::Software { clock, .. } = &mut self.conversion {
            *clock = Some(crate::native::timing::Clock::new());
        }
    }

    /// Upload exactly once into a free slot. False retains the prior image without an upload.
    /// Image storage and its caller's CPU admission stay retained through real GPU completion.
    pub fn submit_software(&mut self, image: Arc<ImageData>) -> Result<bool, GpuFrameError> {
        if !matches!(self.conversion, Conversion::Software { .. }) || image.descriptor() != self.stream.descriptor() {
            return Err(GpuFrameError::Layout);
        }
        let Conversion::Software { sequence, .. } = self.conversion else {
            unreachable!()
        };
        let next = sequence.checked_add(1).ok_or(GpuFrameError::Capacity)?;
        let Some(index) = self.begin(Input::Software { _image: image.clone() })? else {
            return Ok(false);
        };
        let Conversion::Software { sequence, clock } = &mut self.conversion else {
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
        let upload = upload_image(
            &self.device.context,
            &self.slots[index].texture,
            &image,
            self.stream.id(),
            next,
            *clock,
        );
        *sequence = next;
        self.pending.as_mut().unwrap().software_upload = Some(upload);
        // Keep optional accounting outside the measured source API interval.
        if let Some(uploads) = &self.uploads {
            uploads.record_upload(super::super::budget::UploadKind::Software, image.pixels().len() as u64);
        }
        Ok(true)
    }
}

pub(super) fn upload_image(
    context: &ID3D11DeviceContext,
    target: &Texture,
    image: &ImageData,
    stream: uuid::Uuid,
    sequence: u64,
    clock: Option<Clock>,
) -> SoftwareUploadTrace {
    let extent = image.descriptor().extent;
    let row_pitch = extent.width() * 4;
    let begin = clock.and_then(Clock::stamp);
    // SAFETY: the caller validated tightly packed source and exact target descriptor
    // and owns the scratch or free keyed slot on this immediate context.
    unsafe {
        context.UpdateSubresource(&target.texture, 0, None, image.pixels().as_ptr().cast(), row_pitch, 0);
    }
    SoftwareUploadTrace {
        stream,
        sequence,
        row_pitch,
        rows: extent.height(),
        bytes: image.pixels().len() as u64,
        cpu: begin
            .zip(clock.and_then(Clock::stamp))
            .filter(|(begin, end)| end >= begin)
            .map(|(begin, end)| CpuInterval { begin, end }),
    }
}
