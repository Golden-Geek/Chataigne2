use std::path::PathBuf;

use golden_audio::{
    AudioDirection, AudioError, AudioErrorCategory, AudioEvent,
    AudioStreamStatus, PlaybackFailure, PlaybackId, PlaybackInfo, VoiceId,
};
use golden_core::{
    edit::Edit,
    engine::EngineTime,
    events::{CustomEvent, CustomEventRetention, EventKind},
    node::{Node, NodeMetaPatch},
    parameter::ParamValue,
    process_ctx::{ExecutionPhase, ProcessCtx},
    script::ScriptSource,
};

use super::*;
use super::super::{
    integration::{
        SoundCardCommandResultEvent, SOUND_CARD_COMMAND_RESULT_TOPIC,
    },
    script::{
        AUDIO_BACKEND_STATUS_CHANGED_CALLBACK,
        AUDIO_DEVICE_STATUS_CHANGED_CALLBACK, PLAYBACK_FAILED_CALLBACK,
        PLAYBACK_FINISHED_CALLBACK, PLAYBACK_STARTED_CALLBACK,
        PLAYBACK_STOPPED_CALLBACK, SOUND_CARD_SCRIPT_METHODS,
    },
};
use crate::app::{
    module_modules_audio_sound_card_commands::{
        SoundCardCommandRequest, SoundCardPlayFileCommand,
        SoundCardSetMasterVolumeCommand, SoundCardStopAllFilesCommand,
    },
};

#[test]
fn command_nodes_execute_manual_auto_and_external_requests() {
    let (mut engine, module) = create_sound_card_module();
    let tester = find_path(&engine, module, "command_tester").expect("command tester");
    engine.add_node(
        SoundCardSetMasterVolumeCommand::create().into(),
        Some(tester),
    );
    stabilize(&mut engine);
    let command = direct_children_of_type(
        &engine,
        tester,
        SoundCardSetMasterVolumeCommand::NODE_TYPE,
    )[0];
    let volume = find_path(&engine, command, "volume_db").expect("command volume");
    let trigger = find_path(&engine, command, "trigger").expect("command trigger");
    let auto_trigger =
        find_path(&engine, command, "auto_trigger").expect("auto trigger");

    engine.clear_ui_event_log();
    set_param(&mut engine, volume, ParamValue::Float(-6.0));
    set_param(&mut engine, trigger, ParamValue::Trigger());
    flush_command_flow(&mut engine);
    let manual = command_requests(&engine);
    assert_eq!(manual.len(), 1);
    assert_eq!(
        manual[0].payload,
        serde_json::to_value(SoundCardCommandRequest::SetMasterVolume {
            gain: golden_audio::GainDb::new(-6.0).unwrap(),
        })
        .unwrap()
    );
    assert_admitted_results(&engine, 1);
    assert_eq!(
        param_value(&engine, module, "parameters/master_volume_db"),
        Some(&ParamValue::Float(-6.0)),
        "the command must update the authored master-volume parameter",
    );

    engine.clear_ui_event_log();
    set_param(&mut engine, auto_trigger, ParamValue::Bool(true));
    flush_command_flow(&mut engine);
    engine.clear_ui_event_log();
    set_param(&mut engine, volume, ParamValue::Float(-9.0));
    flush_command_flow(&mut engine);
    assert_eq!(command_requests(&engine).len(), 1);
    assert_admitted_results(&engine, 1);
    assert_eq!(
        param_value(&engine, module, "parameters/master_volume_db"),
        Some(&ParamValue::Float(-9.0)),
    );

    engine.add_node(
        SoundCardStopAllFilesCommand::create().into(),
        None,
    );
    stabilize(&mut engine);
    let external = direct_children_of_type(
        &engine,
        engine.root,
        SoundCardStopAllFilesCommand::NODE_TYPE,
    )[0];
    let target =
        find_path(&engine, external, "target_module").expect("external target");
    let external_trigger =
        find_path(&engine, external, "trigger").expect("external trigger");
    engine.clear_ui_event_log();
    let module_reference = node_reference(&engine, module);
    set_param(
        &mut engine,
        target,
        ParamValue::Reference(module_reference),
    );
    set_param(&mut engine, external_trigger, ParamValue::Trigger());
    flush_command_flow(&mut engine);
    let requests = command_requests(&engine);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].module_id, module);
    assert_eq!(
        serde_json::from_value::<SoundCardCommandRequest>(
            requests[0].payload.clone()
        )
        .unwrap(),
        SoundCardCommandRequest::StopAllFiles
    );
    assert_admitted_results(&engine, 1);
}

