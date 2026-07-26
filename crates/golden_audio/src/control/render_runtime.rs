use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle, Thread},
    time::{Duration, Instant},
};

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use crate::{
    AnalysisController, AnalysisObservationSnapshot, AudioCallbackTimestamp, AudioChannelId, AudioDirection,
    AudioEngineConfig, AudioError, AudioErrorCategory, AudioStreamHandler, ClockBridgeConfig, ConfigGeneration,
    DriftControllerConfig, EngineLimits, GainDb, InputClockReader, InputClockWriter, InputReadError, InputWriteError,
    InterleavedInput, InterleavedOutput, PlanPublishError, PlanarBuffer, PlaybackRendererRetirement,
    PlaybackVoiceRenderer, RealtimePlanRetirement, RealtimePlanSlot, RealtimeScope, RenderPlan, RenderPlanPublisher,
    RenderProcessor, RenderRuntimeObservation, acknowledged_plan_exchange, analysis_pipeline, input_clock_bridge,
};

const CONTROL_QUEUE_CAPACITY: usize = 4_096;
const OUTPUT_QUEUE_BLOCKS: usize = 8;
const INPUT_RING_CAPACITY_FRAMES: usize = 8_192;
const BRIDGE_PUBLISH_ATTEMPTS: usize = 50;

#[derive(Clone, Copy, Debug)]
enum RenderControl {
    MasterGain(GainDb),
    ChannelGain { channel: AudioChannelId, gain: GainDb },
}

#[derive(Debug)]
struct RenderRuntimePlan {
    processor: RenderProcessor,
    physical_inputs: PlanarBuffer,
    physical_outputs: PlanarBuffer,
}

impl RenderRuntimePlan {
    fn new(
        generation: ConfigGeneration,
        plan: RenderPlan,
        limits: &EngineLimits,
    ) -> Result<(Self, AnalysisController), AudioError> {
        let physical_input_channels = plan.physical_inputs.len();
        let physical_output_channels = plan.physical_outputs.len();
        let block_frames = plan.internal_block_frames.get() as usize;
        let (analysis_controller, analysis_renderer) = analysis_pipeline(generation, &plan, limits)?;
        let mut processor = RenderProcessor::new(plan)?;
        processor.attach_analysis(analysis_renderer)?;
        Ok((
            Self {
                processor,
                physical_inputs: PlanarBuffer::new(physical_input_channels, block_frames)?,
                physical_outputs: PlanarBuffer::new(physical_output_channels, block_frames)?,
            },
            analysis_controller,
        ))
    }
}

#[derive(Debug)]
struct InputRuntimeBridge {
    reader: InputClockReader,
    channels: usize,
}

#[derive(Debug)]
struct OutputRuntimeBridge {
    producer: Producer<f32>,
    channels: usize,
}

#[derive(Debug, Default)]
struct SharedRenderObservation {
    rendered_blocks: AtomicU64,
    rendered_frames: AtomicU64,
    render_time_micros: AtomicU64,
    maximum_render_time_micros: AtomicU64,
    deadline_miss_count: AtomicU64,
    control_queue_pressure_count: AtomicU64,
    input_underflow_count: AtomicU64,
    input_overflow_count: AtomicU64,
    output_underflow_count: AtomicU64,
    output_overflow_count: AtomicU64,
}

impl SharedRenderObservation {
    fn snapshot(&self) -> RenderRuntimeObservation {
        let deadline_miss_count = self.deadline_miss_count.load(Ordering::Relaxed);
        let input_underflow_count = self.input_underflow_count.load(Ordering::Relaxed);
        let input_overflow_count = self.input_overflow_count.load(Ordering::Relaxed);
        let output_underflow_count = self.output_underflow_count.load(Ordering::Relaxed);
        let output_overflow_count = self.output_overflow_count.load(Ordering::Relaxed);
        RenderRuntimeObservation {
            rendered_blocks: self.rendered_blocks.load(Ordering::Relaxed),
            rendered_frames: self.rendered_frames.load(Ordering::Relaxed),
            render_time_micros: self.render_time_micros.load(Ordering::Relaxed),
            maximum_render_time_micros: self.maximum_render_time_micros.load(Ordering::Relaxed),
            deadline_miss_count,
            xrun_count: deadline_miss_count
                .saturating_add(input_underflow_count)
                .saturating_add(input_overflow_count)
                .saturating_add(output_underflow_count)
                .saturating_add(output_overflow_count),
            control_queue_pressure_count: self.control_queue_pressure_count.load(Ordering::Relaxed),
            input_underflow_count,
            input_overflow_count,
            output_underflow_count,
            output_overflow_count,
        }
    }
}

