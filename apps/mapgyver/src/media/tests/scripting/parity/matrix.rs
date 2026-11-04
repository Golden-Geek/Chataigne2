use super::*;
use crate::matrix::protocol::*;

#[test]
fn contextual_cell_creation_is_backend_owned_for_all_media_kinds_and_public_callers() {
    let scene = scene();
    let row = scene
        .snapshot
        .items
        .iter()
        .rev()
        .find_map(|row| match row.id {
            MatrixItemId::Layer { id } => Some(id),
            _ => None,
        })
        .unwrap();
    let column = scene.snapshot.columns[1].id;
    for caller in CALLERS {
        for choice in crate::media::creation::media_choices() {
            let mut engine = loaded(&scene.document);
            let reply = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: scene.root.media,
                    edit: MatrixEdit::CreateCellMedia {
                        layer: row,
                        column,
                        media_kind: choice.media_kind,
                    },
                },
            )
            .unwrap();
            let MediaEditResult::MatrixMediaCreated { cell, media } = serde_json::from_value(reply).unwrap() else {
                panic!()
            };
            let snapshot = crate::matrix::inspect(&engine, scene.root.media).unwrap();
            let created = snapshot.cells.iter().find(|entry| entry.id == cell).unwrap();
            assert_eq!(created.media, Some(media));
            assert_eq!(
                crate::media::authoring::media(&engine, media).unwrap().1,
                choice.media_kind
            );
        }
    }
}

#[test]
fn new_matrix_three_by_three_sparse_defaults_match_every_public_caller() {
    for caller in CALLERS {
        let mut engine = fixture();
        let reply = run(
            &mut engine,
            caller,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::Matrix,
                label: Some("Default Matrix".into()),
            },
        )
        .unwrap();
        let MediaEditResult::Created { reference } = serde_json::from_value(reply).unwrap() else {
            panic!()
        };
        let snapshot = crate::matrix::inspect(&engine, reference.media).unwrap();
        assert_eq!(snapshot.items.len(), 3, "{caller:?}");
        assert_eq!(snapshot.columns.len(), 3, "{caller:?}");
        assert!(snapshot.cells.is_empty());
        assert!(
            snapshot
                .items
                .iter()
                .all(|row| row.layer == Some(LayerSettings::default()) && row.settings == CompositeSettings::default())
        );
        assert!(
            snapshot
                .columns
                .iter()
                .all(|column| column.definition == ColumnDefinition::default())
        );
    }
}

#[test]
fn independent_cell_launch_preserves_anchor_and_authored_history_for_all_callers() {
    let scene = scene();
    let row = scene
        .snapshot
        .items
        .iter()
        .find_map(|item| match item.id {
            MatrixItemId::Layer { id } => Some(id),
            _ => None,
        })
        .unwrap();
    for caller in CALLERS {
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
                edit: MatrixEdit::LaunchCell {
                    layer: row,
                    column: scene.snapshot.columns[1].id,
                    mode: LaunchMode::Cut,
                },
            },
        )
        .unwrap();
        let MediaEditResult::MatrixLaunched { snapshot } = serde_json::from_value(reply).unwrap() else {
            panic!()
        };
        assert_eq!(snapshot.desired_column, Some(scene.snapshot.columns[0].id));
        assert_eq!(snapshot.layers[0].layer, row);
        assert_eq!(snapshot.layers[0].column, Some(scene.snapshot.columns[1].id));
    }
}

