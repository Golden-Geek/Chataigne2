use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext};
use windows::core::{Error, HRESULT, Result};

use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{OutputId, TextureDescriptor};

use super::config::{D3d11Config, OutputBinding};
use super::observation::{D3d11Control, OutputState, OutputStatus};
use super::texture::Texture;
use super::timing::{CpuInterval, DiagnosticDelivery, Recorder, SpoutImageVersion, WindowPresentIdentity};
use lifetime::Endpoint;
use recovery::Recovery;

mod lifetime;
mod recovery;
pub(crate) mod spout;
mod window;

const INVALID_ARGUMENT: HRESULT = HRESULT(0x80070057_u32 as i32);

/// Local failures preserve healthy outputs. DeviceLost is reserved for an actual failed
/// GetDeviceRemovedReason query, never an output HRESULT alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    Accepted,
    Busy,
    Unavailable,
    Failed(HRESULT),
    DeviceLost(HRESULT),
}

impl From<Delivery> for DiagnosticDelivery {
    fn from(delivery: Delivery) -> Self {
        match delivery {
            Delivery::Accepted => Self::Accepted,
            Delivery::Busy => Self::Busy,
            Delivery::Unavailable => Self::Unavailable,
            Delivery::Failed(error) => Self::Failed { hresult: error.0 },
            Delivery::DeviceLost(error) => Self::DeviceLost { hresult: error.0 },
        }
    }
}

enum Output {
    Window(window::WindowOutput),
    Spout(spout::SpoutOutput),
}

impl Output {
    fn new(device: &ID3D11Device, binding: &OutputBinding, diagnostics: bool) -> Result<Self> {
        match binding {
            OutputBinding::Window { window, .. } => {
                window::WindowOutput::new(device, *window, binding.descriptor(), diagnostics).map(Self::Window)
            }
            OutputBinding::Spout { name, .. } => {
                spout::SpoutOutput::new(device, name.clone(), binding.descriptor()).map(Self::Spout)
            }
        }
    }
    fn admit(&mut self) -> std::result::Result<(), Delivery> {
        match self {
            Self::Window(output) => output.admit(),
            Self::Spout(_) => Ok(()),
        }
    }
    fn deliver(
        &mut self,
        context: &ID3D11DeviceContext,
        texture: &Texture,
        recorder: Option<&Recorder>,
        consumer_api: &mut Option<CpuInterval>,
    ) -> Delivery {
        match self {
            Self::Window(output) => output.deliver(context, texture, recorder, consumer_api),
            Self::Spout(output) => output.deliver(texture, recorder, consumer_api),
        }
    }
}

/// Exclusive endpoint admission for one renderer batch. Conversion may run only while
/// this handle retains the binding; delivery consumes it without waiting on readiness again.
pub(super) struct AdmittedOutput<'a> {
    binding: &'a mut Binding,
    device: &'a ID3D11Device,
    generation: DeviceGeneration,
    control: &'a D3d11Control,
    token: CompletionToken,
}

pub(super) struct Delivered {
    pub outcome: Delivery,
    pub spout_image: Option<SpoutImageVersion>,
    pub window_present: Option<WindowPresentIdentity>,
}

impl AdmittedOutput<'_> {
    pub(super) fn deliver(
        self,
        context: &ID3D11DeviceContext,
        texture: &Texture,
        recorder: Option<&Recorder>,
        consumer_api: &mut Option<CpuInterval>,
    ) -> Delivered {
        let endpoint = self
            .binding
            .endpoint
            .begin_use(self.token.serial)
            .expect("exclusively admitted endpoint");
        let delivered = endpoint.deliver(context, texture, recorder, consumer_api);
        let image = match (&endpoint, delivered, recorder.is_some()) {
            (Output::Spout(sender), Delivery::Accepted, true) => sender.copied_image_version(),
            _ => None,
        };
        let window_present = match (&endpoint, consumer_api.is_some(), recorder.is_some()) {
            (Output::Window(window), true, true) => window.present_identity(self.binding.recovery.generation),
            _ => None,
        };
        let delivered = self.binding.delivered(self.device, delivered, Instant::now());
        self.binding.publish(self.control, self.generation);
        self.control
            .observe_output(self.binding.config.id(), self.token, delivered);
        Delivered {
            outcome: delivered,
            spout_image: image.filter(|_| delivered == Delivery::Accepted),
            window_present,
        }
    }
}

struct Binding {
    config: OutputBinding,
    endpoint: Endpoint<Output>,
    recovery: Recovery,
    flush_before_create: bool,
    diagnostics: bool,
}

impl Binding {
    fn create(
        &mut self,
        device: &ID3D11Device,
        context: Option<&ID3D11DeviceContext>,
        now: Instant,
    ) -> Option<Delivery> {
        if !self.endpoint.can_create() || !self.recovery.begin_attempt(now) {
            return None;
        }
        if self.flush_before_create
            && let Some(context) = context
        {
            // Completed old uses and released references precede this nonblocking flush. DXGI
            // can defer flip-chain destruction; never overlap replacements on the same HWND.
            unsafe {
                context.Flush();
            }
            self.flush_before_create = false;
        }
        match self
            .endpoint
            .create(|| Output::new(device, &self.config, self.diagnostics))
        {
            Ok(true) => {
                self.recovery.created();
                None
            }
            Ok(false) => None,
            Err(error) => {
                self.flush_before_create = matches!(self.config, OutputBinding::Window { .. });
                if let Some(removed) = removed(device) {
                    self.recovery.device_lost(removed);
                    Some(Delivery::DeviceLost(removed))
                } else {
                    self.recovery.failed(error.code(), now);
                    Some(Delivery::Failed(error.code()))
                }
            }
        }
    }

