use super::super::{metadata::Probe, registry::Source};
use super::*;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::*,
};
use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};

fn edit(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}
fn create(engine: &mut AppEngine) -> ChannelReference {
    let MediaEditResult::Created { reference } = edit(
        engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Video,
            label: None,
        },
    ) else {
        panic!()
    };
    reference
}
fn file(engine: &mut AppEngine, reference: ChannelReference, file: &str) {
    edit(
        engine,
        MediaEdit::ConfigureVideoFile {
            reference,
            file: file.into(),
        },
    );
}
fn control(engine: &AppEngine, reference: ChannelReference) -> Arc<VideoControl> {
    let Some(AppNode::Media(node)) = engine.nodes.get(engine.node_id_by_uuid(reference.media.0).unwrap()) else {
        panic!()
    };
    node.video_control().unwrap()
}
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/golden_video/src/decode/tests/fixtures")
}
fn probe(engine: &AppEngine) -> Probe {
    let source = Source::capture(engine).unwrap();
    let mut probe = Probe::default();
    probe.reconcile(&source, &root()).unwrap();
    probe.run_due(&source, || false).unwrap();
    probe
}

#[test]
fn unopened_variants_define_the_longest_timeline_and_authored_clear_removal_history_and_reload_reconcile() {
    let mut engine: AppEngine = golden_core::app::create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let first = create(&mut engine);
    let MediaEditResult::Created { reference: second } = edit(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    file(&mut engine, first, "h264.mp4");
    file(&mut engine, second, "h264-keyframes.mp4");
    let control = control(&engine, first);
    let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    probe(&engine);
    assert_eq!(engine.undo_len(), history, "metadata is not authored history");
    assert_eq!(golden_core::app::to_sparse_project_json_pretty(&engine).unwrap(), saved);
    edit(
        &mut engine,
        MediaEdit::Video {
            media: first.media,
            edit: protocol::VideoEdit::Seek { position_ms: 400.0 },
        },
    );
    assert_eq!(
        control.sample().unwrap().position,
        Duration::from_millis(400),
        "the unopened variant extends the timeline beyond the short clip"
    );
    file(&mut engine, second, "");
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(250));
    assert!(engine.undo().unwrap());
    probe(&engine);
    edit(
        &mut engine,
        MediaEdit::Video {
            media: first.media,
            edit: protocol::VideoEdit::Seek { position_ms: 400.0 },
        },
    );
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(400));
    edit(&mut engine, MediaEdit::RemoveChannel { reference: second });
    assert_eq!(control.metadata_files().unwrap().len(), 1);
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(250));
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded: AppEngine = golden_core::app::from_sparse_project_json(&json).unwrap();
    loaded.apply_edits().unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
    probe(&loaded);
    let loaded_control = self::control(&loaded, first);
    assert!(!Arc::ptr_eq(&control, &loaded_control));
    assert_eq!(loaded_control.sample().unwrap().position, Duration::from_millis(250));
    edit(&mut engine, MediaEdit::RemoveMedia { media: first.media });
    assert!(
        Source::capture(&engine).unwrap().registry.controls().is_empty(),
        "history retention alone must not keep file probing alive"
    );
    assert_eq!(
        control.metadata_files().unwrap().len(),
        1,
        "an outgoing captured producer may still own the retired media"
    );
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(250));
}