#[derive(Debug)]
struct RuntimeThreadRetirement {
    plans: RealtimePlanRetirement<Option<RenderRuntimePlan>>,
    inputs: RealtimePlanRetirement<Option<InputRuntimeBridge>>,
    outputs: RealtimePlanRetirement<Option<OutputRuntimeBridge>>,
    playback: PlaybackRendererRetirement,
}

#[derive(Debug)]
pub(super) struct ManagedRenderRuntime {
    plan_publisher: RenderPlanPublisher<Option<RenderRuntimePlan>>,
    input_publisher: RenderPlanPublisher<Option<InputRuntimeBridge>>,
    output_publisher: RenderPlanPublisher<Option<OutputRuntimeBridge>>,
    controls: Producer<RenderControl>,
    observation: Arc<SharedRenderObservation>,
    active_analysis: Option<AnalysisController>,
    pending_analysis: Option<AnalysisController>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<RuntimeThreadRetirement>>,
    limits: EngineLimits,
    config: AudioEngineConfig,
}

impl ManagedRenderRuntime {
    pub(super) fn start(
        config: &AudioEngineConfig,
        limits: &EngineLimits,
        playback: PlaybackVoiceRenderer,
    ) -> Result<Self, AudioError> {
        let (plan_publisher, plans) = acknowledged_plan_exchange(Box::new(None));
        let (input_publisher, inputs) = acknowledged_plan_exchange(Box::new(None));
        let (output_publisher, outputs) = acknowledged_plan_exchange(Box::new(None));
        let (controls, control_reader) = RingBuffer::new(CONTROL_QUEUE_CAPACITY);
        let observation = Arc::new(SharedRenderObservation::default());
        let thread_observation = Arc::clone(&observation);
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let block_frames = config.internal_block_frames.get() as usize;
        let sample_rate = config.sample_rate.get();
        let playback_buffer = PlanarBuffer::new(usize::from(limits.max_virtual_outputs), block_frames)?;
        let worker = thread::Builder::new()
            .name("golden-audio-render".to_owned())
            .spawn(move || {
                run_render_thread(
                    plans,
                    inputs,
                    outputs,
                    control_reader,
                    playback,
                    playback_buffer,
                    block_frames,
                    sample_rate,
                    thread_observation,
                    thread_shutdown,
                )
            })
            .map_err(|error| {
                AudioError::new(
                    AudioErrorCategory::InternalInvariant,
                    format!("failed to start managed audio render thread: {error}"),
                )
            })?;
        Ok(Self {
            plan_publisher,
            input_publisher,
            output_publisher,
            controls,
            observation,
            active_analysis: None,
            pending_analysis: None,
            shutdown,
            thread: Some(worker),
            limits: limits.clone(),
            config: config.clone(),
        })
    }

    pub(super) fn publish_plan(&mut self, generation: ConfigGeneration, plan: RenderPlan) -> Result<(), AudioError> {
        self.reclaim();
        let (runtime_plan, analysis) = RenderRuntimePlan::new(generation, plan, &self.limits)?;
        self.plan_publisher
            .publish(Box::new(Some(runtime_plan)))
            .map_err(|error| publish_error(error, "managed_render_plan"))?;
        self.pending_analysis = Some(analysis);
        self.unpark();
        Ok(())
    }

    pub(super) fn prepare_stream_handler(
        &mut self,
        direction: AudioDirection,
        channels: u16,
    ) -> Result<Box<dyn AudioStreamHandler>, AudioError> {
        if channels == 0 {
            return Err(AudioError::invalid_configuration(
                "managed stream handler requires at least one channel",
            ));
        }
        match direction {
            AudioDirection::Input => self.prepare_input_handler(channels),
            AudioDirection::Output => self.prepare_output_handler(channels),
        }
    }

    pub(super) fn disable_stream_handler(&mut self, direction: AudioDirection) -> Result<(), AudioError> {
        match direction {
            AudioDirection::Input => self.publish_input_bridge(None),
            AudioDirection::Output => self.publish_output_bridge(None),
        }
    }

    pub(super) fn set_master_gain(&mut self, gain: GainDb) -> Result<(), AudioError> {
        self.push_control(RenderControl::MasterGain(gain))
    }

    pub(super) fn set_channel_gain(&mut self, channel: AudioChannelId, gain: GainDb) -> Result<(), AudioError> {
        self.push_control(RenderControl::ChannelGain { channel, gain })
    }

