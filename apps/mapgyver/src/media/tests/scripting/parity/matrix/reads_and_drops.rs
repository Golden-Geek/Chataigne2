use super::*;

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
                    axis_start: 0,
                    axis_count: 16,
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
                    axis_start: 0,
                    axis_count: 16,
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
                            assert_eq!(
                                cell.definition.source,
                                Some(crate::media::resolve_source_node(&engine, media.0).unwrap())
                            );
                            media
                        }
                        MediaEditResult::InstancesCreated { instances } => {
                            let snapshot = crate::media::composition::inspect(&engine, scene.sources[1]).unwrap();
                            assert_eq!(
                                snapshot
                                    .instances
                                    .iter()
                                    .find(|item| item.reference == instances[0])
                                    .unwrap()
                                    .definition
                                    .source,
                                Some(crate::media::resolve_source_node(&engine, instances[0].instance.0).unwrap())
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
