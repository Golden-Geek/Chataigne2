use super::*;
fn id() -> ApplicationId {
    ApplicationId(golden_core::model::NodeUuid(uuid::Uuid::new_v4()))
}
fn columns(count: usize) -> Vec<DemandColumn> {
    (0..count)
        .map(|_| DemandColumn {
            id: ColumnId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
            bindings: Vec::new(),
        })
        .collect()
}
fn summary(id: ApplicationId) -> DefinitionState {
    DefinitionState {
        application: id,
        availability: DefinitionAvailability::Configured,
    }
}
fn cursor(column: ColumnId, reason: DemandReason) -> DemandCursor {
    DemandCursor {
        scope: uuid::Uuid::new_v4(),
        column,
        reason,
    }
}
fn demand(
    catalog: &[DefinitionState],
    columns: &[DemandColumn],
    cursors: &[DemandCursor],
    holds: &[DemandHold],
) -> Vec<ApplicationDemand> {
    resolve(
        catalog,
        &[MatrixDemand {
            columns,
            spans: &[],
            cursors,
            holds,
        }],
        DemandLimits::default(),
    )
    .unwrap()
}

#[test]
fn column_three_pre_and_post_one_clamp_without_wrap_and_direct_jumps_only_resolve_destination() {
    let app = id();
    let catalog = [summary(app)];
    let mut columns = columns(5);
    columns[2].bindings.push(DemandBinding {
        application: app,
        pre: 1,
        post: 1,
    });
    for (index, expected) in [(0, false), (1, true), (2, true), (3, true), (4, false)] {
        let result = demand(
            &catalog,
            &columns,
            &[cursor(columns[index].id, DemandReason::Active)],
            &[],
        );
        assert_eq!(!result.is_empty(), expected);
        if index == 1 {
            assert_eq!(result[0].reasons, [DemandReason::Neighbor].into_iter().collect());
        }
        if index == 2 {
            assert!(result[0].reasons.contains(&DemandReason::Active));
        }
    }
    let overlap = demand(
        &catalog,
        &columns,
        &[
            cursor(columns[1].id, DemandReason::Preparation),
            cursor(columns[2].id, DemandReason::Active),
        ],
        &[],
    );
    assert_eq!(
        overlap[0].reasons,
        [DemandReason::Neighbor, DemandReason::Active].into_iter().collect()
    );
    columns[2].bindings[0].pre = u16::MAX;
    assert_eq!(
        demand(
            &catalog,
            &columns,
            &[cursor(columns[0].id, DemandReason::Preparation)],
            &[]
        )
        .len(),
        1
    );
    columns.swap(0, 2);
    assert!(demand(&catalog, &columns, &[cursor(columns[4].id, DemandReason::Active)], &[]).is_empty());
    assert_eq!(
        demand(&catalog, &columns, &[cursor(columns[0].id, DemandReason::Active)], &[]).len(),
        1
    );
}

#[test]
fn shared_scopes_and_detached_final_use_holds_preserve_one_atomic_demand_through_delete_or_disable() {
    let app = id();
    let catalog = [summary(app)];
    let mut columns = columns(5);
    for column in &mut columns {
        column.bindings.push(DemandBinding {
            application: app,
            pre: 0,
            post: 0,
        });
    }
    let loading = cursor(columns[4].id, DemandReason::Preparation);
    let outgoing = cursor(columns[0].id, DemandReason::Outgoing);
    let result = demand(&catalog, &columns, &[loading, outgoing], &[]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].reasons.len(), 2);
    let retained = [DemandHold {
        application: app,
        reason: DemandReason::FinalUse,
    }];
    let before = demand(&catalog, &columns, &[outgoing], &[]);
    columns.remove(0);
    let after = demand(&catalog, &columns, &[loading, outgoing], &retained);
    assert_eq!(
        before[0].application, after[0].application,
        "the aggregate set retains the same application identity across reconciliation"
    );
    assert!(after[0].reasons.contains(&DemandReason::FinalUse));
    let missing = demand(&[], &columns, &[], &retained);
    assert_eq!(missing[0].availability, DefinitionAvailability::Missing);
    let mut disabled = summary(app);
    disabled.availability = DefinitionAvailability::Disabled;
    assert_eq!(
        demand(&[disabled], &columns, &[], &retained)[0].availability,
        DefinitionAvailability::Disabled
    );
    assert!(demand(&catalog, &columns, &[], &[]).is_empty());
}

