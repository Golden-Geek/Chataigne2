#![cfg(all(windows, feature = "native-qualification"))]
//! Actual app plans under unchanged 1080p/4K byte ceilings, including supersample scratch.

use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor,
    native::{AdapterInfo, D3d11Config, InputLimits, OutputBinding, adapters, query_adapter_memory},
    runtime::{FailureCode, OutcomeKind, RequestId, VideoRuntime},
};
use mapgyver::{
    app::{AppEngine, AppNode},
    media::{capture_channels, protocol::*, rendering::ChannelRenderer},
    screens::{
        protocol::*,
        rendering::{ScreenRoute, capture_screen},
    },
};
use serde_json::{Value, json};
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

const MIB: u64 = 1024 * 1024;
const WAIT: Duration = Duration::from_secs(8);

fn edit(engine: &mut AppEngine, namespace: &str, payload: impl serde::Serialize) -> Value {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: namespace.into(),
            operation: "edit".into(),
            payload: serde_json::to_value(payload).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    ack.result.unwrap().value
}
fn screen_edit(engine: &mut AppEngine, payload: ScreenEdit) -> ScreenEditResult {
    serde_json::from_value(edit(engine, SCREEN_NAMESPACE, payload)).unwrap()
}
fn media_edit(engine: &mut AppEngine, payload: MediaEdit) -> MediaEditResult {
    serde_json::from_value(edit(engine, MEDIA_NAMESPACE, payload)).unwrap()
}

struct Scene {
    engine: AppEngine,
    channel: ChannelReference,
    screens: Vec<ScreenId>,
    second: SurfaceReference,
}
impl Scene {
    fn new(count: usize) -> Self {
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let MediaEditResult::Created { reference: channel } = media_edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::SolidColor,
                label: None,
            },
        ) else {
            panic!()
        };
        media_edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: channel.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(8, 8).unwrap(),
                },
            },
        );
        let mut screens = Vec::new();
        let mut second = None;
        for _ in 0..count {
            let ScreenEditResult::ScreenCreated { screen, surface } =
                screen_edit(&mut engine, ScreenEdit::CreateScreen { label: None })
            else {
                panic!()
            };
            let ScreenEditResult::SurfaceCreated { reference } =
                screen_edit(&mut engine, ScreenEdit::AddSurface { screen, label: None })
            else {
                panic!()
            };
            for reference in [surface, reference] {
                screen_edit(
                    &mut engine,
                    ScreenEdit::ConfigureSurface {
                        reference,
                        definition: SurfaceDefinition {
                            source: Some(channel),
                            opacity: 0.5,
                            ..Default::default()
                        },
                    },
                );
            }
            second.get_or_insert(reference);
            screens.push(screen);
        }
        Self {
            engine,
            channel,
            screens,
            second: second.unwrap(),
        }
    }
    fn configure(&mut self, mode: ScreenAntiAliasing, extent: Extent2D, multi: bool) {
        for &screen in &self.screens {
            screen_edit(
                &mut self.engine,
                ScreenEdit::ConfigureScreen {
                    screen,
                    definition: ScreenDefinition {
                        anti_aliasing: mode,
                        enabled: true,
                        target: Some(ScreenTarget::Spout {
                            sender: format!("Authored {screen:?}"),
                            extent,
                        }),
                    },
                },
            );
        }
        screen_edit(
            &mut self.engine,
            ScreenEdit::ConfigureSurface {
                reference: self.second,
                definition: SurfaceDefinition {
                    source: Some(self.channel),
                    enabled: multi,
                    opacity: 0.5,
                    ..Default::default()
                },
            },
        );
    }
    fn publish(&self, renderer: &mut ChannelRenderer, count: usize, extent: Extent2D) -> RequestId {
        self.publish_to(renderer, count, extent, 1)
    }
    fn publish_to(
        &self,
        renderer: &mut ChannelRenderer,
        count: usize,
        extent: Extent2D,
        first_output: u64,
    ) -> RequestId {
        let routes = self.screens[..count]
            .iter()
            .enumerate()
            .map(|(index, &screen)| ScreenRoute {
                output: Some(OutputId::new(index as u64 + first_output).unwrap()),
                extent,
                screen: capture_screen(&self.engine, screen, 8).unwrap(),
            })
            .collect();
        renderer
            .publish_screen_targets(
                capture_channels(&self.engine, &[self.channel], 8).unwrap(),
                Vec::new(),
                routes,
                None,
            )
            .unwrap()
    }
}

