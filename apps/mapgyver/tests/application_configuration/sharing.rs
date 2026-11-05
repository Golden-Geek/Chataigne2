use super::*;
use golden_core::node::Node;
use mapgyver::{applications::driver::ApplicationDriver, matrix::protocol::*, media::protocol::MediaKind};

fn matrix(engine: &mut AppEngine, media: mapgyver::media::protocol::MediaId, edit: MatrixEdit) -> MediaEditResult {
    media_edit(engine, MediaEdit::Matrix { media, edit })
}
fn service(driver: &mut ApplicationDriver, engine: &AppEngine) {
    driver.service(engine, golden_io::WorkerNotifier::new(|| {})).unwrap();
}
fn settled(
    driver: &ApplicationDriver,
    check: impl Fn(&ApplicationRuntimeSnapshot) -> bool,
) -> ApplicationRuntimeSnapshot {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let snapshot = driver.snapshot().unwrap();
        if check(&snapshot) {
            return snapshot;
        }
        assert!(Instant::now() < deadline, "{snapshot:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn columns_and_cells_in_two_matrices_share_one_process_and_only_effective_key_changes_split_it() {
    let (mut engine, first) = fixture(ApplicationReadiness::ProcessStarted);
    let mut definition = mapgyver::applications::inspect(&engine, first).unwrap().definition;
    definition.environment = Some(vec![("B".into(), "two".into()), ("A".into(), "one".into())]);
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application: first,
            definition: definition.clone(),
        },
    );
    let first_node = engine.nodes.get(engine.node_id_by_uuid(first.0).unwrap()).unwrap();
    let root = engine.nodes.get(first_node.node_data().parent.unwrap()).unwrap();
    let owner = engine.nodes.get(root.node_data().parent.unwrap()).unwrap();
    let first_media = mapgyver::media::protocol::MediaId(owner.node_data().meta.uuid);
    let MediaEditResult::Created { reference } = media_edit(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let second_media = reference.media;
    let MediaEditResult::MatrixCreated { columns, .. } = matrix(
        &mut engine,
        second_media,
        MatrixEdit::AddColumns { parent: None, count: 3 },
    ) else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { items, .. } = matrix(
        &mut engine,
        second_media,
        MatrixEdit::AddLayers { parent: None, count: 1 },
    ) else {
        panic!()
    };
    let MatrixItemId::Layer { id: layer } = items[0] else {
        panic!()
    };
    let mut equivalent = definition.clone();
    equivalent.environment.as_mut().unwrap().reverse();
    let executable = std::path::PathBuf::from(equivalent.executable.as_ref().unwrap());
    equivalent.executable = Some(
        executable
            .parent()
            .unwrap()
            .join(".")
            .join(executable.file_name().unwrap())
            .to_string_lossy()
            .replace('\\', "/"),
    );
    matrix(
        &mut engine,
        second_media,
        MatrixEdit::ConfigureColumnApplication {
            column: columns[1],
            definition: equivalent,
            pre: 1,
            post: 0,
        },
    );
    let second = ApplicationId(columns[1].0);
    let MediaEditResult::MatrixCellPrepared { application: cell, .. } = matrix(
        &mut engine,
        second_media,
        MatrixEdit::ConfigureCellApplication {
            layer,
            column: columns[1],
            definition: definition.clone(),
            pre: 1,
            post: 0,
        },
    ) else {
        panic!()
    };
    matrix(
        &mut engine,
        first_media,
        MatrixEdit::LaunchColumn {
            column: ColumnId(first.0),
            mode: LaunchMode::Cut,
        },
    );
    let mut driver = ApplicationDriver::default();
    service(&mut driver, &engine);
    let initial = settled(&driver, |s| {
        s.applications.len() == 1 && s.applications[0].phase == ApplicationPhase::Ready
    });
    let original = initial.applications[0].clone();
    let retirement = driver.retirement().unwrap();
    let history = engine.undo_len();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    matrix(
        &mut engine,
        second_media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    service(&mut driver, &engine);
    let shared = settled(&driver, |s| {
        s.applications.len() == 1 && s.applications[0].consumers.len() == 3
    });
    assert_eq!(shared.applications[0].runtime, original.runtime);
    assert_eq!(shared.applications[0].process_id, original.process_id);
    assert_eq!(shared.applications[0].launch, original.launch);
    assert_eq!(
        shared.applications[0]
            .consumers
            .iter()
            .map(|c| c.application)
            .collect::<std::collections::HashSet<_>>(),
        [first, second, cell].into()
    );
    matrix(&mut engine, first_media, MatrixEdit::ClearPlayback);
    service(&mut driver, &engine);
    matrix(
        &mut engine,
        second_media,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Cut,
        },
    );
    service(&mut driver, &engine);
    let handed_off = settled(&driver, |s| {
        s.applications.len() == 1 && s.applications[0].consumers.len() == 2
    });
    assert_eq!(handed_off.applications[0].process_id, original.process_id);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);

    let mut changed = definition.clone();
    changed.arguments.push("independent".into());
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application: cell,
            definition: changed,
        },
    );
    service(&mut driver, &engine);
    let split = settled(&driver, |s| {
        s.applications
            .iter()
            .filter(|r| r.phase == ApplicationPhase::Ready && r.process_id.is_some())
            .count()
            == 2
    });
    let retained = split
        .applications
        .iter()
        .find(|r| r.runtime == original.runtime)
        .unwrap();
    assert_eq!(retained.process_id, original.process_id);
    assert_eq!(
        retained.consumers.iter().map(|c| c.application).collect::<Vec<_>>(),
        vec![second]
    );
    let other = split
        .applications
        .iter()
        .find(|r| r.runtime != original.runtime)
        .unwrap();
    assert_ne!(other.process_id, original.process_id);
    let other_id = other.runtime;
    edit(
        &mut engine,
        ApplicationEdit::Configure {
            application: cell,
            definition,
        },
    );
    service(&mut driver, &engine);
    let reunited = settled(&driver, |s| {
        s.applications
            .iter()
            .any(|r| r.runtime == original.runtime && r.consumers.len() == 2)
            && s.applications
                .iter()
                .any(|r| r.runtime == other_id && r.phase == ApplicationPhase::Idle && r.process_id.is_none())
    });
    assert_eq!(
        reunited
            .applications
            .iter()
            .find(|r| r.runtime == original.runtime)
            .unwrap()
            .process_id,
        original.process_id
    );
    assert_eq!(engine.undo_len(), history + 2);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    matrix(&mut engine, second_media, MatrixEdit::ClearPlayback);
    service(&mut driver, &engine);
    settled(&driver, |s| {
        s.applications
            .iter()
            .all(|r| r.phase == ApplicationPhase::Idle && r.process_id.is_none())
    });
    drop(driver);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
}
