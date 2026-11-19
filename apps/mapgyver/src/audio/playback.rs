//! One cancellable PCM producer per host. Video clock reads, file decode and mixing stay off
//! Golden's audio render worker and off the project actor.
mod decoder;
mod matrix;
#[cfg(all(windows, target_arch = "x86_64"))]
mod ndi;
#[cfg(all(windows, target_arch = "x86_64"))]
pub(super) mod ndi_screen;
mod source;

use super::{preferences, voice};
use crate::{app::AppNode, media::video::registry};
use golden_audio::{
    AudioChannelId, AudioConfiguration, AudioEngineConfig, ConfigGeneration, EngineLimits, PcmPlaybackWriter,
};
use golden_core::{application::ProductionRuntime, parameter::ParameterEnumOptions};
use golden_io::{BoundedWorker, RetirementPool, WorkerLimits};
use source::Sources;
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(super) const QUEUE_BLOCKS: usize = 16;
const LOOKAHEAD_BLOCKS: u64 = 10;
const PRIME_BLOCKS: u64 = 2;

pub(super) struct Layout {
    generation: ConfigGeneration,
    lanes: HashMap<AudioChannelId, usize>,
    device_buffer_frames: u64,
    #[cfg(all(windows, target_arch = "x86_64"))]
    virtual_lanes: HashMap<AudioChannelId, usize>,
    local_output_ready: bool,
}

#[derive(Clone)]
pub(super) struct Output(Arc<Mutex<OutputState>>);
struct OutputState {
    layout: Option<Arc<Layout>>,
    options: ParameterEnumOptions,
}

impl Default for Output {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(OutputState {
            layout: None,
            options: voice::default_options(),
        })))
    }
}

impl Output {
    pub fn publish(
        &self,
        settings: &preferences::Settings,
        applied: Option<(&AudioConfiguration, ConfigGeneration, Option<u32>)>,
    ) {
        let mut state = self.0.lock().unwrap();
        state.options = std::iter::once(preferences::option(voice::UNASSIGNED, voice::UNASSIGNED.into()))
            .chain(
                settings
                    .outputs
                    .iter()
                    .map(|output| preferences::option(&output.key, output.label.clone())),
            )
            .collect::<Vec<_>>()
            .into();
        let Some((config, generation, device_buffer_frames)) = applied else {
            state.layout = None;
            return;
        };
        let local_output_ready = device_buffer_frames.is_some();
        let device_buffer_frames = u64::from(device_buffer_frames.unwrap_or(0));
        if state.layout.as_ref().is_some_and(|current| {
            current.generation == generation
                && current.device_buffer_frames == device_buffer_frames
                && current.local_output_ready == local_output_ready
        }) {
            return;
        }
        let lanes = config
            .playback_patch
            .iter()
            .filter(|route| {
                local_output_ready
                    && config
                        .output_patch
                        .iter()
                        .any(|patch| patch.source == route.destination)
            })
            .map(|route| (route.destination, usize::from(route.source_channel)))
            .collect();
        state.layout = Some(Arc::new(Layout {
            generation,
            lanes,
            device_buffer_frames,
            #[cfg(all(windows, target_arch = "x86_64"))]
            virtual_lanes: config
                .playback_patch
                .iter()
                .map(|route| (route.destination, usize::from(route.source_channel)))
                .collect(),
            local_output_ready,
        }));
    }

    pub fn disable(&self) {
        self.0.lock().unwrap().layout = None;
    }
}

