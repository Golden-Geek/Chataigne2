use super::*;
use golden_runtime::ControlErrorKind;
use std::sync::mpsc;

crate::define_node_enum!(
    pub(super) enum ControlTestNode {}
);
impl ProjectLifecycle for ControlTestNode {}

#[test]
fn committed_save_reports_publication_stage_and_allows_a_later_save() {
    use golden_runtime::PendingControl;
    let runtime = ProductionRuntime::<ControlTestNode>::new(
        Engine::new(crate::node::Folder::new("root").into()),
        UiProjectFileSpec::default(),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("committed.noisette");
    let release = Arc::new(Mutex::new(None::<mpsc::Sender<()>>));
    struct Release(Arc<Mutex<Option<mpsc::Sender<()>>>>);
    impl Drop for Release {
        fn drop(&mut self) {
            if let Some(sender) = self.0.lock().unwrap().take() {
                let _ = sender.send(());
            }
        }
    }
    let release_guard = Release(release.clone());
    let queued = Arc::new(Mutex::new(Vec::<PendingControl<()>>::new()));
    let pending = queued.clone();
    let handle = runtime.inner.control.handle();
    runtime.set_project_save_fault_hook(Some(Arc::new(move |stage, _| {
        if stage != ProjectSaveStage::BeforeMetadataPublication {
            return;
        }
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *release.lock().unwrap() = Some(release_tx);
        let active = handle
            .submit(move |_| {
                entered_tx.send(()).unwrap();
                let _ = release_rx.recv();
            })
            .unwrap();
        entered_rx.recv().unwrap();
        let mut pending = pending.lock().unwrap();
        pending.push(active);
        while let Ok(operation) = handle.submit(|_| ()) {
            pending.push(operation);
        }
    })));
    let request = ProjectSaveRequest {
        path: target.to_string_lossy().into_owned(),
        ui_state: None,
    };
    let error = runtime.save_project(request.clone()).unwrap_err();
    drop(release_guard);
    for operation in queued.lock().unwrap().drain(..) {
        operation.wait().unwrap();
    }
    runtime.set_project_save_fault_hook(None);
    assert!(target.exists(), "file commit precedes the rejected publication");
    assert_eq!(error.control_error().unwrap().kind(), ControlErrorKind::Overloaded);
    let status = runtime.project_persistence_status().unwrap();
    assert!(status.dirty);
    assert!(status.current_path.is_none());
    assert!(
        error.to_string().contains("committed"),
        "failure must disclose a committed file: {error}"
    );
    let ApplicationError::SavePublication { committed, .. } = &error else {
        panic!("typed save-publication failure")
    };
    assert_eq!(
        std::path::Path::new(&committed.path).canonicalize().unwrap(),
        target.canonicalize().unwrap()
    );
    assert_eq!(committed.project_generation, runtime.current_project_generation());
    assert_eq!(committed.document_revision, status.document_revision);
    assert!(std::error::Error::source(&error).is_some());
    let saved = runtime.save_project(request).unwrap();
    assert!(saved.request_id > committed.request_id);
    assert!(saved.metadata_applied);
    assert!(!saved.dirty);
}

#[test]
fn saturated_public_transaction_returns_an_error_without_mutating_or_panicking() {
    let runtime = ProductionRuntime::<ControlTestNode>::new(
        Engine::new(crate::node::Folder::new("root").into()),
        UiProjectFileSpec::default(),
    )
    .unwrap();
    let before = runtime.read_model().snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let handle = runtime.inner.control.handle();
    let active = handle
        .submit(move |_| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        })
        .unwrap();
    entered_rx.recv().unwrap();
    let mut queued = Vec::new();
    for _ in 0..golden_runtime::ControlActorConfig::default().pending_capacity.get() {
        queued.push(handle.submit(|_| ()).unwrap());
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.apply_ui_transaction(UiEditIntent::Undo, None)
    }));
    release_tx.send(()).unwrap();
    active.wait().unwrap();
    for pending in queued {
        pending.wait().unwrap();
    }
    assert!(result.is_ok(), "public actor saturation must not panic");
    let error = result.unwrap().unwrap_err();
    assert_eq!(error.control_error().unwrap().kind(), ControlErrorKind::Overloaded);
    let after = runtime.read_model().snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    assert_eq!(
        serde_json::to_value(after).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    let next = runtime
        .apply_ui_transaction(UiEditIntent::ReevaluateGraph, None)
        .unwrap();
    assert!(
        next.acknowledgement.success,
        "actor must continue after rejected admission"
    );
}

#[test]
fn cancelled_public_work_is_distinct_from_actor_disconnection() {
    let runtime = ProductionRuntime::<ControlTestNode>::new(
        Engine::new(crate::node::Folder::new("root").into()),
        UiProjectFileSpec::default(),
    )
    .unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let active = runtime
        .inner
        .control
        .handle()
        .submit(move |_| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            panic!("injected actor loss");
        })
        .unwrap();
    entered_rx.recv().unwrap();
    let waiting = runtime.clone();
    let queued = std::thread::spawn(move || waiting.apply_ui_transaction(UiEditIntent::Undo, None));
    let deadline = Instant::now() + Duration::from_secs(2);
    while runtime.runtime_metrics().control_queue_depth == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    let admitted = runtime.runtime_metrics().control_queue_depth > 0;
    release_tx.send(()).unwrap();
    assert!(admitted, "public operation must be admitted before actor loss");
    assert_eq!(active.wait().unwrap_err().kind(), ControlErrorKind::Disconnected);
    let cancelled = queued.join().unwrap().unwrap_err();
    assert_eq!(cancelled.control_error().unwrap().kind(), ControlErrorKind::Cancelled);
    let unavailable = runtime.history_state().unwrap_err();
    assert_eq!(
        unavailable.control_error().unwrap().kind(),
        ControlErrorKind::Disconnected
    );
}
