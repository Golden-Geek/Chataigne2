use std::cell::Cell;

use crate::image::ImageStream;
use crate::runtime::backend::BackendError;

use super::image::ImageTexture;
use super::pipeline::Pipelines;

pub(super) struct Stream {
    source: ImageStream,
    texture: ImageTexture,
    sequence: Cell<Option<u64>>,
}

pub(super) struct Update {
    sequence: u64,
    staging: Option<wgpu::Buffer>,
}

impl Stream {
    pub fn new(gpu: &wgpu::Device, pipelines: &Pipelines, source: ImageStream) -> Self {
        let texture = ImageTexture::new(gpu, pipelines, source.descriptor().extent);
        Self {
            source,
            texture,
            sequence: Cell::new(None),
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.texture.view()
    }

    pub fn prepare(&self, gpu: &wgpu::Device) -> Result<Option<Update>, BackendError> {
        let Some(sample) = self
            .source
            .sample()
            .filter(|sample| Some(sample.sequence) != self.sequence.get())
        else {
            return Ok(None);
        };
        let staging = sample
            .image
            .as_deref()
            .map(|image| ImageTexture::prepare(gpu, image))
            .transpose()?;
        Ok(Some(Update {
            sequence: sample.sequence,
            staging,
        }))
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, pipelines: &Pipelines, update: &Update) {
        self.texture.encode(encoder, pipelines, update.staging.as_ref());
    }

    pub fn submitted(&self, update: Update) {
        self.sequence.set(Some(update.sequence));
    }
}
