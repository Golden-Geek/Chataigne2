use golden_audio::{
    AudioDeviceInspectorState, AudioDeviceTargetId, AudioDirection, AudioObservationReader, PhysicalChannelKey,
};
use golden_io::{PayloadPool, RetainedPayload};
use std::sync::{Mutex, OnceLock};

const FRAME_BYTES: usize = 8 * 1024 * 1024;
const METADATA_ITEMS: usize = 131_072;

/// Routing metadata has its own admission, independent of analysis and metering.
pub(super) struct RoutePreparer {
    pool: PayloadPool,
    previous: RouteView,
}

#[derive(Clone, Default)]
pub(crate) struct RouteView {
    frame: Option<RetainedPayload<RouteFrame>>,
    error: Option<RouteValidationError>,
    revision: u64,
}

struct RouteFrame {
    revision: u64,
    input: Option<Vec<PhysicalChannelKey>>,
    output: Option<Vec<PhysicalChannelKey>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RouteValidationError {
    Stale,
    Capacity,
    Oversized,
    NoDevice,
    UnavailableChannel,
}

impl RouteValidationError {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Stale => "Sound Card device inventory is updating; retry the route connection",
            Self::Capacity => "Sound Card route inventory lifetime capacity exhausted; retry the route connection",
            Self::Oversized => "Sound Card route inventory exceeds 8 MiB / 131072 metadata entries",
            Self::NoDevice => "Sound Card device is not ready",
            Self::UnavailableChannel => "Sound Card physical channel is not available on the selected device",
        }
    }
}

impl RouteView {
    #[cfg(test)]
    pub(super) fn same_storage(&self, other: &Self) -> bool {
        match (&self.frame, &other.frame) {
            (Some(left), Some(right)) => std::ptr::eq(&**left, &**right),
            (None, None) => true,
            _ => false,
        }
    }

    pub(crate) fn validate(
        &self,
        revision: u64,
        direction: AudioDirection,
        key: &str,
    ) -> Result<(), RouteValidationError> {
        self.frame
            .as_ref()
            .filter(|frame| frame.revision == revision)
            .ok_or_else(|| {
                self.error
                    .filter(|_| self.revision == revision)
                    .unwrap_or(RouteValidationError::Stale)
            })?
            .validate(direction, key)
    }
}

impl RoutePreparer {
    pub(super) fn new() -> Result<Self, std::io::Error> {
        static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
        let mut pool = POOL
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if pool.is_none() {
            *pool = Some(PayloadPool::new("sound-card-route-disposer", 64, 256 * 1024 * 1024)?);
        }
        Ok(Self::using(pool.as_ref().unwrap().clone()))
    }

    pub(super) fn using(pool: PayloadPool) -> Self {
        Self {
            pool,
            previous: RouteView::default(),
        }
    }

    pub(super) fn prepare(&mut self, reader: &AudioObservationReader) -> RouteView {
        let revision = reader.device_revision();
        if self
            .previous
            .frame
            .as_ref()
            .is_some_and(|frame| frame.revision == revision)
            || (self.previous.revision == revision && self.previous.error == Some(RouteValidationError::Oversized))
        {
            return self.previous.clone();
        }
        let result = self.capture(reader);
        match result {
            Ok(frame) => {
                self.previous = RouteView {
                    revision: frame.revision,
                    frame: Some(frame),
                    error: None,
                }
            }
            Err((revision, error)) => {
                self.previous.revision = revision;
                self.previous.error = Some(error);
            }
        }
        self.previous.clone()
    }

    fn capture(
        &self,
        reader: &AudioObservationReader,
    ) -> Result<RetainedPayload<RouteFrame>, (u64, RouteValidationError)> {
        let reservation = self
            .pool
            .try_reserve(FRAME_BYTES)
            .map_err(|_| (reader.device_revision(), RouteValidationError::Capacity))?;
        let mut frame = reader.inspect(Self::draft)?;
        // Both matching and copying happen natively; sorting never holds the
        // source lock or the actor-facing publication lock.
        frame.sort();
        let revision = frame.revision;
        let bytes = frame.retained_bytes();
        if bytes > FRAME_BYTES {
            return Err((revision, RouteValidationError::Oversized));
        }
        reservation
            .retain(frame, bytes)
            .map_err(|_| (revision, RouteValidationError::Oversized))
    }

    fn draft(snapshot: &golden_audio::AudioObservationSnapshot) -> Result<RouteFrame, (u64, RouteValidationError)> {
        snapshot
            .device
            .retained_bytes_within(FRAME_BYTES, METADATA_ITEMS)
            .ok_or((snapshot.device_revision, RouteValidationError::Oversized))?;
        Ok(RouteFrame::from_state(&snapshot.device, snapshot.device_revision))
    }
}

impl RouteFrame {
    fn from_state(state: &AudioDeviceInspectorState, revision: u64) -> Self {
        Self {
            revision,
            input: selected_channels(state, AudioDirection::Input),
            output: selected_channels(state, AudioDirection::Output),
        }
    }

    fn sort(&mut self) {
        for channels in [&mut self.input, &mut self.output].into_iter().flatten() {
            channels.sort_unstable();
        }
    }

    fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + 2 * size_of::<usize>()
            + [&self.input, &self.output]
                .into_iter()
                .flatten()
                .map(|channels| {
                    channels.capacity() * size_of::<PhysicalChannelKey>()
                        + channels
                            .iter()
                            .map(PhysicalChannelKey::retained_heap_bytes)
                            .sum::<usize>()
                })
                .sum::<usize>()
    }

    fn validate(&self, direction: AudioDirection, key: &str) -> Result<(), RouteValidationError> {
        let channels = match direction {
            AudioDirection::Input => &self.input,
            AudioDirection::Output => &self.output,
        }
        .as_ref()
        .ok_or(RouteValidationError::NoDevice)?;
        channels
            .binary_search_by(|channel| channel.as_str().cmp(key))
            .map(|_| ())
            .map_err(|_| RouteValidationError::UnavailableChannel)
    }
}

fn selected_channels(state: &AudioDeviceInspectorState, direction: AudioDirection) -> Option<Vec<PhysicalChannelKey>> {
    let status = match direction {
        AudioDirection::Input => &state.input,
        AudioDirection::Output => &state.output,
    };
    let selected = status.active_target.as_ref().or(status.selected_target.as_ref())?;
    let device = state.devices.iter().find(|device| {
        device.supports(direction)
            && match selected {
                AudioDeviceTargetId::Device { .. } => device.target == *selected,
                AudioDeviceTargetId::SystemDefault { backend } => {
                    device.target.backend() == backend
                        && match direction {
                            AudioDirection::Input => device.is_system_default_input,
                            AudioDirection::Output => device.is_system_default_output,
                        }
                }
            }
    })?;
    let channels = match direction {
        AudioDirection::Input => &device.input_channels,
        AudioDirection::Output => &device.output_channels,
    };
    Some(channels.iter().map(|channel| channel.key.clone()).collect())
}

#[cfg(test)]
mod tests;
