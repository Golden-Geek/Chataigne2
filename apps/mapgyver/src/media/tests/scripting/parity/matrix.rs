use super::*;
use crate::matrix::protocol::*;

fn matrix_edit(engine: &mut AppEngine, reference: ChannelReference, edit: MatrixEdit) -> MediaEditResult {
    apply(
        engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit,
        },
    )
}
struct Scene {
    document: String,
    root: ChannelReference,
    sources: [ChannelReference; 2],
    snapshot: MatrixSnapshot,
}
fn scene() -> Scene {
    let mut engine = fixture();
    let sources = [
        create(&mut engine, MediaKind::SolidColor),
        create(&mut engine, MediaKind::Composition),
    ];
    let root = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { items, .. } = matrix_edit(
        &mut engine,
        root,
        MatrixEdit::AddGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Group { id: group } = items[0] else {
        panic!()
    };
    matrix_edit(
        &mut engine,
        root,
        MatrixEdit::AddLayers {
            parent: Some(group),
            count: 2,
        },
    );
    matrix_edit(&mut engine, root, MatrixEdit::AddColumns { count: 2 });
    let snapshot = crate::matrix::inspect(&engine, root.media).unwrap();
    let layers = snapshot
        .items
        .iter()
        .filter_map(|item| match item.id {
            MatrixItemId::Layer { id } => Some(id),
            _ => None,
        })
        .collect::<Vec<_>>();
    matrix_edit(
        &mut engine,
        root,
        MatrixEdit::SetCells {
            assignments: vec![
                CellAssignment {
                    layer: layers[0],
                    column: snapshot.columns[0].id,
                    source: Some(sources[0]),
                },
                CellAssignment {
                    layer: layers[1],
                    column: snapshot.columns[0].id,
                    source: Some(sources[1]),
                },
                CellAssignment {
                    layer: layers[0],
                    column: snapshot.columns[1].id,
                    source: Some(sources[0]),
                },
            ],
        },
    );
    let snapshot = crate::matrix::inspect(&engine, root.media).unwrap();
    Scene {
        document: to_sparse_project_json_pretty(&engine).unwrap(),
        root,
        sources,
        snapshot,
    }
}

#[test]
fn matrix_authoring_inspection_and_atomic_rejection_match_all_public_callers() {
    let scene = scene();
    let group = scene.snapshot.items[0].id;
    let layer = scene.snapshot.items[1].id;
    let MatrixItemId::Layer { id } = layer else { panic!() };
    let a = scene.snapshot.columns[0].id;
    let b = scene.snapshot.columns[1].id;
    let assignment = |source| CellAssignment {
        layer: id,
        column: b,
        source,
    };
    let cases = vec![
        MatrixEdit::Inspect,
        MatrixEdit::Configure {
            definition: MatrixDefinition {
                canvas: Extent2D::new(64, 48).unwrap(),
                ..Default::default()
            },
        },
        MatrixEdit::ConfigureItem {
            item: group,
            settings: CompositeSettings {
                opacity: 0.4,
                blend: golden_video::plan::BlendMode::Screen,
                enabled: false,
            },
        },
        MatrixEdit::RenameItem {
            item: layer,
            label: "Parity layer".into(),
        },
        MatrixEdit::MoveItem {
            item: layer,
            parent: None,
            after: Some(group),
        },
        MatrixEdit::RemoveItem { item: layer },
        MatrixEdit::RenameColumn {
            column: a,
            label: "Parity column".into(),
        },
        MatrixEdit::ConfigureColumn {
            column: a,
            definition: ColumnDefinition {
                transition: Some(TransitionDefinition {
                    duration_ms: 125,
                    curve: TransitionCurve::SmoothStep,
                }),
                applications: vec![],
            },
        },
        MatrixEdit::ReorderColumn {
            column: a,
            after: Some(b),
        },
        MatrixEdit::RemoveColumn { column: a },
        MatrixEdit::SetCells {
            assignments: vec![assignment(Some(scene.sources[1]))],
        },
        MatrixEdit::SetCells {
            assignments: vec![assignment(None)],
        },
        MatrixEdit::SetCells {
            assignments: vec![assignment(Some(scene.root))],
        },
        MatrixEdit::SetCells {
            assignments: vec![assignment(Some(scene.sources[0])), assignment(Some(scene.sources[1]))],
        },
        MatrixEdit::MoveItem {
            item: layer,
            parent: None,
            after: Some(layer),
        },
        MatrixEdit::ConfigureItem {
            item: layer,
            settings: CompositeSettings {
                opacity: 2.0,
                ..Default::default()
            },
        },
    ];
    for edit in cases {
        let operation = MediaEdit::Matrix {
            media: scene.root.media,
            edit,
        };
        let mut canonical = None;
        for caller in CALLERS {
            let mut engine = loaded(&scene.document);
            let result = run(&mut engine, caller, operation.clone());
            let actual = (result, authored(&engine));
            if let Some(expected) = &canonical {
                assert_eq!(&actual, expected, "{caller:?}: {operation:?}");
            } else {
                canonical = Some(actual);
            }
        }
    }
}

#[test]
fn matrix_creation_and_copy_return_committed_exports_and_columns_for_every_caller() {
    let scene = scene();
    let group = scene.snapshot.items[0].id;
    let MatrixItemId::Group { id } = group else { panic!() };
    for caller in CALLERS {
        let mut engine = fixture();
        let value = run(
            &mut engine,
            caller,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::Matrix,
                label: None,
            },
        )
        .unwrap();
        let MediaEditResult::Created { reference } = serde_json::from_value(value).unwrap() else {
            panic!()
        };
        assert_eq!(
            crate::matrix::inspect(&engine, reference.media).unwrap().final_channel,
            reference
        );
        for edit in [
            MatrixEdit::AddGroup {
                parent: None,
                label: None,
            },
            MatrixEdit::AddLayers {
                parent: Some(id),
                count: 2,
            },
            MatrixEdit::AddColumns { count: 2 },
            MatrixEdit::DuplicateItem { item: group },
            MatrixEdit::DuplicateColumn {
                column: scene.snapshot.columns[0].id,
            },
        ] {
            let mut engine = loaded(&scene.document);
            let value = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: scene.root.media,
                    edit,
                },
            )
            .unwrap();
            let MediaEditResult::MatrixCreated { items, columns } = serde_json::from_value(value).unwrap() else {
                panic!()
            };
            assert!(!items.is_empty() || !columns.is_empty());
            let snapshot = crate::matrix::inspect(&engine, scene.root.media).unwrap();
            for item in items {
                assert!(!scene.snapshot.items.iter().any(|old| old.id == item));
                let item = snapshot.items.iter().find(|entry| entry.id == item).unwrap();
                assert!(channel(&engine, item.channel).is_ok());
            }
            for column in columns {
                assert!(!scene.snapshot.columns.iter().any(|old| old.id == column));
                assert!(snapshot.columns.iter().any(|entry| entry.id == column));
            }
        }
    }
}

