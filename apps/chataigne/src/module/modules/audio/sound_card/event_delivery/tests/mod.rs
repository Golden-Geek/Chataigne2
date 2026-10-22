use super::super::prepared_event::{AudioEventDraft, EventPreparer};
use super::*;
use golden_audio::{AudioBackendState, AudioBackendStatus, BackendId};
use golden_core::{
    node::{Folder, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

fn backend_event() -> AudioEvent {
    AudioEvent::BackendStatusChanged(AudioBackendStatus {
        backend: BackendId::from_static("null"),
        label: "Fixture".into(),
        state: AudioBackendState::Available,
        detail: None,
    })
}

fn invalid_edit() -> Edit {
    Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    }
}

fn callback_count(engine: &crate::app::AppEngine, name: &str) -> usize {
    engine
        .ui_event_log()
        .iter()
        .filter(|event| {
            matches!(&event.kind,
                golden_core::events::EventKind::Custom(custom) if custom.payload["callback"] == name
            )
        })
        .count()
}

#[test]
fn failed_preceding_edit_retries_one_callback_without_replaying_an_applied_occurrence() {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let module = SoundCardModule::create();
    let preparer = EventPreparer::new().unwrap();
    let mut draft = AudioEventDraft::new(backend_event());
    let mut pending = PendingAudioEvents::default();
    pending.push(preparer.try_prepare(&mut draft).unwrap(), engine.root);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    pending.service(&mut ctx, &module);
    let count = ctx.edits.pending.len();
    pending.service(&mut ctx, &module);
    assert_eq!(ctx.edits.pending.len(), count, "pending receipt must not stage twice");
    engine.edits.push(invalid_edit());
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert_eq!(callback_count(&engine, "audioBackendStatusChanged"), 0);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    pending.service(&mut ctx, &module);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    pending.service(&mut ctx, &module);
    assert!(pending.is_empty());
    assert!(ctx.edits.pending.is_empty());
    assert_eq!(callback_count(&engine, "audioBackendStatusChanged"), 1);
}

fn status_fixture() -> (crate::app::AppEngine, NodeId, PendingAudioEvents) {
    use super::super::tests;
    let (mut engine, id) = tests::create_sound_card();
    tests::select_enum_parameter(&mut engine, id, "connection/audio_driver", "receipt_fixture");
    let mut status = golden_audio::AudioStreamStatus::disabled(golden_audio::AudioDirection::Output);
    status.enabled = true;
    status.selected_target = Some(golden_audio::AudioDeviceTargetId::SystemDefault {
        backend: BackendId::from_static("receipt_fixture"),
    });
    status.selected_label = Some("Fixture".into());
    status.readiness = golden_audio::AudioDeviceReadiness::Busy;
    let mut draft = AudioEventDraft::new(AudioEvent::DeviceStatusChanged(status));
    let event = EventPreparer::new().unwrap().try_prepare(&mut draft).unwrap();
    let mut pending = PendingAudioEvents::default();
    pending.push(event, id);
    engine.clear_ui_event_log();
    (engine, id, pending)
}

fn service(engine: &crate::app::AppEngine, id: NodeId, pending: &mut PendingAudioEvents) -> ProcessCtx {
    let crate::app::AppNode::SoundCardModule(module) = engine.nodes.get(id).unwrap() else {
        panic!("Sound Card");
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        pending.service(&mut ctx, module);
        if !pending.0.front().is_some_and(|front| front.external_wait) {
            return ctx;
        }
        assert!(ctx.edits.pending.is_empty());
        assert!(
            std::time::Instant::now() < deadline,
            "native feedback logging must complete"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn warning_retry_commits_before_callback_and_a_later_edit_failure_never_replays_it() {
    let (mut engine, id, mut pending) = status_fixture();
    let mut failed = service(&engine, id, &mut pending);
    engine.edits.push(invalid_edit());
    engine.absorb_edits(&mut failed).unwrap();
    assert!(engine.apply_edits().is_err());
    assert_eq!(callback_count(&engine, "audioDeviceStatusChanged"), 0);
    let mut warning = service(&engine, id, &mut pending);
    engine.absorb_edits(&mut warning).unwrap();
    engine.apply_edits().unwrap();
    let output = super::super::find_path(&engine.process_tree_snapshot(), id, "connection/output_device").unwrap();
    assert!(engine
        .nodes
        .get(output)
        .unwrap()
        .node_data()
        .meta
        .presentation
        .warning(Some("sound-card-output-device-connection"))
        .is_some());
    assert_eq!(callback_count(&engine, "audioDeviceStatusChanged"), 0);
    let mut callback = service(&engine, id, &mut pending);
    engine.absorb_edits(&mut callback).unwrap();
    engine.edits.push(invalid_edit());
    assert!(engine.apply_edits().is_err());
    let settled = service(&engine, id, &mut pending);
    assert!(pending.is_empty());
    assert!(settled.edits.pending.is_empty());
    assert_eq!(callback_count(&engine, "audioDeviceStatusChanged"), 1);
}

#[test]
fn deselection_suppresses_a_failed_warning_retry_but_preserves_the_callback_occurrence() {
    let (mut engine, id, mut pending) = status_fixture();
    drop(service(&engine, id, &mut pending));
    super::super::tests::select_enum_parameter(
        &mut engine,
        id,
        "connection/output_device",
        super::super::NO_AUDIO_DEVICE,
    );
    let mut callback = service(&engine, id, &mut pending);
    engine.absorb_edits(&mut callback).unwrap();
    engine.apply_edits().unwrap();
    let settled = service(&engine, id, &mut pending);
    assert!(pending.is_empty());
    assert!(settled.edits.pending.is_empty());
    let output = super::super::find_path(&engine.process_tree_snapshot(), id, "connection/output_device").unwrap();
    assert!(engine
        .nodes
        .get(output)
        .unwrap()
        .node_data()
        .meta
        .presentation
        .warning(Some("sound-card-output-device-connection"))
        .is_none());
    assert_eq!(callback_count(&engine, "audioDeviceStatusChanged"), 1);
}

#[test]
fn disabled_module_finishes_pending_publication_after_its_native_facade_is_gone() {
    let (mut engine, id) = super::super::tests::create_sound_card();
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.clear_ui_event_log();
    let mut draft = AudioEventDraft::new(backend_event());
    let event = EventPreparer::new().unwrap().try_prepare(&mut draft).unwrap();
    let crate::app::AppNode::SoundCardModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Sound Card");
    };
    assert!(module.runtime.is_none());
    module.pending_events.push(event, id);
    assert!(module.worker_wake_when_disabled());
    assert!(!module.worker_wake_requires_tree_snapshot());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module.on_worker_wake(&mut ctx);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let crate::app::AppNode::SoundCardModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Sound Card");
    };
    let mut settled = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module.on_worker_wake(&mut settled);
    assert!(module.pending_events.is_empty());
    assert!(!module.worker_wake_when_disabled());
    assert_eq!(callback_count(&engine, "audioBackendStatusChanged"), 1);
}

#[test]
fn removing_the_warning_parameter_cannot_strand_the_native_callback() {
    let (mut engine, id, mut pending) = status_fixture();
    drop(service(&engine, id, &mut pending));
    let output = super::super::find_path(&engine.process_tree_snapshot(), id, "connection/output_device").unwrap();
    engine.edits.push(Edit::RemoveNode { node: output });
    engine.apply_edits().unwrap();
    // Parameter handles are refreshed at the same engine callback boundary used
    // by native wake service, without recreating the deleted authored selector.
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    let crate::app::AppNode::SoundCardModule(module) = engine.nodes.get(id).unwrap() else {
        panic!("Sound Card");
    };
    assert!(!module.output_device.is_bound());
    let mut callback = service(&engine, id, &mut pending);
    engine.absorb_edits(&mut callback).unwrap();
    engine.apply_edits().unwrap();
    service(&engine, id, &mut pending);
    assert!(pending.is_empty());
    assert_eq!(callback_count(&engine, "audioDeviceStatusChanged"), 1);
}

#[test]
fn receipt_waits_reuse_selection_validation_until_the_parameter_values_change() {
    let (mut engine, id, mut pending) = status_fixture();
    let mut warning = service(&engine, id, &mut pending);
    for _ in 0..128 {
        let waiting = service(&engine, id, &mut pending);
        assert!(waiting.edits.pending.is_empty());
    }
    assert_eq!(pending.0.front().unwrap().selection.checks, 1);
    engine.absorb_edits(&mut warning).unwrap();
    engine.apply_edits().unwrap();
    let mut callback = service(&engine, id, &mut pending);
    assert_eq!(pending.0.front().unwrap().selection.checks, 1);
    engine.absorb_edits(&mut callback).unwrap();
    engine.apply_edits().unwrap();
    service(&engine, id, &mut pending);
    assert!(pending.is_empty());
    assert_eq!(callback_count(&engine, "audioDeviceStatusChanged"), 1);
}

#[test]
fn rejected_feedback_controls_warn_once_and_preserve_the_original_status_callback() {
    let (mut engine, id, mut pending) = status_fixture();
    let parameter = super::super::find_path(&engine.process_tree_snapshot(), id, "connection/output_device").unwrap();
    let mut constraints = engine
        .process_tree_snapshot()
        .node(parameter)
        .unwrap()
        .param_constraints
        .clone()
        .unwrap();
    constraints.enum_options = Default::default();
    engine.edits.push(Edit::SetParamConstraints {
        node: parameter,
        constraints,
    });
    engine.edits.push(Edit::SetParam {
        node: parameter,
        value: ParamValue::Enum("x".repeat(256 * 1024 + 1)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    assert!(
        matches!(engine.process_tree_snapshot().node(parameter).unwrap().param_value.as_ref(), Some(ParamValue::Enum(value)) if value.len() == 256 * 1024 + 1)
    );
    // Direct service calls bypass normal callback preprocessing of typed handles.
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    engine.apply_edits().unwrap();
    let mut warning = service(&engine, id, &mut pending);
    engine.edits.push(invalid_edit());
    engine.absorb_edits(&mut warning).unwrap();
    assert!(engine.apply_edits().is_err());
    let mut retry = service(&engine, id, &mut pending);
    engine.absorb_edits(&mut retry).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(callback_count(&engine, "audioDeviceStatusChanged"), 0);
    let mut callback = service(&engine, id, &mut pending);
    engine.absorb_edits(&mut callback).unwrap();
    engine.apply_edits().unwrap();
    service(&engine, id, &mut pending);
    assert!(pending.is_empty());
    assert_eq!(callback_count(&engine, "audioDeviceStatusChanged"), 1);
    assert!(engine
        .nodes
        .get(id)
        .unwrap()
        .node_data()
        .meta
        .presentation
        .warning(Some("sound-card-feedback-selection"))
        .is_some());
}

#[test]
fn selection_changes_cannot_reopen_feedback_after_callback_publication_is_queued() {
    let (mut engine, id, mut pending) = status_fixture();
    super::super::tests::select_enum_parameter(
        &mut engine,
        id,
        "connection/output_device",
        super::super::NO_AUDIO_DEVICE,
    );
    let mut callback = service(&engine, id, &mut pending);
    assert!(matches!(pending.0.front().unwrap().phase, DeliveryPhase::Callback));
    super::super::tests::select_enum_parameter(
        &mut engine,
        id,
        "connection/output_device",
        super::super::SYSTEM_DEFAULT_DEVICE,
    );
    let waiting = service(&engine, id, &mut pending);
    assert!(
        waiting.edits.pending.is_empty(),
        "later selection must not insert feedback after the queued callback"
    );
    engine.absorb_edits(&mut callback).unwrap();
    engine.apply_edits().unwrap();
    let settled = service(&engine, id, &mut pending);
    assert!(pending.is_empty());
    assert!(settled.edits.pending.is_empty());
    let output = super::super::find_path(&engine.process_tree_snapshot(), id, "connection/output_device").unwrap();
    assert!(engine
        .nodes
        .get(output)
        .unwrap()
        .node_data()
        .meta
        .presentation
        .warning(Some("sound-card-output-device-connection"))
        .is_none());
    assert_eq!(callback_count(&engine, "audioDeviceStatusChanged"), 1);
}

fn native_wait_fixture() -> (crate::app::AppEngine, SoundCardModule, PendingAudioEvents) {
    let engine = crate::app::AppEngine::new(Folder::new("root").into());
    let mut module = SoundCardModule::create();
    module.node_data_mut().id = engine.root;
    module.runtime_wake =
        Some(super::super::runtime::RuntimeWakeSender::new(engine.node_waker(engine.root).unwrap()).unwrap());
    let event = EventPreparer::new()
        .unwrap()
        .try_prepare(&mut AudioEventDraft::new(backend_event()))
        .unwrap();
    let mut pending = PendingAudioEvents::default();
    pending.push(event, engine.root);
    (engine, module, pending)
}

#[test]
fn native_logging_wait_does_not_self_wake_or_overtake_callbacks() {
    let (mut engine, module, mut pending) = native_wait_fixture();
    let complete = pending
        .0
        .front_mut()
        .unwrap()
        .logging
        .pending_for_test(module.runtime_wake.clone().unwrap());
    let event = EventPreparer::new()
        .unwrap()
        .try_prepare(&mut AudioEventDraft::new(backend_event()))
        .unwrap();
    pending.push(event, engine.root);
    for _ in 0..64 {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        pending.service(&mut ctx, &module);
        assert!(ctx.edits.pending.is_empty());
        assert!(pending.0.front().unwrap().external_wait);
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    }
    complete(true);
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 1);
    for expected in [1, 2] {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        pending.service(&mut ctx, &module);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        assert_eq!(callback_count(&engine, "audioBackendStatusChanged"), expected);
    }
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    pending.service(&mut ctx, &module);
    assert!(pending.is_empty());
    assert!(ctx.edits.pending.is_empty());
}

#[test]
fn failed_logging_warning_retries_before_the_single_callback_occurrence() {
    let (mut engine, module, mut pending) = native_wait_fixture();
    let complete = pending
        .0
        .front_mut()
        .unwrap()
        .logging
        .pending_for_test(module.runtime_wake.clone().unwrap());
    complete(false);
    let mut warning = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    pending.service(&mut warning, &module);
    engine.edits.push(invalid_edit());
    engine.absorb_edits(&mut warning).unwrap();
    assert!(engine.apply_edits().is_err());
    assert_eq!(callback_count(&engine, "audioBackendStatusChanged"), 0);
    let mut retry = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    pending.service(&mut retry, &module);
    engine.absorb_edits(&mut retry).unwrap();
    engine.apply_edits().unwrap();
    assert!(engine
        .nodes
        .get(engine.root)
        .unwrap()
        .node_data()
        .meta
        .presentation
        .warning(Some("sound-card-event-logging"))
        .is_some());
    assert_eq!(callback_count(&engine, "audioBackendStatusChanged"), 0);
    let mut callback = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    pending.service(&mut callback, &module);
    engine.absorb_edits(&mut callback).unwrap();
    engine.apply_edits().unwrap();
    let mut settled = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    pending.service(&mut settled, &module);
    assert!(pending.is_empty());
    assert_eq!(callback_count(&engine, "audioBackendStatusChanged"), 1);
}
