use super::*;
use mapgyver::{
    applications::driver::ApplicationDriver,
    matrix::protocol::*,
    media::protocol::{MediaId, MediaKind},
};
use std::collections::HashSet;

fn matrix(engine: &mut AppEngine, media: MediaId, edit: MatrixEdit) -> MediaEditResult {
    media_edit(engine, MediaEdit::Matrix { media, edit })
}
fn group(engine: &mut AppEngine, media: MediaId, parent: Option<ColumnGroupId>) -> ColumnGroupId {
    let MediaEditResult::MatrixColumnGroupCreated { group } =
        matrix(engine, media, MatrixEdit::AddColumnGroup { parent, label: None })
    else {
        panic!()
    };
    group
}
fn columns(engine: &mut AppEngine, media: MediaId, parent: Option<ColumnGroupId>, count: u16) -> Vec<ColumnId> {
    let MediaEditResult::MatrixCreated { columns, .. } =
        matrix(engine, media, MatrixEdit::AddColumns { parent, count })
    else {
        panic!()
    };
    columns
}
fn service(driver: &mut ApplicationDriver, engine: &AppEngine) {
    driver.service(engine, golden_io::WorkerNotifier::new(|| {})).unwrap();
}
fn phase(driver: &ApplicationDriver, expected: ApplicationPhase) -> ApplicationRuntimeStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = driver.snapshot();
        if let Some(status) = snapshot.as_ref().and_then(|s| s.applications.first())
            && status.phase == expected
        {
            return status.clone();
        }
        assert!(Instant::now() < deadline, "expected {expected:?}, got {snapshot:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn launch(
    driver: &mut ApplicationDriver,
    engine: &mut AppEngine,
    media: MediaId,
    column: ColumnId,
) -> ApplicationRuntimeStatus {
    matrix(
        engine,
        media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    service(driver, engine);
    let expected: HashSet<_> = mapgyver::applications::logical::capture_logical_applications(engine)
        .unwrap()
        .demand
        .iter()
        .map(|d| d.application)
        .collect();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = phase(driver, ApplicationPhase::Ready);
        if status.consumers.iter().map(|c| c.application).collect::<HashSet<_>>() == expected {
            return status;
        }
        assert!(Instant::now() < deadline, "consumer handoff did not settle: {status:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn nested_groups_share_one_owned_process_across_switches_twenty_cycles_and_stopping_reentry() {
    let (mut engine, original) = fixture(ApplicationReadiness::ProcessStarted);
    let mut definition = mapgyver::applications::inspect(&engine, original).unwrap().definition;
    definition.stop.graceful_ms = 150;
    let MediaEditResult::Created { reference } = media_edit(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let media = reference.media;
    let outer = group(&mut engine, media, None);
    let inner = group(&mut engine, media, Some(outer));
    let peer = group(&mut engine, media, None);
    let children = columns(&mut engine, media, Some(inner), 2);
    let peer_column = columns(&mut engine, media, Some(peer), 1)[0];
    for group in [outer, inner, peer] {
        edit(
            &mut engine,
            ApplicationEdit::Configure {
                application: ApplicationId(group.0),
                definition: definition.clone(),
            },
        );
    }
    let history = engine.undo_len();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut driver = ApplicationDriver::default();
    let mut previous_launch = None;
    for _ in 0..20 {
        let initial = launch(&mut driver, &mut engine, media, children[0]);
        assert!(initial.process_id.is_some());
        assert_ne!(initial.launch, previous_launch);
        assert_eq!(
            initial.consumers.iter().map(|c| c.application).collect::<HashSet<_>>(),
            [ApplicationId(outer.0), ApplicationId(inner.0)].into()
        );
        let switched = launch(&mut driver, &mut engine, media, children[1]);
        assert_eq!(
            (switched.runtime, switched.launch, switched.process_id),
            (initial.runtime, initial.launch, initial.process_id)
        );
        let handed_off = launch(&mut driver, &mut engine, media, peer_column);
        assert_eq!(
            (handed_off.runtime, handed_off.launch, handed_off.process_id),
            (initial.runtime, initial.launch, initial.process_id)
        );
        assert_eq!(handed_off.consumers.len(), 1);
        assert_eq!(handed_off.consumers[0].application, ApplicationId(peer.0));
        assert_eq!(driver.snapshot().unwrap().applications.len(), 1);
        previous_launch = initial.launch;
        matrix(&mut engine, media, MatrixEdit::ClearPlayback);
        service(&mut driver, &engine);
        phase(&driver, ApplicationPhase::Idle);
    }
    for _ in 0..5 {
        let before = launch(&mut driver, &mut engine, media, children[0]);
        matrix(&mut engine, media, MatrixEdit::ClearPlayback);
        service(&mut driver, &engine);
        phase(&driver, ApplicationPhase::Stopping);
        let after = launch(&mut driver, &mut engine, media, children[1]);
        assert_ne!(after.launch, before.launch);
        assert_ne!(after.process_id, before.process_id);
        matrix(&mut engine, media, MatrixEdit::ClearPlayback);
        service(&mut driver, &engine);
        phase(&driver, ApplicationPhase::Idle);
    }
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let mut restored = from_sparse_project_json::<AppNode>(&saved).unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut restored).unwrap();
    let ready = launch(&mut driver, &mut restored, media, children[0]);
    assert_eq!(ready.consumers.len(), 2);
    matrix(&mut restored, media, MatrixEdit::RemoveColumnGroup { group: outer });
    service(&mut driver, &restored);
    phase(&driver, ApplicationPhase::Idle);
    assert!(restored.undo().unwrap());
    service(&mut driver, &restored);
    phase(&driver, ApplicationPhase::Ready);
    let retirement = driver.retirement().unwrap();
    drop(driver);
    assert!(retirement.wait_for_idle(Duration::from_secs(5)));
}
