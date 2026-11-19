//! Cell-owned files use the same bounded PCM reader, virtual lanes and worker as Video.
use super::{frame_nanos, source::Head};
use crate::{
    matrix::{
        audio::{FileUse, Weight},
        protocol::LayerId,
    },
    media::protocol::AssetLocator,
};
use golden_audio::{AudioChannelId, GainDb, GainSmoother};
use golden_io::{RevisionGuard, WorkerCancellation};
use std::{
    collections::{HashMap, hash_map::Entry},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

#[derive(Default)]
pub(super) struct Files {
    takes: HashMap<(LayerId, uuid::Uuid), Take>,
    sweep: bool,
}

struct Take {
    locator: AssetLocator,
    root: PathBuf,
    path: PathBuf,
    start: Instant,
    weight: Weight,
    lanes: [Option<usize>; 2],
    gain: GainSmoother,
    target_gain: GainDb,
    pending_gain: GainDb,
    head: Head,
    fingerprint: Option<(u64, SystemTime)>,
    check_at: Instant,
    sweep: bool,
}

impl Files {
    pub fn reconcile(
        &mut self,
        mut files: Vec<FileUse>,
        root: &Path,
        outputs: &HashMap<AudioChannelId, usize>,
        capacity: usize,
    ) -> (bool, bool) {
        files.sort_unstable_by_key(|file| (file.id.0.0.0, file.id.1));
        self.sweep = !self.sweep;
        let mut admitted = 0;
        let mut changed = false;
        for source in files {
            let lanes = source
                .routing
                .destinations
                .map(|id| id.and_then(|id| outputs.get(&id).copied()));
            if !source.routing.enabled || lanes == [None, None] {
                continue;
            }
            if admitted == capacity {
                break;
            }
            admitted += 1;
            match self.takes.entry(source.id) {
                Entry::Occupied(mut entry) if entry.get().locator == source.file && entry.get().root == root => {
                    let take = entry.get_mut();
                    changed |= take.lanes != lanes;
                    changed |= take.refresh();
                    take.lanes = lanes;
                    take.weight = source.weight;
                    take.pending_gain = source.routing.gain;
                    take.sweep = self.sweep;
                }
                entry => {
                    changed = true;
                    let path = source.file.resolve(root);
                    let take = Take {
                        locator: source.file,
                        root: root.into(),
                        path,
                        start: source.start,
                        weight: source.weight,
                        lanes,
                        gain: GainSmoother::settled(0.0),
                        target_gain: GainDb::SILENCE,
                        pending_gain: source.routing.gain,
                        head: Default::default(),
                        fingerprint: None,
                        check_at: Instant::now(),
                        sweep: self.sweep,
                    };
                    entry.insert_entry(take);
                }
            }
        }
        let indexed = self.takes.len();
        self.takes.retain(|_, take| take.sweep == self.sweep);
        (!self.takes.is_empty(), changed || indexed != self.takes.len())
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
        for take in self.takes.values_mut() {
            if !guard.is_current() || cancel.is_cancelled() {
                break;
            }
            take.mix(output, present, frames, rate, guard, cancel);
        }
    }
}

impl Take {
    fn refresh(&mut self) -> bool {
        let now = Instant::now();
        if now < self.check_at {
            return false;
        }
        self.check_at = now + Duration::from_secs(1);
        let fingerprint = fs::metadata(&self.path)
            .ok()
            .and_then(|info| Some((info.len(), info.modified().ok()?)));
        if self.fingerprint == fingerprint {
            return false;
        }
        self.fingerprint = fingerprint;
        self.head = Head::default();
        true
    }

    fn mix(
        &mut self,
        output: &mut [f32],
        present: Instant,
        frames: usize,
        rate: u32,
        guard: &RevisionGuard,
        cancel: &WorkerCancellation,
    ) {
        if self.pending_gain != self.target_gain {
            self.target_gain = self.pending_gain;
            self.gain.set_target(self.target_gain.to_linear(), (rate / 100).max(1));
        }
        for frame in 0..frames {
            let instant = present + Duration::from_nanos(frame_nanos(frame as u64, rate));
            let weight = self.weight.at(instant);
            let Some(position) = instant.checked_duration_since(self.start).filter(|_| weight > 0.0) else {
                continue;
            };
            let pcm = self.head.sample(&self.path, None, position, rate, || {
                let guard = guard.clone();
                let cancel = cancel.clone();
                move || !guard.is_current() || cancel.is_cancelled()
            });
            if let Ok(Some((left, right))) = pcm {
                let gain = weight * self.gain.next_gain();
                for (lane, sample) in self.lanes.into_iter().zip([left, right]) {
                    if let Some(lane) = lane {
                        output[lane * frames + frame] += sample * gain;
                    }
                }
            }
        }
    }
}
