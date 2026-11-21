use std::cell::Cell;

use crate::plan::{Layer, SampleFilter, UnitRect};
use crate::runtime::backend::BackendError;
use crate::{Extent2D, LinearColor};

pub(super) struct Pipelines {
    pub capture: wgpu::RenderPipeline,
    pub premultiply: wgpu::RenderPipeline,
    sample_u8: wgpu::RenderPipeline,
    sample_f16: wgpu::RenderPipeline,
    composite_u8: wgpu::RenderPipeline,
    composite_f16: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    layer_layout: wgpu::BindGroupLayout,
    nearest: wgpu::Sampler,
    linear: wgpu::Sampler,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Golden Video source"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Golden Video presentation"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let layer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Golden Video layer parameters"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(LayerDraw::PARAMETER_BYTES),
                },
                count: None,
            }],
        });
        let composite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Golden Video composite"),
            bind_group_layouts: &[Some(&layout), Some(&layer_layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Golden Video presentation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("presentation.wgsl").into()),
        });
        let create = |entry, format, layout, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("fullscreen"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        Self {
            capture: create("capture", wgpu::TextureFormat::Bgra8UnormSrgb, &pipeline_layout, None),
            premultiply: create("premultiply", wgpu::TextureFormat::Rgba16Float, &pipeline_layout, None),
            sample_u8: create("sample", wgpu::TextureFormat::Rgba8Unorm, &pipeline_layout, None),
            sample_f16: create("sample", wgpu::TextureFormat::Rgba16Float, &pipeline_layout, None),
            composite_u8: create(
                "composite",
                wgpu::TextureFormat::Rgba8Unorm,
                &composite_layout,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            ),
            composite_f16: create(
                "composite",
                wgpu::TextureFormat::Rgba16Float,
                &composite_layout,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            ),
            layout,
            layer_layout,
            nearest: sampler(wgpu::FilterMode::Nearest),
            linear: sampler(wgpu::FilterMode::Linear),
        }
    }

    pub fn sample_for(&self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        match format {
            wgpu::TextureFormat::Rgba8Unorm => &self.sample_u8,
            wgpu::TextureFormat::Rgba16Float => &self.sample_f16,
            _ => unreachable!("the plan only admits two sample targets"),
        }
    }

    pub fn composite_for(&self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        match format {
            wgpu::TextureFormat::Rgba8Unorm => &self.composite_u8,
            wgpu::TextureFormat::Rgba16Float => &self.composite_f16,
            _ => unreachable!("the plan only admits two compositing targets"),
        }
    }

    pub fn bind(&self, device: &wgpu::Device, view: &wgpu::TextureView, filter: SampleFilter) -> wgpu::BindGroup {
        let sampler = match filter {
            SampleFilter::Nearest => &self.nearest,
            SampleFilter::Linear => &self.linear,
        };
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

pub(super) enum Draw {
    Clear(wgpu::TextureView, wgpu::Color),
    Sample {
        target: wgpu::TextureView,
        pipeline: wgpu::RenderPipeline,
        source: wgpu::BindGroup,
    },
    Composite {
        target: wgpu::TextureView,
        pipeline: wgpu::RenderPipeline,
        layers: Vec<LayerDraw>,
    },
}

impl Draw {
    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        match self {
            Self::Clear(target, color) => {
                drop(pass(encoder, target, *color));
            }
            Self::Sample {
                target,
                pipeline,
                source,
            } => sample(encoder, target, pipeline, source),
            Self::Composite {
                target,
                pipeline,
                layers,
            } => {
                for layer in layers {
                    layer.initialize(encoder);
                }
                let mut render = pass(encoder, target, wgpu::Color::TRANSPARENT);
                render.set_pipeline(pipeline);
                for layer in layers {
                    render.set_bind_group(0, &layer.source, &[]);
                    render.set_bind_group(1, &layer.parameters, &[]);
                    render.draw(0..3, 0..1);
                }
            }
        }
    }
}

pub(super) struct LayerDraw {
    source: wgpu::BindGroup,
    parameters: wgpu::BindGroup,
    constants: wgpu::Buffer,
    initialization: Cell<Option<wgpu::Buffer>>,
}

impl LayerDraw {
    const PARAMETER_BYTES: u64 = 80;
    pub const RESOURCE_BYTES: u64 = Self::PARAMETER_BYTES * 2;

    pub fn new(
        device: &wgpu::Device,
        pipelines: &Pipelines,
        layer: &Layer,
        view: &wgpu::TextureView,
    ) -> Result<Self, BackendError> {
        let (inverse, crop) = layer.mapping.map_or_else(
            || {
                (
                    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]],
                    UnitRect::FULL,
                )
            },
            |mapping| (mapping.inverse_rows(), mapping.crop()),
        );
        let mut bytes = [0; Self::PARAMETER_BYTES as usize];
        let data = inverse
            .into_iter()
            .flatten()
            .chain([crop.x, crop.y, crop.width, crop.height, layer.opacity.get()]);
        for (target, value) in bytes.chunks_exact_mut(4).zip(data) {
            target.copy_from_slice(&value.to_le_bytes());
        }
        let initialization = upload_buffer(device, "Golden Video layer parameter upload", Self::PARAMETER_BYTES);
        initialization
            .get_mapped_range_mut(..)
            .map_err(|_| BackendError::SubmissionFailed)?
            .copy_from_slice(&bytes);
        initialization.unmap();
        let constants = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Golden Video layer geometry and opacity"),
            size: Self::PARAMETER_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let parameters = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipelines.layer_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: constants.as_entire_binding(),
            }],
        });
        Ok(Self {
            source: pipelines.bind(device, view, layer.filter),
            parameters,
            constants,
            initialization: Cell::new(Some(initialization)),
        })
    }

    fn initialize(&self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(upload) = self.initialization.take() {
            encoder.copy_buffer_to_buffer(&upload, 0, &self.constants, 0, Self::PARAMETER_BYTES);
        }
    }
}

pub(super) fn upload_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    // Map this buffer directly. WGPU's implicit staging otherwise queues writes at unmap,
    // before runtime admission decides whether the prepared revision will ever submit.
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: true,
    })
}

pub(super) fn sample(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    source: &wgpu::BindGroup,
) {
    let mut render = pass(encoder, target, wgpu::Color::TRANSPARENT);
    render.set_pipeline(pipeline);
    render.set_bind_group(0, source, &[]);
    render.draw(0..3, 0..1);
}

fn pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    color: wgpu::Color,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(color),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    })
}

pub(super) fn texture(
    device: &wgpu::Device,
    extent: Extent2D,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: size(extent),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: usage | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

pub(super) fn size(extent: Extent2D) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: extent.width(),
        height: extent.height(),
        depth_or_array_layers: 1,
    }
}

pub(super) fn to_wgpu_color(color: LinearColor) -> wgpu::Color {
    let [r, g, b, a] = color.components().map(f64::from);
    wgpu::Color { r, g, b, a }
}
