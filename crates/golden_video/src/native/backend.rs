use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use golden_io::WorkerNotifier;
use golden_time::TimeSnapshot;
use windows::Win32::Foundation::{E_INVALIDARG, E_NOINTERFACE};
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_INVALID_CALL, DXGI_ERROR_UNSUPPORTED};
use windows::core::HRESULT;

use crate::plan::{BlendMode, CompiledPlan, ExternalSource, RenderOperation, SampleFilter};
use crate::runtime::backend::{Backend, BackendError};
use crate::runtime::{CompletionToken, DeviceGeneration, GenerationInfo};
use crate::{LinearColor, OutputId, PassId, TextureDescriptor, TextureId};

use super::completion::EventCompletion;
use super::device::Device;
use super::inputs::{CopyFrame, SourceDemands, SourcePool};
use super::outputs::OutputPool;
use super::pipeline::{CONSTANT_BYTES, Pipeline};
use super::texture::Texture;
use super::timing::{
    CpuScope, DiagnosticContext, DiagnosticEvent, FrameCpuTrace, GpuTiming, OutputTrace, Recorder, SourceUseTrace,
};
use super::{D3d11Config, D3d11Control, InputLimits, NativeFailure, OutputBinding};

mod resources;

struct NativeState {
    #[cfg(all(test, feature = "qualification"))]
    copy_tap: Option<tests::copy_tap::Tap>,
    textures: resources::TextureCache,
    timing: Option<GpuTiming>,
    inputs: SourcePool,
    outputs: OutputPool,
    pipeline: Pipeline,
    completion: EventCompletion,
    device: Device,
}

impl Drop for NativeState {
    fn drop(&mut self) {
        #[cfg(all(test, feature = "qualification"))]
        if let Some(tap) = &mut self.copy_tap {
            tap.finish(&self.device);
        }
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
    diagnostics: Option<Recorder>,
    schedule_clock: super::timing::ScheduleClockSource,
    schedule_epoch: Option<golden_time::TimeEpoch>,
}

pub(crate) struct D3d11Resources {
    generation: GenerationInfo,
    textures: BTreeMap<TextureId, Rc<Texture>>,
    scratch: BTreeMap<PassId, Rc<Texture>>,
    outputs: BTreeMap<OutputId, Rc<Texture>>,
    input_sequences: BTreeMap<TextureId, Cell<Option<CopyFrame>>>,
    _demands: Option<SourceDemands>,
}

impl D3d11Backend {
    #[cfg(test)]
    pub(crate) fn new(config: D3d11Config, control: D3d11Control, generation: DeviceGeneration) -> Self {
        let diagnostics = config.diagnostics.map(Recorder::new);
        Self::with_diagnostics(config, control, generation, diagnostics)
    }

    pub(crate) fn with_diagnostics(
        config: D3d11Config,
        control: D3d11Control,
        generation: DeviceGeneration,
        diagnostics: Option<Recorder>,
    ) -> Self {
        *control.diagnostics.lock().unwrap_or_else(|error| error.into_inner()) = diagnostics.clone();
        Self {
            config,
            control,
            generation,
            native: None,
            notify: None,
            diagnostics,
            schedule_clock: super::timing::ScheduleClockSource::new(false),
            schedule_epoch: None,
        }
    }

    pub(crate) fn with_notifier(mut self, notifier: WorkerNotifier) -> Self {
        self.notify = Some(notifier);
        self
    }

    pub(crate) fn with_schedule_clock(
        mut self,
        source: super::timing::ScheduleClockSource,
        epoch: golden_time::TimeEpoch,
    ) -> Self {
        self.schedule_clock = source;
        self.schedule_epoch = Some(epoch);
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
                    InputLimits::source_storage_bytes(source.descriptor)
                        .map_err(|_| BackendError::ResourceSizeOverflow)?,
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
        debug_assert!(self.native.is_none());
        self.control.clear_outputs();
        self.control.update(|state| {
            state.initialized = false;
            state.adapter = None;
            state.device = None;
            state.cached_texture_entries = 0;
            state.initialization_attempts = state.initialization_attempts.saturating_add(1);
        });
        let result = (|| {
            let selected = self.config.adapter;
            #[cfg(test)]
            let selected = self
                .control
                .initialization_gate
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .as_ref()
                .map_or(selected, |gate| gate.adapter(selected));
            let device = Device::new(selected)?;
            let pipeline = Pipeline::new(&device.device)?;
            let completion = EventCompletion::new(&device)?;
            let timing = self
                .diagnostics
                .as_ref()
                .map(|recorder| GpuTiming::new(&device.device, recorder.clone(), DiagnosticContext::Renderer));
            let outputs = OutputPool::new(
                &device.device,
                self.config.clone(),
                self.control.clone(),
                self.generation,
            )?;
            let inputs = SourcePool::new(
                self.config.adapter,
                self.config.inputs,
                &self.control,
                self.notify.clone(),
                self.generation,
            );
            Ok::<_, windows::core::Error>(NativeState {
                #[cfg(all(test, feature = "qualification"))]
                copy_tap: tests::copy_tap::Tap::new(&device, self.control.copy_tap.clone())?,
                textures: Default::default(),
                timing,
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
                    state.hresult = None;
                });
                Ok(())
            }
            Err(error) => {
                self.control.clear_outputs();
                self.control.update(|state| {
                    state.initialized = false;
                    state.adapter = None;
                    state.device = None;
                    state.failure = Some(NativeFailure::Initialization);
                    state.hresult = Some(error.code().0);
                });
                Err(initialization_error(error.code()))
            }
        }
    }
}

impl D3d11Backend {
    fn close_diagnostics(&mut self) {
        if let Some(native) = &mut self.native
            && let Some(timing) = &mut native.timing
        {
            if native.device.removed() {
                timing.abandon();
            } else if let Ok(done) = native.completion.completed(&native.device, self.generation) {
                timing.finish(&native.device.context, done);
            } else {
                timing.abandon();
            }
        }
    }
}

impl Backend for D3d11Backend {
    type Resources = D3d11Resources;

