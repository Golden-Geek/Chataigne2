use golden_audio::{AudioDeviceInspectorState, AudioDeviceTargetId, AudioDirection, AudioObservationReader};
use golden_io::{PayloadPool, RetainedPayload};
use std::sync::{Mutex, OnceLock};

const FRAME_BYTES: usize = 16 * 1024 * 1024;
const METADATA_BYTES: usize = 8 * 1024 * 1024;
const METADATA_ITEMS: usize = 131_072;

/// Device metadata has its own admission, independent of analysis and metering.
pub(super) struct InventoryPreparer {
    pool: PayloadPool,
    previous: DeviceView,
}

#[derive(Clone, Default)]
pub(crate) struct DeviceView {
    frame: Option<RetainedPayload<InventoryFrame>>,
    error: Option<RouteValidationError>,
    revision: u64,
}

struct InventoryFrame {
    revision: u64,
    metadata: AudioDeviceInspectorState,
    input: Option<DirectionIndex>,
    output: Option<DirectionIndex>,
}

struct DirectionIndex {
    device: usize,
    channels: Vec<usize>,
}

/// A borrowed native catalog; cloning this handle never clones device metadata.
#[derive(Clone)]
pub(crate) struct DeviceInventory(RetainedPayload<InventoryFrame>);

impl DeviceInventory {
    pub(crate) fn revision(&self) -> u64 {
        self.0.revision
    }

    pub(crate) fn state(&self) -> &AudioDeviceInspectorState {
        &self.0.metadata
    }
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
            Self::Oversized => {
                "Sound Card device inventory exceeds 8 MiB / 131072 source entries or 16 MiB prepared storage"
            }
            Self::NoDevice => "Sound Card device is not ready",
            Self::UnavailableChannel => "Sound Card physical channel is not available on the selected device",
        }
    }
}

impl DeviceView {
    pub(crate) fn catalog(&self, revision: u64) -> Option<DeviceInventory> {
        self.frame
            .as_ref()
            .filter(|frame| frame.revision == revision)
            .cloned()
            .map(DeviceInventory)
    }
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

impl InventoryPreparer {
    pub(super) fn new() -> Result<Self, std::io::Error> {
        static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
        let mut pool = POOL
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if pool.is_none() {
            *pool = Some(PayloadPool::new(
                "sound-card-inventory-disposer",
                64,
                512 * 1024 * 1024,
            )?);
        }
        Ok(Self::using(pool.as_ref().unwrap().clone()))
    }

    pub(super) fn using(pool: PayloadPool) -> Self {
        Self {
            pool,
            previous: DeviceView::default(),
        }
    }

    pub(super) fn prepare(&mut self, reader: &AudioObservationReader) -> DeviceView {
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
                self.previous = DeviceView {
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
    ) -> Result<RetainedPayload<InventoryFrame>, (u64, RouteValidationError)> {
        let reservation = self
            .pool
            .try_reserve(FRAME_BYTES)
            .map_err(|_| (reader.device_revision(), RouteValidationError::Capacity))?;
        let mut frame = reader.inspect(Self::draft)?;
        // Both matching and copying happen natively; sorting never holds the
        // source lock or the actor-facing publication lock.
        frame.sort();
        let revision = frame.revision;
        let bytes = frame
            .retained_bytes()
            .ok_or((revision, RouteValidationError::Oversized))?;
        if bytes > FRAME_BYTES {
            return Err((revision, RouteValidationError::Oversized));
        }
        reservation
            .retain(frame, bytes)
            .map_err(|_| (revision, RouteValidationError::Oversized))
    }

    fn draft(snapshot: &golden_audio::AudioObservationSnapshot) -> Result<InventoryFrame, (u64, RouteValidationError)> {
        snapshot
            .device
            .retained_bytes_within(METADATA_BYTES, METADATA_ITEMS)
            .ok_or((snapshot.device_revision, RouteValidationError::Oversized))?;
        Ok(InventoryFrame::from_state(&snapshot.device, snapshot.device_revision))
    }
}

impl InventoryFrame {
    fn from_state(state: &AudioDeviceInspectorState, revision: u64) -> Self {
        Self {
            revision,
            metadata: state.clone(),
            input: selected_channels(state, AudioDirection::Input),
            output: selected_channels(state, AudioDirection::Output),
        }
    }

    fn sort(&mut self) {
        for (index, direction) in [
            (&mut self.input, AudioDirection::Input),
            (&mut self.output, AudioDirection::Output),
        ] {
            if let Some(index) = index {
                let channels = physical_channels(&self.metadata, index.device, direction);
                index
                    .channels
                    .sort_unstable_by(|left, right| channels[*left].key.as_str().cmp(channels[*right].key.as_str()));
            }
        }
    }

    fn retained_bytes(&self) -> Option<usize> {
        Some(
            size_of::<Self>() - size_of::<AudioDeviceInspectorState>()
                + 2 * size_of::<usize>()
                + self.metadata.retained_bytes_within(METADATA_BYTES, METADATA_ITEMS)?
                + [&self.input, &self.output]
                    .into_iter()
                    .flatten()
                    .map(|index| index.channels.capacity() * size_of::<usize>())
                    .sum::<usize>(),
        )
    }

    fn validate(&self, direction: AudioDirection, key: &str) -> Result<(), RouteValidationError> {
        let index = match direction {
            AudioDirection::Input => &self.input,
            AudioDirection::Output => &self.output,
        }
        .as_ref()
        .ok_or(RouteValidationError::NoDevice)?;
        let channels = physical_channels(&self.metadata, index.device, direction);
        index
            .channels
            .binary_search_by(|position| channels[*position].key.as_str().cmp(key))
            .map(|_| ())
            .map_err(|_| RouteValidationError::UnavailableChannel)
    }
}

fn selected_channels(state: &AudioDeviceInspectorState, direction: AudioDirection) -> Option<DirectionIndex> {
    let status = match direction {
        AudioDirection::Input => &state.input,
        AudioDirection::Output => &state.output,
    };
    let selected = status.active_target.as_ref().or(status.selected_target.as_ref())?;
    let device = state.devices.iter().position(|device| {
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
    let channels = physical_channels(state, device, direction);
    Some(DirectionIndex {
        device,
        channels: (0..channels.len()).collect(),
    })
}

fn physical_channels(
    state: &AudioDeviceInspectorState,
    device: usize,
    direction: AudioDirection,
) -> &[golden_audio::PhysicalChannelDescriptor] {
    match direction {
        AudioDirection::Input => &state.devices[device].input_channels,
        AudioDirection::Output => &state.devices[device].output_channels,
    }
}

#[cfg(test)]
pub(crate) mod tests;
