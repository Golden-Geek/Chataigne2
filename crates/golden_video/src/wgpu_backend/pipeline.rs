use crate::plan::SampleFilter;
use crate::{Extent2D, LinearColor};

pub(super) struct Pipelines {
    pub capture: wgpu::RenderPipeline,
    sample_u8: wgpu::RenderPipeline,
    sample_f16: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
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
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Golden Video presentation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("presentation.wgsl").into()),
        });
        let create = |entry, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
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
                    targets: &[Some(format.into())],
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
            capture: create("capture", wgpu::TextureFormat::Bgra8UnormSrgb),
            sample_u8: create("sample", wgpu::TextureFormat::Rgba8Unorm),
            sample_f16: create("sample", wgpu::TextureFormat::Rgba16Float),
            layout,
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
        }
    }
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