    pub(super) fn refresh_observation(&mut self) -> (RenderRuntimeObservation, Option<AnalysisObservationSnapshot>) {
        self.reclaim();
        let analysis = self
            .active_analysis
            .as_ref()
            .map(|controller| controller.observations().latest());
        (self.observation.snapshot(), analysis)
    }

    pub(super) fn shutdown(&mut self) -> Result<(), AudioError> {
        let Some(worker) = self.thread.take() else {
            return Ok(());
        };
        self.shutdown.store(true, Ordering::Release);
        worker.thread().unpark();
        let retirement = worker.join().map_err(|_| {
            AudioError::new(
                AudioErrorCategory::InternalInvariant,
                "managed audio render thread panicked during shutdown",
            )
        })?;
        if let Some(mut analysis) = self.active_analysis.take() {
            analysis.shutdown()?;
        }
        if let Some(mut analysis) = self.pending_analysis.take() {
            analysis.shutdown()?;
        }
        retirement.plans.reclaim();
        retirement.inputs.reclaim();
        retirement.outputs.reclaim();
        retirement.playback.reclaim();
        Ok(())
    }

    fn prepare_input_handler(&mut self, channels: u16) -> Result<Box<dyn AudioStreamHandler>, AudioError> {
        let (writer, reader) = input_clock_bridge(ClockBridgeConfig {
            device_sample_rate: self.config.sample_rate,
            engine_sample_rate: self.config.sample_rate,
            channels,
            engine_block_frames: self.config.internal_block_frames,
            ring_capacity_frames: INPUT_RING_CAPACITY_FRAMES,
            output_buffer_frames: self.config.internal_block_frames.get(),
            drift: DriftControllerConfig::default(),
        })?;
        self.publish_input_bridge(Some(InputRuntimeBridge {
            reader,
            channels: usize::from(channels),
        }))?;
        Ok(Box::new(ManagedInputHandler {
            writer,
            observation: Arc::clone(&self.observation),
            wake: self.render_thread()?,
        }))
    }

    fn prepare_output_handler(&mut self, channels: u16) -> Result<Box<dyn AudioStreamHandler>, AudioError> {
        let channel_count = usize::from(channels);
        let sample_capacity = channel_count
            .checked_mul(self.config.internal_block_frames.get() as usize)
            .and_then(|samples| samples.checked_mul(OUTPUT_QUEUE_BLOCKS))
            .ok_or_else(|| AudioError::capacity_exceeded("managed output queue capacity overflowed"))?;
        let (producer, consumer) = RingBuffer::new(sample_capacity);
        self.publish_output_bridge(Some(OutputRuntimeBridge {
            producer,
            channels: channel_count,
        }))?;
        Ok(Box::new(ManagedOutputHandler {
            consumer,
            channels: channel_count,
            observation: Arc::clone(&self.observation),
            wake: self.render_thread()?,
        }))
    }

    fn publish_input_bridge(&mut self, bridge: Option<InputRuntimeBridge>) -> Result<(), AudioError> {
        let bridge = Box::new(bridge);
        for _ in 0..BRIDGE_PUBLISH_ATTEMPTS {
            self.reclaim();
            if !self.input_publisher.has_pending_plan() {
                self.input_publisher
                    .publish(bridge)
                    .map_err(|error| publish_error(error, "managed_input_bridge"))?;
                self.unpark();
                return Ok(());
            }
            self.unpark();
            thread::sleep(Duration::from_millis(1));
        }
        drop(bridge);
        Err(AudioError::queue_full("managed_input_bridge"))
    }

    fn publish_output_bridge(&mut self, bridge: Option<OutputRuntimeBridge>) -> Result<(), AudioError> {
        let bridge = Box::new(bridge);
        for _ in 0..BRIDGE_PUBLISH_ATTEMPTS {
            self.reclaim();
            if !self.output_publisher.has_pending_plan() {
                self.output_publisher
                    .publish(bridge)
                    .map_err(|error| publish_error(error, "managed_output_bridge"))?;
                self.unpark();
                return Ok(());
            }
            self.unpark();
            thread::sleep(Duration::from_millis(1));
        }
        drop(bridge);
        Err(AudioError::queue_full("managed_output_bridge"))
    }

    fn push_control(&mut self, control: RenderControl) -> Result<(), AudioError> {
        match self.controls.push(control) {
            Ok(()) => {
                self.unpark();
                Ok(())
            }
            Err(PushError::Full(_)) => {
                self.observation
                    .control_queue_pressure_count
                    .fetch_add(1, Ordering::Relaxed);
                Err(AudioError::queue_full("managed_render_control"))
            }
        }
    }