pub(super) fn start(
    runtime: ProductionRuntime<AppNode>,
    output: Output,
    mut writer: PcmPlaybackWriter,
    config: &AudioEngineConfig,
    limits: &EngineLimits,
    pool: &RetirementPool,
    #[cfg(all(windows, target_arch = "x86_64"))] mut ndi_tap: golden_audio::VirtualOutputTapReader,
) -> io::Result<BoundedWorker<(), ()>> {
    let frames = config.internal_block_frames.get() as u64;
    let rate = config.sample_rate.get();
    let channels = usize::from(limits.max_virtual_outputs);
    let voices = usize::from(limits.max_voices);
    BoundedWorker::spawn(
        "mapgyver-video-audio",
        pool,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 1,
            event_weight: 1,
        },
        move |mut context| {
            #[cfg(windows)]
            let _ = context.enable_precise_deadlines();
            #[cfg(not(windows))]
            let _ = &mut context;
            let mut sources = Sources::new(voices);
            #[cfg(all(windows, target_arch = "x86_64"))]
            let mut ndi = ndi::Mix::default();
            #[cfg(all(windows, target_arch = "x86_64"))]
            let mut ndi_screens = ndi_screen::Mix::default();
            let mut project: Option<registry::Source> = None;
            let mut catalog_due = Instant::now();
            let mut next_frame = None;
            let mut current_layout: Option<Arc<Layout>> = None;
            let mut samples = vec![0.0; frames as usize * channels];
            let mut root = PathBuf::new();
            while !context.cancellation().is_cancelled() {
                let checkpoint = context.wake_checkpoint();
                let now = Instant::now();
                if project.as_ref().is_some_and(|project| !project.guard.is_current()) {
                    writer.invalidate();
                    sources = Sources::new(voices);
                    #[cfg(all(windows, target_arch = "x86_64"))]
                    ndi.clear();
                    #[cfg(all(windows, target_arch = "x86_64"))]
                    {
                        ndi_tap.set_enabled(false);
                        ndi_tap.drain(8, |_| {});
                        ndi_screens = ndi_screen::Mix::default();
                    }
                    project = None;
                    next_frame = None;
                    catalog_due = now;
                }
                let refresh_sources = now >= catalog_due;
                if refresh_sources {
                    catalog_due = now + Duration::from_millis(40);
                    if project.is_none() {
                        project = runtime.capture_app(registry::Source::capture).ok().and_then(Result::ok);
                    }
                    if let Some(project) = &project {
                        let options = output.0.lock().unwrap().options.clone();
                        root = project_root(&runtime);
                        sources.reconcile(project, &root, &options, now);
                        project.matrix_audio.options(&options, now);
                        #[cfg(all(windows, target_arch = "x86_64"))]
                        ndi_screens.refresh(project.ndi_outputs.as_deref(), &options);
                    }
                }
                let layout = output.0.lock().unwrap().layout.clone();
                let layout_changed = match (&current_layout, &layout) {
                    (Some(previous), Some(new)) => !Arc::ptr_eq(previous, new),
                    (None, None) => false,
                    _ => true,
                };
                if layout_changed {
                    writer.invalidate();
                    current_layout = layout.clone();
                    next_frame = None;
                }
                #[cfg(all(windows, target_arch = "x86_64"))]
                let playback_lanes = ndi_screens.service(layout.as_deref(), &mut ndi_tap);
                if let (Some(layout), Some(project)) = (layout.as_deref(), &project) {
                    let mix = project.matrix_audio.collect(now);
                    #[cfg(not(all(windows, target_arch = "x86_64")))]
                    let playback_lanes = &layout.lanes;
                    let (active, changed) =
                        sources.observe(playback_lanes, now, refresh_sources || layout_changed, mix, &root);
                    #[cfg(all(windows, target_arch = "x86_64"))]
                    let (active, changed) = {
                        let options = output.0.lock().unwrap().options.clone();
                        let (ndi_active, ndi_changed) = ndi.observe(
                            &runtime,
                            &project.ndi_inputs,
                            layout,
                            &options,
                            refresh_sources || layout_changed,
                            now,
                        );
                        (active || ndi_active, changed || ndi_changed)
                    };
                    if changed {
                        writer.invalidate();
                        next_frame = None;
                    }
                    let cursor = writer.cursor();
                    if active && cursor.generation == layout.generation {
                        let mut frame = next_frame.unwrap_or(0).max(cursor.next_frame + PRIME_BLOCKS * frames);
                        let end = cursor.next_frame + LOOKAHEAD_BLOCKS * frames;
                        while frame < end && project.guard.is_current() && !context.cancellation().is_cancelled() {
                            let offsets = frame - cursor.next_frame
                                + cursor.queued_output_frames as u64
                                + layout.device_buffer_frames;
                            let present = cursor.observed_at + Duration::from_nanos(frame_nanos(offsets, rate));
                            samples.fill(0.0);
                            sources.mix(
                                &mut samples,
                                present,
                                frames as usize,
                                rate,
                                &project.guard,
                                context.cancellation(),
                            );
                            #[cfg(all(windows, target_arch = "x86_64"))]
                            ndi.render(&mut samples, present, frames as usize, rate);
                            if !project.guard.is_current()
                                || !writer.try_write(layout.generation, frame, &samples).unwrap_or(false)
                            {
                                break;
                            }
                            frame += frames;
                        }
                        next_frame = Some(frame);
                    }
                } else {
                    sources.release_inactive();
                    #[cfg(all(windows, target_arch = "x86_64"))]
                    ndi.clear();
                }
                let interval = if layout.is_some() {
                    Duration::from_millis(3)
                } else {
                    Duration::from_millis(30)
                };
                context.wait_until(checkpoint, Instant::now() + interval);
            }
            writer.invalidate();
        },
    )
    .map_err(io::Error::other)
}

fn project_root(runtime: &ProductionRuntime<AppNode>) -> PathBuf {
    let file = runtime.read_model().current_project_file();
    file.current_path
        .as_deref()
        .and_then(|path| Path::new(path).parent())
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

fn frame_nanos(frames: u64, rate: u32) -> u64 {
    (u128::from(frames) * 1_000_000_000 / u128::from(rate)).min(u128::from(u64::MAX)) as u64
}
