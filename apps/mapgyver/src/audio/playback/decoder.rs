use golden_video::decode::{AudioChunk, AudioReader, DecodeError};
use std::{collections::VecDeque, time::Duration};

/// Retain enough decoded history for reverse playback and enough lookahead for interpolation.
const HISTORY: i64 = 8_192;
const MAX_CHUNKS: usize = 48;
const MAX_FRAMES: usize = 65_536;

pub(super) struct Decoder {
    reader: AudioReader,
    chunks: VecDeque<AudioChunk>,
    window_start: i64,
    seek_target: i64,
    end: i64,
    eof: bool,
    retained: usize,
}

impl Decoder {
    pub fn new(reader: AudioReader) -> Self {
        Self {
            reader,
            chunks: VecDeque::new(),
            window_start: 0,
            seek_target: 0,
            end: 0,
            eof: false,
            retained: 0,
        }
    }

    pub fn sample(&mut self, position: Duration, sample_rate: u32) -> Result<(f32, f32), DecodeError> {
        let sample = position.as_nanos() * u128::from(sample_rate);
        let index = i64::try_from(sample / 1_000_000_000).map_err(|_| DecodeError::OutsideStream)?;
        let not_catching_up = index > self.seek_target.saturating_add(HISTORY);
        if index < self.window_start || (index > self.end.saturating_add(HISTORY) && !self.eof && not_catching_up) {
            let start = index.saturating_sub(HISTORY / 2).max(0);
            self.reader.seek(start as u64)?;
            self.seek_target = index;
            self.chunks.clear();
            self.window_start = start;
            self.end = start;
            self.eof = false;
            self.retained = 0;
        }
        for _ in 0..MAX_CHUNKS {
            if self.eof || self.end > index.saturating_add(1) {
                break;
            }
            let Some(chunk) = self.reader.next_chunk()? else {
                self.eof = true;
                break;
            };
            self.end = chunk.start_frame.saturating_add(chunk.samples.len() as i64);
            self.retained += chunk.samples.len();
            self.chunks.push_back(chunk);
            while self.retained > MAX_FRAMES
                || self.chunks.len() > MAX_CHUNKS
                || self.chunks.front().is_some_and(|chunk| {
                    chunk.start_frame.saturating_add(chunk.samples.len() as i64) < index.saturating_sub(HISTORY)
                })
            {
                if let Some(old) = self.chunks.pop_front() {
                    self.retained -= old.samples.len();
                    self.window_start = self
                        .window_start
                        .max(old.start_frame.saturating_add(old.samples.len() as i64));
                }
            }
        }
        let current = self.frame(index);
        let fraction = (sample % 1_000_000_000) as f32 / 1_000_000_000.0;
        if fraction == 0.0 {
            return Ok(current);
        }
        let next = self.frame(index.saturating_add(1));
        Ok((
            current.0 + (next.0 - current.0) * fraction,
            current.1 + (next.1 - current.1) * fraction,
        ))
    }

    fn frame(&self, frame: i64) -> (f32, f32) {
        self.chunks
            .iter()
            .find_map(|chunk| {
                let offset = usize::try_from(frame.checked_sub(chunk.start_frame)?).ok()?;
                chunk.samples.get(offset).copied()
            })
            .unwrap_or_default()
    }
}
