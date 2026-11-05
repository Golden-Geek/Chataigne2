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
        matrix_edit(&mut engine, matrix, MatrixEdit::AddColumns { parent: None, count: 5 })
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
        matrix_edit(&mut engine, matrix, MatrixEdit::AddColumns { parent: None, count: 1 })
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
        matrix_edit(&mut engine, matrix, MatrixEdit::AddColumns { parent: None, count: 3 })
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

#[test]
fn nested_group_applications_survive_same_group_switch_and_deleted_outgoing_children_until_settlement() {
    let mut engine = engine();
    let matrix = create(&mut engine, MediaKind::Matrix);
    matrix_edit(&mut engine, matrix, MatrixEdit::AddLayers { parent: None, count: 1 });
    let mut group = |parent| {
        let MediaEditResult::MatrixColumnGroupCreated { group } =
            matrix_edit(&mut engine, matrix, MatrixEdit::AddColumnGroup { parent, label: None })
        else {
            panic!()
        };
        group
    };
    let outer = group(None);
    let inner = group(Some(outer));
    let other = group(None);
    let mut columns = |parent, count| {
        let MediaEditResult::MatrixCreated { columns, .. } = matrix_edit(
            &mut engine,
            matrix,
            MatrixEdit::AddColumns {
                parent: Some(parent),
                count,
            },
        ) else {
            panic!()
        };
        columns
    };
    let children = columns(inner, 2);
    let destination = columns(other, 1)[0];
    let apps = [outer, inner, other].map(|group| {
        let application = ApplicationId(group.0);
        edit(
            &mut engine,
            MediaEdit::Applications {
                edit: ApplicationEdit::Configure {
                    application,
                    definition: ApplicationDefinition {
                        executable: Some("C:\\missing\\not-started.exe".into()),
                        ..Default::default()
                    },
                },
            },
        );
        application
    });
    let owner = renderer();
    let anchor = owner.read_time().unwrap();
    let at = |millis| {
        anchor
            .at_monotonic(anchor.monotonic().checked_add(Duration::from_millis(millis)).unwrap())
            .unwrap()
    };
    let launch =
        |engine: &mut AppEngine, column, mode| matrix_edit(engine, matrix, MatrixEdit::LaunchColumn { column, mode });
    launch(&mut engine, children[0], LaunchMode::Cut);
    let initial = crate::media::capture_channels_at(&engine, &[matrix], 8, at(0)).unwrap();
    assert_eq!(
        initial
            .plan_applications()
            .into_iter()
            .collect::<std::collections::HashSet<_>>(),
        apps[..2].iter().copied().collect()
    );
    launch(&mut engine, children[1], LaunchMode::Configured);
    let switched = crate::media::capture_channels_at(&engine, &[matrix], 8, at(100)).unwrap();
    assert_eq!(switched.application_demand().len(), 2);
    assert!(
        switched
            .application_demand()
            .iter()
            .all(|d| d.reasons.contains(&DemandReason::Active))
    );
    launch(&mut engine, destination, LaunchMode::Configured);
    let outgoing = crate::media::capture_channels_at(&engine, &[matrix], 8, at(200)).unwrap();
    for app in &apps[..2] {
        assert!(
            outgoing
                .application_demand()
                .iter()
                .find(|d| d.application == *app)
                .unwrap()
                .reasons
                .contains(&DemandReason::Outgoing)
        );
    }
    matrix_edit(&mut engine, matrix, MatrixEdit::RemoveColumnGroup { group: outer });
    let deleted = crate::media::capture_channels_at(&engine, &[matrix], 8, at(250)).unwrap();
    assert_eq!(deleted.plan_applications().len(), 3);
    for app in &apps[..2] {
        assert_eq!(
            deleted
                .application_demand()
                .iter()
                .find(|d| d.application == *app)
                .unwrap()
                .availability,
            crate::applications::demand::DefinitionAvailability::Missing
        );
    }
    let mut settled = deleted.clone();
    settled.settle_transitions(at(1000)).unwrap();
    assert_eq!(settled.plan_applications(), vec![apps[2]]);
    assert_eq!(
        crate::media::capture_channels_at(&engine, &[matrix], 8, at(1000))
            .unwrap()
            .plan_applications(),
        vec![apps[2]]
    );
    assert_eq!(
        deleted.plan_applications().len(),
        3,
        "the already captured outgoing plan keeps its final-use references"
    );
    close(owner);
}
