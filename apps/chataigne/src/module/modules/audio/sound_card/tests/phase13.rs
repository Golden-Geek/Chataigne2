use std::path::PathBuf;

use chataigne_sound_card_protocol::{
    SOUND_CARD_UI_CONTROL_TOPIC, SoundCardPlaybackLifecycle,
    SoundCardUiControlRequest,
};
use golden_audio::{
    AudioEvent, PlaybackId, PlaybackInfo, PlaybackStopInfo,
    PlaybackStopReason, SampleRate, VoiceId,
};
use golden_core::{
    engine::EngineTime,
    events::CustomEvent,
    node::DeclId,
    parameter::ParameterEventBehaviour,
    process_ctx::{ExecutionPhase, ProcessCtx},
    ui_sync::{UiCreateUserItemInitialParam, UiEditIntent},
};

use super::*;

#[test]
fn playback_lifecycle_projection_tracks_started_and_terminal_events() {
    let mut runtime = runtime::SoundCardRuntime::start(
        SampleRate::new(48_000).expect("sample rate"),
    )
    .expect("Sound Card runtime");
    let playback_id = PlaybackId::new("ui-lifecycle").expect("playback ID");
    let info = PlaybackInfo {
        playback_id: playback_id.clone(),
        path: PathBuf::from("C:/audio/music.wav"),
        voice: VoiceId::new(3, 9),
    };

    runtime.observe_event_for_test(&AudioEvent::PlaybackStarted(info.clone()));
    let voices = runtime.playback_voices_for_test();
    assert_eq!(voices.len(), 1);
    assert_eq!(voices[0].playback_id, "ui-lifecycle");
    assert_eq!(voices[0].path, "C:/audio/music.wav");
    assert_eq!(voices[0].voice, "3:9");
    assert_eq!(voices[0].lifecycle, SoundCardPlaybackLifecycle::Playing);

    runtime.observe_event_for_test(&AudioEvent::PlaybackStopped(
        PlaybackStopInfo {
            playback_id,
            voice: Some(info.voice),
            reason: PlaybackStopReason::Requested,
        },
    ));
    assert!(runtime.playback_voices_for_test().is_empty());
    runtime.stop();
}

#[test]
fn ui_stop_control_targets_the_module_runtime_without_project_edits() {
    let (engine, module) = create_sound_card_module();
    let snapshot = engine.process_tree_snapshot();
    let crate::app::AppNode::SoundCardModule(sound_card) =
        engine.nodes.get(module).expect("Sound Card module")
    else {
        panic!("expected Sound Card module");
    };
    let mut ctx = phase13_process_ctx();
    ctx.set_tree_snapshot(snapshot);
    let event = CustomEvent::from_payload(
        SOUND_CARD_UI_CONTROL_TOPIC,
        Some(module),
        &SoundCardUiControlRequest::StopAllFiles,
    )
    .expect("UI control event");

    assert!(sound_card.handle_sound_card_ui_control_event(&mut ctx, &event));
    assert!(!sound_card.handle_sound_card_ui_control_event(
        &mut ctx,
        &CustomEvent::new("unrelated", Some(module), serde_json::Value::Null),
    ));
}

#[test]
fn matrix_intents_create_atomically_and_gain_and_removal_round_trip_history() {
    let (mut engine, module) = create_sound_card_module();
    let monitoring = find_path(
        &engine,
        module,
        "parameters/monitoring_routes",
    )
    .expect("monitoring routes");
    let input = typed_children(
        &engine,
        module,
        VIRTUAL_INPUTS_PATH,
        SoundCardVirtualInput::NODE_TYPE,
    )[0];
    let output = typed_children(
        &engine,
        module,
        VIRTUAL_OUTPUTS_PATH,
        SoundCardVirtualOutput::NODE_TYPE,
    )[0];

    let created = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: monitoring,
        node_type: SoundCardMonitorRoute::NODE_TYPE.to_string(),
        label: None,
        initial_params: vec![
            UiCreateUserItemInitialParam {
                decl_id: DeclId("virtual_input".to_string()),
                value: ParamValue::Reference(node_reference(&engine, input)),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("virtual_output".to_string()),
                value: ParamValue::Reference(node_reference(&engine, output)),
            },
            UiCreateUserItemInitialParam {
                decl_id: DeclId("gain_db".to_string()),
                value: ParamValue::Float(0.0),
            },
        ],
    });
    assert!(created.success, "matrix route creation should be admitted");
    let route = direct_children_of_type(
        &engine,
        monitoring,
        SoundCardMonitorRoute::NODE_TYPE,
    )[0];
    let gain = find_path(&engine, route, "gain_db").expect("route gain");
    assert_eq!(
        param_value(&engine, route, "gain_db"),
        Some(&ParamValue::Float(0.0))
    );

    assert!(
        engine
            .apply_ui_intent(UiEditIntent::BeginEdit {
                client_edit_id: "phase13-gain".to_string(),
                label: Some("Edit monitor gain".to_string()),
            })
            .success
    );
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: gain,
                value: ParamValue::Float(-6.0),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .success
    );
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::EndEdit {
                client_edit_id: "phase13-gain".to_string(),
            })
            .success
    );
    assert_eq!(
        param_value(&engine, route, "gain_db"),
        Some(&ParamValue::Float(-6.0))
    );

    assert!(engine.apply_ui_intent(UiEditIntent::Undo).success);
    assert_eq!(
        param_value(&engine, route, "gain_db"),
        Some(&ParamValue::Float(0.0))
    );
    assert!(engine.apply_ui_intent(UiEditIntent::Redo).success);
    assert_eq!(
        param_value(&engine, route, "gain_db"),
        Some(&ParamValue::Float(-6.0))
    );

    assert!(
        engine
            .apply_ui_intent(UiEditIntent::BeginEdit {
                client_edit_id: "phase13-remove".to_string(),
                label: Some("Remove monitor route".to_string()),
            })
            .success
    );
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::RemoveNode { node: route })
            .success
    );
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::EndEdit {
                client_edit_id: "phase13-remove".to_string(),
            })
            .success
    );
    assert!(!engine.nodes.contains(route));
    assert!(engine.apply_ui_intent(UiEditIntent::Undo).success);
    assert!(engine.nodes.contains(route));
    assert!(engine.apply_ui_intent(UiEditIntent::Redo).success);
    assert!(!engine.nodes.contains(route));
}

fn phase13_process_ctx() -> ProcessCtx {
    ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    )
}
