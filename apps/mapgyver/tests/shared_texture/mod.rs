//! Fixed M04 app-path qualification; source processes and observations stay finite and owned.
use super::*;
use golden_io::PayloadPool;
use golden_video::native::{AdapterId, SourceState, SpoutDiscovery, SpoutProbe};
use mapgyver::media::rendering::PreparedChannel;

struct Fixture {
    owned: OwnedRenderer,
    names: [String; 3],
}

impl Fixture {
    fn new(adapter: AdapterId) -> Self {
        let names = std::array::from_fn(|index| format!("MapGyverSharedPixels-{}-{index}", uuid::Uuid::new_v4()));
        let config = config();
        let runtime = VideoRuntime::spawn_d3d11(
            config.clone(),
            D3d11Config {
                adapter,
                diagnostics: None,
                inputs: InputLimits {
                    max_sources: 4,
                    max_storage_bytes: 4 * InputLimits::source_storage_bytes(descriptor(4, 4)).unwrap(),
                },
                outputs: names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| OutputBinding::Spout {
                        id: OutputId::new(index as u64 + 1).unwrap(),
                        name: name.clone(),
                        descriptor: TextureDescriptor {
                            alpha_mode: AlphaMode::Straight,
                            ..descriptor(if index == 2 { 3 } else { 2 }, 2)
                        },
                    })
                    .collect(),
            },
        )
        .unwrap();
        let renderer = ChannelRenderer::new(runtime, config.plan_limits);
        wait("Shared Texture startup", || renderer.snapshot().resources.is_some());
        Self {
            owned: OwnedRenderer(Some(renderer)),
            names,
        }
    }
    fn live(&mut self) -> &mut ChannelRenderer {
        self.owned.0.as_mut().unwrap()
    }
    fn publish(&mut self, engine: &AppEngine, references: [ChannelReference; 2]) {
        self.live()
            .publish(
                capture_channels(engine, &references, 16).unwrap(),
                shared_routes(references),
            )
            .unwrap();
    }
    fn settle(&mut self, expected: [(ChannelAvailability, Option<Extent2D>); 2], healthy: Option<PreparedChannel>) {
        let mut installed = None;
        let mut accepted = None;
        wait("Shared Texture installed output", || {
            let live = self.live();
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
            if let Some(healthy) = healthy {
                assert_eq!(
                    live.channels()
                        .iter()
                        .find(|channel| channel.reference == healthy.reference)
                        .copied(),
                    Some(healthy),
                    "healthy peer source was replaced"
                );
            }
            let resolved = live.channels().len() == 2
                && live
                    .channels()
                    .iter()
                    .zip(expected)
                    .all(|(channel, (availability, extent))| {
                        channel.resolution.availability == availability && channel.resolution.extent == extent
                    });
            if resolved
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
    fn pixels(&self, adapter: AdapterId, colors: [[u8; 4]; 2], label: &str) {
        let budget = PayloadPool::new("shared-texture-pixels", 9, 65536).unwrap();
        let mut probes = self
            .names
            .iter()
            .map(|name| SpoutProbe::new(name, adapter, budget.clone()).unwrap())
            .collect::<Vec<_>>();
        for (index, probe) in probes.iter_mut().enumerate() {
            let mut image = None;
            wait("Shared Texture pixels", || {
                image = probe.capture_pixels().unwrap();
                image.is_some()
            });
            let image = image.unwrap();
            let channel = usize::from(index == 2);
            assert_eq!(image.extent(), descriptor(if index == 2 { 3 } else { 2 }, 2).extent);
            for pixel in image.pixels() {
                assert!(
                    pixel
                        .iter()
                        .zip(colors[channel])
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "{label}/{index}: {pixel:?} != {:?}",
                    colors[channel]
                );
            }
            eprintln!(
                "Shared Texture {label} output={index} extent={:?} sample={:?} tolerance2/255",
                image.extent(),
                image.pixels()[0]
            );
        }
        drop(probes);
        assert!(budget.wait_for_idle(Duration::from_secs(1)));
    }
    fn healthy_advances(&mut self, channel: PreparedChannel) {
        let texture = channel.source.unwrap();
        let native = self.live().native_control().unwrap();
        let before = native
            .sources()
            .iter()
            .find(|source| source.texture == texture)
            .unwrap()
            .publications;
        wait("healthy source publications during peer failure", || {
            native
                .sources()
                .iter()
                .any(|source| source.texture == texture && source.publications > before)
        });
    }
}

fn shared_routes(references: [ChannelReference; 2]) -> Vec<ChannelRoute> {
    [references[0], references[0], references[1]]
        .into_iter()
        .enumerate()
        .map(|(index, channel)| ChannelRoute {
            output: OutputId::new(index as u64 + 1).unwrap(),
            channel,
        })
        .collect()
}
fn available(width: u32, height: u32) -> (ChannelAvailability, Option<Extent2D>) {
    (ChannelAvailability::Ready, Some(Extent2D::new(width, height).unwrap()))
}
fn catalog(discovery: &mut SpoutDiscovery, present: &[&str], absent: &[&str]) {
    let names = discovery.snapshot(NonZeroUsize::new(256).unwrap()).unwrap().names;
    for name in present {
        assert!(names.iter().any(|entry| entry == name), "missing sender {name}");
    }
    for name in absent {
        assert!(!names.iter().any(|entry| entry == name), "unexpected sender {name}");
    }
}

#[test]
#[ignore = "explicit M04 Shared Texture lifecycle/pixels and two-hardware-adapter qualification"]
fn authored_shared_textures_isolate_failure_resize_rename_and_adapter_mismatch() {
    let children = Arc::new(Mutex::new(Vec::<Weak<Mutex<Child>>>::new()));
    let watched = children.clone();
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            finished.recv_timeout(Duration::from_secs(120)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            for child in watched.lock().unwrap().iter().filter_map(Weak::upgrade) {
                let _ = child.lock().unwrap().kill();
            }
            eprintln!("Shared Texture qualification/retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let inventory = adapters()
        .unwrap()
        .into_iter()
        .enumerate()
        .filter(|(_, adapter)| !adapter.software)
        .collect::<Vec<_>>();
    assert!(
        inventory.len() >= 2,
        "explicit adapter mismatch requires two hardware adapters"
    );
    for (index, adapter) in &inventory {
        let foreign = inventory.iter().find(|(_, other)| other.id != adapter.id).unwrap();
        let identity = uuid::Uuid::new_v4();
        let names = [
            format!("MapGyverSharedA-{identity}"),
            format!("MapGyverSharedB-{identity}"),
            format!("MapGyverRenamed-{identity}"),
            format!("MapGyverForeign-{identity}"),
        ];
        let mut discovery = SpoutDiscovery::new().unwrap();
        catalog(&mut discovery, &[], &[&names[0], &names[1], &names[2], &names[3]]);
        let mut a = Producer::new(*index, &names[0], &children);
        let mut b = Producer::new(*index, &names[1], &children);
        a.command("start 2 2 1 0 0");
        b.command("start 3 2 0 0 1");
        catalog(&mut discovery, &[&names[0], &names[1]], &[]);
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let MediaEditResult::Created { reference: first } = edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::SharedTexture,
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
        configure(&mut engine, first, &names[0]);
        configure(&mut engine, second, &names[1]);
        let mut definition = channel(&engine, second).unwrap().definition;
        definition.size = SizePolicy::Fixed {
            extent: Extent2D::new(3, 2).unwrap(),
        };
        edit(
            &mut engine,
            MediaEdit::ConfigureChannel {
                reference: second,
                definition,
            },
        );
        let mut fixture = Fixture::new(adapter.id);
        fixture.publish(&engine, references);
        fixture.settle([available(2, 2), available(3, 2)], None);
        fixture.pixels(
            adapter.id,
            [[255, 0, 0, 255], [0, 0, 255, 255]],
            "independent-and-shared",
        );
        let sources = fixture.live().native_control().unwrap().sources();
        assert_eq!(
            sources.len(),
            2,
            "three consumers must share two productions/subscriptions"
        );
        assert_ne!(sources[0].binding, sources[1].binding);
        let healthy = fixture.live().channels()[1];
        let device = fixture.live().snapshot().resources.unwrap().last_submitted.device;
        let unavailable = (ChannelAvailability::Unavailable, None);
        a.command("stop");
        catalog(&mut discovery, &[&names[1]], &[&names[0]]);
        fixture.settle([unavailable, available(3, 2)], Some(healthy));
        fixture.healthy_advances(healthy);
        fixture.pixels(adapter.id, [[0; 4], [0, 0, 255, 255]], "absent-peer-live");
        a.command("start 4 3 0 1 0");
        catalog(&mut discovery, &[&names[0], &names[1]], &[]);
        fixture.settle([available(4, 3), available(3, 2)], Some(healthy));
        fixture.pixels(adapter.id, [[0, 255, 0, 255], [0, 0, 255, 255]], "restart-resize");
        a.command("start 8 8 1 0 0");
        fixture.settle(
            [(ChannelAvailability::Incompatible, None), available(3, 2)],
            Some(healthy),
        );
        assert!(
            fixture
                .live()
                .native_control()
                .unwrap()
                .sources()
                .iter()
                .any(|source| source.state == SourceState::DescriptorMismatch)
        );
        fixture.healthy_advances(healthy);
        fixture.pixels(adapter.id, [[0; 4], [0, 0, 255, 255]], "oversized-peer-live");
        a.command("start 2 2 1 0 0");
        fixture.settle([available(2, 2), available(3, 2)], Some(healthy));
        fixture.pixels(adapter.id, [[255, 0, 0, 255], [0, 0, 255, 255]], "size-recovery");
        a.command("stop");
        let mut renamed = Producer::new(*index, &names[2], &children);
        renamed.command("start 2 2 0 1 0");
        catalog(&mut discovery, &[&names[1], &names[2]], &[&names[0]]);
        fixture.settle([unavailable, available(3, 2)], Some(healthy));
        fixture.pixels(adapter.id, [[0; 4], [0, 0, 255, 255]], "rename-no-implicit-selection");
        configure(&mut engine, first, &names[2]);
        fixture.publish(&engine, references);
        fixture.settle([available(2, 2), available(3, 2)], Some(healthy));
        fixture.pixels(
            adapter.id,
            [[0, 255, 0, 255], [0, 0, 255, 255]],
            "explicit-renamed-selection",
        );
        let healthy_a = fixture.live().channels()[0];
        b.command("start 4 3 0 1 1");
        fixture.settle([available(2, 2), available(3, 2)], Some(healthy_a));
        assert!(
            fixture
                .live()
                .native_control()
                .unwrap()
                .sources()
                .iter()
                .any(|source| source.texture == healthy.source.unwrap()
                    && source.observed_descriptor == Some(descriptor(4, 3)))
        );
        fixture.pixels(
            adapter.id,
            [[0, 255, 0, 255], [0, 255, 255, 255]],
            "fixed-size-input-resize",
        );
        let healthy = fixture.live().channels()[1];
        let mut other = Producer::new(foreign.0, &names[3], &children);
        other.command("start 2 2 1 0 0");
        configure(&mut engine, first, &names[3]);
        fixture.publish(&engine, references);
        fixture.settle([unavailable, available(3, 2)], Some(healthy));
        let foreign_texture = fixture.live().channels()[0].source.unwrap();
        let native = fixture.live().native_control().unwrap();
        wait("explicit cross-adapter failure", || {
            native.sources().iter().any(|source| {
                source.texture == foreign_texture
                    && source.state == SourceState::Recovering
                    && source.hresult.is_some_and(|error| error < 0)
                    && source.publications == 0
            })
        });
        eprintln!(
            "Shared Texture cross-adapter selected={:?} sender={:?} source={:?}",
            adapter.id,
            foreign.1.id,
            native
                .sources()
                .iter()
                .find(|source| source.texture == foreign_texture)
                .unwrap()
        );
        assert_eq!(native.snapshot().adapter, Some(adapter.id));
        assert_eq!(
            fixture.live().snapshot().resources.unwrap().last_submitted.device,
            device
        );
        fixture.healthy_advances(healthy);
        fixture.pixels(adapter.id, [[0; 4], [0, 255, 255, 255]], "adapter-mismatch-peer-live");
        configure(&mut engine, first, &names[2]);
        fixture.publish(&engine, references);
        fixture.settle([available(2, 2), available(3, 2)], Some(healthy));
        fixture.pixels(adapter.id, [[0, 255, 0, 255], [0, 255, 255, 255]], "adapter-recovery");
        assert!(
            fixture
                .live()
                .channels()
                .iter()
                .zip(references)
                .all(|(channel, reference)| channel.reference == reference)
        );
        drop(fixture);
        for source in [&mut a, &mut b, &mut renamed, &mut other] {
            source.command("exit");
            assert!(source.child.lock().unwrap().wait().unwrap().success());
        }
        catalog(&mut discovery, &[], &[&names[0], &names[1], &names[2], &names[3]]);
        eprintln!(
            "M04 Shared Texture qualified adapter={:?} description={}",
            adapter.id, adapter.description
        );
    }
    done.send(()).unwrap();
}