    fn reclaim(&mut self) {
        if self.plan_publisher.reclaim_acknowledged() > 0 {
            if let Some(mut previous) = self.active_analysis.take() {
                let _ = previous.shutdown();
            }
            self.active_analysis = self.pending_analysis.take();
        }
        self.input_publisher.reclaim_acknowledged();
        self.output_publisher.reclaim_acknowledged();
    }

    fn render_thread(&self) -> Result<Thread, AudioError> {
        self.thread
            .as_ref()
            .map(|worker| worker.thread().clone())
            .ok_or_else(|| {
                AudioError::new(
                    AudioErrorCategory::ShuttingDown,
                    "managed render thread is no longer available",
                )
            })
    }

    fn unpark(&self) {
        if let Some(worker) = &self.thread {
            worker.thread().unpark();
        }
    }
}

impl Drop for ManagedRenderRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[derive(Debug)]
struct ManagedInputHandler {
    writer: InputClockWriter,
    observation: Arc<SharedRenderObservation>,
    wake: Thread,
}

impl AudioStreamHandler for ManagedInputHandler {
    fn process_input(&mut self, samples: InterleavedInput<'_>, timestamp: AudioCallbackTimestamp) {
        let device_timestamp = (timestamp.device_nanos != 0).then_some(timestamp.device_nanos);
        if matches!(
            self.writer.write_callback_input(samples, device_timestamp),
            Err(InputWriteError::Overflow | InputWriteError::InvalidShape)
        ) {
            self.observation.input_overflow_count.fetch_add(1, Ordering::Relaxed);
        }
        self.wake.unpark();
    }
}

#[derive(Debug)]
struct ManagedOutputHandler {
    consumer: Consumer<f32>,
    channels: usize,
    observation: Arc<SharedRenderObservation>,
    wake: Thread,
}

impl AudioStreamHandler for ManagedOutputHandler {
    fn process_output(&mut self, mut samples: InterleavedOutput<'_>, _timestamp: AudioCallbackTimestamp) {
        samples.fill_silence();
        if samples.is_empty() || !samples.len().is_multiple_of(self.channels) {
            self.observation.output_underflow_count.fetch_add(1, Ordering::Relaxed);
            self.wake.unpark();
            return;
        }
        let mut underflowed = false;
        for index in 0..samples.len() {
            match self.consumer.pop() {
                Ok(sample) => samples.set_normalized(index, sample),
                Err(_) => {
                    underflowed = true;
                    break;
                }
            }
        }
        if underflowed {
            self.observation.output_underflow_count.fetch_add(1, Ordering::Relaxed);
        }
        self.wake.unpark();
    }
}

#[allow(clippy::too_many_arguments)]
fn run_render_thread(
    mut plans: RealtimePlanSlot<Option<RenderRuntimePlan>>,
    mut inputs: RealtimePlanSlot<Option<InputRuntimeBridge>>,
    mut outputs: RealtimePlanSlot<Option<OutputRuntimeBridge>>,
    mut controls: Consumer<RenderControl>,
    mut playback: PlaybackVoiceRenderer,
    mut playback_buffer: PlanarBuffer,
    block_frames: usize,
    sample_rate: u32,
    observation: Arc<SharedRenderObservation>,
    shutdown: Arc<AtomicBool>,
) -> RuntimeThreadRetirement {
    let block_duration = Duration::from_secs_f64(block_frames as f64 / f64::from(sample_rate));
    let mut next_deadline = Instant::now() + block_duration;
    while !shutdown.load(Ordering::Acquire) {
        {
            let _scope = RealtimeScope::enter();
            let _ = plans.begin_block();
            let _ = inputs.begin_block();
            let _ = outputs.begin_block();
            apply_controls(plans.active_mut(), &mut controls);
        }

        let output_driven = output_has_consumer(outputs.active(), block_frames);
        let now = Instant::now();
        if (!output_driven && now < next_deadline)
            || (output_driven && !output_has_capacity(outputs.active(), block_frames))
        {
            let wait = if output_driven {
                block_duration
            } else {
                next_deadline.duration_since(now)
            };
            thread::park_timeout(wait);
            continue;
        }

        let started = Instant::now();
        {
            let _scope = RealtimeScope::enter();
            render_block(
                plans.active_mut(),
                inputs.active_mut(),
                outputs.active_mut(),
                &mut playback,
                &mut playback_buffer,
                block_frames,
                &observation,
            );
        }
        let elapsed_duration = started.elapsed();
        let elapsed = u64::try_from(elapsed_duration.as_micros()).unwrap_or(u64::MAX);
        observation.rendered_blocks.fetch_add(1, Ordering::Relaxed);
        observation
            .rendered_frames
            .fetch_add(block_frames as u64, Ordering::Relaxed);
        observation.render_time_micros.fetch_add(elapsed, Ordering::Relaxed);
        observation
            .maximum_render_time_micros
            .fetch_max(elapsed, Ordering::Relaxed);
        if elapsed_duration > block_duration {
            observation.deadline_miss_count.fetch_add(1, Ordering::Relaxed);
        }
        if output_driven {
            next_deadline = Instant::now() + block_duration;
        } else {
            next_deadline += block_duration;
            if Instant::now() > next_deadline + block_duration {
                next_deadline = Instant::now() + block_duration;
            }
        }
    }
    RuntimeThreadRetirement {
        plans: plans.retire(),
        inputs: inputs.retire(),
        outputs: outputs.retire(),
        playback: playback.into_retirement(),
    }
}

