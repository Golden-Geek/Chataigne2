use super::{Layout, decoder::Decoder, frame_nanos, matrix::Files};
use crate::media::{
    protocol::ChannelId,
    video::{VideoControl, audio::Source, registry},
};
use golden_audio::{GainDb, GainSmoother};
use golden_core::parameter::ParameterEnumOptions;
use golden_io::{RevisionGuard, WorkerCancellation};
use golden_time::TimeEpoch;
use golden_video::decode::{AudioReader, DecodeError};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

pub(super) struct Sources {
    voices: BTreeMap<uuid::Uuid, Voice>,
    membership_changed: bool,
    capacity: usize,
    at_capacity: bool,
    files: Files,
    matrix_media: std::collections::HashMap<ChannelId, Vec<crate::matrix::audio::Weight>>,
}

impl Sources {
    pub fn new(capacity: usize) -> Self {
        Self {
            voices: BTreeMap::new(),
            membership_changed: false,
            capacity,
            at_capacity: false,
            files: Default::default(),
            matrix_media: Default::default(),
        }
    }

    pub fn reconcile(&mut self, project: &registry::Source, root: &Path, options: &ParameterEnumOptions, now: Instant) {
        let mut old = std::mem::take(&mut self.voices);
        for control in project.registry.controls() {
            for source in control.audio_sources() {
                source.binding.update_options(options);
                let (Some(locator), Some(duration)) = (&source.file, source.duration) else {
                    continue;
                };
                let path = locator.resolve(root);
                let repeat = source
                    .repeat
                    .as_ref()
                    .map(|(file, duration)| (file.resolve(root), duration.unwrap_or(Duration::MAX)));
                let next = match old.remove(&source.channel.0.0) {
                    Some(mut voice)
                        if voice.generation == source.generation && voice.path == path && voice.repeat == repeat =>
                    {
                        voice.duration = duration;
                        self.membership_changed |= voice.refresh_file(now);
                        voice
                    }
                    previous => {
                        self.membership_changed |= previous.is_some_and(|voice| voice.active.is_some());
                        Voice::new(&control, path, source, duration, repeat)
                    }
                };
                self.voices.insert(next.channel.0.0, next);
            }
        }
        self.membership_changed |= old.values().any(|voice| voice.active.is_some());
    }

    pub fn observe(
        &mut self,
        layout: &Layout,
        now: Instant,
        scan_inactive: bool,
        mix: crate::matrix::audio::Mix,
        root: &Path,
    ) -> (bool, bool) {
        let mut admitted = 0;
        let mut at_capacity = false;
        let mut changed = std::mem::take(&mut self.membership_changed);
        for voice in self
            .voices
            .values_mut()
            .filter(|voice| scan_inactive || voice.active.is_some() || mix.media.contains_key(&voice.channel))
        {
            let weights = mix.media.get(&voice.channel);
            let active =
                if weights.is_some_and(Vec::is_empty) || (weights.is_none() && !voice.control.standalone_audio()) {
                    None
                } else {
                    voice.next_voice(layout, now)
                };
            let active = active.filter(|_| {
                if admitted == self.capacity {
                    at_capacity = true;
                    return false;
                }
                admitted += 1;
                true
            });
            if voice.active != active {
                changed = true;
                voice.active = active;
            }
            if active.is_none() {
                voice.release();
            }
        }
        if scan_inactive {
            if at_capacity && !self.at_capacity {
                eprintln!(
                    "[mapgyver-audio] Simultaneous Video audio channel limit reached ({})",
                    self.capacity
                );
            }
            self.at_capacity = at_capacity;
        }
        self.matrix_media = mix.media;
        let (file_active, file_changed) = self.files.reconcile(mix.files, root, layout, self.capacity - admitted);
        (admitted != 0 || file_active, changed || file_changed)
    }

    pub fn mix(
        &mut self,
        output: &mut [f32],
        present: Instant,
        frames: usize,
        rate: u32,
        guard: &RevisionGuard,
        cancel: &WorkerCancellation,
    ) {
        for voice in self.voices.values_mut().filter(|voice| voice.active.is_some()) {
            if !guard.is_current() || cancel.is_cancelled() {
                break;
            }
            let weights = self.matrix_media.get(&voice.channel).map(Vec::as_slice);
            voice.mix(output, present, frames, rate, guard, cancel, weights);
        }
        self.files.mix(output, present, frames, rate, guard, cancel);
    }

