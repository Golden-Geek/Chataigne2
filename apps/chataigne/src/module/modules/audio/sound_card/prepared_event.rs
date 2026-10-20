use std::{
    mem::size_of,
    sync::{Mutex, OnceLock},
};

use golden_audio::AudioEvent;
use golden_core::{
    events::{CustomEvent, CustomEventRetention},
    node::NodeId,
};
use golden_io::{PayloadPool, RetainedPayload};

use super::connection_feedback::{ConnectionLog, ConnectionWarning};
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPayload, NativeCallbackPreparer};

pub(super) type PreparedAudioEvent = RetainedPayload<AudioEventData>;

/// The native owner prepares this once. Actor copies are shallow, and the pool
/// retains the final owner through callback receipts and node removal.
pub(super) struct AudioEventData {
    pub event: AudioEvent,
    pub outgoing_log: Option<String>,
    pub connection: Option<ConnectionFeedback>,
    pub preparation_error: Option<String>,
    callback: Option<NativeCallbackPayload>,
    retention: CustomEventRetention,
    pub retained_bytes: usize,
    #[cfg(test)]
    drop_probe: Option<tests::DropProbe>,
}

pub(super) struct ConnectionFeedback {
    pub log: Option<ConnectionLog>,
    pub warning: ConnectionWarning,
}

pub(super) struct AudioEventDraft {
    data: Option<AudioEventData>,
    callback: Option<NativeCallbackDraft>,
}

pub(super) struct EventPreparer {
    events: PayloadPool,
    callbacks: NativeCallbackPreparer,
}

impl AudioEventData {
    pub(super) fn callback(&self, module: NodeId) -> Option<CustomEvent> {
        self.callback.as_ref().map(|callback| {
            let mut event = callback.event(module);
            event.retention = self.retention;
            event
        })
    }
}

impl AudioEventDraft {
    pub(super) fn new(event: AudioEvent) -> Self {
        let mut retention = CustomEventRetention::Replay;
        let mut preparation_error = None;
        let callback = super::script::audio_event_callback(&event).and_then(|(name, args, policy)| {
            retention = policy;
            match NativeCallbackDraft::new(name, args) {
                Ok(draft) => Some(draft),
                Err(error) => {
                    preparation_error = Some(error);
                    None
                }
            }
        });
        let connection = if let AudioEvent::DeviceStatusChanged(status) = &event {
            Some(ConnectionFeedback {
                log: super::connection_feedback::log_for_status(status),
                warning: super::connection_feedback::warning_for_status(status),
            })
        } else {
            None
        };
        let mut data = AudioEventData {
            outgoing_log: super::traffic_logging::outgoing_audio_log_message(&event),
            event,
            connection,
            preparation_error,
            callback: None,
            retention,
            retained_bytes: 0,
            #[cfg(test)]
            drop_probe: None,
        };
        data.retained_bytes = retained_bytes(&data, callback.as_ref());
        // Golden Audio's normalized event is at most 1 MiB. Its typed callback
        // duplicates only bounded IDs/paths and fixed-shape status structures.
        // Treat a future preparation-contract violation as an explicit result
        // rejection, while preserving the raw native outcome and its ordering.
        let callback = if data.retained_bytes > super::runtime::EVENT_BYTES {
            data.preparation_error = Some("Sound Card callback exceeded its 8 MiB prepared-event bound".into());
            data.retained_bytes = retained_bytes(&data, None);
            None
        } else {
            callback
        };
        Self {
            data: Some(data),
            callback,
        }
    }
}

impl EventPreparer {
    pub(super) fn new() -> Result<Self, String> {
        static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
        let mut pool = POOL
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if pool.is_none() {
            *pool = Some(
                PayloadPool::new("sound-card-event-disposer", 2048, 256 * 1024 * 1024)
                    .map_err(|error| format!("Sound Card event disposer unavailable: {error}"))?,
            );
        }
        Ok(Self {
            events: pool.as_ref().expect("installed event disposer").clone(),
            callbacks: NativeCallbackPreparer::new()?,
        })
    }

    #[cfg(test)]
    pub(super) fn using(events: PayloadPool, callbacks: PayloadPool) -> Self {
        Self {
            events,
            callbacks: NativeCallbackPreparer::using(callbacks),
        }
    }

    /// A full pool preserves the already measured draft. Only the native worker
    /// retries; actor service never allocates JSON or polls capacity.
    pub(super) fn try_prepare(&self, draft: &mut AudioEventDraft) -> Option<PreparedAudioEvent> {
        let data = draft.data.as_mut().expect("unpublished audio draft");
        if let Some(callback) = &draft.callback {
            data.callback = Some(self.callbacks.try_retain(callback)?);
            draft.callback = None;
        }
        let data = draft.data.take().expect("unpublished audio draft");
        let weight = data.retained_bytes;
        match self.events.try_retain(data, weight) {
            Ok(prepared) => Some(prepared),
            Err(returned) => {
                draft.data = Some(returned.into_value());
                None
            }
        }
    }
}

fn retained_bytes(data: &AudioEventData, callback: Option<&NativeCallbackDraft>) -> usize {
    let mut bytes = size_of::<AudioEventData>() + 2 * size_of::<usize>() + data.event.retained_bytes();
    bytes += data.outgoing_log.as_ref().map_or(0, String::capacity);
    bytes += data.preparation_error.as_ref().map_or(0, String::capacity);
    bytes += callback.map_or(0, NativeCallbackDraft::retained_bytes);
    if let Some(connection) = &data.connection {
        bytes += connection.log.as_ref().map_or(0, |log| log.message.capacity());
        if let ConnectionWarning::Set { message, detail } = &connection.warning {
            bytes += message.capacity() + detail.capacity();
        }
    }
    bytes
}

#[cfg(test)]
mod tests;