fn settled(renderer: &mut ChannelRenderer, request: RequestId, output_id: OutputId) {
    let deadline = Instant::now() + WAIT;
    let mut installed = None;
    loop {
        renderer.refresh().unwrap();
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        for outcome in outcomes.into_iter().filter(|o| o.request == request) {
            match outcome.result {
                OutcomeKind::Installed { generation, .. } => installed = Some(generation.identity),
                OutcomeKind::Rejected(error) => panic!("Screen resource publication rejected: {error:?}"),
                _ => {}
            }
        }
        let snapshot = renderer.snapshot();
        if let (Some(identity), Some(frame), Some(resources)) = (installed, snapshot.last_frame, snapshot.resources) {
            assert!(resources.bytes_used <= resources.byte_limit && resources.peak_bytes <= resources.byte_limit);
            assert!(
                resources.in_flight <= 2 && resources.retiring_generations <= 2 && resources.prepared_generations <= 1
            );
            if frame.identity == identity
                && resources.prepared_generations == 0
                && resources.retiring_generations == 0
                && renderer
                    .native_control()
                    .unwrap()
                    .output_snapshots()
                    .iter()
                    .any(|output| {
                        output.id == output_id
                            && output.last_accepted.is_some_and(|token| {
                                token.device == frame.completion.device && token.serial >= frame.completion.serial
                            })
                    })
            {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "Screen resource publication timed out: {snapshot:?}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn qualify(adapter: &AdapterInfo, width: u32, height: u32) -> Value {
    let extent = Extent2D::new(width, height).unwrap();
    let pixels = u64::from(width) * u64::from(height) * 4;
    let ceiling = if width == 1920 { 512 * MIB } else { 1024 * MIB };
    let memory = query_adapter_memory(adapter.id).unwrap();
    let ceiling = memory.admission_limit(ceiling).unwrap();
    let config = mapgyver::preview::config::configuration(ceiling, None).video;
    let descriptor = TextureDescriptor {
        extent,
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    };
    let runtime = VideoRuntime::spawn_d3d11(
        config.clone(),
        D3d11Config {
            adapter: adapter.id,
            diagnostics: None,
            inputs: InputLimits {
                max_sources: 0,
                max_storage_bytes: 0,
            },
            outputs: (1..=8)
                .map(|index| OutputBinding::Spout {
                    id: OutputId::new(index).unwrap(),
                    name: format!("MapGyver-AA-Resources-{}-{index}", uuid::Uuid::new_v4()),
                    descriptor,
                })
                .collect(),
        },
    )
    .unwrap();
    let mut renderer = ChannelRenderer::new(runtime, config.plan_limits);
    let control = renderer.native_control().unwrap();
    let count = if width == 1920 { 7 } else { 3 };
    let mut scene = Scene::new(count);
    let baseline = 15248 + 8 * pixels;
    // SDK endpoints report UnknownSdk until their first accepted send. Initialize every
    // bound endpoint sequentially before asserting complete native inventory totals.
    scene.configure(ScreenAntiAliasing::Raw, extent, false);
    for output in 1..=8 {
        let request = scene.publish_to(&mut renderer, 1, extent, output);
        settled(&mut renderer, request, OutputId::new(output).unwrap());
    }
    let mut samples = Vec::new();
    for multi in [false, true] {
        for mode in [ScreenAntiAliasing::Raw, ScreenAntiAliasing::Smooth2x] {
            scene.configure(mode, extent, multi);
            let request = scene.publish(&mut renderer, 1, extent);
            settled(&mut renderer, request, OutputId::new(1).unwrap());
            let resources = renderer.snapshot().resources.unwrap();
            // One 8x8 producer; final composition, optional high-resolution target,
            // multi-layer ping-pong and independent SDK conversion each count once.
            let factor = match (mode, multi) {
                (ScreenAntiAliasing::Raw, false) => 2,
                (ScreenAntiAliasing::Raw, true) => 3,
                (ScreenAntiAliasing::Smooth2x, false) => 6,
                (ScreenAntiAliasing::Smooth2x, true) => 10,
            };
            let generation_bytes = factor * pixels + 256;
            assert_eq!(resources.active.unwrap().bytes, generation_bytes);
            assert_eq!(resources.bytes_used, baseline + generation_bytes);
            let allocations = control.allocations_snapshot();
            assert_eq!(allocations.plan.texel_bytes, generation_bytes);
            assert_eq!(allocations.owned_texel_bytes, Some(generation_bytes + 8 * pixels));
            assert_eq!(renderer.channels().len(), 1, "supersampling shares one producer");
            samples.push(json!({"mode":mode,"surfaces":if multi {2} else {1},"generation_bytes":generation_bytes,
                "reserved_bytes":resources.bytes_used,"peak_reserved_bytes":resources.peak_bytes,"allocations":allocations}));
        }
    }
    let before = renderer.snapshot().resources.unwrap();
    let before_native = control.allocations_snapshot();
    let request = scene.publish(&mut renderer, count, extent);
    let deadline = Instant::now() + WAIT;
    let rejection = loop {
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        if let Some(outcome) = outcomes.into_iter().find(|o| o.request == request) {
            let OutcomeKind::Rejected(error) = outcome.result else {
                panic!("overload must reject atomically: {outcome:?}")
            };
            assert_eq!(error.code(), FailureCode::ResourcePressure);
            break error.to_string();
        }
        assert!(Instant::now() < deadline, "Screen resource rejection timed out");
        std::thread::sleep(Duration::from_millis(2));
    };
    let rejected = renderer.snapshot().resources.unwrap();
    assert_eq!(
        rejected.active, before.active,
        "overload preserves Smooth; no Raw fallback"
    );
    assert_eq!(rejected.bytes_used, before.bytes_used);
    assert_eq!(control.allocations_snapshot().plan.created, before_native.plan.created);
    let recovery = scene.publish(&mut renderer, 1, extent);
    settled(&mut renderer, recovery, OutputId::new(1).unwrap());
    let recovered = renderer.snapshot().resources.unwrap();
    assert_eq!(recovered.active.unwrap().bytes, 10 * pixels + 256);
    assert!(recovered.peak_bytes <= ceiling);
    let retirement = renderer.retirement();
    let start = Instant::now();
    renderer.close();
    drop(renderer);
    assert!(
        retirement.wait(Duration::from_secs(2)),
        "original two-second retirement bound"
    );
    let elapsed = start.elapsed();
    let final_inventory = control.allocations_snapshot();
    assert_eq!(final_inventory.plan.live, 0);
    assert_eq!(final_inventory.plan.created, final_inventory.plan.released);
    assert!(final_inventory.outputs.is_empty() && final_inventory.sources.is_empty());
    json!({"adapter":adapter,"extent":extent,"adapter_memory":memory,"ceiling":ceiling,"baseline_reserved_bytes":baseline,
        "samples":samples,"pressure_screen_count":count,"rejection":rejection,"peak_reserved_bytes":recovered.peak_bytes,
        "retirement_ms":elapsed.as_secs_f64()*1000.0,"final_inventory":final_inventory,
        "timing_qualification":false,"driver_physical_bytes":"unknown; descriptor payload and conservative admission are separate"})
}

#[test]
#[ignore = "explicit M12/8 optimized both-GPU 1080p/4K allocation, overload and recovery check"]
fn screen_supersampling_allocation_admission_and_recovery() {
    let (done, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            wait.recv_timeout(Duration::from_secs(180)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Screen resource watchdog expired");
            std::process::exit(1);
        }
    });
    let mut results = Vec::new();
    for adapter in adapters().unwrap().into_iter().filter(|a| !a.software) {
        for (width, height) in [(1920, 1080), (3840, 2160)] {
            let result = qualify(&adapter, width, height);
            eprintln!("Screen resources {result}");
            results.push(result);
        }
    }
    assert!(!results.is_empty());
    if let Some(directory) = std::env::var_os("MAPGYVER_SCREEN_RESOURCE_DIRECTORY") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("results.json"),
            serde_json::to_vec_pretty(&results).unwrap(),
        )
        .unwrap();
    }
    done.send(()).unwrap();
}
