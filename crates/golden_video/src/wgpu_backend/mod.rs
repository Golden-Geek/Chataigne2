//! Portable GPU execution of Golden Video's compiled render plans.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use golden_io::WorkerNotifier;
use golden_time::TimeSnapshot;

use crate::plan::{BlendMode, CompiledPlan, ExternalSource, RenderOperation};
use crate::readback::ReadbackSink;
use crate::runtime::backend::{Backend, BackendError};
use crate::runtime::{CompletionToken, DeviceGeneration, GenerationInfo, PlanIdentity, RuntimeStartError};
use crate::{AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor};

mod capture;
mod image;
mod pipeline;

use capture::Capture;
use image::Upload;
use pipeline::{Draw, LayerDraw, Pipelines};

/// An output sent to a bounded CPU preview consumer.
pub struct ReadbackOutput {
    /// Output identity used by the logical render plan.
    pub id: OutputId,
    /// Fixed capture extent before the consumer's final aspect fit.
    pub extent: Extent2D,
    /// Nonblocking handoff of completed frames.
    pub frames: ReadbackSink,
}

/// App-neutral configuration for the portable renderer.
pub struct WgpuConfig {
    /// Available bindings. Allocations are made only for outputs demanded by the active plan.
    pub outputs: Vec<ReadbackOutput>,
}

impl WgpuConfig {
    pub(crate) fn validate(&self) -> Result<(), RuntimeStartError> {
        let mut ids = HashSet::new();
        if self.outputs.len() > 32 || self.outputs.iter().any(|output| !ids.insert(output.id)) {
            return Err(RuntimeStartError::InvalidConfig(
                "invalid wgpu readback output bindings",
            ));
        }
        Ok(())
    }
}

pub(crate) struct WgpuBackend {
    outputs: HashMap<OutputId, ReadbackOutput>,
    device_generation: DeviceGeneration,
    device: Option<Device>,
    notify: WorkerNotifier,
}

struct Device {
    gpu: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: Pipelines,
    completed: Arc<AtomicU64>,
    lost: Arc<AtomicBool>,
}

pub(crate) struct Resources {
    plan: PlanIdentity,
    uploads: RefCell<Vec<Upload>>,
    draws: Vec<Draw>,
    outputs: Vec<Capture>,
}

impl WgpuBackend {
    pub(crate) fn new(config: WgpuConfig, device_generation: DeviceGeneration, notify: WorkerNotifier) -> Self {
        Self {
            outputs: config.outputs.into_iter().map(|output| (output.id, output)).collect(),
            device_generation,
            device: None,
            notify,
        }
    }
}

impl Backend for WgpuBackend {
    type Resources = Resources;

