use super::*;
use golden_audio::{PlaybackId, PlaybackInfo, VoiceId};
use std::time::Duration;

fn started() -> AudioEvent {
    AudioEvent::PlaybackStarted(PlaybackInfo {
        playback_id: PlaybackId::from_static("fixture"),
        path: "clip.wav".into(),
        voice: VoiceId::new(1, 2),
    })
}

#[test]
fn prepared_playback_keeps_callback_shape_retention_and_log_without_actor_serialization() {
    let preparer = EventPreparer::new().unwrap();
    let mut draft = AudioEventDraft::new(started());
    let event = preparer.try_prepare(&mut draft).unwrap();
    assert!(event.preparation_error.is_none());
    assert!(event.retained_bytes <= super::super::runtime::EVENT_BYTES);
    let callback = event.callback(NodeId(42)).unwrap();
    assert_eq!(callback.origin, Some(NodeId(42)));
    assert_eq!(callback.retention, CustomEventRetention::Transient);
    assert_eq!(callback.payload["callback"], "playbackStarted");
    assert_eq!(callback.payload["args"][0], "fixture");
    assert_eq!(callback.payload["args"][1], "clip.wav");
    assert!(event.outgoing_log.as_ref().unwrap().contains("Started playing"));
}

#[test]
fn both_lifetime_pools_recover_without_repreparing_the_waiting_callback() {
    let events = PayloadPool::new("audio-prepared-test", 1, 1024 * 1024).unwrap();
    let callbacks = PayloadPool::new("audio-callback-test", 1, 1024 * 1024).unwrap();
    let preparer = EventPreparer::using(events.clone(), callbacks.clone());
    let mut first = AudioEventDraft::new(started());
    let first = preparer.try_prepare(&mut first).unwrap();
    let held = first.callback(NodeId(1)).unwrap();
    let mut next = AudioEventDraft::new(started());
    let original = next.callback.as_ref().unwrap().retained_bytes();
    assert!(preparer.try_prepare(&mut next).is_none());
    assert_eq!(next.callback.as_ref().unwrap().retained_bytes(), original);
    drop(first);
    assert!(events.wait_for_idle(Duration::from_secs(2)));
    assert!(
        preparer.try_prepare(&mut next).is_none(),
        "exported callback still pins its own quota"
    );
    drop(held);
    assert!(callbacks.wait_for_idle(Duration::from_secs(2)));
    let second = preparer.try_prepare(&mut next).unwrap();
    assert_eq!(second.callback(NodeId(1)).unwrap().payload["args"][0], "fixture");
}

#[test]
fn event_pool_pressure_keeps_the_already_admitted_callback_and_original_json() {
    let events = PayloadPool::new("audio-envelope-pressure", 1, 1024 * 1024).unwrap();
    let callbacks = PayloadPool::new("audio-json-pressure", 1, 1024 * 1024).unwrap();
    let occupied = events.try_reserve(1).unwrap();
    let preparer = EventPreparer::using(events.clone(), callbacks.clone());
    let mut draft = AudioEventDraft::new(started());
    assert!(preparer.try_prepare(&mut draft).is_none());
    assert!(draft.callback.is_none());
    let original = draft.data.as_ref().unwrap().callback(NodeId(1)).unwrap();
    assert!(callbacks.try_reserve(1).is_err());
    assert!(preparer.try_prepare(&mut draft).is_none());
    drop(occupied);
    let prepared = preparer.try_prepare(&mut draft).unwrap();
    let result = prepared.callback(NodeId(2)).unwrap();
    assert!(std::sync::Arc::ptr_eq(&original.payload, &result.payload));
    assert_eq!(result.origin, Some(NodeId(2)));
}

pub(super) struct DropProbe {
    started: std::sync::mpsc::Sender<std::thread::ThreadId>,
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.started.send(std::thread::current().id()).unwrap();
        self.release.lock().unwrap().recv().unwrap();
    }
}

#[test]
fn final_envelope_disposal_stays_off_actor_and_holds_capacity_through_cleanup() {
    let events = PayloadPool::new("audio-final-disposal", 1, 1024 * 1024).unwrap();
    let callbacks = PayloadPool::new("audio-disposal-callbacks", 1, 1024 * 1024).unwrap();
    let preparer = EventPreparer::using(events.clone(), callbacks);
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut draft = AudioEventDraft::new(AudioEvent::ShutdownComplete);
    draft.data.as_mut().unwrap().drop_probe = Some(DropProbe {
        started: started_tx,
        release: std::sync::Mutex::new(release_rx),
    });
    let prepared = preparer.try_prepare(&mut draft).unwrap();
    drop(prepared);
    let thread = started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_ne!(thread, std::thread::current().id());
    let mut next = AudioEventDraft::new(AudioEvent::ShutdownComplete);
    let still_charged = preparer.try_prepare(&mut next).is_none();
    release_tx.send(()).unwrap();
    assert!(still_charged, "blocked final destruction must retain its quota");
    assert!(events.wait_for_idle(Duration::from_secs(2)));
    assert!(preparer.try_prepare(&mut next).is_some());
}

#[test]
fn maximum_playback_identity_and_path_fit_the_prepared_bound_without_truncation() {
    let id = PlaybackId::new("p".repeat(PlaybackId::MAX_BYTES)).unwrap();
    let path = std::path::PathBuf::from("x".repeat(golden_audio::PlayFileRequest::MAX_PATH_BYTES));
    let preparer = EventPreparer::new().unwrap();
    let mut draft = AudioEventDraft::new(AudioEvent::PlaybackStarted(PlaybackInfo {
        playback_id: id.clone(),
        path: path.clone(),
        voice: VoiceId::new(1, 1),
    }));
    let event = preparer.try_prepare(&mut draft).unwrap();
    assert!(event.preparation_error.is_none());
    assert!(event.retained_bytes < super::super::runtime::EVENT_BYTES);
    let callback = event.callback(NodeId(1)).unwrap();
    assert_eq!(callback.payload["args"][0], id.as_str());
    assert_eq!(callback.payload["args"][1], path.to_string_lossy().as_ref());
    assert_eq!(callback.payload["args"][2]["playback_id"], id.as_str());
    assert_eq!(callback.payload["args"][2]["path"], path.to_string_lossy().as_ref());
}
