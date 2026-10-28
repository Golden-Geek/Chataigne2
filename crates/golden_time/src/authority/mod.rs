use std::sync::Arc;
use std::time::Instant;

use uuid::Uuid;

use crate::{MonotonicTime, PlaybackRate, ShowPosition, TimeError, TimeMapping};

/// Lightweight, read-only monotonic observation source.
///
/// Reading must not advance a domain timeline or perform blocking native work. Frame callbacks,
/// sender arrival and monitor presentation are consumers rather than authorities for show time.
pub trait Clock: Send + Sync + 'static {
    /// Current coordinate relative to this clock's declared origin.
    fn now(&self) -> Result<MonotonicTime, TimeError>;
}

impl<C: Clock + ?Sized> Clock for Arc<C> {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        self.as_ref().now()
    }
}

/// Production monotonic source anchored to one `std::time::Instant`.
#[derive(Clone, Debug)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// Creates a fresh coordinate origin. Clones retain that same origin.
    pub fn new() -> Self {
        Self { origin: Instant::now() }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        u64::try_from(self.origin.elapsed().as_nanos())
            .map(MonotonicTime::from_nanos)
            .map_err(|_| TimeError::Overflow)
    }
}

/// One authority lifetime plus a checked discontinuity generation.
///
/// Equality includes authority identity. Two replacement authorities at generation zero do not
/// share an epoch; generation alone is diagnostic and must not fence scheduled work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimeEpoch {
    authority: Uuid,
    generation: u64,
}

impl TimeEpoch {
    fn new() -> Self {
        Self {
            authority: Uuid::new_v4(),
            generation: 0,
        }
    }

    fn next(self) -> Result<Self, TimeError> {
        Ok(Self {
            generation: self.generation.checked_add(1).ok_or(TimeError::Overflow)?,
            ..self
        })
    }

    /// Discontinuities within this authority lifetime, for diagnostics.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Whether two epochs share the same monotonic authority lifetime.
    pub fn same_authority(self, other: Self) -> bool {
        self.authority == other.authority
    }
}

/// Consistent time observation with its original immutable mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeSnapshot {
    monotonic: MonotonicTime,
    show: ShowPosition,
    mapping: TimeMapping,
}

impl TimeSnapshot {
    /// Observed monotonic coordinate.
    pub const fn monotonic(self) -> MonotonicTime {
        self.monotonic
    }

    /// Show position derived from the authority's original anchor.
    pub const fn show_position(self) -> ShowPosition {
        self.show
    }

    /// Authority/discontinuity fence for scheduled work.
    pub const fn epoch(self) -> TimeEpoch {
        self.mapping.epoch()
    }

    /// Original mapping, preserving rational phase across repeated observations.
    pub const fn mapping(self) -> TimeMapping {
        self.mapping
    }

    /// Projects a reader observation through this same original mapping without advancing
    /// or publishing authority state. Scheduled work still requires current-epoch validation.
    pub fn at_monotonic(self, observed: MonotonicTime) -> Result<Self, TimeError> {
        if observed < self.monotonic {
            return Err(TimeError::ClockRegressed {
                previous: self.monotonic,
                observed,
            });
        }
        let show = self.mapping.show_at(observed)?;
        Ok(Self {
            monotonic: observed,
            show,
            mapping: self.mapping,
        })
    }
}

/// Single owner of show-position mapping and discontinuity issuance.
///
/// App transport policy chooses when to reanchor. Observing time computes from an anchor instead
/// of incrementing a counter, so multiple consumers cannot duplicate advancement or effects.
pub struct TimeAuthority<C: Clock> {
    clock: C,
    mapping: TimeMapping,
    last: MonotonicTime,
}

impl<C: Clock> TimeAuthority<C> {
    /// Observes the initial clock and creates a distinct authority lifetime.
    pub fn new(clock: C, show: ShowPosition, rate: PlaybackRate) -> Result<Self, TimeError> {
        let now = clock.now()?;
        Ok(Self {
            clock,
            mapping: TimeMapping::new(TimeEpoch::new(), now, show, rate),
            last: now,
        })
    }

    /// Samples the monotonic source and maps it without accumulating rounded deltas.
    ///
    /// Clock regression or conversion overflow leaves the previous accepted state unchanged.
    pub fn sample(&mut self) -> Result<TimeSnapshot, TimeError> {
        let now = self.observe()?;
        let show = self.mapping.show_at(now)?;
        self.last = now;
        Ok(TimeSnapshot {
            monotonic: now,
            show,
            mapping: self.mapping,
        })
    }

    /// Starts a new epoch at the current clock coordinate and app-supplied show/rate intent.
    ///
    /// This primitive chooses no play/pause/seek/loop policy. Failed observation or epoch
    /// exhaustion commits nothing, and old scheduled work remains fenced from the new epoch.
    pub fn discontinuity(&mut self, show: ShowPosition, rate: PlaybackRate) -> Result<TimeSnapshot, TimeError> {
        let now = self.observe()?;
        let mapping = TimeMapping::new(self.mapping.epoch().next()?, now, show, rate);
        self.mapping = mapping;
        self.last = now;
        Ok(TimeSnapshot {
            monotonic: now,
            show,
            mapping,
        })
    }

    fn observe(&self) -> Result<MonotonicTime, TimeError> {
        let observed = self.clock.now()?;
        if observed < self.last {
            return Err(TimeError::ClockRegressed {
                previous: self.last,
                observed,
            });
        }
        Ok(observed)
    }
}

#[cfg(test)]
mod tests;
