//! M04/7 functional native matrix; bounded pixel reads are separate from throughput evidence.
use super::*;
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};
use mapgyver::{
    matrix::protocol::*,
    media::{capture_channels_at, composition::protocol::*},
};

fn create(engine: &mut AppEngine, kind: MediaKind) -> ChannelReference {
    let MediaEditResult::Created { reference } = edit(
        engine,
        MediaEdit::CreateMedia {
            media_kind: kind,
            label: None,
        },
    ) else {
        panic!()
    };
    edit(
        engine,
        MediaEdit::ConfigureMediaResolution {
            media: reference.media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(4, 4).unwrap(),
            },
        },
    );
    reference
}
fn mirror(engine: &mut AppEngine, target: MediaId, channel: ChannelId) -> ChannelReference {
    let MediaEditResult::ReferenceCreated { media } = edit(
        engine,
        MediaEdit::CreateReference {
            target: Some(target),
            label: None,
        },
    ) else {
        panic!()
    };
    ChannelReference { media, channel }
}
fn matrix_edit(engine: &mut AppEngine, matrix: MediaId, operation: MatrixEdit) -> MediaEditResult {
    edit(
        engine,
        MediaEdit::Matrix {
            media: matrix,
            edit: operation,
        },
    )
}
fn publish(fixture: &mut Fixture, engine: &AppEngine, references: [ChannelReference; 2]) {
    let time = fixture.live().read_time().unwrap();
    let captured = capture_channels_at(engine, &references, 32, time).unwrap();
    fixture.live().publish(captured, shared_routes(references)).unwrap();
}
fn settle(
    fixture: &mut Fixture,
    references: [ChannelReference; 2],
    expected: [(ChannelAvailability, Option<Extent2D>); 2],
) {
    settle_with_input(fixture, references, expected, None);
}
fn settle_with_input(
    fixture: &mut Fixture,
    references: [ChannelReference; 2],
    expected: [(ChannelAvailability, Option<Extent2D>); 2],
    ready_input: Option<ChannelReference>,
) {
    let mut installed = None;
    let mut accepted = None;
    wait("Reference mixed installed outputs", || {
        let live = fixture.live();
        if let Err(error) = live.refresh() {
            assert!(error.contains("source observations"), "{error}");
        }
        let mut outcomes = Vec::new();
        live.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        for outcome in outcomes {
            match outcome.result {
                OutcomeKind::Installed { generation, .. } => {
                    installed = Some((outcome.request, generation.identity));
                    accepted = None;
                }
                OutcomeKind::Rejected(error) => assert_eq!(
                    error.code(),
                    golden_video::runtime::FailureCode::StaleSource,
                    "{error:?}"
                ),
                _ => {}
            }
        }
        let resolved = references.into_iter().zip(expected).all(|(reference, expected)| {
            live.channels()
                .iter()
                .find(|channel| channel.reference == reference)
                .is_some_and(|channel| (channel.resolution.availability, channel.resolution.extent) == expected)
        });
        // A procedural Matrix canvas is ready while a newly copied input is still connecting.
        let input_ready = ready_input.is_none_or(|reference| {
            live.channels().iter().any(|channel| {
                channel.reference == reference && channel.resolution.availability == ChannelAvailability::Ready
            })
        });
        if resolved
            && input_ready
            && let Some((request, identity)) = installed
            && Some(request) == live.latest_request()
            && let Some(frame) = live.snapshot().last_frame
            && frame.identity == identity
        {
            let token = *accepted.get_or_insert(frame.completion);
            let outputs = live.native_control().unwrap().output_snapshots();
            return outputs.len() == 3
                && outputs.iter().all(|output| {
                    output
                        .last_accepted
                        .is_some_and(|last| last.device == token.device && last.serial >= token.serial)
                });
        }
        false
    });
}
fn target(fixture: &mut Fixture, reference: ChannelReference) -> golden_video::TextureId {
    fixture
        .live()
        .channels()
        .iter()
        .find(|channel| channel.reference == reference)
        .unwrap()
        .target
}
fn source_entry_count(fixture: &mut Fixture, expected: usize) {
    wait("Reference native source retirement", || {
        // Idle bindings are evicted on preparation after old generations release their demand.
        // Drive the production notification path while waiting, rather than only reading metadata.
        let live = fixture.live();
        if let Err(error) = live.refresh() {
            assert!(error.contains("source observations"), "{error}");
        }
        let mut outcomes = Vec::new();
        live.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        for outcome in outcomes {
            if let OutcomeKind::Rejected(error) = outcome.result {
                assert_eq!(
                    error.code(),
                    golden_video::runtime::FailureCode::StaleSource,
                    "{error:?}"
                );
            }
        }
        live.native_control().unwrap().sources().len() == expected
    });
}