#[test]
fn multiplex_execute_snapshot_overrides_path_and_playback_id_per_lane() {
    let (mut engine, module) = create_sound_card_module();
    let tester = find_path(&engine, module, "command_tester").expect("command tester");
    engine.add_node(SoundCardPlayFileCommand::create().into(), Some(tester));
    stabilize(&mut engine);
    let command = direct_children_of_type(
        &engine,
        tester,
        SoundCardPlayFileCommand::NODE_TYPE,
    )[0];
    let path = find_path(&engine, command, "audio_file").expect("audio file");
    let playback_id =
        find_path(&engine, command, "playback_id").expect("playback ID");

    let lanes = [
        ("C:/audio/lane-a.wav", "lane-a"),
        ("C:/audio/lane-b.wav", "lane-b"),
        ("C:/audio/lane-a-replacement.wav", "lane-a"),
    ];
    let mut admitted_sequences = Vec::new();
    for (lane_path, lane_id) in lanes {
        engine.clear_ui_event_log();
        emit_multiplex_execute(
            &mut engine,
            command,
            vec![
                crate::app::module_command::ModuleCommandParamOverride {
                    param_id: path,
                    value: ParamValue::File(lane_path.to_string()),
                },
                crate::app::module_command::ModuleCommandParamOverride {
                    param_id: playback_id,
                    value: ParamValue::Str(lane_id.to_string()),
                },
            ],
        );
        flush_command_flow(&mut engine);

        let requests = command_requests(&engine);
        assert_eq!(requests.len(), 1);
        assert_eq!(
            serde_json::from_value::<SoundCardCommandRequest>(
                requests[0].payload.clone()
            )
            .unwrap(),
            SoundCardCommandRequest::PlayFile {
                path: PathBuf::from(lane_path),
                playback_id: PlaybackId::new(lane_id).unwrap(),
            }
        );
        let results = command_results(&engine);
        assert_eq!(results.len(), 1);
        admitted_sequences.push(
            results[0]
                .admitted_sequence
                .expect("multiplex request should be admitted"),
        );
    }
    assert!(
        admitted_sequences.windows(2).all(|pair| pair[0] < pair[1]),
        "same-lane replacement and cross-lane requests must preserve order"
    );
}

#[test]
fn channel_gain_admission_accepts_only_live_local_virtual_outputs() {
    let (mut engine, module) = create_sound_card_module();
    engine.add_node(SoundCardModule::create().into(), None);
    stabilize(&mut engine);
    let modules =
        direct_children_of_type(&engine, engine.root, SoundCardModule::NODE_TYPE);
    let local_input = typed_children(
        &engine,
        module,
        VIRTUAL_INPUTS_PATH,
        SoundCardVirtualInput::NODE_TYPE,
    )[0];
    let local_outputs = typed_children(
        &engine,
        module,
        VIRTUAL_OUTPUTS_PATH,
        SoundCardVirtualOutput::NODE_TYPE,
    );
    let foreign_output = typed_children(
        &engine,
        modules[1],
        VIRTUAL_OUTPUTS_PATH,
        SoundCardVirtualOutput::NODE_TYPE,
    )[0];

    let snapshot = engine.process_tree_snapshot();
    assert!(admit_channel_gain(
        &engine,
        module,
        &snapshot,
        node_reference(&engine, local_outputs[0])
    )
    .is_ok());
    assert_eq!(
        admit_channel_gain(
            &engine,
            module,
            &snapshot,
            node_reference(&engine, local_input)
        )
        .unwrap_err()
        .category,
        AudioErrorCategory::InvalidConfiguration
    );
    assert!(admit_channel_gain(
        &engine,
        module,
        &snapshot,
        node_reference(&engine, foreign_output)
    )
    .is_err());

    let deleted_reference = node_reference(&engine, local_outputs[1]);
    engine
        .edits
        .push(Edit::RemoveNode { node: local_outputs[1] });
    engine.apply_edits().expect("output removal should apply");
    let deleted_snapshot = engine.process_tree_snapshot();
    assert!(admit_channel_gain(
        &engine,
        module,
        &deleted_snapshot,
        deleted_reference
    )
    .is_err());
}