    fn observe_install(&mut self, generation: GenerationInfo) {
        use crate::runtime::PlanObserver;
        if let Some(recorder) = &self.diagnostics {
            recorder.observe(generation.identity, crate::runtime::PlanStage::Installed { generation });
        }
    }

    fn observes_schedule(&self) -> bool {
        self.diagnostics.is_some()
    }

    fn observe_schedule(
        &mut self,
        device: DeviceGeneration,
        schedule: crate::runtime::FrameSchedule,
        admission: crate::runtime::FrameAdmission,
    ) {
        use super::timing::{ScheduleClock, ScheduleClockError, ScheduleDisposition, ScheduleTrace};
        use crate::runtime::FrameAdmission;
        let Some(recorder) = &self.diagnostics else {
            return;
        };
        let (token, disposition) = match admission {
            FrameAdmission::Submitted(token) => (Some(token), ScheduleDisposition::Submitted),
            FrameAdmission::Busy => (None, ScheduleDisposition::Busy),
            FrameAdmission::NoDemand => (None, ScheduleDisposition::NoDemand),
            FrameAdmission::Rejected => (None, ScheduleDisposition::Rejected),
            FrameAdmission::Fence => (None, ScheduleDisposition::Fence),
        };
        let clock = self.schedule_epoch.map_or(
            ScheduleClock::Unavailable {
                reason: ScheduleClockError::InitializationUnavailable,
            },
            |epoch| self.schedule_clock.project(schedule, epoch),
        );
        recorder.push(super::timing::DiagnosticEvent::Schedule(ScheduleTrace {
            device,
            token,
            cadence: schedule.cadence,
            frame_index: schedule.frame_index,
            anchor_monotonic_ns: schedule.anchor.as_nanos(),
            slot_start_monotonic_ns: schedule.slot_start.as_nanos(),
            slot_end_monotonic_ns: schedule.slot_end.as_nanos(),
            observed_monotonic_ns: schedule.observed.monotonic().as_nanos(),
            rate_numerator: schedule.rate.numerator(),
            rate_denominator: schedule.rate.denominator(),
            coalesced_slots: schedule.coalesced_slots,
            skipped_slots: schedule.skipped_slots,
            epoch_generation: schedule.observed.epoch().generation(),
            clock,
            disposition,
            recorded_qpc: recorder.stamp(),
        }));
    }

    fn base_resource_bytes(&self) -> Result<u64, BackendError> {
        // This accounts every owned texture pool and the constant buffer. DXGI/driver physical
        // residency, alignment and native-object overhead need separate measured qualification.
        let base = CONSTANT_BYTES
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
        if self.native.is_some() { Ok(()) } else { self.allocate() }
    }

    fn quiesce(&mut self) {
        if let Some(native) = &mut self.native {
            native.inputs.quiesce();
            native.outputs.quiesce();
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
            if matches!(binding, OutputBinding::Spout { .. }) {
                total = total
                    .checked_add(bytes(binding.descriptor())?)
                    .ok_or(BackendError::ResourceSizeOverflow)?;
            }
        }
        Ok(total)
    }