#[test]
fn matrix_launch_and_clear_are_ephemeral_committed_callbacks_for_every_caller() {
    let scene = scene();
    for caller in CALLERS {
        for mode in [LaunchMode::Cut, LaunchMode::Configured] {
            let mut engine = loaded(&scene.document);
            let reply = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: scene.root.media,
                    edit: MatrixEdit::LaunchColumn {
                        column: scene.snapshot.columns[0].id,
                        mode,
                    },
                },
            )
            .unwrap();
            let MediaEditResult::MatrixLaunched { snapshot } = serde_json::from_value(reply).unwrap() else {
                panic!()
            };
            assert_eq!(snapshot.desired_column, Some(scene.snapshot.columns[0].id));
            assert!(snapshot.launch_id.is_some());
            assert_eq!(
                snapshot.transition.duration_ms,
                if matches!(mode, LaunchMode::Cut) {
                    0
                } else {
                    scene.snapshot.definition.transition.duration_ms
                }
            );
        }
        let mut engine = loaded(&scene.document);
        matrix_edit(
            &mut engine,
            scene.root,
            MatrixEdit::LaunchColumn {
                column: scene.snapshot.columns[0].id,
                mode: LaunchMode::Cut,
            },
        );
        let reply = run(
            &mut engine,
            caller,
            MediaEdit::Matrix {
                media: scene.root.media,
                edit: MatrixEdit::ClearPlayback,
            },
        )
        .unwrap();
        let MediaEditResult::MatrixPlayback { snapshot } = serde_json::from_value(reply).unwrap() else {
            panic!()
        };
        assert_eq!(snapshot.desired_column, None);
        assert_eq!(snapshot.transition.duration_ms, 0);
    }
}

#[test]
fn matrix_bounded_reads_never_launch_or_author_through_any_caller() {
    let scene = scene();
    let MatrixItemId::Group { id } = scene.snapshot.items[0].id else {
        panic!()
    };
    for caller in CALLERS {
        for query in [
            MatrixEdit::InspectPlayback,
            MatrixEdit::InspectViewport {
                selection: Some(crate::matrix::protocol::MatrixSelection {
                    item: Some(scene.snapshot.items[0].id),
                    column: Some(scene.snapshot.columns[0].id),
                }),
                viewport: MatrixViewport {
                    row_start: 0,
                    row_count: 32,
                    column_start: 0,
                    column_count: 16,
                },
            },
            MatrixEdit::InspectTargets {
                query: MatrixTargetQuery {
                    scope: MatrixTargetScope::Groups,
                    search: String::new(),
                    start: 0,
                    count: 16,
                },
            },
            MatrixEdit::InspectTargets {
                query: MatrixTargetQuery {
                    scope: MatrixTargetScope::Items { parent: Some(id) },
                    search: String::new(),
                    start: 0,
                    count: 16,
                },
            },
            MatrixEdit::InspectTargets {
                query: MatrixTargetQuery {
                    scope: MatrixTargetScope::Columns,
                    search: String::new(),
                    start: 0,
                    count: 16,
                },
            },
            MatrixEdit::InspectViewport {
                selection: None,
                viewport: MatrixViewport {
                    row_start: 0,
                    row_count: 0,
                    column_start: 0,
                    column_count: 16,
                },
            },
        ] {
            let mut engine = loaded(&scene.document);
            let result = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: scene.root.media,
                    edit: query.clone(),
                },
            );
            if let Some(value) = result {
                match serde_json::from_value::<MediaEditResult>(value).unwrap() {
                    MediaEditResult::MatrixPlayback { snapshot } => assert!(snapshot.desired_column.is_none()),
                    MediaEditResult::MatrixViewport { snapshot } => {
                        assert_eq!(snapshot.rows.len(), 3);
                        assert_eq!(snapshot.columns.len(), 2);
                        assert_eq!(snapshot.cells.len(), 3);
                        assert!(snapshot.playback.desired_column.is_none());
                    }
                    MediaEditResult::MatrixTargets { page } => assert!((1..=2).contains(&page.choices.len())),
                    other => panic!("unexpected read reply {other:?}"),
                }
            } else {
                assert!(matches!(
                    query,
                    MatrixEdit::InspectViewport {
                        viewport: MatrixViewport { row_count: 0, .. },
                        ..
                    }
                ));
            }
        }
    }
}