#[test]
fn disabled_module_rejects_commands_but_stop_is_independent_of_output_readiness() {
    let (mut engine, module) = create_sound_card_module();
    let output_enabled =
        find_path(&engine, module, "connection/output_enabled").unwrap();
    set_param(&mut engine, output_enabled, ParamValue::Bool(false));
    engine.apply_edits().expect("output disable should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("output disable should dispatch");
    let snapshot = engine.process_tree_snapshot();
    assert!(admit_request(
        &engine,
        module,
        &snapshot,
        SoundCardCommandRequest::StopFile {
            playback_id: PlaybackId::new("missing-output-lane").unwrap(),
        }
    )
    .is_ok());
    assert!(admit_request(
        &engine,
        module,
        &snapshot,
        SoundCardCommandRequest::StopAllFiles
    )
    .is_ok());

    engine.edits.push(Edit::PatchMeta {
        node: module,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..NodeMetaPatch::default()
        },
    });
    engine.apply_edits().expect("module disable should apply");
    let disabled_snapshot = engine.process_tree_snapshot();
    assert_eq!(
        admit_request(
            &engine,
            module,
            &disabled_snapshot,
            SoundCardCommandRequest::StopAllFiles
        )
        .unwrap_err()
        .category,
        AudioErrorCategory::InvalidConfiguration
    );
}

#[test]
fn script_descriptor_template_and_host_dispatch_cover_the_sound_card_surface() {
    let (mut engine, module) = create_sound_card_module();
    let descriptor = engine
        .nodes
        .get(module)
        .expect("Sound Card")
        .engine_script_descriptor();
    for method in SOUND_CARD_SCRIPT_METHODS {
        assert!(
            descriptor
                .methods
                .iter()
                .any(|candidate| candidate == method),
            "Sound Card descriptor should advertise {method}"
        );
    }

    let config = crate::app::module::script_api::module_script_config(
        SoundCardModule::NODE_TYPE,
    );
    let ScriptSource::Inline(source) = config.source else {
        panic!("Sound Card template should expand inline");
    };
    for expected in [
        "local.playFile(path, playbackId)",
        "local.setChannelVolume(channel, volumeDb)",
        "function playbackStarted",
        "function audioDeviceStatusChanged",
    ] {
        assert!(
            source.contains(expected),
            "expanded Sound Card template should contain {expected}"
        );
    }
    assert!(!source.contains("{{include:"));

    engine.edits.push(Edit::CallNodeScriptMethod {
        node: module,
        method: "stopAllFiles".to_string(),
        args: Vec::new(),
    });
    engine
        .apply_edits()
        .expect("stopAllFiles script method should admit immediately");

    engine.edits.push(Edit::CallNodeScriptMethod {
        node: module,
        method: "setChannelVolume".to_string(),
        args: vec![
            ParamValue::Str("physical:0".to_string()),
            ParamValue::Float(0.0),
        ],
    });
    assert!(
        engine.apply_edits().is_err(),
        "physical channel strings must be rejected by script dispatch"
    );
}

#[test]
fn playback_callbacks_are_transient_and_have_the_documented_argument_shapes() {
    let module = SoundCardModule::create();
    let playback_id = PlaybackId::new("callback-lane").unwrap();
    let info = PlaybackInfo {
        playback_id: playback_id.clone(),
        path: PathBuf::from("callback.wav"),
        voice: VoiceId::new(3, 7),
    };
    let events = [
        AudioEvent::PlaybackStarted(info.clone()),
        AudioEvent::PlaybackFinished(info.clone()),
        AudioEvent::PlaybackStopped(golden_audio::PlaybackStopInfo {
            playback_id: playback_id.clone(),
            voice: Some(info.voice),
            reason: golden_audio::PlaybackStopReason::Requested,
        }),
        AudioEvent::PlaybackFailed(PlaybackFailure {
            playback_id,
            path: PathBuf::from("callback.wav"),
            error: AudioError::new(
                AudioErrorCategory::DecodeFailed,
                "fixture failure",
            ),
        }),
    ];
    let expected = [
        (PLAYBACK_STARTED_CALLBACK, 3),
        (PLAYBACK_FINISHED_CALLBACK, 2),
        (PLAYBACK_STOPPED_CALLBACK, 3),
        (PLAYBACK_FAILED_CALLBACK, 3),
    ];

    for (event, (callback, argument_count)) in events.iter().zip(expected) {
        let mut ctx = test_process_ctx();
        module.emit_audio_event_callback(&mut ctx, event);
        let custom = last_emitted_custom_event(&ctx);
        assert_eq!(custom.retention, CustomEventRetention::Transient);
        assert_eq!(custom.payload["callback"], callback);
        assert_eq!(
            custom.payload["args"].as_array().unwrap().len(),
            argument_count
        );
    }
}