#[test]
fn admitted_large_sparse_columns_and_invalid_input_bounds_do_not_materialize_cells() {
    let app = id();
    let catalog = [summary(app)];
    let mut columns = columns(10_000);
    columns[9999].bindings.push(DemandBinding {
        application: app,
        pre: 1,
        post: 0,
    });
    let active = [cursor(columns[9998].id, DemandReason::Active)];
    assert_eq!(demand(&catalog, &columns, &active, &[]).len(), 1);
    assert!(
        resolve(
            &catalog,
            &[MatrixDemand {
                columns: &columns,
                spans: &[],
                cursors: &active,
                holds: &[]
            }],
            DemandLimits {
                columns: 100,
                ..Default::default()
            }
        )
        .is_err()
    );
    let bad = [cursor(columns[0].id, DemandReason::Neighbor)];
    assert!(
        resolve(
            &catalog,
            &[MatrixDemand {
                columns: &columns,
                spans: &[],
                cursors: &bad,
                holds: &[]
            }],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn group_ranges_preserve_exact_reasons_and_use_flat_neighbor_distances_without_wrap() {
    let app = id();
    let columns = columns(8);
    let spans = [DemandSpan {
        start: 2,
        end: 5,
        binding: DemandBinding {
            application: app,
            pre: 1,
            post: 2,
        },
    }];
    for index in 0..8 {
        let cursors = [cursor(columns[index].id, DemandReason::Active)];
        let result = resolve(
            &[summary(app)],
            &[MatrixDemand {
                columns: &columns,
                spans: &spans,
                cursors: &cursors,
                holds: &[],
            }],
            Default::default(),
        )
        .unwrap();
        if (1..7).contains(&index) {
            let reason = if (2..5).contains(&index) {
                DemandReason::Active
            } else {
                DemandReason::Neighbor
            };
            assert_eq!(result[0].reasons, [reason].into_iter().collect(), "column {index}");
        } else {
            assert!(result.is_empty(), "column {index}");
        }
    }
    let cursors = [
        cursor(columns[1].id, DemandReason::Preparation),
        cursor(columns[2].id, DemandReason::Outgoing),
        cursor(columns[3].id, DemandReason::Active),
        cursor(columns[4].id, DemandReason::Preparation),
    ];
    let result = resolve(
        &[],
        &[MatrixDemand {
            columns: &columns,
            spans: &spans,
            cursors: &cursors,
            holds: &[],
        }],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        result[0].reasons,
        [
            DemandReason::Neighbor,
            DemandReason::Preparation,
            DemandReason::Active,
            DemandReason::Outgoing
        ]
        .into_iter()
        .collect()
    );
    let empty = [DemandSpan {
        start: 4,
        end: 4,
        binding: DemandBinding {
            application: app,
            pre: u16::MAX,
            post: u16::MAX,
        },
    }];
    assert!(
        resolve(
            &[],
            &[MatrixDemand {
                columns: &columns,
                spans: &empty,
                cursors: &cursors,
                holds: &[]
            }],
            Default::default()
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn group_range_admission_rejects_invalid_ranges_and_bounds_total_associations() {
    let columns = columns(3);
    for (start, end) in [(2, 1), (0, 4)] {
        let spans = [DemandSpan {
            start,
            end,
            binding: DemandBinding {
                application: id(),
                pre: 0,
                post: 0,
            },
        }];
        assert!(
            resolve(
                &[],
                &[MatrixDemand {
                    columns: &columns,
                    spans: &spans,
                    cursors: &[],
                    holds: &[]
                }],
                Default::default()
            )
            .is_err()
        );
    }
    let spans = [DemandSpan {
        start: 0,
        end: 3,
        binding: DemandBinding {
            application: id(),
            pre: 0,
            post: 0,
        },
    }];
    let matrices = [
        MatrixDemand {
            columns: &columns,
            spans: &spans,
            cursors: &[],
            holds: &[],
        },
        MatrixDemand {
            columns: &columns,
            spans: &spans,
            cursors: &[],
            holds: &[],
        },
    ];
    assert!(
        resolve(
            &[],
            &matrices,
            DemandLimits {
                applications: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
}