    fn prepare(&mut self, plan: &CompiledPlan, generation: GenerationInfo) -> Result<Self::Resources, BackendError> {
        let recorder = self.diagnostics.clone();
        let begin = recorder.as_ref().and_then(Recorder::stamp);
        let mut counts = resources::PreparationCounts::default();
        let prepared = (|| {
            self.validate(plan)?;
            if generation.device != self.generation {
                return Err(BackendError::DeviceLost);
            }
            let native = self.native.as_mut().ok_or(BackendError::DeviceLost)?;
            if native.device.removed() {
                return Err(BackendError::DeviceLost);
            }
            native.inputs.ensure(plan)?;
            let prepared = resources::prepare(native, plan, generation, &mut counts).map_err(|_| {
                if native.device.removed() {
                    BackendError::DeviceLost
                } else {
                    BackendError::AllocationFailed
                }
            });
            native.textures.prune();
            prepared
        })();
        let cache_entries = self.native.as_ref().map_or(0, |native| native.textures.len());
        self.control.update(|state| {
            state.created_textures = state.created_textures.saturating_add(counts.created);
            state.reused_textures = state.reused_textures.saturating_add(counts.reused);
            state.cached_texture_entries = cache_entries;
            state.uploaded_images = state.uploaded_images.saturating_add(counts.uploaded_images);
            state.uploaded_image_bytes = state.uploaded_image_bytes.saturating_add(counts.uploaded_bytes);
        });
        if let Some(recorder) = &recorder {
            recorder.push(DiagnosticEvent::Preparation(super::timing::PreparationTrace {
                generation,
                wall: recorder.interval(begin),
                succeeded: prepared.is_ok(),
                created_textures: counts.created,
                reused_textures: counts.reused,
                cache_entries,
            }));
        }
        prepared
    }

