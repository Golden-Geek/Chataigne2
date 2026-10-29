use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::Duration;

use golden_io::WorkerNotifier;
use golden_time::TimeSnapshot;

use crate::plan::{BlendMode, CompiledPlan, ExternalSource, RenderOperation, SampleFilter};
use crate::runtime::backend::{Backend, BackendError};
use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{LinearColor, OutputId, PassId, TextureDescriptor, TextureId};

use super::completion::EventCompletion;
use super::device::Device;
use super::inputs::{SourceDemands, SourcePool};
use super::outputs::{Delivery, OutputPool};
use super::pipeline::Pipeline;
use super::texture::Texture;
use super::{D3d11Config, D3d11Control, NativeFailure};

struct NativeState {
    inputs: SourcePool,
    outputs: OutputPool,
    pipeline: Pipeline,
    completion: EventCompletion,
    device: Device,
}

impl Drop for NativeState {
    fn drop(&mut self) {
        // SAFETY: core retirement has completed all uses (or confirmed removal) before this
        // owner is destroyed. Release context bindings before output/pipeline/device fields.
        unsafe {
            self.device.context.ClearState();
            self.device.context.Flush();
        }
    }
}

pub(crate) struct D3d11Backend {
    config: D3d11Config,
    control: D3d11Control,
    generation: DeviceGeneration,
    native: Option<NativeState>,
    notify: Option<WorkerNotifier>,
}

pub(crate) struct D3d11Resources {
    textures: BTreeMap<TextureId, Texture>,
    scratch: BTreeMap<PassId, Texture>,
    outputs: BTreeMap<OutputId, Texture>,
    input_sequences: BTreeMap<TextureId, Cell<Option<u64>>>,
    _demands: Option<SourceDemands>,
}

impl D3d11Backend {
    pub(crate) fn new(config: D3d11Config, control: D3d11Control, generation: DeviceGeneration) -> Self {
        Self {
            config,
            control,
            generation,
            native: None,
            notify: None,
        }
    }

    pub(crate) fn with_notifier(mut self, notifier: WorkerNotifier) -> Self {
        self.notify = Some(notifier);
        self
    }

    fn validate(&self, plan: &CompiledPlan) -> Result<(), BackendError> {
        if plan.external_textures().iter().any(|external| match &external.source {
            Some(ExternalSource::Spout(binding)) => {
                !binding.accepts(external.descriptor)
                    || binding.descriptor.extent.width() > 16_384
                    || binding.descriptor.extent.height() > 16_384
            }
            Some(ExternalSource::Image(image)) => image.value().descriptor() != external.descriptor,
            None => true,
        }) {
            self.control
                .update(|state| state.failure = Some(NativeFailure::UnsupportedExternalInput));
            return Err(BackendError::UnsupportedPlan);
        }
        let spout_sources = plan
            .external_textures()
            .iter()
            .filter_map(|source| source.source.as_ref().and_then(ExternalSource::spout));
        let source_bytes = spout_sources.clone().try_fold(0_u64, |total, source| {
            total
                .checked_add(
                    bytes(source.descriptor)?
                        .checked_mul(5)
                        .ok_or(BackendError::ResourceSizeOverflow)?,
                )
                .ok_or(BackendError::ResourceSizeOverflow)
        })?;
        if spout_sources.count() > self.config.inputs.max_sources || source_bytes > self.config.inputs.max_storage_bytes
        {
            return Err(BackendError::SourceCapacity);
        }
        if plan
            .outputs()
            .iter()
            .any(|output| !self.config.outputs.iter().any(|binding| binding.id() == output.id))
        {
            self.control
                .update(|state| state.failure = Some(NativeFailure::UnboundOutput));
            return Err(BackendError::UnsupportedPlan);
        }
        Ok(())
    }

    fn allocate(&mut self) -> Result<(), BackendError> {
        let result = (|| {
            let device = Device::new(self.config.adapter)?;
            let pipeline = Pipeline::new(&device.device)?;
            let completion = EventCompletion::new(&device)?;
            let outputs = OutputPool::new(&device.device, self.config.clone())?;
            let inputs = SourcePool::new(
                self.config.adapter,
                self.config.inputs,
                &self.control,
                self.notify.clone(),
                self.generation,
            );
            Ok::<_, windows::core::Error>(NativeState {
                inputs,
                outputs,
                pipeline,
                completion,
                device,
            })
        })();
        match result {
            Ok(native) => {
                let adapter = native.device.adapter;
                self.native = Some(native);
                self.control.update(|state| {
                    state.adapter = Some(adapter);
                    state.device = Some(self.generation);
                    state.initialized = true;
                    state.failure = None;
                });
                Ok(())
            }
            Err(error) => {
                self.control.update(|state| {
                    state.initialized = false;
                    state.failure = Some(NativeFailure::Initialization);
                    state.hresult = Some(error.code().0);
                });
                Err(BackendError::AllocationFailed)
            }
        }
    }
}

