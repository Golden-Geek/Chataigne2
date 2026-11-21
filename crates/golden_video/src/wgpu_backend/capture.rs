use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Extent2D;
use crate::plan::SampleFilter;
use crate::readback::{ReadbackFrame, ReadbackSink};
use crate::runtime::PlanIdentity;

use super::{ReadbackOutput, pipeline};

pub(super) struct Capture {
    extent: Extent2D,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    source: wgpu::BindGroup,
    staging: wgpu::Buffer,
    idle: Arc<AtomicBool>,
    frames: ReadbackSink,
}

impl Capture {
    pub fn new(
        device: &wgpu::Device,
        pipelines: &pipeline::Pipelines,
        binding: &ReadbackOutput,
        source: &wgpu::TextureView,
    ) -> Self {
        let extent = binding.extent;
        let target = pipeline::texture(
            device,
            extent,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureUsages::COPY_SRC,
        );
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Golden Video readback"),
            size: Self::staging_bytes(extent),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            extent,
            target,
            view,
            staging,
            source: pipelines.bind(device, source, SampleFilter::Linear),
            idle: Arc::new(AtomicBool::new(true)),
            frames: binding.frames.clone(),
        }
    }

    pub fn staging_bytes(extent: Extent2D) -> u64 {
        row_pitch(extent) * u64::from(extent.height())
    }

    pub fn resource_bytes(extent: Extent2D) -> u64 {
        Self::staging_bytes(extent) + u64::from(extent.width()) * u64::from(extent.height()) * 4
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, pipelines: &pipeline::Pipelines, plan: PlanIdentity) {
        if !self.idle.swap(false, Ordering::AcqRel) {
            return;
        }
        pipeline::sample(encoder, &self.view, &pipelines.capture, &self.source);
        let pitch = row_pitch(self.extent) as u32;
        encoder.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch),
                    rows_per_image: None,
                },
            },
            pipeline::size(self.extent),
        );
        let staging = self.staging.clone();
        let idle = self.idle.clone();
        let frames = self.frames.clone();
        let row_bytes = self.extent.width() as usize * 4;
        let height = self.extent.height() as usize;
        encoder.map_buffer_on_submit(&self.staging, wgpu::MapMode::Read, .., move |result| {
            if result.is_ok() {
                if let Ok(mapped) = staging.get_mapped_range(..) {
                    let mut pixels = Vec::with_capacity(row_bytes * height);
                    for row in mapped.chunks_exact(pitch as usize) {
                        pixels.extend_from_slice(&row[..row_bytes]);
                    }
                    let _ = frames.try_send(ReadbackFrame { plan, pixels });
                }
                staging.unmap();
            }
            idle.store(true, Ordering::Release);
        });
    }
}

fn row_pitch(extent: Extent2D) -> u64 {
    (u64::from(extent.width()) * 4).next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
}