#[test]
#[ignore = "explicit M04/7 Reference mixed routing, sharing, resampling, recovery and retirement on hardware"]
fn references_share_native_production_through_composition_matrix_and_owned_retirement() {
    let children = Arc::new(Mutex::new(Vec::<Weak<Mutex<Child>>>::new()));
    let watched = children.clone();
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            finished.recv_timeout(Duration::from_secs(240)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            for child in watched.lock().unwrap().iter().filter_map(Weak::upgrade) {
                let _ = child.lock().unwrap().kill();
            }
            eprintln!("Reference native matrix watchdog expired");
            std::process::exit(1);
        }
    });
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .enumerate()
        .filter(|(_, adapter)| !adapter.software)
        .collect::<Vec<_>>();
    assert!(
        hardware.len() >= 2,
        "Reference acceptance requires both recorded hardware adapters"
    );
    for (index, adapter) in hardware {
        for kind in [MediaKind::SolidColor, MediaKind::Picture, MediaKind::SharedTexture] {
            let root = tempfile::tempdir().unwrap();
            let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
            engine.apply_edits().unwrap();
            let source = create(&mut engine, kind);
            // Native sources retain their natural dimensions; procedural sources stay explicitly 4x4.
            if kind != MediaKind::SolidColor {
                edit(
                    &mut engine,
                    MediaEdit::ConfigureMediaResolution {
                        media: source.media,
                        size: SizePolicy::Automatic,
                    },
                );
            }
            let mut producer = None;
            let expected = if kind == MediaKind::Picture {
                [255, 0, 0, 128]
            } else {
                [255, 0, 0, 255]
            };
            match kind {
                MediaKind::SolidColor => {
                    edit(
                        &mut engine,
                        MediaEdit::ConfigureChannel {
                            reference: source,
                            source: ChannelSource::SolidColor {
                                rgba: [1.0, 0.0, 0.0, 1.0],
                            },
                        },
                    );
                }
                MediaKind::Picture => {
                    image::save_buffer_with_format(
                        root.path().join("reference.png"),
                        &expected.repeat(4),
                        2,
                        2,
                        image::ColorType::Rgba8,
                        image::ImageFormat::Png,
                    )
                    .unwrap();
                    let MediaEditResult::AssetCreated { asset } = edit(
                        &mut engine,
                        MediaEdit::ImportAsset {
                            locator: AssetLocator::ProjectRelative {
                                path: "reference.png".to_owned().try_into().unwrap(),
                            },
                            label: None,
                        },
                    ) else {
                        panic!()
                    };
                    let file = mapgyver::media::asset(&engine, asset).unwrap().locator;
                    edit(
                        &mut engine,
                        MediaEdit::ConfigureChannel {
                            reference: source,
                            source: ChannelSource::Picture { file: Some(file) },
                        },
                    );
                }
                MediaKind::SharedTexture => {
                    let sender = format!("MapGyverReference-{}", uuid::Uuid::new_v4());
                    let mut owned = Producer::new(index, &sender, &children);
                    owned.command("start 2 2 1 0 0");
                    configure(&mut engine, source, &sender);
                    producer = Some(owned);
                }
                _ => unreachable!(),
            }
            let a = mirror(&mut engine, source.media, source.channel);
            let b = mirror(&mut engine, a.media, source.channel);
            let mut fixture = Fixture::new(adapter.id);
            if kind == MediaKind::Picture {
                fixture
                    .live()
                    .enable_pictures(
                        root.path(),
                        golden_assets::PreparationLimits {
                            max_source_bytes: 65536,
                            max_decode_bytes: NonZeroUsize::new(128 * 1024).unwrap(),
                            max_prepared_items: NonZeroUsize::new(8).unwrap(),
                            max_prepared_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
                            ..Default::default()
                        },
                        None,
                    )
                    .unwrap();
            }
            let natural = if kind == MediaKind::SolidColor {
                available(4, 4)
            } else {
                available(2, 2)
            };
            publish(&mut fixture, &engine, [a, b]);
            settle(&mut fixture, [a, b], [natural; 2]);
            assert_eq!(target(&mut fixture, source), target(&mut fixture, a));
            assert_eq!(target(&mut fixture, source), target(&mut fixture, b));
            source_entry_count(&mut fixture, usize::from(kind == MediaKind::SharedTexture));
            fixture.pixels(adapter.id, [expected; 2], &format!("{kind:?}/automatic"));
            edit(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: b.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(3, 2).unwrap(),
                    },
                },
            );
            publish(&mut fixture, &engine, [a, b]);
            settle(&mut fixture, [a, b], [natural, available(3, 2)]);
            assert_ne!(target(&mut fixture, a), target(&mut fixture, b));
            source_entry_count(&mut fixture, usize::from(kind == MediaKind::SharedTexture));
            fixture.pixels(adapter.id, [expected; 2], &format!("{kind:?}/resampled"));
            let composition = create(&mut engine, MediaKind::Composition);
            let MediaEditResult::InstancesCreated { instances } = edit(
                &mut engine,
                MediaEdit::Composition {
                    reference: composition,
                    edit: CompositionEdit::DropSource {
                        source: a.media.0,
                        instance: None,
                        copy: false,
                        position: Point { x: 2.0, y: 2.0 },
                    },
                },
            ) else {
                panic!()
            };
            let MediaEditResult::Composition { snapshot: document } = edit(
                &mut engine,
                MediaEdit::Composition {
                    reference: composition,
                    edit: CompositionEdit::Inspect,
                },
            ) else {
                panic!()
            };
            let mut placement = document.instances[0].definition.clone();
            if kind != MediaKind::SolidColor {
                placement.transform.scale = [2.0, 2.0];
            }
            edit(
                &mut engine,
                MediaEdit::Composition {
                    reference: composition,
                    edit: CompositionEdit::ConfigureInstance {
                        instance: instances[0].instance,
                        definition: placement,
                    },
                },
            );
            let matrix = create(&mut engine, MediaKind::Matrix);
            matrix_edit(
                &mut engine,
                matrix.media,
                MatrixEdit::AddLayers { parent: None, count: 1 },
            );
            matrix_edit(
                &mut engine,
                matrix.media,
                MatrixEdit::AddColumns { parent: None, count: 2 },
            );
            let MediaEditResult::Matrix { snapshot } = matrix_edit(&mut engine, matrix.media, MatrixEdit::Inspect)
            else {
                panic!()
            };
            let MatrixItemId::Layer { id: layer } = snapshot.items[0].id else {
                panic!()
            };
            matrix_edit(
                &mut engine,
                matrix.media,
                MatrixEdit::DropSource {
                    layer,
                    column: snapshot.columns[0].id,
                    source: composition.media.0,
                    copy: false,
                },
            );
            matrix_edit(
                &mut engine,
                matrix.media,
                MatrixEdit::LaunchColumn {
                    column: snapshot.columns[0].id,
                    mode: LaunchMode::Cut,
                },
            );
            publish(&mut fixture, &engine, [matrix, b]);
            settle(&mut fixture, [matrix, b], [available(4, 4), available(3, 2)]);
            source_entry_count(&mut fixture, usize::from(kind == MediaKind::SharedTexture));
            fixture.pixels(adapter.id, [expected; 2], &format!("{kind:?}/mixed"));
            let saved = to_sparse_project_json_pretty(&engine).unwrap();
            let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
            loaded.apply_edits().unwrap();
            matrix_edit(
                &mut loaded,
                matrix.media,
                MatrixEdit::LaunchColumn {
                    column: snapshot.columns[0].id,
                    mode: LaunchMode::Cut,
                },
            );
            publish(&mut fixture, &loaded, [matrix, b]);
            settle(&mut fixture, [matrix, b], [available(4, 4), available(3, 2)]);
            fixture.pixels(adapter.id, [expected; 2], &format!("{kind:?}/reload"));
            edit(&mut loaded, MediaEdit::RemoveMedia { media: source.media });
            publish(&mut fixture, &loaded, [matrix, b]);
            settle(
                &mut fixture,
                [matrix, b],
                [
                    available(4, 4),
                    (ChannelAvailability::Missing, Some(Extent2D::new(3, 2).unwrap())),
                ],
            );
            fixture.pixels(adapter.id, [[0, 0, 0, 0]; 2], &format!("{kind:?}/missing"));
            assert!(loaded.undo().unwrap());
            publish(&mut fixture, &loaded, [matrix, b]);
            settle(&mut fixture, [matrix, b], [available(4, 4), available(3, 2)]);
            fixture.pixels(adapter.id, [expected; 2], &format!("{kind:?}/recovered"));
            let MediaEditResult::MatrixMediaCreated { media: copy, .. } = matrix_edit(
                &mut loaded,
                matrix.media,
                MatrixEdit::DropSource {
                    layer,
                    column: snapshot.columns[1].id,
                    source: source.media.0,
                    copy: true,
                },
            ) else {
                panic!()
            };
            let MediaEditResult::Channels { page } = edit(
                &mut loaded,
                MediaEdit::InspectChannels {
                    query: MediaChannelsQuery {
                        media: copy,
                        start: 0,
                        count: 1,
                    },
                },
            ) else {
                panic!()
            };
            let copied = &page.channels[0];
            assert_ne!(copied.reference.channel, source.channel);
            assert_eq!(
                copied.definition.source,
                mapgyver::media::channel(&loaded, source).unwrap().definition.source
            );
            matrix_edit(
                &mut loaded,
                matrix.media,
                MatrixEdit::LaunchColumn {
                    column: snapshot.columns[1].id,
                    mode: LaunchMode::Cut,
                },
            );
            publish(&mut fixture, &loaded, [matrix, b]);
            settle_with_input(
                &mut fixture,
                [matrix, b],
                [available(4, 4), available(3, 2)],
                Some(copied.reference),
            );
            fixture.pixels(adapter.id, [expected; 2], &format!("{kind:?}/independent-copy"));
            edit(&mut loaded, MediaEdit::RemoveMedia { media: copy });
            publish(&mut fixture, &loaded, [matrix, b]);
            settle(&mut fixture, [matrix, b], [available(4, 4), available(3, 2)]);
            fixture.pixels(adapter.id, [expected; 2], &format!("{kind:?}/owned-retained"));
            matrix_edit(&mut loaded, matrix.media, MatrixEdit::ClearPlayback);
            publish(&mut fixture, &loaded, [matrix, b]);
            settle(&mut fixture, [matrix, b], [available(4, 4), available(3, 2)]);
            source_entry_count(&mut fixture, usize::from(kind == MediaKind::SharedTexture));
            fixture.pixels(
                adapter.id,
                [[0, 0, 0, 0], expected],
                &format!("{kind:?}/owned-final-use"),
            );
            drop(fixture);
            if let Some(mut producer) = producer {
                producer.command("exit");
                assert!(producer.child.lock().unwrap().wait().unwrap().success());
            }
            eprintln!(
                "M04/7 native Reference passed kind={kind:?} adapter={:?} description={}",
                adapter.id, adapter.description
            );
        }
    }
    done.send(()).unwrap();
}