    fn submit(
        &mut self,
        resources: &Self::Resources,
        plan: &CompiledPlan,
        token: CompletionToken,
        time: TimeSnapshot,
    ) -> Result<(), BackendError> {
        let native = self.native.as_mut().ok_or(BackendError::DeviceLost)?;
        if token.device != self.generation || native.device.removed() {
            return Err(BackendError::DeviceLost);
        }
        if !native.completion.can_submit() {
            return Err(BackendError::SubmissionFailed);
        }
        let recorder = self.diagnostics.as_ref();
        let cpu = recorder.map(CpuScope::new);
        let submission = native.completion.begin(&native.device, token);
        let context = &native.device.context;
        let mut gpu = native.timing.as_mut().map(|timing| timing.begin(context, token));
        let inputs_begin = recorder.and_then(Recorder::stamp);
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
                sequence.set(Some(CopyFrame::default()));
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
                    if sequence.get().is_some_and(|frame| frame.sequence != 0) {
                        unsafe {
                            context.ClearRenderTargetView(&target.rtv, &[0.0; 4]);
                        }
                        sequence.set(Some(CopyFrame::default()));
                    }
                }
                Err(error) => {
                    native.inputs.fault(external.id, error);
                    unsafe {
                        context.ClearRenderTargetView(&target.rtv, &[0.0; 4]);
                    }
                    sequence.set(Some(CopyFrame::default()));
                }
            }
            if let Some(recorder) = recorder {
                let frame = sequence.get().unwrap_or_default();
                recorder.push(DiagnosticEvent::SourceUse(SourceUseTrace {
                    token,
                    texture: external.id,
                    sequence: frame.sequence,
                    publication: frame.timing,
                }));
            }
        }
        if let Some(gpu) = &mut gpu {
            gpu.inputs_complete();
        }
        let inputs = recorder.and_then(|recorder| recorder.interval(inputs_begin));
        let render_begin = recorder.and_then(Recorder::stamp);
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
                        output.as_ref().into(),
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
                        let layer = layer.evaluated(time);
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
                        pipeline.draw_layer(
                            context,
                            layer
                                .source
                                .texture()
                                .map(|source| resources.textures[&source].as_ref()),
                            previous.map(Rc::as_ref),
                            target.as_ref().into(),
                            &layer,
                        );
                    }
                }
                RenderOperation::Regions { regions, visibility } => {
                    // Reconstruct every region each frame; hidden/unregistered views cannot
                    // leave pixels behind in a reused target or expose a previous source.
                    pipeline.clear(context, output, LinearColor::TRANSPARENT);
                    if visibility.as_ref().is_none_or(|guard| guard.is_current()) {
                        for region in regions {
                            pipeline.draw_region(context, &resources.textures[&region.source], output, region);
                        }
                    }
                }
            }
        }
        if let Some(gpu) = &mut gpu {
            gpu.render_complete();
        }
        let render = recorder.and_then(|recorder| recorder.interval(render_begin));
        for output in plan.outputs() {
            let converted = resources.outputs.get(&output.id).map(Rc::as_ref);
            let descriptor = native.outputs.descriptor(output.id).expect("validated native output");
            let admission_begin = recorder.and_then(Recorder::stamp);
            let admission = native.outputs.admit(output.id, context, descriptor, token);
            let mut consumer_api = None;
            let mut window_present = None;
            let (conversion, delivery, outcome, spout_image) = match admission {
                Err(outcome) => (
                    None,
                    recorder.and_then(|recorder| recorder.interval(admission_begin)),
                    outcome,
                    None,
                ),
                Ok(mut admitted) => {
                    let conversion_begin = recorder.and_then(Recorder::stamp);
                    let source = &resources.textures[&output.source];
                    // Equal extents map texel centres exactly; avoid four-tap interpolation
                    // while retaining color/alpha conversion. Scaling stays linear.
                    let target = admitted.render_target(converted);
                    let filter = if source.descriptor.extent == target.descriptor.extent {
                        SampleFilter::Nearest
                    } else {
                        SampleFilter::Linear
                    };
                    pipeline.draw(context, source, None, target, 1.0, BlendMode::Normal, filter);
                    let conversion = recorder.and_then(|recorder| recorder.interval(conversion_begin));
                    #[cfg(all(test, feature = "qualification"))]
                    if let Some(tap) = &mut native.copy_tap
                        && let Some(converted) = converted
                    {
                        let input = &resources.textures[&plan.external_textures()[0].id];
                        tap.record(context, input, converted, token);
                    }
                    let delivery_begin = recorder.and_then(Recorder::stamp);
                    let delivered = admitted.deliver(converted, recorder, &mut consumer_api);
                    window_present = delivered.window_present;
                    let delivery = recorder.and_then(|recorder| recorder.interval(delivery_begin));
                    (conversion, delivery, delivered.outcome, delivered.spout_image)
                }
            };
            if let Some(recorder) = recorder {
                recorder.push(DiagnosticEvent::Output(OutputTrace {
                    token,
                    output: output.id,
                    conversion,
                    delivery,
                    consumer_api,
                    outcome: outcome.into(),
                    spout_image,
                    window_present,
                }));
            }
        }
        drop(gpu);
        let completion_begin = recorder.and_then(Recorder::stamp);
        drop(submission);
        let completion = recorder.and_then(|recorder| recorder.interval(completion_begin));
        self.control
            .update(|state| state.submitted = state.submitted.saturating_add(1));
        let removed = native.device.removed();
        if let Some(cpu) = cpu {
            let measured = cpu.finish();
            recorder.unwrap().push(DiagnosticEvent::FrameCpu(FrameCpuTrace {
                generation: resources.generation,
                token,
                wall: measured.wall,
                thread_cpu: measured.thread_cpu,
                inputs,
                render,
                completion,
            }));
        }
        if removed { Err(BackendError::DeviceLost) } else { Ok(()) }
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
        #[cfg(all(test, feature = "qualification"))]
        let completed = self.control.completion_gate.observe(completed);
        native.inputs.retire(completed.serial);
        native.outputs.retire(completed.serial);
        if let Some(timing) = &mut native.timing {
            timing.poll(&native.device.context, completed);
        }
        Ok(completed)
    }

    fn wait_for_completion(&mut self, timeout: Duration) -> Result<(), BackendError> {
        self.completion()?;
        std::thread::sleep(timeout.min(Duration::from_millis(20)));
        Ok(())
    }

    fn reset(&mut self, device: DeviceGeneration) -> Result<(), BackendError> {
        // SAFETY: metadata query on this worker's retained old device, before its destruction.
        #[cfg(all(test, feature = "qualification"))]
        if let Some(native) = &self.native
            && let Err(error) = unsafe { native.device.device.GetDeviceRemovedReason() }
        {
            self.control.completion_gate.removed(self.generation, error.code().0);
        }
        // Core calls only after confirmed removal and old generation disposal. The baseline
        // reservation survives this destruction/recreation, including unsuccessful attempts.
        self.close_diagnostics();
        self.native = None;
        self.generation = device;
        self.allocate()
    }
}

impl Drop for D3d11Backend {
    fn drop(&mut self) {
        self.close_diagnostics();
        self.native = None;
        self.control.clear_outputs();
        self.control.update(|state| {
            state.initialized = false;
            state.adapter = None;
            state.device = None;
        });
    }
}

fn initialization_error(error: HRESULT) -> BackendError {
    match error {
        DXGI_ERROR_INVALID_CALL | DXGI_ERROR_UNSUPPORTED | E_INVALIDARG | E_NOINTERFACE => {
            BackendError::UnsupportedInitialization
        }
        // Missing selected hardware and transient driver/allocation failures can recover on
        // the same admitted baseline. No initialization HRESULT establishes device loss.
        _ => BackendError::AllocationFailed,
    }
}

fn bytes(descriptor: TextureDescriptor) -> Result<u64, BackendError> {
    descriptor.byte_len().map_err(|_| BackendError::ResourceSizeOverflow)
}

#[cfg(test)]
pub(in crate::native) mod tests;
