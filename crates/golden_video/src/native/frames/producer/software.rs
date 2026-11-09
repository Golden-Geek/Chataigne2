//! Software RGBA upload on its source worker, before immutable shared GPU publication.
use super::*;
use crate::image::ImageData;

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
        let lease = Arc::new(budget.0.claim(bytes).ok_or(GpuFrameError::Capacity)?);
        Self::assemble(
            Device::new(adapter)?,
            descriptor,
            Conversion::Software {
                id: uuid::Uuid::new_v4(),
                sequence: 0,
                clock: None,
            },
            lease,
            wake,
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
        let Conversion::Software { id, sequence, clock } = &mut self.conversion else {
            unreachable!()
        };
        let _mark = self
            .completion
            .begin(&self.device, self.pending.as_ref().unwrap().token);
        let begin = clock.and_then(|clock| clock.stamp());
        // SAFETY: validated tight RGBA8 storage, exact descriptor, keyed ownership of a free
        // slot, source-worker context. No earlier GPU reader can still own this slot.
        unsafe {
            self.device.context.UpdateSubresource(
                &self.slots[index].texture.texture,
                0,
                None,
                image.pixels().as_ptr().cast(),
                image.descriptor().extent.width() * 4,
                0,
            );
        }
        *sequence = next;
        self.pending.as_mut().unwrap().software_upload = Some(crate::native::SoftwareUploadTrace {
            stream: *id,
            sequence: next,
            row_pitch: image.descriptor().extent.width() * 4,
            rows: image.descriptor().extent.height(),
            bytes: image.pixels().len() as u64,
            cpu: begin
                .zip(clock.and_then(|clock| clock.stamp()))
                .filter(|(begin, end)| end >= begin)
                .map(|(begin, end)| crate::native::CpuInterval { begin, end }),
        });
        Ok(true)
    }
}
