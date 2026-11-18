//! A pure projection from an authoritative clock coordinate to looping source timestamps.
use golden_time::ShowPosition;
use std::time::Duration;

/// Two source samples at one clock position. Weights are complementary in linear light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopBlend {
    /// Timestamp of the incoming sample within the original source.
    pub secondary_position: Duration,
    /// Smoothstep weight of the incoming sample.
    pub secondary_weight: f32,
}

impl LoopBlend {
    /// Smoothstep complement for the outgoing sample.
    pub fn primary_weight(self) -> f32 {
        1.0 - self.secondary_weight
    }
}

/// A projected source timestamp in one loop cycle and its optional seam blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopSample {
    /// File-relative timestamp of the outgoing sample, before any channel-specific clamp.
    pub position: Duration,
    /// Cycle relative to the intro in the current clock epoch, signed by playback direction.
    pub cycle: i64,
    /// Incoming timestamp and weight during a positive-duration overlap.
    pub blend: Option<LoopBlend>,
}

/// Map a signed nanosecond clock to original source time and a shortened repeat period.
///
/// At a new clock anchor, cycle zero preserves the explicit file position. After the first
/// wrap, each cycle skips the interval consumed by the preceding blend. The caller owns clock
/// continuity, pause/seek epochs, and whether to use looping at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopTimeline {
    duration: i64,
    overlap: i64,
}

impl LoopTimeline {
    /// Admit a known nonzero duration on the signed timeline. The overlap is capped at half
    /// the footage, so its two source ranges never overlap each other and repeats still advance.
    pub fn new(duration: Duration, requested_overlap: Duration) -> Option<Self> {
        let duration = i64::try_from(duration.as_nanos()).ok()?;
        let overlap = requested_overlap.as_nanos().min((duration / 2) as u128) as i64;
        (duration > 0).then_some(Self { duration, overlap })
    }

    /// Effective overlap in media time, independent of playback speed.
    pub fn overlap(self) -> Duration {
        media_time(self.overlap)
    }

    /// Time traversed per repeat after the first wrap.
    pub fn cycle_duration(self) -> Duration {
        media_time(self.period())
    }

    /// First file position in the next repeat, once the incoming overlap was consumed.
    pub fn restart_position(self, reverse: bool) -> Duration {
        media_time(if reverse { self.period() } else { self.overlap })
    }

    /// Bounds of file timestamps that belong to the cycle. The caller owns endpoint inclusion
    /// when selecting a half-open decode window.
    pub fn source_bounds(self, cycle: i64, reverse: bool) -> (Duration, Duration) {
        let (first, last) = match (cycle == 0, reverse) {
            (true, _) => (0, self.duration),
            (false, false) => (self.overlap, self.duration),
            (false, true) => (0, self.period()),
        };
        (media_time(first), media_time(last))
    }

    /// Offset from file timestamps to the given cycle's unwrapped nanosecond coordinates.
    ///
    /// Keep this wide until after adding the frame's file timestamp. Near the limits of the
    /// signed clock, an offset itself can fall outside that range while the sum still fits.
    pub fn origin_nanos(self, cycle: i64) -> i128 {
        i128::from(cycle) * i128::from(self.period())
    }

    /// Project the unwrapped coordinate from the shared clock. Reverse owns the upper seam
    /// endpoint so a hard wrap can select the previous cycle's final sample.
    ///
    /// Returns None if the signed cycle itself is unrepresentable.
    pub fn sample(self, raw: ShowPosition, reverse: bool) -> Option<LoopSample> {
        let raw = i128::from(raw.as_nanos());
        let period = i128::from(self.period());
        let duration = i128::from(self.duration);
        let overlap = i128::from(self.overlap);
        let (position, cycle) = if reverse {
            if raw > 0 && raw <= duration {
                (raw, 0)
            } else {
                // Reverse intervals contain their upper endpoint and exclude the lower one.
                let distance = raw - 1;
                (distance.rem_euclid(period) + 1, distance.div_euclid(period))
            }
        } else if raw >= 0 && raw < duration {
            (raw, 0)
        } else {
            let distance = raw - overlap;
            (distance.rem_euclid(period) + overlap, distance.div_euclid(period))
        };
        let position = position as i64;
        Some(LoopSample {
            position: media_time(position),
            cycle: i64::try_from(cycle).ok()?,
            blend: self.blend(position, reverse),
        })
    }

    fn period(self) -> i64 {
        self.duration - self.overlap
    }

    /// Project the blend at a file position, including explicit seeks or paused playback.
    /// The exact terminal endpoint evaluates to full incoming weight. Positions outside
    /// the source duration do not describe a seam.
    pub fn blend_at(self, position: Duration, reverse: bool) -> Option<LoopBlend> {
        let position = i64::try_from(position.as_nanos()).ok()?;
        (position <= self.duration)
            .then(|| self.blend(position, reverse))
            .flatten()
    }

    fn blend(self, position: i64, reverse: bool) -> Option<LoopBlend> {
        if self.overlap == 0 {
            return None;
        }
        let (consumed, incoming) = if reverse && position <= self.overlap {
            (self.overlap - position, self.period() + position)
        } else if !reverse && position >= self.period() {
            let consumed = position - self.period();
            (consumed, consumed)
        } else {
            return None;
        };
        let phase = consumed as f64 / self.overlap as f64;
        let weight = phase * phase * (3.0 - 2.0 * phase);
        Some(LoopBlend {
            secondary_position: media_time(incoming),
            secondary_weight: weight as f32,
        })
    }
}

fn media_time(nanos: i64) -> Duration {
    Duration::from_nanos(nanos as u64)
}

#[cfg(test)]
mod tests;