    pub fn release_inactive(&mut self) {
        self.files = Files::default();
        for voice in self.voices.values_mut().filter(|voice| voice.active.is_some()) {
            voice.active = None;
            voice.release();
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Active {
    epoch: TimeEpoch,
    lanes: [Option<usize>; 2],
}

#[derive(Default)]
pub(super) struct Head {
    decoder: Option<Decoder>,
    no_audio: bool,
    retry_at: Option<Instant>,
}

struct Voice {
    control: Arc<VideoControl>,
    channel: ChannelId,
    generation: uuid::Uuid,
    binding: Arc<crate::audio::voice::Binding>,
    path: PathBuf,
    duration: Duration,
    repeat: Option<(PathBuf, Duration)>,
    active: Option<Active>,
    heads: [Head; 4],
    gain: GainSmoother,
    target_gain: GainDb,
    next_file_check: Option<Instant>,
    fingerprints: [Option<(u64, SystemTime)>; 2],
}

impl Voice {
    fn new(
        control: &Arc<VideoControl>,
        path: PathBuf,
        source: Source,
        duration: Duration,
        repeat: Option<(PathBuf, Duration)>,
    ) -> Self {
        Self {
            control: control.clone(),
            channel: source.channel,
            generation: source.generation,
            binding: source.binding,
            path,
            duration,
            repeat,
            active: None,
            heads: Default::default(),
            gain: GainSmoother::settled(0.0),
            target_gain: GainDb::SILENCE,
            next_file_check: None,
            fingerprints: [None, None],
        }
    }

    fn release(&mut self) {
        self.heads = Default::default();
        self.gain = GainSmoother::settled(0.0);
        self.target_gain = GainDb::SILENCE;
    }

    fn refresh_file(&mut self, now: Instant) -> bool {
        if self.active.is_none() || self.next_file_check.is_some_and(|due| now < due) {
            return false;
        }
        self.next_file_check = Some(now + Duration::from_secs(1));
        let fingerprint = |path: &Path| {
            fs::metadata(path)
                .ok()
                .and_then(|info| Some((info.len(), info.modified().ok()?)))
        };
        let next = [
            fingerprint(&self.path),
            self.repeat.as_ref().and_then(|(path, _)| fingerprint(path)),
        ];
        if self.fingerprints != next {
            self.release();
            self.fingerprints = next;
            return true;
        }
        false
    }

    fn next_voice(&self, layout: &Layout, now: Instant) -> Option<Active> {
        let routing = self.binding.routing().filter(|route| route.enabled)?;
        let lanes = routing
            .destinations
            .map(|id| id.and_then(|id| layout.lanes.get(&id).copied()));
        if lanes == [None, None] || !self.control.owns_audio(self.channel, self.generation) {
            return None;
        }
        let timeline = self.control.audio_timeline_at(now)?;
        Some(Active {
            epoch: timeline.epoch(),
            lanes,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn mix(
        &mut self,
        output: &mut [f32],
        present: Instant,
        frames: usize,
        rate: u32,
        guard: &RevisionGuard,
        cancel: &WorkerCancellation,
        weights: Option<&[crate::matrix::audio::Weight]>,
    ) {
        let Some(active) = self.active else {
            return;
        };
        let Some(timeline) = self
            .control
            .audio_timeline_at(present)
            .filter(|timeline| timeline.epoch() == active.epoch)
        else {
            return;
        };
        let Some(routing) = self.binding.routing().filter(|routing| routing.enabled) else {
            return;
        };
        if routing.gain != self.target_gain {
            self.target_gain = routing.gain;
            self.gain.set_target(routing.gain.to_linear(), (rate / 100).max(1));
        }
        let repeat_duration = self.repeat.as_ref().map(|(_, duration)| *duration);
        for frame in 0..frames {
            let mut stereo = (0.0, 0.0);
            let offset = frame_nanos(frame as u64, rate);
            let positions = timeline.samples(offset, self.duration, repeat_duration);
            for (head, sample) in positions.into_iter().enumerate() {
                let Some(sample) = sample else {
                    continue;
                };
                let head = head + usize::from(sample.repeat) * 2;
                if let Ok(Some((left, right))) = self.read_sample(head, sample.position, rate, guard, cancel) {
                    stereo.0 += left * sample.weight;
                    stereo.1 += right * sample.weight;
                }
            }
            // Shared Video tracks can be present in more than one Cell. Combine their
            // weights without emitting extra copies of the one transport's audio.
            let matrix = weights.map_or(1.0, |weights| {
                let time = present + Duration::from_nanos(offset);
                weights.iter().map(|weight| weight.at(time)).sum::<f32>().min(1.0)
            });
            let gain = self.gain.next_gain() * matrix;
            for (lane, sample) in active.lanes.into_iter().zip([stereo.0, stereo.1]) {
                if let Some(lane) = lane {
                    output[lane * frames + frame] += sample * gain;
                }
            }
        }
    }

    fn read_sample(
        &mut self,
        head: usize,
        position: Duration,
        rate: u32,
        guard: &RevisionGuard,
        cancel: &WorkerCancellation,
    ) -> Result<Option<(f32, f32)>, DecodeError> {
        let path = if head >= 2 {
            &self
                .repeat
                .as_ref()
                .expect("sample selects an authored repeat source")
                .0
        } else {
            &self.path
        };
        self.heads[head].sample(path, position, rate, || {
            let (channel, generation) = (self.channel, self.generation);
            let control = Arc::downgrade(&self.control);
            let guard = guard.clone();
            let cancel = cancel.clone();
            move || {
                cancel.is_cancelled()
                    || !guard.is_current()
                    || control
                        .upgrade()
                        .is_none_or(|control| !control.owns_audio(channel, generation))
            }
        })
    }
}

impl Head {
    pub(super) fn sample<C: FnMut() -> bool + Send + 'static>(
        &mut self,
        path: &Path,
        position: Duration,
        rate: u32,
        interrupt: impl FnOnce() -> C,
    ) -> Result<Option<(f32, f32)>, DecodeError> {
        let result = self.read(path, position, rate, interrupt);
        if result.is_err() {
            self.decoder = None;
            self.retry_at = Some(Instant::now() + Duration::from_millis(500));
        }
        result
    }

    fn read<C: FnMut() -> bool + Send + 'static>(
        &mut self,
        path: &Path,
        position: Duration,
        rate: u32,
        interrupt: impl FnOnce() -> C,
    ) -> Result<Option<(f32, f32)>, DecodeError> {
        let head = self;
        if head.no_audio || head.retry_at.is_some_and(|due| Instant::now() < due) {
            return Ok(None);
        }
        if head.decoder.is_none() {
            head.decoder = AudioReader::open(path, rate, interrupt())?.map(Decoder::new);
            head.no_audio = head.decoder.is_none();
            head.retry_at = None;
        }
        head.decoder
            .as_mut()
            .map(|decoder| decoder.sample(position, rate))
            .transpose()
    }
}
