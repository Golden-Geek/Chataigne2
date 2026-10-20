use std::{
    ops::Deref,
    sync::{Arc, Mutex, OnceLock},
};

use golden_audio::{AudioObservationReader, AudioObservationSnapshot};
use golden_io::{PayloadPool, RetainedPayload};

pub(crate) const SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const SNAPSHOT_ITEMS: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObservationCaptureError {
    Capacity,
    Oversized,
}

impl ObservationCaptureError {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Capacity => "Sound Card observation lifetime capacity exhausted",
            Self::Oversized => "Sound Card observation exceeds 8 MiB / 65536 collection entries",
        }
    }
}

pub(crate) struct ObservationView {
    snapshot: RetainedPayload<AudioObservationSnapshot>,
    pub error: Option<ObservationCaptureError>,
    pub header: ObservationHeader,
    pub sequence: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct ObservationHeader {
    pub enabled: bool,
    pub input_ready: bool,
    pub output_ready: bool,
    pub output_rms: f32,
}

impl ObservationHeader {
    pub(crate) fn connected(self) -> bool {
        self.enabled && (self.input_ready || self.output_ready)
    }

    fn from_snapshot(snapshot: &AudioObservationSnapshot) -> Self {
        Self {
            enabled: snapshot.enabled,
            input_ready: super::super::direction_ready(&snapshot.device.input),
            output_ready: super::super::direction_ready(&snapshot.device.output),
            output_rms: snapshot.output_global_max_rms,
        }
    }
}

impl Deref for ObservationView {
    type Target = AudioObservationSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

struct State {
    snapshot: RetainedPayload<AudioObservationSnapshot>,
    error: Option<ObservationCaptureError>,
    header: ObservationHeader,
    sequence: u64,
}

#[derive(Clone)]
pub(crate) struct ObservationCapture(Arc<Mutex<State>>);

pub(crate) struct ObservationCollector {
    reader: AudioObservationReader,
    pool: PayloadPool,
    capture: ObservationCapture,
}

impl ObservationCapture {
    pub(crate) fn latest(&self) -> ObservationView {
        let state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        ObservationView {
            snapshot: state.snapshot.clone(),
            error: state.error,
            header: state.header,
            sequence: state.sequence,
        }
    }
}

impl ObservationCollector {
    pub(crate) fn new(reader: AudioObservationReader) -> Result<(ObservationCapture, Self), String> {
        static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
        let mut pool = POOL
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if pool.is_none() {
            *pool = Some(
                PayloadPool::new("sound-card-observation-disposer", 128, 256 * 1024 * 1024)
                    .map_err(|error| format!("Sound Card observation disposer unavailable: {error}"))?,
            );
        }
        Self::using(reader, pool.as_ref().expect("installed observation disposer").clone())
            .map_err(|error| error.message().to_owned())
    }

    fn using(
        reader: AudioObservationReader,
        pool: PayloadPool,
    ) -> Result<(ObservationCapture, Self), ObservationCaptureError> {
        let snapshot = capture(&reader, &pool)?;
        let header = reader.inspect(ObservationHeader::from_snapshot);
        let state = ObservationCapture(Arc::new(Mutex::new(State {
            snapshot,
            error: None,
            header,
            sequence: 1,
        })));
        Ok((
            state.clone(),
            Self {
                reader,
                pool,
                capture: state,
            },
        ))
    }

    pub(crate) fn refresh(&mut self) {
        let result = capture(&self.reader, &self.pool);
        let header = self.reader.inspect(ObservationHeader::from_snapshot);
        let previous = {
            let mut state = self.capture.0.lock().unwrap_or_else(|p| p.into_inner());
            state.header = header;
            match result {
                Ok(snapshot) => {
                    state.error = None;
                    state.sequence = state.sequence.wrapping_add(1);
                    Some(std::mem::replace(&mut state.snapshot, snapshot))
                }
                Err(error) => {
                    state.error = Some(error);
                    None
                }
            }
        };
        // No disposer/pool lock is acquired while holding the publication lock.
        drop(previous);
    }
}

/// Charges opaque reader-handle metadata. Mutable source storage belongs to the
/// engine's configuration/resource budget. Retiring the wrapper off the actor
/// also releases the original source snapshot off the actor after native exit.
pub(crate) fn retain_reader(reader: AudioObservationReader) -> Result<RetainedPayload<AudioObservationReader>, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("sound-card-reader-disposer", 64, 64 * 1024)
                .map_err(|error| format!("Sound Card reader disposer unavailable: {error}"))?,
        );
    }
    pool.as_ref()
        .expect("installed reader disposer")
        .try_retain(reader, size_of::<AudioObservationReader>() + 2 * size_of::<usize>())
        .map_err(|_| "Sound Card reader lifetime capacity exhausted".into())
}

fn capture(
    reader: &AudioObservationReader,
    pool: &PayloadPool,
) -> Result<RetainedPayload<AudioObservationSnapshot>, ObservationCaptureError> {
    let reservation = pool
        .try_reserve(SNAPSHOT_BYTES)
        .map_err(|_| ObservationCaptureError::Capacity)?;
    let (snapshot, bytes) = reader
        .inspect(|snapshot| {
            let bytes = snapshot.retained_bytes_within(SNAPSHOT_BYTES - 2 * size_of::<usize>(), SNAPSHOT_ITEMS)?;
            Some((snapshot.clone(), bytes + 2 * size_of::<usize>()))
        })
        .ok_or(ObservationCaptureError::Oversized)?;
    reservation
        .retain(snapshot, bytes)
        .map_err(|_| ObservationCaptureError::Oversized)
}

#[cfg(test)]
mod tests;
