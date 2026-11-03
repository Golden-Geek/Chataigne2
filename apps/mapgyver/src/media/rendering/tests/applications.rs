use super::*;
use crate::{
    applications::{demand::DemandReason, protocol::*},
    matrix::protocol::*,
};

fn configure(engine: &mut AppEngine, matrix: ChannelReference, column: ColumnId, pre: u16, post: u16) -> ApplicationId {
    matrix_edit(
        engine,
        matrix,
        MatrixEdit::ConfigureColumnApplication {
            column,
            definition: ApplicationDefinition {
                executable: Some("C:\\mapgyver-fixture-missing\\never.exe".into()),
                ..Default::default()
            },
            pre,
            post,
        },
    );
    ApplicationId(column.0)
}
fn matrix_edit(engine: &mut AppEngine, reference: ChannelReference, operation: MatrixEdit) -> MediaEditResult {
    edit(
        engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit: operation,
        },
    )
}

#[test]
fn committed_column_capture_resolves_neighbors_and_keeps_active_bindings_through_removal() {
    let mut engine = engine();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { columns, .. } =
        matrix_edit(&mut engine, matrix, MatrixEdit::AddColumns { count: 5 })
    else {
        panic!()
    };
    let application = configure(&mut engine, matrix, columns[2], 1, 1);
    for (index, expected) in [(0, false), (1, true), (4, false), (2, true)] {
        matrix_edit(
            &mut engine,
            matrix,
            MatrixEdit::LaunchColumn {
                column: columns[index],
                mode: LaunchMode::Cut,
            },
        );
        let capture = capture_channels(&engine, &[matrix], 8).unwrap();
        assert_eq!(!capture.application_demand().is_empty(), expected);
        if index == 1 {
            assert_eq!(
                capture.application_demand()[0].availability,
                crate::applications::demand::DefinitionAvailability::Configured
            );
            assert!(
                capture.plan_applications().is_empty(),
                "neighbor preload is not displayed/final-use demand"
            );
            assert_eq!(
                capture.application_demand()[0].reasons,
                [DemandReason::Neighbor].into_iter().collect()
            );
        }
        if index == 2 {
            assert_eq!(capture.plan_applications(), vec![application]);
        }
    }
    matrix_edit(&mut engine, matrix, MatrixEdit::RemoveColumn { column: columns[2] });
    let retained = capture_channels(&engine, &[matrix], 8).unwrap();
    assert_eq!(retained.plan_applications(), vec![application]);
    assert!(retained.application_demand()[0].reasons.contains(&DemandReason::Active));
    assert!(retained.is_current());
    let missing = capture_channels(&engine, &[matrix], 8).unwrap();
    assert_eq!(missing.plan_applications(), vec![application]);
    matrix_edit(
        &mut engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    assert!(
        capture_channels(&engine, &[matrix], 8)
            .unwrap()
            .application_demand()
            .is_empty()
    );
}

#[test]
fn rendering_app_bound_columns_requires_the_matching_admitted_lifecycle_owner() {
    let mut engine = engine();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { columns, .. } =
        matrix_edit(&mut engine, matrix, MatrixEdit::AddColumns { count: 1 })
    else {
        panic!()
    };
    configure(&mut engine, matrix, columns[0], 0, 0);
    matrix_edit(
        &mut engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let capture = capture_channels(&engine, &[matrix], 8).unwrap();
    let mut renderer = renderer();
    assert!(
        renderer
            .publish(
                capture.clone(),
                vec![ChannelRoute {
                    output: OutputId::new(1).unwrap(),
                    channel: matrix
                }]
            )
            .is_err()
    );
    let mut applications = crate::applications::runtime::ApplicationRuntime::start(&engine, None).unwrap();
    applications.reconcile(&engine, &capture.application_demand()).unwrap();
    renderer.bind_applications(applications.plan_retainer());
    let request = renderer
        .publish(
            capture,
            vec![ChannelRoute {
                output: OutputId::new(1).unwrap(),
                channel: matrix,
            }],
        )
        .unwrap();
    installed(&renderer, request);
    let retirement = applications.retirement();
    drop(applications);
    close(renderer);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
}

#[test]
fn interrupted_application_fades_keep_removed_outgoing_references_until_transition_settlement() {
    let mut engine = engine();
    let matrix = create(&mut engine, MediaKind::Matrix);
    matrix_edit(&mut engine, matrix, MatrixEdit::AddLayers { parent: None, count: 1 });
    let MediaEditResult::MatrixCreated { columns, .. } =
        matrix_edit(&mut engine, matrix, MatrixEdit::AddColumns { count: 3 })
    else {
        panic!()
    };
    let apps = columns
        .iter()
        .map(|column| configure(&mut engine, matrix, *column, 0, 0))
        .collect::<Vec<_>>();
    let owner = renderer();
    let anchor = owner.read_time().unwrap();
    let at = |millis| {
        anchor
            .at_monotonic(anchor.monotonic().checked_add(Duration::from_millis(millis)).unwrap())
            .unwrap()
    };
    matrix_edit(
        &mut engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    crate::media::capture_channels_at(&engine, &[matrix], 8, at(0)).unwrap();
    matrix_edit(
        &mut engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    let initial = crate::media::capture_channels_at(&engine, &[matrix], 8, at(100)).unwrap();
    assert!(
        initial
            .application_demand()
            .iter()
            .find(|entry| entry.application == apps[0])
            .unwrap()
            .reasons
            .contains(&DemandReason::Outgoing)
    );
    matrix_edit(
        &mut engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column: columns[2],
            mode: LaunchMode::Configured,
        },
    );
    crate::media::capture_channels_at(&engine, &[matrix], 8, at(200)).unwrap();
    matrix_edit(&mut engine, matrix, MatrixEdit::RemoveColumn { column: columns[0] });
    let interrupted = crate::media::capture_channels_at(&engine, &[matrix], 8, at(250)).unwrap();
    assert_eq!(interrupted.plan_applications().len(), 3);
    for application in &apps[..2] {
        assert!(
            interrupted
                .application_demand()
                .iter()
                .find(|entry| entry.application == *application)
                .unwrap()
                .reasons
                .contains(&DemandReason::Outgoing)
        );
    }
    let settled = crate::media::capture_channels_at(&engine, &[matrix], 8, at(500)).unwrap();
    assert_eq!(settled.plan_applications(), vec![apps[2]]);
    assert_eq!(settled.application_demand().len(), 1);
    close(owner);
}
