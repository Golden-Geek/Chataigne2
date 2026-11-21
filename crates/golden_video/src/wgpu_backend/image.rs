use crate::image::ImageData;
use crate::plan::SampleFilter;
use crate::runtime::backend::BackendError;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

use super::pipeline::{self, Pipelines};

pub(super) struct Upload {
    extent: Extent2D,
    staging: wgpu::Buffer,
    encoded: wgpu::Texture,
    linear: wgpu::TextureView,
    source: wgpu::BindGroup,
}

impl Upload {
    pub fn accepts(descriptor: TextureDescriptor) -> bool {
        descriptor.format == PixelFormat::Rgba8Unorm
            && descriptor.color_encoding == ColorEncoding::Srgb
            && descriptor.alpha_mode == AlphaMode::Straight
    }

    pub fn staging_bytes(extent: Extent2D) -> u64 {
        u64::from(row_pitch(extent)) * u64::from(extent.height())
    }

    pub fn resource_bytes(extent: Extent2D) -> u64 {
        // Straight sRGBA8 input plus a filterable, premultiplied linear RGBA16F texture.
        Self::staging_bytes(extent) + u64::from(extent.width()) * u64::from(extent.height()) * 12
    }

    pub fn new(gpu: &wgpu::Device, pipelines: &Pipelines, image: &ImageData) -> Result<Self, BackendError> {
        let extent = image.descriptor().extent;
        let staging = pipeline::upload_buffer(gpu, "Golden Video image upload", Self::staging_bytes(extent));
        {
            let mut mapped = staging
                .get_mapped_range_mut(..)
                .map_err(|_| BackendError::SubmissionFailed)?;
            let row_bytes = extent.width() as usize * 4;
            let pitch = row_pitch(extent) as usize;
            for (row, source) in image.pixels().chunks_exact(row_bytes).enumerate() {
                let offset = row * pitch;
                mapped.slice(offset..offset + row_bytes).copy_from_slice(source);
            }
        }
        staging.unmap();

        let encoded = gpu.create_texture(&wgpu::TextureDescriptor {
            label: Some("Golden Video straight sRGBA8 image"),
            size: pipeline::size(extent),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let source = pipelines.bind(
            gpu,
            &encoded.create_view(&wgpu::TextureViewDescriptor::default()),
            SampleFilter::Nearest,
        );
        let linear = pipeline::texture(
            gpu,
            extent,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::TEXTURE_BINDING,
        )
        .create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Self {
            extent,
            staging,
            encoded,
            linear,
            source,
        })
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.linear
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, pipelines: &Pipelines) {
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &self.staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_pitch(self.extent)),
                    rows_per_image: None,
                },
            },
            self.encoded.as_image_copy(),
            pipeline::size(self.extent),
        );
        pipeline::sample(encoder, &self.linear, &pipelines.premultiply, &self.source);
    }
}

fn row_pitch(extent: Extent2D) -> u32 {
    (extent.width() * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
}
