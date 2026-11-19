use golden_video::ndi::NdiTiming;
use std::time::{Duration, Instant};

const DELAY: Duration = Duration::from_millis(90);
const DISCONTINUITY: Duration = Duration::from_millis(180);

/// Maps both streams from the same NDI connection into the local playback clock.
#[derive(Default)]
pub(super) struct Clock {
    anchor: Option<(i64, Instant)>,
}

impl Clock {
    pub fn playout(&mut self, timing: NdiTiming, arrival: Instant) -> (Instant, bool) {
        let nominal = arrival + DELAY;
        let Some(stamp) = timing.timecode_100ns.or(timing.timestamp_100ns) else {
            return (nominal, false);
        };
        let Some((origin, local)) = self.anchor else {
            self.anchor = Some((stamp, nominal));
            return (nominal, false);
        };
        let delta = Duration::from_nanos(origin.abs_diff(stamp).saturating_mul(100));
        let mapped = if stamp >= origin {
            local.checked_add(delta)
        } else {
            local.checked_sub(delta)
        };
        if let Some(mapped) = mapped
            && mapped
                .saturating_duration_since(nominal)
                .max(nominal.saturating_duration_since(mapped))
                < DISCONTINUITY
        {
            return (mapped, false);
        }
        self.anchor = Some((stamp, nominal));
        (nominal, true)
    }
}
