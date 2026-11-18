use super::SubtitleError;
use std::{collections::BTreeSet, time::Duration};

const MAX_CUES: usize = 16_384;
const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
const MAX_ACTIVE_CUES: usize = 8;
pub(super) const MAX_CUE_BYTES: usize = 4096;

/// One unstyled subtitle interval in original video source time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubtitleCue {
    /// First included instant.
    pub start: Duration,
    /// First excluded instant.
    pub end: Duration,
    /// Plain UTF-8 dialogue. Newlines are preserved.
    pub text: String,
}

/// Immutable, bounded subtitles ready for arbitrary source-time seeks in either direction.
#[derive(Debug, Default)]
pub struct SubtitleTimeline {
    cues: Vec<SubtitleCue>,
    changes: Vec<Change>,
}

#[derive(Debug)]
struct Change {
    time: Duration,
    indices: [u16; MAX_ACTIVE_CUES],
    count: u8,
}

impl SubtitleTimeline {
    /// All admitted cues in start-time order, preserving source order at the same start.
    pub fn cues(&self) -> &[SubtitleCue] {
        &self.cues
    }

    /// Active cues in display order. Lookup is binary search plus at most eight cue references.
    pub fn active_at(&self, source_time: Duration) -> impl ExactSizeIterator<Item = &SubtitleCue> {
        let containing = self.changes.partition_point(|change| change.time <= source_time);
        let indices = containing.checked_sub(1).map_or(&[][..], |index| {
            let change = &self.changes[index];
            &change.indices[..usize::from(change.count)]
        });
        indices.iter().map(|index| &self.cues[usize::from(*index)])
    }
}

#[derive(Default)]
pub(super) struct CueBuilder {
    cues: Vec<SubtitleCue>,
    bytes: usize,
}

impl CueBuilder {
    pub fn push(&mut self, cue: SubtitleCue) -> Result<(), SubtitleError> {
        let bytes = cue.text.len();
        if cue.start >= cue.end {
            return Err(SubtitleError::InvalidTime);
        }
        if self.cues.len() >= MAX_CUES || bytes > MAX_CUE_BYTES || self.bytes + bytes > MAX_TEXT_BYTES {
            return Err(SubtitleError::LimitExceeded);
        }
        self.bytes += bytes;
        self.cues.push(cue);
        Ok(())
    }

    pub fn finish(mut self) -> Result<SubtitleTimeline, SubtitleError> {
        self.cues.sort_by_key(|cue| cue.start);
        let mut events = self
            .cues
            .iter()
            .enumerate()
            .flat_map(|(index, cue)| [(cue.start, true, index as u16), (cue.end, false, index as u16)])
            .collect::<Vec<_>>();
        // An ending cue must leave before a following cue starts at the same instant.
        events.sort_unstable();
        let mut active = BTreeSet::new();
        let mut changes = Vec::new();
        for group in events.chunk_by(|a, b| a.0 == b.0) {
            for &(_, starts, index) in group {
                if starts {
                    active.insert(index);
                } else {
                    active.remove(&index);
                }
            }
            if active.len() > MAX_ACTIVE_CUES {
                return Err(SubtitleError::LimitExceeded);
            }
            let mut indices = [0; MAX_ACTIVE_CUES];
            for (slot, index) in indices.iter_mut().zip(&active) {
                *slot = *index;
            }
            changes.push(Change {
                time: group[0].0,
                indices,
                count: active.len() as u8,
            });
        }
        Ok(SubtitleTimeline {
            cues: self.cues,
            changes,
        })
    }
}
