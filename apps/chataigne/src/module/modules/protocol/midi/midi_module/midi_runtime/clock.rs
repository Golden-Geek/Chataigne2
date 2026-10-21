use super::*;
use std::collections::VecDeque;

const MIDI_CLOCK_STATUS: u8 = 0xF8;
const MIDI_CLOCK_PULSES_PER_BEAT: u8 = 24;
const MIDI_CLOCK_BPM_WINDOW_BEATS: usize = 4;

#[derive(Default)]
pub(super) struct MidiClockAnalyzer {
    tick_in_beat: u8,
    last_beat_at: Option<Duration>,
    recent_beat_durations: VecDeque<Duration>,
}

impl MidiClockAnalyzer {
    pub(super) fn observe(&mut self, bytes: &[u8], received_at: Duration) -> Option<MidiClockTiming> {
        if bytes != [MIDI_CLOCK_STATUS] {
            return None;
        }

        self.tick_in_beat = self.tick_in_beat.saturating_add(1);
        if self.tick_in_beat < MIDI_CLOCK_PULSES_PER_BEAT {
            return Some(MidiClockTiming {
                beat_triggered: false,
                bpm: None,
            });
        }

        self.tick_in_beat = 0;
        let mut bpm = None;
        if let Some(last_beat_at) = self.last_beat_at {
            let beat_duration = received_at.saturating_sub(last_beat_at);
            if !beat_duration.is_zero() {
                self.recent_beat_durations.push_back(beat_duration);
                while self.recent_beat_durations.len() > MIDI_CLOCK_BPM_WINDOW_BEATS {
                    self.recent_beat_durations.pop_front();
                }
                bpm = stable_midi_clock_bpm(&self.recent_beat_durations);
            }
        }
        self.last_beat_at = Some(received_at);

        Some(MidiClockTiming {
            beat_triggered: true,
            bpm,
        })
    }
}

fn stable_midi_clock_bpm(beat_durations: &VecDeque<Duration>) -> Option<f64> {
    if beat_durations.is_empty() {
        return None;
    }

    let mut samples = beat_durations.iter().map(Duration::as_secs_f64).collect::<Vec<_>>();
    samples.sort_by(f64::total_cmp);
    let trimmed = if samples.len() >= 4 {
        &samples[1..samples.len() - 1]
    } else {
        samples.as_slice()
    };
    let average_beat_seconds = trimmed.iter().sum::<f64>() / trimmed.len() as f64;
    if average_beat_seconds <= 0.0 {
        return None;
    }

    Some(60.0 / average_beat_seconds)
}
