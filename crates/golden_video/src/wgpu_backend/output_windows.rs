//! Desktop surface bindings retained through the renderer's normal retirement fence.

use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::plan::SampleFilter;
use crate::runtime::backend::BackendError;
use crate::{Extent2D, OutputId};

use super::pipeline::{self, Pipelines};

/// Native output surfaces shared between a desktop window owner and one render worker.
#[derive(Clone, Default)]
pub struct WindowOutputs {
    inner: Arc<Registry>,
}

#[derive(Default)]
struct Registry {
    instance: wgpu::Instance,
    outputs: Mutex<HashMap<OutputId, Arc<Endpoint>>>,
}

struct Retirement(Option<Box<dyn FnOnce() + Send + Sync>>);
type Activate = Box<dyn FnOnce() -> Result<(), String> + Send + Sync>;

impl Drop for Retirement {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            callback();
        }
    }
}

pub(super) struct Endpoint {
    // The native-window retirement callback runs after the surface is released.
    surface: wgpu::Surface<'static>,
    _retirement: Retirement,
    extent: Extent2D,
    wayland: bool,
    withdrawn: AtomicBool,
    lost: AtomicBool,
    activate: Mutex<Option<Activate>>,
    configuration: Mutex<Option<(wgpu::Device, wgpu::SurfaceConfiguration)>>,
}

impl WindowOutputs {
    /// Create a surface on the desktop UI thread. The window owner must retain its native
    /// window until `on_retired`; the callback also runs on startup failure. `on_activate`
    /// makes the window visible on its first admitted frame.
    pub fn attach<W>(
        &self,
        id: OutputId,
        window: W,
        extent: Extent2D,
        on_activate: impl FnOnce() -> Result<(), String> + Send + Sync + 'static,
        on_retired: impl FnOnce() + Send + Sync + 'static,
    ) -> Result<(), String>
    where
        W: wgpu::DisplayAndWindowHandle + 'static,
    {
        let retirement = Retirement(Some(Box::new(on_retired)));
        let wayland = window
            .window_handle()
            .is_ok_and(|handle| matches!(handle.as_raw(), wgpu::rwh::RawWindowHandle::Wayland(_)));
        let surface = self
            .inner
            .instance
            .create_surface(wgpu::SurfaceTarget::DisplayAndWindow(Box::new(window)))
            .map_err(|error| error.to_string())?;
        let mut outputs = self.inner.outputs.lock().expect("Golden Video window outputs poisoned");
        if outputs.len() >= 32 || outputs.contains_key(&id) {
            return Err("invalid or duplicate window output binding".into());
        }
        outputs.insert(
            id,
            Arc::new(Endpoint {
                surface,
                _retirement: retirement,
                extent,
                wayland,
                withdrawn: AtomicBool::new(false),
                lost: AtomicBool::new(false),
                activate: Mutex::new(Some(Box::new(on_activate))),
                configuration: Mutex::new(None),
            }),
        );
        Ok(())
    }

    /// Stop presenting to an output. GPU plans keep their endpoint alive until final use.
    pub fn detach(&self, id: OutputId) {
        let endpoint = self
            .inner
            .outputs
            .lock()
            .expect("Golden Video window outputs poisoned")
            .remove(&id);
        if let Some(endpoint) = endpoint {
            endpoint.withdrawn.store(true, Ordering::Release);
        }
    }

    /// A lost native surface must be replaced on the desktop UI thread.
    pub fn needs_reopen(&self, id: OutputId) -> bool {
        self.endpoint(id)
            .is_some_and(|endpoint| endpoint.lost.load(Ordering::Acquire))
    }

    pub(super) fn endpoint(&self, id: OutputId) -> Option<Arc<Endpoint>> {
        self.inner
            .outputs
            .lock()
            .expect("Golden Video window outputs poisoned")
            .get(&id)
            .cloned()
    }

    pub(super) fn instance(&self) -> wgpu::Instance {
        self.inner.instance.clone()
    }
}

impl Endpoint {
    pub fn extent(&self) -> Extent2D {
        self.extent
    }

    pub fn resource_bytes(&self) -> u64 {
        // Charge four RGBA buffers to cover the desired latency and presentation ownership.
        u64::from(self.extent.width()) * u64::from(self.extent.height()) * 16
    }

    fn configure(&self, gpu: &wgpu::Device, adapter: &wgpu::Adapter) -> Result<wgpu::TextureFormat, BackendError> {
        let mut configuration = self
            .configuration
            .lock()
            .expect("Golden Video surface configuration poisoned");
        if let Some((owner, config)) = configuration.as_ref()
            && owner == gpu
        {
            return Ok(config.format);
        }
        let capabilities = self.surface.get_capabilities(adapter);
        let format = [wgpu::TextureFormat::Bgra8UnormSrgb, wgpu::TextureFormat::Rgba8UnormSrgb]
            .into_iter()
            .find(|format| capabilities.formats.contains(format))
            .ok_or(BackendError::UnsupportedPlan)?;
        // Wayland compositors can stop sending frame callbacks when the selected output
        // disappears. Mesa FIFO presentation then blocks the worker that must retire it.
        let present_mode = if self.wayland {
            [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
                .into_iter()
                .find(|mode| capabilities.present_modes.contains(mode))
                .ok_or(BackendError::UnsupportedPlan)?
        } else {
            wgpu::PresentMode::Fifo
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: self.extent.width(),
            height: self.extent.height(),
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: Vec::new(),
        };
        self.surface.configure(gpu, &config);
        *configuration = Some((gpu.clone(), config));
        Ok(format)
    }

    fn acquire(&self, gpu: &wgpu::Device) -> Option<wgpu::SurfaceTexture> {
        if self.withdrawn.load(Ordering::Acquire) || self.lost.load(Ordering::Acquire) {
            return None;
        }
        let activate = self
            .activate
            .lock()
            .expect("Golden Video window activation poisoned")
            .take();
        if let Some(activate) = activate
            && let Err(error) = activate()
        {
            eprintln!("[golden-video] native output activation: {error}");
            self.lost.store(true, Ordering::Release);
            return None;
        }
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => None,
            wgpu::CurrentSurfaceTexture::Outdated => {
                if let Some((_, config)) = self
                    .configuration
                    .lock()
                    .expect("Golden Video surface configuration poisoned")
                    .as_ref()
                {
                    self.surface.configure(gpu, config);
                }
                None
            }
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Validation => {
                self.lost.store(true, Ordering::Release);
                None
            }
        }
    }
}

pub(super) struct WindowDraw {
    endpoint: Arc<Endpoint>,
    pipeline: wgpu::RenderPipeline,
    source: wgpu::BindGroup,
}

impl WindowDraw {
    pub fn new(
        gpu: &wgpu::Device,
        adapter: &wgpu::Adapter,
        pipelines: &Pipelines,
        endpoint: Arc<Endpoint>,
        source: &wgpu::TextureView,
    ) -> Result<Self, BackendError> {
        let format = endpoint.configure(gpu, adapter)?;
        Ok(Self {
            endpoint,
            pipeline: pipelines.present_for(format).clone(),
            source: pipelines.bind(gpu, source, SampleFilter::Linear),
        })
    }

    pub fn encode(&self, gpu: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) -> Option<wgpu::SurfaceTexture> {
        let frame = self.endpoint.acquire(gpu)?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        pipeline::sample(encoder, &view, &self.pipeline, &self.source);
        Some(frame)
    }
}