#[test]
fn generic_file_parameter_edits_bind_and_clear_without_a_render_consumer() {
    use golden_core::{
        edit::Edit,
        parameter::{ParamValue, ParameterEventBehaviour},
    };
    let mut engine: AppEngine = golden_core::app::create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let reference = create(&mut engine);
    file(&mut engine, reference, "hevc.mp4");
    probe(&engine);
    let control = control(&engine, reference);
    let id = engine.node_id_by_uuid(reference.channel.0).unwrap();
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(id) else {
        panic!()
    };
    let parameter = channel.file_parameter();
    engine.edits.push(Edit::SetParam {
        node: parameter,
        value: ParamValue::File("".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.settle_edit_notifications(16).unwrap();
    assert!(control.metadata_files().unwrap().is_empty());
    assert!(!control.snapshot(reference.media).unwrap().ready);
}

#[test]
fn authoritative_headless_metadata_service_prepares_unrendered_files_and_retires_without_history() {
    use golden_core::{
        application::{ProductionRuntime, ProjectTransactions},
        ui_sync::UiProjectFileSpec,
    };
    let mut engine: AppEngine = golden_core::app::create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let reference = create(&mut engine);
    file(&mut engine, reference, root().join("prores.mov").to_str().unwrap());
    let control = control(&engine, reference);
    let history = engine.undo_len();
    let runtime = ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap();
    let service = super::super::metadata::Service::start(runtime.clone()).unwrap();
    let pool = service.pool.clone();
    let deadline = std::time::Instant::now() + seconds(5);
    while !control.snapshot(reference.media).unwrap().ready {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(runtime.capture_app(|engine| engine.undo_len()).unwrap(), history);
    let ack = runtime
        .apply_transaction(UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::Video {
                media: reference.media,
                edit: protocol::VideoEdit::Seek { position_ms: 200.0 },
            })
            .unwrap(),
        })
        .unwrap();
    assert!(ack.success);
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(200));
    drop(service);
    assert!(pool.wait_for_idle(seconds(5)));
    assert_eq!(runtime.capture_app(|engine| engine.undo_len()).unwrap(), history + 1);
}

#[test]
fn relative_root_changes_missing_file_retry_and_relink_preserve_only_the_matching_known_metadata() {
    let mut engine: AppEngine = golden_core::app::create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let reference = create(&mut engine);
    file(&mut engine, reference, "clip.mp4");
    let control = control(&engine, reference);
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    std::fs::copy(root().join("h264.mp4"), first.path().join("clip.mp4")).unwrap();
    std::fs::copy(root().join("h264-keyframes.mp4"), second.path().join("clip.mp4")).unwrap();
    let source = Source::capture(&engine).unwrap();
    let mut probe = Probe::default();
    probe.reconcile(&source, first.path()).unwrap();
    probe.run_due(&source, || false).unwrap();
    edit(
        &mut engine,
        MediaEdit::Video {
            media: reference.media,
            edit: protocol::VideoEdit::Seek { position_ms: 400.0 },
        },
    );
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(250));
    probe.reconcile(&source, second.path()).unwrap();
    probe.run_due(&source, || false).unwrap();
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(400));
    std::fs::remove_file(second.path().join("clip.mp4")).unwrap();
    std::thread::sleep(Duration::from_millis(1050));
    probe.run_due(&source, || false).unwrap();
    assert_eq!(
        control.sample().unwrap().position,
        Duration::from_millis(400),
        "temporary endpoint loss preserves known timing"
    );
    file(&mut engine, reference, "replacement.mp4");
    probe.reconcile(&source, second.path()).unwrap();
    probe.run_due(&source, || false).unwrap();
    assert!(
        !control.snapshot(reference.media).unwrap().ready,
        "a different authored File cannot inherit old timing"
    );
    std::fs::copy(root().join("h264.mp4"), second.path().join("replacement.mp4")).unwrap();
    std::thread::sleep(Duration::from_millis(1050));
    probe.run_due(&source, || false).unwrap();
    assert!(control.snapshot(reference.media).unwrap().ready);
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(250));
}

#[test]
fn moving_a_real_channel_changes_its_metadata_owner_and_a_project_capture_cannot_survive_replacement() {
    use golden_core::edit::Edit;
    let mut engine: AppEngine = golden_core::app::create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let first = create(&mut engine);
    let other = create(&mut engine);
    let MediaEditResult::Created { reference: second } = edit(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    file(&mut engine, first, "h264.mp4");
    file(&mut engine, second, "h264-keyframes.mp4");
    probe(&engine);
    let original = control(&engine, first);
    let destination = control(&engine, other);
    engine.edits.push(Edit::MoveNode {
        node: engine.node_id_by_uuid(second.channel.0).unwrap(),
        new_parent: engine.node_id_by_uuid(other.media.0).unwrap(),
        new_prev_sibling: None,
    });
    engine.settle_edit_notifications(16).unwrap();
    probe(&engine);
    assert_eq!(original.metadata_files().unwrap().len(), 1);
    assert_eq!(destination.metadata_files().unwrap().len(), 1);
    edit(
        &mut engine,
        MediaEdit::Video {
            media: other.media,
            edit: protocol::VideoEdit::Seek { position_ms: 400.0 },
        },
    );
    assert_eq!(destination.sample().unwrap().position, Duration::from_millis(400));
    let source = Source::capture(&engine).unwrap();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    engine.edits.push(Edit::RemoveNode { node: manager });
    engine.settle_edit_notifications(16).unwrap();
    assert!(!source.guard.is_current());
}
