//! Revised M04/2: concurrent variants share authored sizing, not source identity or content.
use super::*;
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};

fn image(root: &std::path::Path, width: u32, height: u32, pixel: [u8; 4]) {
    image::save_buffer_with_format(
        root.join("variant.png"),
        &pixel.repeat((width * height) as usize),
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .unwrap();
}

fn asset(engine: &mut AppEngine, path: &str) -> AssetId {
    let MediaEditResult::AssetCreated { asset } = edit(
        engine,
        MediaEdit::ImportAsset {
            locator: AssetLocator::ProjectRelative {
                path: path.to_owned().try_into().unwrap(),
            },
            label: None,
        },
    ) else {
        panic!()
    };
    asset
}

fn source(engine: &mut AppEngine, reference: ChannelReference, source: ChannelSource) {
    edit(engine, MediaEdit::ConfigureChannel { reference, source });
}

fn order(engine: &mut AppEngine, media: MediaId) -> Vec<ChannelReference> {
    let MediaEditResult::Manager { snapshot } = edit(engine, MediaEdit::InspectManager) else {
        panic!()
    };
    snapshot
        .media
        .into_iter()
        .find(|entry| entry.id == media)
        .unwrap()
        .channels
        .into_iter()
        .map(|channel| channel.reference)
        .collect()
}

fn fixture(adapter: AdapterId, kind: MediaKind, root: &std::path::Path) -> Fixture {
    let mut limits = config();
    if kind == MediaKind::SolidColor {
        // The required disabled procedural canvas is 1920x1080; keep the tiny native-source
        // fixture limits for Picture/Shared Texture. These are existing functional workloads.
        limits.plan_limits.max_texture_dimension = 1920;
        limits.plan_limits.max_texture_bytes = 64 * 1024 * 1024;
        limits.resource_limits.max_bytes = 64 * 1024 * 1024;
    }
    let mut fixture = Fixture::with_config(adapter, limits);
    if kind == MediaKind::Picture {
        fixture
            .live()
            .enable_pictures(
                root,
                golden_assets::PreparationLimits {
                    max_source_bytes: 64 * 1024,
                    max_decode_bytes: NonZeroUsize::new(128 * 1024).unwrap(),
                    max_prepared_items: NonZeroUsize::new(8).unwrap(),
                    max_prepared_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
                    ..Default::default()
                },
                None,
            )
            .unwrap();
    }
    fixture
}

#[test]
#[ignore = "explicit revised M04/2 variant sizing, pixels, history and shared consumers on hardware"]
fn same_media_variants_share_resolution_and_preserve_native_identity_through_history_and_reload() {
    let children = Arc::new(Mutex::new(Vec::<Weak<Mutex<Child>>>::new()));
    let watched = children.clone();
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            finished.recv_timeout(Duration::from_secs(180)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            for child in watched.lock().unwrap().iter().filter_map(Weak::upgrade) {
                let _ = child.lock().unwrap().kill();
            }
            eprintln!("variant Resolution/retirement watchdog expired");
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
        "M04/2 qualification requires both recorded hardware adapters"
    );
    for (index, adapter) in hardware {
        for kind in [MediaKind::SolidColor, MediaKind::Picture, MediaKind::SharedTexture] {
            let root = tempfile::tempdir().unwrap();
            image(root.path(), 2, 2, [255, 0, 0, 255]);
            std::fs::rename(root.path().join("variant.png"), root.path().join("first.png")).unwrap();
            image(root.path(), 3, 2, [0, 0, 255, 255]);
            let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
            engine.apply_edits().unwrap();
            let MediaEditResult::Created { reference: first } = edit(
                &mut engine,
                MediaEdit::CreateMedia {
                    media_kind: kind,
                    label: None,
                },
            ) else {
                panic!()
            };
            let MediaEditResult::Created { reference: second } = edit(
                &mut engine,
                MediaEdit::AddChannel {
                    media: first.media,
                    label: None,
                },
            ) else {
                panic!()
            };
            let references = [first, second];
            assert_eq!(first.media, second.media);
            assert_ne!(first.channel, second.channel);
            let names = [
                format!("MapGyverVariantsA-{}", uuid::Uuid::new_v4()),
                format!("MapGyverVariantsB-{}", uuid::Uuid::new_v4()),
            ];
            let mut producers = Vec::new();
            match kind {
                MediaKind::SolidColor => {
                    source(
                        &mut engine,
                        first,
                        ChannelSource::SolidColor {
                            rgba: [1.0, 0.0, 0.0, 1.0],
                        },
                    );
                    source(
                        &mut engine,
                        second,
                        ChannelSource::SolidColor {
                            rgba: [0.0, 0.0, 1.0, 1.0],
                        },
                    );
                }
                MediaKind::Picture => {
                    let a = asset(&mut engine, "first.png");
                    let b = asset(&mut engine, "variant.png");
                    let a = mapgyver::media::asset(&engine, a).unwrap().locator;
                    let b = mapgyver::media::asset(&engine, b).unwrap().locator;
                    source(&mut engine, first, ChannelSource::Picture { file: Some(a) });
                    source(&mut engine, second, ChannelSource::Picture { file: Some(b) });
                }
                MediaKind::SharedTexture => {
                    let mut a = Producer::new(index, &names[0], &children);
                    let mut b = Producer::new(index, &names[1], &children);
                    a.command("start 2 2 1 0 0");
                    b.command("start 3 2 0 0 1");
                    configure(&mut engine, first, &names[0]);
                    configure(&mut engine, second, &names[1]);
                    producers.extend([a, b]);
                }
                _ => unreachable!(),
            }
            let automatic = if kind == MediaKind::SolidColor {
                [available(1920, 1080); 2]
            } else {
                [available(2, 2), available(3, 2)]
            };
            let mut live = fixture(adapter.id, kind, root.path());
            live.publish(&engine, references);
            live.settle(automatic, None);
            live.pixels(
                adapter.id,
                [[255, 0, 0, 255], [0, 0, 255, 255]],
                &format!("{kind:?}/automatic"),
            );
            assert_eq!(live.live().channels().len(), 2, "three consumers share two productions");
            let channels = live.live().channels();
            assert_ne!(channels[0].target, channels[1].target);
            assert_eq!(
                live.live().native_control().unwrap().sources().len(),
                if kind == MediaKind::SharedTexture { 2 } else { 0 }
            );

            let history = engine.undo_len();
            edit(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: first.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(3, 2).unwrap(),
                    },
                },
            );
            assert_eq!(engine.undo_len(), history + 1);
            live.publish(&engine, references);
            live.settle([available(3, 2); 2], None);
            live.pixels(
                adapter.id,
                [[255, 0, 0, 255], [0, 0, 255, 255]],
                &format!("{kind:?}/forced"),
            );
            assert!(engine.undo().unwrap());
            live.publish(&engine, references);
            live.settle(automatic, None);
            assert!(engine.redo().unwrap());
            live.publish(&engine, references);
            live.settle([available(3, 2); 2], None);

            match kind {
                MediaKind::SolidColor => source(
                    &mut engine,
                    second,
                    ChannelSource::SolidColor {
                        rgba: [0.0, 1.0, 1.0, 1.0],
                    },
                ),
                MediaKind::Picture => {
                    image(root.path(), 4, 3, [0, 255, 255, 255]);
                    edit(&mut engine, MediaEdit::ReloadPicture { reference: second });
                }
                MediaKind::SharedTexture => {
                    let native = live.live().native_control().unwrap();
                    let texture = live.live().channels()[1].source.unwrap();
                    let before = native
                        .sources()
                        .into_iter()
                        .find(|source| source.texture == texture)
                        .unwrap()
                        .publications;
                    producers[1].command("start 4 3 0 1 1");
                    wait("forced variant observes resized source publication", || {
                        if let Err(error) = live.live().refresh() {
                            assert!(error.contains("source observations"), "{error}");
                        }
                        native.sources().iter().any(|source| {
                            source.texture == texture
                                && source.publications > before
                                && source
                                    .observed_descriptor
                                    .is_some_and(|descriptor| descriptor.extent == Extent2D::new(4, 3).unwrap())
                        })
                    });
                }
                _ => unreachable!(),
            }
            live.publish(&engine, references);
            live.settle([available(3, 2); 2], None);
            live.pixels(
                adapter.id,
                [[255, 0, 0, 255], [0, 255, 255, 255]],
                &format!("{kind:?}/forced-source-change"),
            );
            edit(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: first.media,
                    size: SizePolicy::Automatic,
                },
            );
            let resized = if kind == MediaKind::SolidColor {
                automatic
            } else {
                [available(2, 2), available(4, 3)]
            };
            live.publish(&engine, references);
            live.settle(resized, None);
            live.pixels(
                adapter.id,
                [[255, 0, 0, 255], [0, 255, 255, 255]],
                &format!("{kind:?}/automatic-restored"),
            );

            let targets = live.live().channels().to_vec();
            edit(
                &mut engine,
                MediaEdit::ReorderChannel {
                    reference: first,
                    after: Some(second.channel),
                },
            );
            assert_eq!(order(&mut engine, first.media), vec![second, first]);
            live.publish(&engine, references);
            live.settle(resized, None);
            assert_eq!(
                live.live().channels(),
                targets,
                "reorder keeps GPU identities bound to channel UUIDs"
            );
            assert!(engine.undo().unwrap());
            assert_eq!(order(&mut engine, first.media), references);
            live.publish(&engine, references);
            live.settle(resized, None);
            assert!(engine.redo().unwrap());
            assert_eq!(order(&mut engine, first.media), vec![second, first]);

            let saved = to_sparse_project_json_pretty(&engine).unwrap();
            let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
            loaded.apply_edits().unwrap();
            mapgyver::media::validate_project(&loaded).unwrap();
            assert_eq!(order(&mut loaded, first.media), vec![second, first]);
            live.publish(&loaded, references);
            live.settle(resized, None);
            assert_eq!(
                live.live()
                    .channels()
                    .iter()
                    .map(|channel| channel.reference)
                    .collect::<Vec<_>>(),
                references
            );
            live.pixels(
                adapter.id,
                [[255, 0, 0, 255], [0, 255, 255, 255]],
                &format!("{kind:?}/reload"),
            );
            drop(live);
            for producer in &mut producers {
                producer.command("exit");
                assert!(producer.child.lock().unwrap().wait().unwrap().success());
            }
            eprintln!(
                "M04/2 variants accepted kind={kind:?} adapter={:?} description={} references={references:?}",
                adapter.id, adapter.description
            );
        }
    }
    done.send(()).unwrap();
}