#[test]
fn device_and_backend_callbacks_are_replayable_status_changes() {
    let module = SoundCardModule::create();
    let events = [
        AudioEvent::DeviceStatusChanged(AudioStreamStatus::disabled(
            AudioDirection::Output,
        )),
        AudioEvent::BackendStatusChanged(AudioBackendStatus {
            backend: BackendId::new("callback-backend").unwrap(),
            label: "Callback Backend".to_string(),
            state: AudioBackendState::Available,
            detail: None,
        }),
    ];
    let expected = [
        AUDIO_DEVICE_STATUS_CHANGED_CALLBACK,
        AUDIO_BACKEND_STATUS_CHANGED_CALLBACK,
    ];
    for (event, callback) in events.iter().zip(expected) {
        let mut ctx = test_process_ctx();
        module.emit_audio_event_callback(&mut ctx, event);
        let custom = last_emitted_custom_event(&ctx);
        assert_eq!(custom.retention, CustomEventRetention::Replay);
        assert_eq!(custom.payload["callback"], callback);
        assert_eq!(custom.payload["args"].as_array().unwrap().len(), 2);
    }
}

fn admit_channel_gain(
    engine: &crate::app::AppEngine,
    module: NodeId,
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    virtual_output: NodeReference,
) -> Result<golden_audio::CommandSequence, AudioError> {
    admit_request(
        engine,
        module,
        snapshot,
        SoundCardCommandRequest::SetChannelVolume {
            virtual_output,
            gain: golden_audio::GainDb::new(-3.0).unwrap(),
        },
    )
}

fn admit_request(
    engine: &crate::app::AppEngine,
    module: NodeId,
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    request: SoundCardCommandRequest,
) -> Result<golden_audio::CommandSequence, AudioError> {
    let crate::app::AppNode::SoundCardModule(sound_card) =
        engine.nodes.get(module).expect("Sound Card module")
    else {
        panic!("expected Sound Card module");
    };
    sound_card.admit_request(snapshot, request)
}

fn emit_multiplex_execute(
    engine: &mut crate::app::AppEngine,
    command: NodeId,
    param_overrides: crate::app::module_command::ModuleCommandParamOverrides,
) {
    let payload = crate::app::module_command::ModuleCommandExecuteEvent {
        command_id: command,
        param_overrides,
        invocation_id: None,
        delivery_policy:
            crate::app::module_command::ModuleCommandDeliveryPolicy::Standard,
    };
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::from_payload(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            &payload,
        )
        .unwrap(),
    });
}

fn flush_command_flow(engine: &mut crate::app::AppEngine) {
    for _ in 0..8 {
        engine.apply_edits().expect("command edits should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("command inbox should dispatch");
    }
}

fn command_requests(
    engine: &crate::app::AppEngine,
) -> Vec<crate::app::module_command::ModuleCommandRequestEvent> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            let EventKind::Custom(custom) = &event.kind else {
                return None;
            };
            crate::app::module_command::decode_module_command_request(custom)
        })
        .collect()
}

fn command_results(
    engine: &crate::app::AppEngine,
) -> Vec<SoundCardCommandResultEvent> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            let EventKind::Custom(custom) = &event.kind else {
                return None;
            };
            (custom.topic == SOUND_CARD_COMMAND_RESULT_TOPIC)
                .then(|| custom.payload_as().ok())
                .flatten()
        })
        .collect()
}

fn assert_admitted_results(engine: &crate::app::AppEngine, expected: usize) {
    let results = command_results(engine);
    assert_eq!(results.len(), expected);
    assert!(
        results
            .iter()
            .all(|result| result.admitted_sequence.is_some()
                && result.error.is_none())
    );
}

fn test_process_ctx() -> ProcessCtx {
    ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    )
}

fn last_emitted_custom_event(
    ctx: &ProcessCtx,
) -> &golden_core::events::CustomEvent {
    let Edit::EmitCustomEvent { event } = &ctx
        .edits
        .pending
        .last()
        .expect("callback should emit an event")
        .edit
    else {
        panic!("callback should emit a custom event");
    };
    event
}
