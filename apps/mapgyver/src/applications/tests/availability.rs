use super::*;
#[test]
fn owner_enablement_fences_admission_and_copies_and_projects_preserve_local_state() {
    let mut engine = fixture();
    let application = create(&mut engine);
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: ApplicationDefinition {
                executable: Some("C:\\mapgyver-fixture-missing\\never.exe".into()),
                ..Default::default()
            },
        },
    );
    let (media, column) = owner(&engine, application);
    edit(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::LaunchColumn {
                column,
                mode: LaunchMode::Cut,
            },
        },
    );
    let mut driver = crate::applications::driver::ApplicationDriver::default();
    let prepared = driver.capture_service(&engine).unwrap();
    let history = engine.undo_len();
    enable(&mut engine, application, false);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(
        driver
            .apply_prepared(prepared, golden_io::WorkerNotifier::new(|| {}))
            .is_err()
    );
    assert!(driver.snapshot().is_none());
    assert_eq!(
        super::super::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::DisabledByParent
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(
        super::super::inspect(&loaded, application).unwrap().enablement,
        ApplicationEnablement::DisabledByParent
    );
    assert!(engine.undo().unwrap());
    assert_eq!(
        super::super::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::Enabled
    );
    let prepared = driver.capture_service(&engine).unwrap();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    engine.edits.push(golden_core::edit::Edit::PatchMeta {
        node: manager,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(
        driver
            .apply_prepared(prepared, golden_io::WorkerNotifier::new(|| {}))
            .is_err()
    );
    assert!(driver.snapshot().is_none());
}