    fn delivered(&mut self, device: &ID3D11Device, delivery: Delivery, now: Instant) -> Delivery {
        let delivery = match delivery {
            Delivery::Failed(error) => removed(device).map_or(Delivery::Failed(error), Delivery::DeviceLost),
            other => other,
        };
        match delivery {
            Delivery::Accepted => self.recovery.accepted(),
            Delivery::Busy => self.recovery.state = OutputState::Busy,
            Delivery::Unavailable => self.recovery.state = OutputState::Unavailable,
            Delivery::Failed(error) => {
                self.endpoint.fail();
                self.recovery.failed(error, now);
            }
            Delivery::DeviceLost(error) => self.recovery.device_lost(error),
        }
        delivery
    }

    fn publish(&self, control: &D3d11Control, device: DeviceGeneration) {
        control.output_state(
            self.config.id(),
            device,
            OutputStatus {
                state: self.recovery.state,
                generation: self.recovery.generation,
                attempts: self.recovery.attempts,
                hresult: self.recovery.error.map(|error| error.0),
            },
        );
    }
}

/// One fixed endpoint slot per binding. Its baseline reservation covers healthy, missing and
/// retiring ownership; no replacement is created before the failed endpoint's completion/drop.
pub(super) struct OutputPool {
    bindings: BTreeMap<OutputId, Binding>,
    device: ID3D11Device,
    generation: DeviceGeneration,
    control: D3d11Control,
    stopping: bool,
}

impl OutputPool {
    pub(super) fn new(
        device: &ID3D11Device,
        config: D3d11Config,
        control: D3d11Control,
        generation: DeviceGeneration,
    ) -> Result<Self> {
        let ids = config.outputs.iter().map(OutputBinding::id).collect::<BTreeSet<_>>();
        if ids.len() != config.outputs.len() {
            return Err(Error::new(INVALID_ARGUMENT, "duplicate native output identity"));
        }
        control.register_outputs(generation, config.outputs.iter().map(OutputBinding::id));
        let diagnostics = config.diagnostics.is_some();
        let mut pool = Self {
            bindings: config
                .outputs
                .into_iter()
                .map(|config| {
                    (
                        config.id(),
                        Binding {
                            config,
                            endpoint: Endpoint::new(),
                            recovery: Recovery::new(),
                            flush_before_create: false,
                            diagnostics,
                        },
                    )
                })
                .collect(),
            device: device.clone(),
            generation,
            control,
            stopping: false,
        };
        for binding in pool.bindings.values_mut() {
            let result = binding.create(device, None, Instant::now());
            binding.publish(&pool.control, generation);
            if let Some(Delivery::DeviceLost(error)) = result {
                return Err(Error::from_hresult(error));
            }
        }
        Ok(pool)
    }

    pub(super) fn descriptor(&self, id: OutputId) -> Option<TextureDescriptor> {
        self.bindings.get(&id).map(|binding| binding.config.descriptor())
    }

    pub(super) fn admit(
        &mut self,
        id: OutputId,
        context: &ID3D11DeviceContext,
        descriptor: TextureDescriptor,
        token: CompletionToken,
    ) -> std::result::Result<AdmittedOutput<'_>, Delivery> {
        let Some(binding) = self.bindings.get_mut(&id) else {
            return Err(Delivery::Unavailable);
        };
        if token.device != self.generation {
            return Err(Delivery::Failed(INVALID_ARGUMENT));
        }
        let now = Instant::now();
        let admission = if self.stopping {
            Err(Delivery::Unavailable)
        } else if descriptor != binding.config.descriptor() {
            Err(binding.delivered(&self.device, Delivery::Failed(INVALID_ARGUMENT), now))
        } else if let Some(delivery) = binding.create(&self.device, Some(context), now) {
            Err(delivery)
        } else if let Some(endpoint) = binding.endpoint.begin_use(token.serial) {
            endpoint
                .admit()
                .map_err(|delivery| binding.delivered(&self.device, delivery, now))
        } else {
            // A recovering endpoint can still retain its old allocation or retry deadline.
            Err(Delivery::Unavailable)
        };
        match admission {
            Ok(()) => Ok(AdmittedOutput {
                binding,
                device: &self.device,
                generation: self.generation,
                control: &self.control,
                token,
            }),
            Err(delivered) => {
                binding.publish(&self.control, self.generation);
                self.control.observe_output(id, token, delivered);
                Err(delivered)
            }
        }
    }

    pub(super) fn retire(&mut self, completed: u64) {
        for binding in self.bindings.values_mut() {
            if binding.endpoint.retire(completed) {
                binding.flush_before_create = matches!(binding.config, OutputBinding::Window { .. });
            }
        }
    }

    pub(super) fn quiesce(&mut self) {
        self.stopping = true;
        for binding in self.bindings.values_mut() {
            binding.recovery.stop();
            binding.publish(&self.control, self.generation);
        }
    }
}

impl Drop for OutputPool {
    fn drop(&mut self) {
        for binding in self.bindings.values_mut() {
            // The backend drops its baseline only after GPU completion or confirmed removal.
            binding.endpoint.clear();
            binding.recovery.state = OutputState::Stopped;
            binding.publish(&self.control, self.generation);
        }
    }
}

fn removed(device: &ID3D11Device) -> Option<HRESULT> {
    // An output HRESULT alone does not establish device removal or permit early destruction.
    unsafe { device.GetDeviceRemovedReason().err().map(|error| error.code()) }
}

#[cfg(test)]
mod tests;
