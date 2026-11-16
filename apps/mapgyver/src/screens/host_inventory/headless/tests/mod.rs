use super::*;
use crate::{
    app::{AppEngine, AppNode},
    screens::{ScreenManagerNode, protocol::*},
};
use golden_core::{
    app::{ProjectLifecycle, from_sparse_project_json, prepare_engine_for_runtime, to_sparse_project_json_pretty},
    application::{ProductionRuntime, ProjectReplacement, ProjectTransactions},
    ui_sync::{UiEditIntent, UiProjectFileSpec},
};

fn panel(id: &str) -> DisplayInfo {
    DisplayInfo {
        id: id.into(),
        name: id.into(),
        edid: None,
        index: 1,
        primary: true,
        internal: Some(false),
        refresh_rate: None,
        bounds: PhysicalRectangle {
            left: 0,
            top: 0,
            extent: golden_video::Extent2D::new(1920, 1080).unwrap(),
        },
    }
}

fn manager(engine: &AppEngine) -> &ScreenManagerNode {
    let id = crate::screens::authoring::manager(engine).unwrap();
    let Some(AppNode::ScreenManager(manager)) = engine.nodes.get(id) else {
        panic!()
    };
    manager
}

fn runtime() -> ProductionRuntime<AppNode> {
    let mut engine = crate::screens::tests::fixture();
    prepare_engine_for_runtime(&mut engine).unwrap();
    ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap()
}

fn wait_catalog(runtime: &ProductionRuntime<AppNode>, expected: &[DisplayInfo]) {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        runtime.run_tick(Duration::from_millis(1)).unwrap();
        let actual = runtime.capture_app(|engine| manager(engine).displays()).unwrap();
        if actual.as_ref() == expected {
            return;
        }
        assert!(Instant::now() < deadline, "catalog did not settle: {actual:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn document(runtime: &ProductionRuntime<AppNode>) -> String {
    runtime.capture_app(to_sparse_project_json_pretty).unwrap().unwrap()
}

fn create(runtime: &ProductionRuntime<AppNode>) {
    let transaction = runtime
        .apply_transaction(UiEditIntent::InvokeAppOperation {
            namespace: SCREEN_NAMESPACE.into(),
            operation: SCREEN_OPERATION.into(),
            payload: serde_json::to_value(ScreenEdit::CreateScreen { label: None }).unwrap(),
        })
        .unwrap();
    assert!(transaction.success, "{transaction:?}");
}

#[test]
fn headless_catalog_loss_recovery_and_replacement_keep_authored_targets_and_history() {
    let runtime = runtime();
    let mut catalog = Catalog::default();
    let first = panel("external-a");
    let second = panel("external-b");
    catalog.update(&runtime, Ok(vec![first.clone()])).unwrap();
    wait_catalog(&runtime, std::slice::from_ref(&first));
    create(&runtime);
    let original = document(&runtime);
    let history = runtime.history_state().unwrap().current_history_state_id;
    assert_eq!(catalog.update(&runtime, Err("offline".into())), Err("offline".into()));
    wait_catalog(&runtime, &[]);
    assert_eq!(document(&runtime), original);
    assert_eq!(runtime.history_state().unwrap().current_history_state_id, history);
    create(&runtime);
    let before_recovery = document(&runtime);
    catalog
        .update(&runtime, Ok(vec![first.clone(), second.clone()]))
        .unwrap();
    wait_catalog(&runtime, &[first, second.clone()]);
    assert_eq!(document(&runtime), before_recovery);
    create(&runtime);
    let snapshot = runtime.capture_app(crate::screens::inspect).unwrap().unwrap();
    assert_eq!(
        snapshot
            .screens
            .iter()
            .map(|screen| screen.definition.target.clone())
            .collect::<Vec<_>>(),
        vec![
            Some(ScreenTarget::Fullscreen {
                display: "external-a".into()
            }),
            None,
            Some(ScreenTarget::Fullscreen {
                display: "external-b".into()
            }),
        ]
    );
    let retired = catalog.source.take().unwrap();
    let replacement = from_sparse_project_json::<AppNode>(&original).unwrap();
    runtime
        .replace_project(ProjectReplacement {
            engine: replacement,
            project_file: UiProjectFileSpec::default(),
            reason: "display catalog test".into(),
            recover: false,
        })
        .unwrap();
    assert!(retired.publisher.is_closed());
    catalog.source = Some(retired);
    let replacement_document = document(&runtime);
    catalog.update(&runtime, Ok(vec![second.clone()])).unwrap();
    wait_catalog(&runtime, &[second]);
    assert_eq!(document(&runtime), replacement_document);
}

#[test]
fn opened_candidate_prepares_missing_inventory_once_without_changing_saved_choices() {
    let mut fresh = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    AppNode::project_opened(&mut fresh).unwrap();
    prepare_engine_for_runtime(&mut fresh).unwrap();
    assert!(manager(&fresh).has_display_catalog());
    assert_eq!(fresh.undo_len(), 0);
    let runtime = runtime();
    create(&runtime);
    let expected = runtime.capture_app(crate::screens::inspect).unwrap().unwrap();
    let saved = document(&runtime);
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(crate::screens::inspect(&loaded).unwrap(), expected);
    let baseline = to_sparse_project_json_pretty(&loaded).unwrap();
    let history = loaded.ui_history_state();
    assert!(!manager(&loaded).has_display_catalog());
    AppNode::project_opened(&mut loaded).unwrap();
    assert!(manager(&loaded).has_display_catalog());
    assert_eq!(to_sparse_project_json_pretty(&loaded).unwrap(), baseline);
    assert_eq!(crate::screens::inspect(&loaded).unwrap(), expected);
    let prepared = manager(&loaded).displays();
    AppNode::project_opened(&mut loaded).unwrap();
    assert!(Arc::ptr_eq(&prepared, &manager(&loaded).displays()));
    assert_eq!(to_sparse_project_json_pretty(&loaded).unwrap(), baseline);
    assert_eq!(loaded.ui_history_state(), history);
    prepare_engine_for_runtime(&mut loaded).unwrap();
    assert_eq!(loaded.undo_len(), 0);
}
