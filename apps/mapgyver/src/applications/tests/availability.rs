use super::*;
use golden_core::{
    edit::Edit,
    node::{Node, NodeMetaPatch},
};

#[test]
fn availability_is_one_undoable_edit_and_disabled_copies_and_projects_preserve_the_local_flag() {
    let mut engine = fixture();
    let application = create(&mut engine);
    let history = engine.undo_len();
    app_edit(
        &mut engine,
        ApplicationEdit::SetEnabled {
            application,
            enabled: true,
        },
    );
    assert_eq!(engine.undo_len(), history);
    app_edit(
        &mut engine,
        ApplicationEdit::SetEnabled {
            application,
            enabled: false,
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(
        super::super::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::Disabled
    );
    assert!(engine.undo().unwrap());
    assert_eq!(
        super::super::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::Enabled
    );
    assert!(engine.redo().unwrap());
    let MediaEditResult::ApplicationCreated { application: copy } =
        app_edit(&mut engine, ApplicationEdit::Duplicate { application })
    else {
        panic!()
    };
    assert_ne!(copy, application);
    assert_eq!(
        super::super::inspect(&engine, copy).unwrap().enablement,
        ApplicationEnablement::Disabled
    );
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    let restored = from_sparse_project_json::<AppNode>(&document).unwrap();
    for id in [application, copy] {
        assert_eq!(
            super::super::inspect(&restored, id).unwrap().enablement,
            ApplicationEnablement::Disabled
        );
    }
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Applications {
                edit: ApplicationEdit::SetEnabled {
                    application: ApplicationId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
                    enabled: false,
                }
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn local_and_inherited_availability_fence_prepared_admission_without_process_or_history_side_effects() {
    let mut engine = fixture();
    let application = create(&mut engine);
    app_edit(
        &mut engine,
        ApplicationEdit::Configure {
            application,
            definition: ApplicationDefinition {
                executable: Some(std::env::current_exe().unwrap().to_string_lossy().into()),
                ..Default::default()
            },
        },
    );
    let (media, columns) = matrix(&mut engine);
    configure_column(&mut engine, media, columns[2], application);
    edit(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::LaunchColumn {
                column: columns[2],
                mode: LaunchMode::Cut,
            },
        },
    );
    let mut driver = crate::applications::driver::ApplicationDriver::default();
    let prepared = driver.capture_service(&engine).unwrap();
    app_edit(
        &mut engine,
        ApplicationEdit::SetEnabled {
            application,
            enabled: false,
        },
    );
    assert!(
        driver
            .apply_prepared(prepared, golden_io::WorkerNotifier::new(|| {}))
            .is_err()
    );
    assert!(driver.snapshot().is_none());
    app_edit(
        &mut engine,
        ApplicationEdit::SetEnabled {
            application,
            enabled: true,
        },
    );
    let prepared = driver.capture_service(&engine).unwrap();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: manager,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert_eq!(
        super::super::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::DisabledByParent
    );
    assert!(
        driver
            .apply_prepared(prepared, golden_io::WorkerNotifier::new(|| {}))
            .is_err()
    );
    let history = engine.undo_len();
    app_edit(
        &mut engine,
        ApplicationEdit::SetEnabled {
            application,
            enabled: true,
        },
    );
    assert_eq!(engine.undo_len(), history);
    app_edit(
        &mut engine,
        ApplicationEdit::SetEnabled {
            application,
            enabled: false,
        },
    );
    assert_eq!(
        super::super::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::Disabled
    );
    app_edit(
        &mut engine,
        ApplicationEdit::SetEnabled {
            application,
            enabled: true,
        },
    );
    assert_eq!(
        super::super::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::DisabledByParent
    );
    assert!(!engine.nodes.get(manager).unwrap().node_data().effective_enabled);
    assert!(driver.snapshot().is_none());
}