impl Backend for D3d11Backend {
    type Resources = D3d11Resources;

    fn base_resource_bytes(&self) -> Result<u64, BackendError> {
        // This accounts every owned texture pool and the constant buffer. DXGI/driver physical
        // residency, alignment and native-object overhead need separate measured qualification.
        let base = 48_u64
            .checked_add(self.config.inputs.max_storage_bytes)
            .ok_or(BackendError::ResourceSizeOverflow)?;
        self.config.outputs.iter().try_fold(base, |total, binding| {
            let bytes = bytes(binding.descriptor())?
                .checked_mul(binding.persistent_copies())
                .ok_or(BackendError::ResourceSizeOverflow)?;
            total.checked_add(bytes).ok_or(BackendError::ResourceSizeOverflow)
        })
    }

    fn initialize(&mut self) -> Result<(), BackendError> {
        self.allocate()
    }

    fn quiesce(&mut self) {
        if let Some(native) = &self.native {
            native.inputs.quiesce();
        }
    }

    fn resource_bytes(&self, plan: &CompiledPlan) -> Result<u64, BackendError> {
        self.validate(plan)?;
        let mut total = plan.footprint().total_texture_bytes;
        for pass in plan.passes() {
            if matches!(&pass.operation, RenderOperation::Composite { layers } if layers.len() > 1) {
                total = total
                    .checked_add(bytes(pass.descriptor)?)
                    .ok_or(BackendError::ResourceSizeOverflow)?;
            }
        }
        for output in plan.outputs() {
            let binding = self
                .config
                .outputs
                .iter()
                .find(|binding| binding.id() == output.id)
                .unwrap();
            total = total
                .checked_add(bytes(binding.descriptor())?)
                .ok_or(BackendError::ResourceSizeOverflow)?;
        }
        Ok(total)
    }

    fn prepare(&mut self, plan: &CompiledPlan, device: DeviceGeneration) -> Result<Self::Resources, BackendError> {
        self.validate(plan)?;
        if device != self.generation {
            return Err(BackendError::DeviceLost);
        }
        let native = self.native.as_mut().ok_or(BackendError::DeviceLost)?;
        if native.device.removed() {
            return Err(BackendError::DeviceLost);
        }
        native.inputs.ensure(plan)?;
        let prepared = (|| {
            let mut resources = D3d11Resources {
                textures: BTreeMap::new(),
                scratch: BTreeMap::new(),
                outputs: BTreeMap::new(),
                input_sequences: BTreeMap::new(),
                _demands: None,
            };
            for external in plan.external_textures() {
                let texture = match &external.source {
                    Some(ExternalSource::Image(image)) => {
                        let texture = Texture::from_image(&native.device.device, image.value())?;
                        self.control.update(|state| {
                            state.uploaded_images = state.uploaded_images.saturating_add(1);
                            state.uploaded_image_bytes = state
                                .uploaded_image_bytes
                                .saturating_add(image.value().pixels().len() as u64);
                        });
                        texture
                    }
                    _ => {
                        resources.input_sequences.insert(external.id, Cell::new(None));
                        Texture::new(&native.device.device, external.descriptor)?
                    }
                };
                resources.textures.insert(external.id, texture);
            }
            for pass in plan.passes() {
                resources
                    .textures
                    .insert(pass.output, Texture::new(&native.device.device, pass.descriptor)?);
                if matches!(&pass.operation, RenderOperation::Composite { layers } if layers.len() > 1) {
                    resources
                        .scratch
                        .insert(pass.id, Texture::new(&native.device.device, pass.descriptor)?);
                }
            }
            for output in plan.outputs() {
                let descriptor = native.outputs.descriptor(output.id).expect("validated output binding");
                resources
                    .outputs
                    .insert(output.id, Texture::new(&native.device.device, descriptor)?);
            }
            resources._demands = Some(native.inputs.demands(plan));
            Ok::<_, windows::core::Error>(resources)
        })();
        prepared.map_err(|_| {
            if native.device.removed() {
                BackendError::DeviceLost
            } else {
                BackendError::AllocationFailed
            }
        })
    }

