use super::*;
use crate::{
    applications::{demand::DemandReason, protocol::*},
    matrix::protocol::*,
};

fn app(engine: &mut AppEngine) -> ApplicationId {
    let MediaEditResult::ApplicationCreated { application } = edit(
        engine,
        MediaEdit::Applications {
            edit: ApplicationEdit::Create { label: None },
        },
    ) else {
        panic!()
    };
    application
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
    let application = app(&mut engine);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { columns, .. } =
        matrix_edit(&mut engine, matrix, MatrixEdit::AddColumns { count: 5 })
    else {
        panic!()
    };
    matrix_edit(
        &mut engine,
        matrix,
        MatrixEdit::ConfigureColumn {
            column: columns[2],
            definition: ColumnDefinition {
                applications: vec![ColumnApplication {
                    application,
                    pre: 1,
                    post: 1,
                }],
                ..Default::default()
            },
        },
    );
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
                crate::applications::demand::DefinitionAvailability::Unconfigured
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
    edit(
        &mut engine,
        MediaEdit::Applications {
            edit: ApplicationEdit::Remove { application },
        },
    );
    assert!(
        !retained.is_current(),
        "definition removal fences pending capture before OS resume"
    );
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
    let application = app(&mut engine);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { columns, .. } =
        matrix_edit(&mut engine, matrix, MatrixEdit::AddColumns { count: 1 })
    else {
        panic!()
    };
    matrix_edit(
        &mut engine,
        matrix,
        MatrixEdit::ConfigureColumn {
            column: columns[0],
            definition: ColumnDefinition {
                applications: vec![ColumnApplication {
                    application,
                    pre: 0,
                    post: 0,
                }],
                ..Default::default()
            },
        },
    );
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
    let apps = [app(&mut engine), app(&mut engine), app(&mut engine)];
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { columns, .. } =
        matrix_edit(&mut engine, matrix, MatrixEdit::AddColumns { count: 3 })
    else {
        panic!()
    };
    for (column, application) in columns.iter().zip(apps) {
        matrix_edit(
            &mut engine,
            matrix,
            MatrixEdit::ConfigureColumn {
                column: *column,
                definition: ColumnDefinition {
                    applications: vec![ColumnApplication {
                        application,
                        pre: 0,
                        post: 0,
                    }],
                    ..Default::default()
                },
            },
        );
    }
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
    edit(
        &mut engine,
        MediaEdit::Applications {
            edit: ApplicationEdit::Remove { application: apps[0] },
        },
    );
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
