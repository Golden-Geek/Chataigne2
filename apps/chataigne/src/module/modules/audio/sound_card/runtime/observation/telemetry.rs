use std::{
    io::{self, Write},
    sync::{Mutex, OnceLock},
};

use golden_audio::{AudioEvent, AudioObservationSnapshot};
use golden_core::{
    events::{CustomEvent, CustomEventRetention},
    node::NodeId,
};
use golden_io::{PayloadPool, SharedPayload};
use serde_json::Value;

use super::capture::ObservationCaptureError;
use crate::app::module::common::json;

mod voices;
use voices::Voices;

const SCRATCH_BYTES: usize = 128 * 1024 * 1024;
const JSON_BYTES: usize = 32 * 1024 * 1024;
const WIRE_BYTES: usize = 4 * 1024 * 1024;
const JSON_ITEMS: usize = 65_536;
const OWNER_BYTES: usize = 2 * size_of::<usize>();

#[derive(Clone, Debug)]
pub(crate) struct PreparedTelemetry {
    payload: SharedPayload<Value>,
    bytes: usize,
}

impl PreparedTelemetry {
    pub(crate) fn event(&self, module: NodeId) -> CustomEvent {
        CustomEvent::from_shared_payload(
            chataigne_sound_card_protocol::SOUND_CARD_TELEMETRY_TOPIC,
            Some(module),
            self.payload.clone(),
            CustomEventRetention::Latest,
        )
        .with_payload_storage_weight(self.bytes)
    }

    pub(crate) fn same_payload(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(self.payload.arc(), other.payload.arc())
    }
}

pub(super) struct TelemetryPreparer {
    pool: PayloadPool,
    voices: Voices,
    previous: Option<PreparedTelemetry>,
}

impl TelemetryPreparer {
    pub(super) fn new() -> Result<Self, ObservationCaptureError> {
        static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
        let mut pool = POOL
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if pool.is_none() {
            *pool = Some(
                PayloadPool::new("sound-card-telemetry-disposer", 128, 512 * 1024 * 1024)
                    .map_err(|_| ObservationCaptureError::TelemetryCapacity)?,
            );
        }
        Ok(Self {
            pool: pool.as_ref().expect("installed telemetry pool").clone(),
            voices: Voices::default(),
            previous: None,
        })
    }

    pub(super) fn observe(&mut self, event: &AudioEvent) {
        self.voices.observe(event);
    }

    pub(super) fn prepare(
        &mut self,
        snapshot: &AudioObservationSnapshot,
    ) -> Result<PreparedTelemetry, ObservationCaptureError> {
        let snapshot_bytes = snapshot
            .retained_bytes_within(super::capture::SNAPSHOT_BYTES, super::capture::SNAPSHOT_ITEMS)
            .ok_or(ObservationCaptureError::Oversized)?;
        if snapshot.analysis.taps.iter().any(|tap| {
            matches!(&tap.result,
                Some(golden_audio::AnalysisResult::Pitch(pitch)) if pitch.note_name.len() > 64
            )
        }) {
            return Err(ObservationCaptureError::ValueOversized);
        }
        let voice_bytes = self.voices.projection_bytes(WIRE_BYTES)?;
        // DTO copies, geometrically growing wire storage and validation-parser
        // scratch remain charged until those temporaries have actually dropped.
        let typed_scratch = self
            .pool
            .try_reserve(snapshot_bytes + voice_bytes + 3 * WIRE_BYTES + 256 * 1024)
            .map_err(|_| ObservationCaptureError::TelemetryCapacity)?;
        let voices = self.voices.project(WIRE_BYTES)?;
        let dto = super::telemetry_from_observation(snapshot, voices);
        let mut wire = BoundedWire(Vec::new());
        serde_json::to_writer(&mut wire, &dto).map_err(|_| ObservationCaptureError::TelemetryOversized)?;
        let source = std::str::from_utf8(&wire.0).map_err(|_| ObservationCaptureError::TelemetryOversized)?;
        let nodes = json::validate(source, WIRE_BYTES, JSON_ITEMS, 64)
            .map_err(|_| ObservationCaptureError::TelemetryOversized)?;
        // Four KiB per validated value covers sparse map nodes, vector spare
        // slots, the weight walk and parser bookkeeping. Strings/keys are also
        // bounded by encoded bytes. The global maximum covers the admitted
        // 65,536-value / 4 MiB shape even when this estimate reaches its cap.
        let json_scratch = (nodes * 4096 + wire.0.len() * 4 + 256 * 1024).min(SCRATCH_BYTES);
        let reservation = self
            .pool
            .try_reserve(json_scratch)
            .map_err(|_| ObservationCaptureError::TelemetryCapacity)?;
        let value: Value = serde_json::from_str(source).map_err(|_| ObservationCaptureError::TelemetryOversized)?;
        let bytes = json::retained_weight(&value, JSON_BYTES - OWNER_BYTES, JSON_ITEMS, 64)
            .map_err(|_| ObservationCaptureError::TelemetryOversized)?
            + OWNER_BYTES;
        if let Some(previous) = &self.previous {
            if previous.payload.arc().as_ref() == &value {
                return Ok(previous.clone());
            }
        }
        drop(dto);
        drop(wire);
        drop(typed_scratch);
        let prepared = PreparedTelemetry {
            payload: reservation
                .share_retained(value, bytes)
                .map_err(|_| ObservationCaptureError::TelemetryOversized)?,
            bytes,
        };
        self.previous = Some(prepared.clone());
        Ok(prepared)
    }
}

struct BoundedWire(Vec<u8>);
impl Write for BoundedWire {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > WIRE_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("Sound Card telemetry exceeds its wire bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
