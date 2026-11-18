use super::CellBinding;
use crate::matrix::{
    playback::{AudioReader, PlaybackControl},
    protocol::LayerId,
    timeline::Interval,
    video::{Key, Prepared},
};
use crate::media::{
    MediaSource,
    protocol::{AssetLocator, ChannelId},
};
use golden_core::parameter::ParameterEnumOptions;
use golden_time::TimeSnapshot;
use golden_video::plan::OpacityEnvelope;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct Registry {
    controls: Mutex<HashMap<uuid::Uuid, AudioReader>>,
    cells: Mutex<CellOptions>,
}

#[derive(Default)]
struct CellOptions {
    bindings: HashMap<uuid::Uuid, Weak<CellBinding>>,
    last: Option<ParameterEnumOptions>,
    cleanup: Option<Instant>,
}

impl Registry {
    pub fn register_matrix(&self, id: uuid::Uuid, reader: AudioReader) {
        self.controls.lock().unwrap().insert(id, reader);
    }
    pub fn register_cell(&self, id: uuid::Uuid, binding: Weak<CellBinding>) {
        let mut state = self.cells.lock().unwrap();
        if let Some(options) = &state.last
            && let Some(cell) = binding.upgrade()
        {
            cell.options.update_options(options);
        }
        state.bindings.insert(id, binding);
    }
    pub fn options(&self, options: &ParameterEnumOptions, now: Instant) {
        let mut state = self.cells.lock().unwrap();
        if state.cleanup.is_none_or(|due| due <= now) {
            state.cleanup = Some(now + Duration::from_secs(1));
            state.bindings.retain(|_, binding| binding.strong_count() != 0);
            self.controls.lock().unwrap().retain(|_, reader| reader.alive());
        }
        if state.last.as_ref() == Some(options) {
            return;
        }
        state.bindings.retain(|_, binding| {
            if let Some(binding) = binding.upgrade() {
                binding.options.update_options(options);
                true
            } else {
                false
            }
        });
        state.last = Some(options.clone());
    }
    pub fn collect(&self, now: Instant) -> Mix {
        let mut controls = Vec::<PlaybackControl>::new();
        self.controls.lock().unwrap().retain(|_, reader| {
            if let Some(control) = reader.upgrade() {
                controls.push(control);
                true
            } else {
                reader.alive()
            }
        });
        let mut mix = Mix::default();
        for control in controls {
            if let Err(err) = control.capture_audio(now, &mut mix) {
                eprintln!("[mapgyver-matrix] Audio mix: {err}");
            }
        }
        mix
    }
}

#[derive(Default)]
pub(crate) struct Mix {
    pub media: HashMap<ChannelId, Vec<Weight>>,
    pub files: Vec<FileUse>,
}

#[derive(Clone, Copy)]
pub(crate) struct Weight {
    fixed: f32,
    envelope: Option<OpacityEnvelope>,
    now: Instant,
    time: TimeSnapshot,
}

impl Weight {
    pub fn at(self, instant: Instant) -> f32 {
        self.envelope
            .and_then(|envelope| {
                let coordinate = self
                    .time
                    .monotonic()
                    .checked_add(instant.saturating_duration_since(self.now))
                    .ok()?;
                let time = self.time.at_monotonic(coordinate).ok()?;
                Some(envelope.sample(time).get())
            })
            .unwrap_or(self.fixed)
    }
}

pub(crate) struct FileUse {
    pub id: (LayerId, uuid::Uuid),
    pub file: AssetLocator,
    pub start: Instant,
    pub weight: Weight,
    pub routing: crate::audio::voice::Routing,
}

struct Launch {
    id: uuid::Uuid,
    start: Instant,
    channel: Option<ChannelId>,
    selection: Option<MediaSource>,
    binding: Arc<CellBinding>,
    enabled: Option<Arc<AtomicBool>>,
}

#[derive(Default)]
pub(in crate::matrix) struct Takes(HashMap<(LayerId, Key), Launch>);

impl Takes {
    pub fn track(&mut self, plan: &Prepared, timeline: &crate::matrix::timeline::Timeline, now: Instant) {
        // An unobserved launch cannot contribute outgoing audio. Retain only contributions
        // that reached the timeline and the committed destination, even with no audio device.
        self.0.retain(|&(layer, key), take| {
            if let Some(reference) = take.selection.and_then(|selection| plan.audio_route(selection)) {
                take.channel = reference.map(|reference| reference.channel);
            }
            plan.rows.get(&layer).is_some_and(|row| row.key == Some(key))
                || timeline.contains_video_clip(layer, key.column, key.version, key.individual)
        });
        for (&layer, row) in &plan.rows {
            let (Some(key), Some(id), Some(binding)) = (row.key, row.identity, &row.audio) else {
                continue;
            };
            if row.source.is_none() && !binding.snapshot().is_some_and(|settings| settings.override_media) {
                continue;
            }
            let start = self
                .0
                .get(&(layer, key))
                .filter(|old| old.id == id)
                .map_or(now, |old| old.start);
            self.0.insert(
                (layer, key),
                Launch {
                    id,
                    start,
                    channel: row.source.as_ref().map(|(reference, _)| reference.channel),
                    selection: row.selection,
                    binding: binding.clone(),
                    enabled: row.audio_column.clone(),
                },
            );
        }
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn capture(&mut self, frame: &crate::matrix::timeline::Frame, now: Instant, time: TimeSnapshot, mix: &mut Mix) {
        self.0.retain(|&(layer, key), take| {
            let frame = frame.layer(layer);
            let Some(entry) = frame.clips.iter().find(|entry| {
                let clip = entry.clip;
                (clip.column, clip.version, clip.individual) == (key.column, key.version, key.individual)
            }) else {
                return false;
            };
            let inherit = take.channel.map(|channel| mix.media.entry(channel).or_default());
            if take
                .enabled
                .as_ref()
                .is_some_and(|value| !value.load(Ordering::Acquire))
            {
                return true;
            }
            let Some(settings) = take.binding.snapshot().filter(|settings| settings.enabled) else {
                return true;
            };
            let interval: Option<Interval> = frame.interval;
            let weight = Weight {
                fixed: entry.to,
                envelope: interval.map(|interval| interval.envelope(entry.from, entry.to)),
                now,
                time,
            };
            if settings.override_media {
                if let Some(file) = settings.file {
                    mix.files.push(FileUse {
                        id: (layer, take.id),
                        file,
                        start: take.start,
                        weight,
                        routing: settings.routing,
                    });
                }
            } else if let Some(inherit) = inherit {
                inherit.push(weight);
            }
            true
        });
    }
}