fn output_has_consumer(output: &Option<OutputRuntimeBridge>, block_frames: usize) -> bool {
    output.as_ref().is_some_and(|bridge| {
        !bridge.producer.is_abandoned()
            && bridge
                .channels
                .checked_mul(block_frames)
                .is_some_and(|samples| samples > 0)
    })
}

fn output_has_capacity(output: &Option<OutputRuntimeBridge>, block_frames: usize) -> bool {
    output.as_ref().is_some_and(|bridge| {
        bridge
            .channels
            .checked_mul(block_frames)
            .is_some_and(|samples| bridge.producer.slots() >= samples)
    })
}

fn render_block(
    plan: &mut Option<RenderRuntimePlan>,
    input: &mut Option<InputRuntimeBridge>,
    output: &mut Option<OutputRuntimeBridge>,
    playback: &mut PlaybackVoiceRenderer,
    playback_buffer: &mut PlanarBuffer,
    block_frames: usize,
    observation: &SharedRenderObservation,
) {
    let _ = playback.render(playback_buffer, block_frames);
    if let Some(plan) = plan {
        fill_physical_input(plan, input.as_mut(), block_frames, observation);
        let _ = plan.processor.render(
            &plan.physical_inputs,
            playback_buffer,
            &mut plan.physical_outputs,
            block_frames,
        );
    }
    if let Some(output) = output {
        write_physical_output(plan.as_ref(), output, block_frames, observation);
    }
}

fn fill_physical_input(
    plan: &mut RenderRuntimePlan,
    input: Option<&mut InputRuntimeBridge>,
    block_frames: usize,
    observation: &SharedRenderObservation,
) {
    let Some(input) = input else {
        let _ = plan.physical_inputs.zero(block_frames);
        return;
    };
    if input.channels != plan.physical_inputs.channels() {
        let _ = plan.physical_inputs.zero(block_frames);
        observation.input_underflow_count.fetch_add(1, Ordering::Relaxed);
        return;
    }
    match input.reader.read_engine_block(&mut plan.physical_inputs) {
        Ok(result) if result.underflowed => {
            observation.input_underflow_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(_) => {}
        Err(InputReadError::InvalidDestination | InputReadError::ResamplerFailure) => {
            let _ = plan.physical_inputs.zero(block_frames);
            observation.input_underflow_count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn write_physical_output(
    plan: Option<&RenderRuntimePlan>,
    output: &mut OutputRuntimeBridge,
    block_frames: usize,
    observation: &SharedRenderObservation,
) {
    let channels_match = plan.is_some_and(|plan| plan.physical_outputs.channels() == output.channels);
    for frame in 0..block_frames {
        for channel in 0..output.channels {
            let sample = if channels_match {
                plan.expect("channel match requires an active plan")
                    .physical_outputs
                    .sample(channel, frame)
            } else {
                0.0
            };
            if output.producer.push(sample).is_err() {
                observation.output_overflow_count.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    }
}

fn apply_controls(plan: &mut Option<RenderRuntimePlan>, controls: &mut Consumer<RenderControl>) {
    while let Ok(control) = controls.pop() {
        let Some(plan) = plan else {
            continue;
        };
        match control {
            RenderControl::MasterGain(gain) => plan.processor.set_master_gain(gain),
            RenderControl::ChannelGain { channel, gain } => {
                let _ = plan.processor.set_output_gain(channel, gain);
            }
        }
    }
}

fn publish_error<T>(error: PlanPublishError<T>, queue: &'static str) -> AudioError {
    drop(error.into_plan());
    AudioError::queue_full(queue)
}