#[test]
fn sparse_cell_app_control_creation_configuration_and_rejection_match_every_public_caller() {
    let scene = scene();
    let row = scene
        .snapshot
        .items
        .iter()
        .rev()
        .find_map(|item| match item.id {
            MatrixItemId::Layer { id } => Some(id),
            _ => None,
        })
        .unwrap();
    let column = scene.snapshot.columns[1].id;
    assert!(
        !scene
            .snapshot
            .cells
            .iter()
            .any(|cell| cell.layer == row && cell.definition.column == column)
    );
    let definition = crate::applications::protocol::ApplicationDefinition {
        executable: Some(std::env::current_exe().unwrap().to_string_lossy().into_owned()),
        arguments: vec!["literal & value".into()],
        environment: Some(vec![("EXACT".into(), "value".into())]),
        ..Default::default()
    };
    for caller in CALLERS {
        for operation in [
            MatrixEdit::EnsureCell { layer: row, column },
            MatrixEdit::ConfigureCellApplication {
                layer: row,
                column,
                definition: definition.clone(),
                pre: 2,
                post: 3,
            },
            MatrixEdit::ConfigureCellApplication {
                layer: row,
                column,
                definition: crate::applications::protocol::ApplicationDefinition {
                    executable: Some("relative.exe".into()),
                    ..Default::default()
                },
                pre: 0,
                post: 0,
            },
        ] {
            let mut engine = from_sparse_project_json::<AppNode>(&scene.document).unwrap();
            let configured = matches!(&operation, MatrixEdit::ConfigureCellApplication { definition, .. } if definition.executable.as_deref() != Some("relative.exe"));
            let rejected = matches!(&operation, MatrixEdit::ConfigureCellApplication { definition, .. } if definition.executable.as_deref() == Some("relative.exe"));
            let result = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: scene.root.media,
                    edit: operation,
                },
            );
            if rejected {
                assert!(result.is_none());
                continue;
            }
            let MediaEditResult::MatrixCellPrepared { application, .. } =
                serde_json::from_value(result.unwrap()).unwrap()
            else {
                panic!()
            };
            if configured {
                assert_eq!(
                    crate::applications::inspect(&engine, application).unwrap().definition,
                    definition
                );
            }
        }
    }
}

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
    crate::matrix::tests::clear_scene(&mut engine, root.media);
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
        MatrixEdit::ConfigureCell {
            layer: id,
            column: b,
            settings: CellSettings {
                position: [-2.0, 8.5],
                opacity: 0.25,
                enabled: false,
            },
        },
        MatrixEdit::Inspect,
        MatrixEdit::ConfigureLayer {
            layer: id,
            settings: LayerSettings {
                position: [12.0, -9.5],
                duration_ms: 750,
                ..Default::default()
            },
        },
        MatrixEdit::ConfigureItem {
            item: group,
            settings: CompositeSettings {
                opacity: 0.4,
                blend: golden_video::plan::BlendMode::Screen,
                enabled: false,
                expose_channel: false,
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
        MatrixEdit::DropItem {
            item: layer,
            target: MatrixItemDropTarget::Root,
        },
        MatrixEdit::DropItem {
            item: layer,
            target: MatrixItemDropTarget::Before { item: group },
        },
        MatrixEdit::DropItem {
            item: layer,
            target: MatrixItemDropTarget::After { item: group },
        },
        MatrixEdit::DropItem {
            item: layer,
            target: MatrixItemDropTarget::Inside {
                group: match group {
                    MatrixItemId::Group { id } => id,
                    _ => panic!(),
                },
            },
        },
        MatrixEdit::DropItem {
            item: group,
            target: MatrixItemDropTarget::Before { item: layer },
        },
        MatrixEdit::DropColumn {
            column: b,
            target: a,
            before: true,
        },
        MatrixEdit::DropColumn {
            column: a,
            target: b,
            before: false,
        },
        MatrixEdit::DropColumn {
            column: a,
            target: a,
            before: true,
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
            // New owned References have fresh identities per caller; compare their stable Cell address.
            let mut state = serde_json::to_string(&(result, authored(&engine))).unwrap();
            for cell in crate::matrix::inspect(&engine, scene.root.media).unwrap().cells {
                if let Some(source) = cell.definition.source
                    && !scene.document.contains(&source.media.0.0.to_string())
                {
                    state = state.replace(&source.media.0.0.to_string(), &format!("owned-by-{}", cell.id.0.0));
                }
            }
            let actual = state;
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
                assert_eq!(channel(&engine, item.channel).is_ok(), item.settings.expose_channel);
                assert!(crate::media::authoring::channel_internal(&engine, item.channel).is_ok());
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
                if matches!(mode, LaunchMode::Cut) { 0 } else { 250 }
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

#[test]
fn matrix_owned_media_drops_return_committed_children_through_every_public_caller() {
    let scene = scene();
    let layer = scene.snapshot.cells[0].layer;
    let column = scene.snapshot.columns[0].id;
    for caller in CALLERS {
        for copy in [false, true] {
            let mut engine = loaded(&scene.document);
            let reply = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: scene.root.media,
                    edit: MatrixEdit::DropSource {
                        layer,
                        column,
                        source: scene.sources[1].media.0,
                        copy,
                    },
                },
            )
            .unwrap();
            let MediaEditResult::MatrixMediaCreated { cell, media } = serde_json::from_value(reply).unwrap() else {
                panic!()
            };
            let snapshot = crate::matrix::inspect(&engine, scene.root.media).unwrap();
            assert_eq!(
                snapshot
                    .cells
                    .iter()
                    .find(|entry| entry.id == cell)
                    .unwrap()
                    .definition
                    .source
                    .unwrap()
                    .media,
                media
            );
            assert_eq!(
                crate::media::authoring::media(&engine, media).unwrap().1,
                if copy {
                    MediaKind::Composition
                } else {
                    MediaKind::Reference
                }
            );
            crate::media::validate_project(&engine).unwrap();
        }
    }
}

#[test]
fn unavailable_reference_drops_return_owned_identities_and_atomic_history_for_every_caller() {
    for caller in CALLERS {
        for missing in [false, true] {
            for copy in [false, true] {
                for matrix in [false, true] {
                    let scene = scene();
                    let mut engine = loaded(&scene.document);
                    let MediaEditResult::ReferenceCreated { media: source } = apply(
                        &mut engine,
                        MediaEdit::CreateReference {
                            target: missing.then_some(scene.sources[0].media),
                            label: None,
                        },
                    ) else {
                        panic!()
                    };
                    if missing {
                        apply(
                            &mut engine,
                            MediaEdit::RemoveMedia {
                                media: scene.sources[0].media,
                            },
                        );
                    }
                    let operation = if matrix {
                        MediaEdit::Matrix {
                            media: scene.root.media,
                            edit: MatrixEdit::DropSource {
                                layer: scene.snapshot.cells[0].layer,
                                column: scene.snapshot.columns[0].id,
                                source: source.0,
                                copy,
                            },
                        }
                    } else {
                        MediaEdit::Composition {
                            reference: scene.sources[1],
                            edit: crate::media::composition::protocol::CompositionEdit::DropSource {
                                source: source.0,
                                instance: None,
                                position: golden_video::plan::Point { x: 1.0, y: 2.0 },
                                copy,
                            },
                        }
                    };
                    let reply = run(&mut engine, caller, operation).unwrap();
                    let owned = match serde_json::from_value::<MediaEditResult>(reply).unwrap() {
                        MediaEditResult::MatrixMediaCreated { cell, media } => {
                            let snapshot = crate::matrix::inspect(&engine, scene.root.media).unwrap();
                            let cell = snapshot.cells.iter().find(|item| item.id == cell).unwrap();
                            assert_eq!(cell.media, Some(media));
                            assert!(cell.definition.source.is_none());
                            media
                        }
                        MediaEditResult::InstancesCreated { instances } => {
                            let snapshot = crate::media::composition::inspect(&engine, scene.sources[1]).unwrap();
                            assert!(
                                snapshot
                                    .instances
                                    .iter()
                                    .find(|item| item.reference == instances[0])
                                    .unwrap()
                                    .definition
                                    .source
                                    .is_none()
                            );
                            MediaId(instances[0].instance.0)
                        }
                        other => panic!("{other:?}"),
                    };
                    assert!(matches!(
                        engine.nodes.get(engine.node_id_by_uuid(owned.0).unwrap()),
                        Some(AppNode::ReferenceMedia(_))
                    ));
                    crate::media::validate_project(&engine).unwrap();
                }
            }
        }
    }
}