    fn submit(
        &mut self,
        resources: &Self::Resources,
        plan: &CompiledPlan,
        token: CompletionToken,
        _time: TimeSnapshot,
    ) -> Result<(), BackendError> {
        let native = self.native.as_mut().ok_or(BackendError::DeviceLost)?;
        if token.device != self.generation || native.device.removed() {
            return Err(BackendError::DeviceLost);
        }
        if !native.completion.can_submit() {
            return Err(BackendError::SubmissionFailed);
        }
        let submission = native.completion.begin(&native.device, token);
        let context = &native.device.context;
        let pipeline = &native.pipeline;
        for external in plan.external_textures() {
            if !matches!(external.source, Some(ExternalSource::Spout(_))) {
                continue;
            }
            let target = &resources.textures[&external.id];
            let sequence = &resources.input_sequences[&external.id];
            if sequence.get().is_none() {
                // Absent input has deterministic zero storage (opaque descriptors read black).
                unsafe {
                    context.ClearRenderTargetView(&target.rtv, &[0.0; 4]);
                }
                sequence.set(Some(0));
            }
            match native.inputs.copy(
                external.id,
                &native.device,
                target,
                token.serial,
                sequence.get().unwrap(),
            ) {
                Ok(Some(next)) => sequence.set(Some(next)),
                Ok(None) => {
                    if sequence.get() != Some(0) {
                        unsafe {
                            context.ClearRenderTargetView(&target.rtv, &[0.0; 4]);
                        }
                        sequence.set(Some(0));
                    }
                }
                Err(error) => {
                    native.inputs.fault(external.id, error);
                    unsafe {
                        context.ClearRenderTargetView(&target.rtv, &[0.0; 4]);
                    }
                    sequence.set(Some(0));
                }
            }
        }
        // Every fallible native allocation/view lookup completed during preparation. From the
        // first command onward, output errors are observations and this token is always marked.
        for pass in plan.passes() {
            let output = &resources.textures[&pass.output];
            match &pass.operation {
                RenderOperation::Clear(color) => pipeline.clear(context, output, *color),
                RenderOperation::Sample { source, filter } => {
                    pipeline.draw(
                        context,
                        &resources.textures[source],
                        None,
                        output,
                        1.0,
                        BlendMode::Normal,
                        *filter,
                    );
                }
                RenderOperation::Composite { layers } => {
                    if layers.is_empty() {
                        pipeline.clear(context, output, LinearColor::TRANSPARENT);
                    }
                    for (index, layer) in layers.iter().enumerate() {
                        let to_output = (layers.len() - index) % 2 == 1;
                        let target = if to_output {
                            output
                        } else {
                            &resources.scratch[&pass.id]
                        };
                        let previous = (index != 0).then(|| {
                            if to_output {
                                &resources.scratch[&pass.id]
                            } else {
                                output
                            }
                        });
                        pipeline.draw(
                            context,
                            &resources.textures[&layer.source],
                            previous,
                            target,
                            layer.opacity.get(),
                            layer.blend,
                            layer.filter,
                        );
                    }
                }
            }
        }
        for output in plan.outputs() {
            let converted = &resources.outputs[&output.id];
            pipeline.draw(
                context,
                &resources.textures[&output.source],
                None,
                converted,
                1.0,
                BlendMode::Normal,
                SampleFilter::Linear,
            );
            let delivered = native.outputs.deliver(output.id, context, converted);
            self.control.update(|state| match delivered {
                Delivery::Accepted => state.accepted_outputs = state.accepted_outputs.saturating_add(1),
                Delivery::Busy => state.busy_outputs = state.busy_outputs.saturating_add(1),
                Delivery::Unavailable => state.unavailable_outputs = state.unavailable_outputs.saturating_add(1),
                Delivery::Failed(error) => {
                    state.failed_outputs = state.failed_outputs.saturating_add(1);
                    state.failure = Some(NativeFailure::Output);
                    state.hresult = Some(error.0);
                }
            });
        }
        drop(submission);
        self.control
            .update(|state| state.submitted = state.submitted.saturating_add(1));
        Ok(())
    }

    fn completion(&mut self) -> Result<CompletionToken, BackendError> {
        let native = self.native.as_mut().ok_or(BackendError::DeviceLost)?;
        if native.device.removed() {
            return Err(BackendError::DeviceLost);
        }
        let completed = native
            .completion
            .completed(&native.device, self.generation)
            .map_err(|_| {
                if native.device.removed() {
                    BackendError::DeviceLost
                } else {
                    BackendError::SubmissionFailed
                }
            })?;
        native.inputs.retire(completed.serial);
        Ok(completed)
    }

    fn wait_for_completion(&mut self, timeout: Duration) -> Result<(), BackendError> {
        self.completion()?;
        std::thread::sleep(timeout.min(Duration::from_millis(20)));
        Ok(())
    }

    fn reset(&mut self, device: DeviceGeneration) -> Result<(), BackendError> {
        // Core calls only after confirmed removal and old generation disposal. The baseline
        // reservation survives this destruction/recreation, including unsuccessful attempts.
        self.native = None;
        self.generation = device;
        self.allocate()
    }
}

impl Drop for D3d11Backend {
    fn drop(&mut self) {
        self.native = None;
        self.control.update(|state| state.initialized = false);
    }
}

fn bytes(descriptor: TextureDescriptor) -> Result<u64, BackendError> {
    descriptor.byte_len().map_err(|_| BackendError::ResourceSizeOverflow)
}

#[cfg(test)]
mod tests;