    fn initialize(&mut self) -> Result<(), BackendError> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|error| {
            eprintln!("[golden-video] wgpu adapter: {error}");
            BackendError::UnsupportedInitialization
        })?;
        let (gpu, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Golden Video"),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|error| {
            eprintln!("[golden-video] wgpu device: {error}");
            BackendError::UnsupportedInitialization
        })?;
        let lost = Arc::new(AtomicBool::new(false));
        let device_lost = lost.clone();
        let wake = self.notify.clone();
        gpu.set_device_lost_callback(move |_, _| {
            device_lost.store(true, Ordering::Release);
            wake.notify();
        });
        self.device = Some(Device {
            pipelines: Pipelines::new(&gpu),
            gpu,
            queue,
            completed: Arc::new(AtomicU64::new(0)),
            lost,
        });
        Ok(())
    }

    fn resource_bytes(&self, plan: &CompiledPlan) -> Result<u64, BackendError> {
        let device = self.device.as_ref().ok_or(BackendError::DeviceLost)?;
        let limits = device.gpu.limits();
        let mut bytes = 0_u64;
        for external in plan.external_textures() {
            let extent = external.descriptor.extent;
            if !matches!(external.source, Some(ExternalSource::Image(_)))
                || !Upload::accepts(external.descriptor)
                || !fits(extent, limits.max_texture_dimension_2d)
                || Upload::staging_bytes(extent) > limits.max_buffer_size
            {
                return Err(BackendError::UnsupportedPlan);
            }
            bytes = bytes
                .checked_add(Upload::resource_bytes(extent))
                .ok_or(BackendError::ResourceSizeOverflow)?;
        }
        for pass in plan.passes() {
            if !fits(pass.descriptor.extent, limits.max_texture_dimension_2d)
                || texture_format(pass.descriptor).is_none()
            {
                return Err(BackendError::UnsupportedPlan);
            }
            let layer_bytes = match &pass.operation {
                RenderOperation::Clear(_) | RenderOperation::Sample { .. } => 0,
                RenderOperation::Composite { layers }
                    if layers.iter().all(|layer| {
                        layer.blend == BlendMode::Normal
                            && layer.source.texture().is_some()
                            && layer.opacity_envelope.is_none()
                    }) =>
                {
                    layers.len() as u64 * LayerDraw::RESOURCE_BYTES
                }
                _ => return Err(BackendError::UnsupportedPlan),
            };
            bytes = bytes
                .checked_add(
                    pass.descriptor
                        .byte_len()
                        .map_err(|_| BackendError::ResourceSizeOverflow)?,
                )
                .and_then(|bytes| bytes.checked_add(layer_bytes))
                .ok_or(BackendError::ResourceSizeOverflow)?;
        }
        for output in plan.outputs() {
            let binding = self.outputs.get(&output.id).ok_or(BackendError::UnsupportedPlan)?;
            if !fits(binding.extent, limits.max_texture_dimension_2d)
                || Capture::staging_bytes(binding.extent) > limits.max_buffer_size
            {
                return Err(BackendError::UnsupportedPlan);
            }
            bytes = bytes
                .checked_add(Capture::resource_bytes(binding.extent))
                .ok_or(BackendError::ResourceSizeOverflow)?;
        }
        Ok(bytes)
    }

    fn prepare(&mut self, plan: &CompiledPlan, generation: GenerationInfo) -> Result<Resources, BackendError> {
        let device = self.device.as_ref().ok_or(BackendError::DeviceLost)?;
        let mut textures = HashMap::with_capacity(plan.passes().len() + plan.external_textures().len());
        let mut uploads = Vec::with_capacity(plan.external_textures().len());
        for external in plan.external_textures() {
            let Some(ExternalSource::Image(image)) = &external.source else {
                return Err(BackendError::UnsupportedPlan);
            };
            let upload = Upload::new(&device.gpu, &device.pipelines, image.value())?;
            textures.insert(external.id, upload.view().clone());
            uploads.push(upload);
        }
        let mut draws = Vec::with_capacity(plan.passes().len());
        for pass in plan.passes() {
            let format = texture_format(pass.descriptor).ok_or(BackendError::UnsupportedPlan)?;
            let target = pipeline::texture(
                &device.gpu,
                pass.descriptor.extent,
                format,
                wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let view = target.create_view(&wgpu::TextureViewDescriptor::default());
            let draw = match &pass.operation {
                RenderOperation::Clear(color) => Draw::Clear(view.clone(), pipeline::to_wgpu_color(*color)),
                RenderOperation::Sample { source, filter } => Draw::Sample {
                    target: view.clone(),
                    pipeline: device.pipelines.sample_for(format).clone(),
                    source: device.pipelines.bind(
                        &device.gpu,
                        textures.get(source).ok_or(BackendError::UnsupportedPlan)?,
                        *filter,
                    ),
                },
                RenderOperation::Composite { layers } => Draw::Composite {
                    target: view.clone(),
                    pipeline: device.pipelines.composite_for(format).clone(),
                    layers: layers
                        .iter()
                        .map(|layer| {
                            let source = layer
                                .source
                                .texture()
                                .and_then(|id| textures.get(&id))
                                .ok_or(BackendError::UnsupportedPlan)?;
                            LayerDraw::new(&device.gpu, &device.pipelines, layer, source)
                        })
                        .collect::<Result<_, BackendError>>()?,
                },
                _ => return Err(BackendError::UnsupportedPlan),
            };
            textures.insert(pass.output, view);
            draws.push(draw);
        }
        let outputs = plan
            .outputs()
            .iter()
            .map(|output| {
                let binding = self.outputs.get(&output.id).ok_or(BackendError::UnsupportedPlan)?;
                let source = textures.get(&output.source).ok_or(BackendError::UnsupportedPlan)?;
                Ok(Capture::new(&device.gpu, &device.pipelines, binding, source))
            })
            .collect::<Result<_, BackendError>>()?;
        Ok(Resources {
            plan: generation.identity,
            uploads: RefCell::new(uploads),
            draws,
            outputs,
        })
    }

    fn submit(
        &mut self,
        resources: &Resources,
        _: &CompiledPlan,
        token: CompletionToken,
        _: TimeSnapshot,
    ) -> Result<(), BackendError> {
        let device = self.device.as_ref().ok_or(BackendError::DeviceLost)?;
        if device.lost.load(Ordering::Acquire) {
            return Err(BackendError::DeviceLost);
        }
        let mut encoder = device
            .gpu
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        for upload in resources.uploads.borrow_mut().drain(..) {
            upload.encode(&mut encoder, &device.pipelines);
        }
        for draw in &resources.draws {
            draw.encode(&mut encoder);
        }
        for output in &resources.outputs {
            output.encode(&mut encoder, &device.pipelines, resources.plan);
        }
        let completed = device.completed.clone();
        let wake = self.notify.clone();
        encoder.on_submitted_work_done(move || {
            completed.fetch_max(token.serial, Ordering::Release);
            wake.notify();
        });
        device.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    fn completion(&mut self) -> Result<CompletionToken, BackendError> {
        let device = self.device.as_ref().ok_or(BackendError::DeviceLost)?;
        device
            .gpu
            .poll(wgpu::PollType::Poll)
            .map_err(|_| BackendError::DeviceLost)?;
        if device.lost.load(Ordering::Acquire) {
            return Err(BackendError::DeviceLost);
        }
        Ok(CompletionToken {
            device: self.device_generation,
            serial: device.completed.load(Ordering::Acquire),
        })
    }

    fn wait_for_completion(&mut self, timeout: Duration) -> Result<(), BackendError> {
        let device = self.device.as_ref().ok_or(BackendError::DeviceLost)?;
        let result = device.gpu.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(timeout),
        });
        if device.lost.load(Ordering::Acquire) {
            return Err(BackendError::DeviceLost);
        }
        result.map(|_| ()).map_err(|_| BackendError::SubmissionFailed)
    }

    fn reset(&mut self, device: DeviceGeneration) -> Result<(), BackendError> {
        self.device = None;
        self.device_generation = device;
        self.initialize()
    }
}

fn fits(extent: Extent2D, maximum: u32) -> bool {
    extent.width() <= maximum && extent.height() <= maximum
}

fn texture_format(descriptor: TextureDescriptor) -> Option<wgpu::TextureFormat> {
    if descriptor.color_encoding != ColorEncoding::Linear || descriptor.alpha_mode != AlphaMode::Premultiplied {
        return None;
    }
    match descriptor.format {
        PixelFormat::Rgba8Unorm => Some(wgpu::TextureFormat::Rgba8Unorm),
        PixelFormat::Rgba16Float => Some(wgpu::TextureFormat::Rgba16Float),
        _ => None,
    }
}
