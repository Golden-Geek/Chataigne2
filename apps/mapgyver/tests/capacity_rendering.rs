#![cfg(all(windows, feature = "native-qualification"))]
//! M11/2 authored 4K outputs, shared Reference targets, measured admission and native ownership.
use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};
use golden_io::PayloadPool;
use golden_video::native::{
    AdapterInfo, D3d11Config, D3d11Control, InputLimits, OutputBinding, SpoutProbe, adapters, query_adapter_memory,
    query_process_resources,
};
use golden_video::runtime::{OutcomeKind, RequestId, VideoRuntime};
use golden_video::{AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor};
use mapgyver::{
    app::{AppEngine, AppNode},
    media::{
        capture_channels,
        protocol::*,
        rendering::{ChannelRenderer, ChannelRoute},
    },
};
use serde_json::json;
use std::{
    collections::HashSet,
    num::NonZeroUsize,
    time::{Duration, Instant},
};

const MIB: u64 = 1024 * 1024;
const WAIT: Duration = Duration::from_secs(8);

fn edit(engine: &mut AppEngine, operation: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(operation).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

fn scene() -> (AppEngine, Vec<ChannelReference>, Vec<[u8; 4]>) {
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let mut references = Vec::new();
    let mut colors = Vec::new();
    for index in 0..8 {
        let MediaEditResult::Created { reference } = edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::SolidColor,
                label: Some(format!("Capacity {index}")),
            },
        ) else {
            panic!()
        };
        let rgba = [
            if index & 1 != 0 { 1.0 } else { 0.0 },
            if index & 2 != 0 { 1.0 } else { 0.0 },
            if index & 4 != 0 { 1.0 } else { 0.0 },
            1.0,
        ];
        edit(
            &mut engine,
            MediaEdit::ConfigureChannel {
                reference,
                source: ChannelSource::SolidColor { rgba },
            },
        );
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: reference.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(4096, 4096).unwrap(),
                },
            },
        );
        references.push(reference);
        colors.push(rgba.map(|value| (value * 255.0) as u8));
    }
    for index in [1, 4] {
        let MediaEditResult::ReferenceCreated { media } = edit(
            &mut engine,
            MediaEdit::CreateReference {
                target: Some(references[index].media),
                label: None,
            },
        ) else {
            panic!()
        };
        references.push(mapgyver::media::resolve_source_node(&engine, media.0).unwrap());
        colors.push(colors[index]);
    }
    (engine, references, colors)
}

struct Owner(Option<ChannelRenderer>);
impl Owner {
    fn renderer(&mut self) -> &mut ChannelRenderer {
        self.0.as_mut().unwrap()
    }
    fn finish(&mut self) -> (D3d11Control, Duration, serde_json::Value) {
        let mut renderer = self.0.take().unwrap();
        let control = renderer.native_control().unwrap();
        let retirement = renderer.retirement();
        let start = Instant::now();
        renderer.close();
        let close_ms = start.elapsed().as_secs_f64() * 1000.0;
        drop(renderer);
        let drop_ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut plan_empty_ms = None;
        let mut outputs_empty_ms = None;
        loop {
            let finished = retirement.wait(Duration::from_millis(20));
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            let allocations = control.allocations_snapshot();
            if allocations.plan.live == 0 {
                plan_empty_ms.get_or_insert(elapsed);
            }
            if allocations.outputs.is_empty() {
                outputs_empty_ms.get_or_insert(elapsed);
            }
            if finished {
                break;
            }
        }
        (
            control,
            start.elapsed(),
            json!({"close_returned_ms":close_ms,"renderer_drop_returned_ms":drop_ms,
            "first_observed_plan_empty_ms":plan_empty_ms,"first_observed_outputs_empty_ms":outputs_empty_ms}),
        )
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if self.0.is_some() {
            self.finish();
        }
    }
}

fn publish(owner: &mut Owner, engine: &AppEngine, references: &[ChannelReference]) {
    let request = request(owner, engine, references);
    let renderer = owner.renderer();
    wait_publication(renderer, request, references.len());
    assert_eq!(
        renderer
            .channels()
            .iter()
            .map(|channel| channel.target)
            .collect::<HashSet<_>>()
            .len(),
        8,
        "two automatic References reuse exactly eight producer targets"
    );
}

fn request(owner: &mut Owner, engine: &AppEngine, references: &[ChannelReference]) -> RequestId {
    owner
        .renderer()
        .publish(
            capture_channels(engine, references, 16).unwrap(),
            references
                .iter()
                .enumerate()
                .map(|(index, reference)| ChannelRoute {
                    output: OutputId::new(index as u64 + 1).unwrap(),
                    channel: *reference,
                })
                .collect(),
        )
        .unwrap()
}

fn wait_publication(renderer: &mut ChannelRenderer, request: RequestId, outputs: usize) {
    let deadline = Instant::now() + WAIT;
    let mut installed = None;
    loop {
        renderer.refresh().unwrap();
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        for outcome in outcomes {
            if outcome.request == request {
                match outcome.result {
                    OutcomeKind::Installed { generation, .. } => installed = Some(generation.identity),
                    OutcomeKind::Rejected(error) => panic!("4K capacity request rejected: {error:?}"),
                    _ => {}
                }
            }
        }
        if let Some(frame) = renderer.snapshot().last_frame
            && Some(frame.identity) == installed
            && (0..outputs).all(|index| {
                renderer
                    .native_control()
                    .unwrap()
                    .output_snapshots()
                    .iter()
                    .find(|output| output.id == OutputId::new(index as u64 + 1).unwrap())
                    .is_some_and(|output| {
                        output.last_accepted.is_some_and(|token| {
                            token.device == frame.completion.device && token.serial >= frame.completion.serial
                        })
                    })
            })
        {
            break;
        }
        assert!(Instant::now() < deadline, "native 4K publication deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn pixels(adapter: &AdapterInfo, names: &[String], expected: &[[u8; 4]]) {
    // One fresh receiver, staging texture and CPU image share this finite payload cap.
    let pool = PayloadPool::new("4K capacity pixels", 3, 192 * MIB as usize).unwrap();
    for (name, color) in names.iter().zip(expected) {
        let mut probe = SpoutProbe::new(name, adapter.id, pool.clone()).unwrap();
        let deadline = Instant::now() + WAIT;
        let image = loop {
            if let Some(image) = probe.capture_pixels().unwrap() {
                break image;
            }
            assert!(Instant::now() < deadline, "native 4K receive/readback deadline: {name}");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(image.extent(), Extent2D::new(4096, 4096).unwrap());
        assert_eq!(image.pixels().len(), 4096 * 4096);
        assert!(
            image.pixels().iter().all(|pixel| pixel
                .iter()
                .zip(color)
                .all(|(actual, wanted)| actual.abs_diff(*wanted) <= 2)),
            "complete 4K output pixels must match authored color: {name}"
        );
        drop(image);
        drop(probe);
    }
    assert!(pool.wait_for_idle(Duration::from_secs(1)));
}

fn qualify(adapter: &AdapterInfo) -> serde_json::Value {
    let memory = query_adapter_memory(adapter.id).unwrap();
    let admission = memory.admission_limit(u64::MAX).unwrap();
    let config = mapgyver::preview::config::configuration(admission, None).video;
    let extent = Extent2D::new(4096, 4096).unwrap();
    let descriptor = TextureDescriptor {
        extent,
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    };
    let (mut engine, references, mut colors) = scene();
    let names = references
        .iter()
        .map(|_| format!("MapGyverCapacity-{}", uuid::Uuid::new_v4()))
        .collect::<Vec<_>>();
    let runtime = VideoRuntime::spawn_d3d11(
        config.clone(),
        D3d11Config {
            output_rates: Vec::new(),
            adapter: adapter.id,
            diagnostics: None,
            inputs: InputLimits {
                max_sources: 8,
                max_storage_bytes: 256 * MIB,
            },
            outputs: names
                .iter()
                .enumerate()
                .map(|(index, name)| OutputBinding::Spout {
                    id: OutputId::new(index as u64 + 1).unwrap(),
                    name: name.clone(),
                    descriptor,
                })
                .collect(),
        },
    )
    .unwrap();
    let mut owner = Owner(Some(ChannelRenderer::new(runtime, config.plan_limits)));
    let deadline = Instant::now() + WAIT;
    while !owner
        .renderer()
        .snapshot()
        .resources
        .is_some_and(|resources| resources.initialized)
    {
        assert!(Instant::now() < deadline, "native adapter startup deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
    let control = owner.renderer().native_control().unwrap();
    publish(&mut owner, &engine, &references);
    assert_eq!(control.snapshot().adapter, Some(adapter.id));
    let initial = control.allocations_snapshot();
    let first_resources = owner.renderer().snapshot().resources.unwrap();
    pixels(adapter, &names, &colors);
    let mut samples = Vec::new();
    let steady_start = owner.renderer().snapshot().frames_submitted;
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        let allocation = control.allocations_snapshot();
        let memory = query_adapter_memory(adapter.id).unwrap();
        let local = memory.local.unwrap();
        assert_eq!(
            allocation.plan, initial.plan,
            "steady native plan allocations must remain bounded"
        );
        assert!(
            local.current_usage_bytes <= local.budget_bytes,
            "actual process-local usage exceeds OS budget"
        );
        samples.push(json!({"allocations": allocation, "memory": memory,
            "process":query_process_resources(std::process::id()).unwrap()}));
        std::thread::sleep(Duration::from_secs(1));
    }
    let steady_frames = owner
        .renderer()
        .snapshot()
        .frames_submitted
        .checked_sub(steady_start)
        .unwrap();
    assert!(
        steady_frames > 0,
        "eight demanded outputs must continue producing frames"
    );
    let before = control.snapshot();
    edit(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: references[0],
            source: ChannelSource::SolidColor {
                rgba: [1.0, 0.0, 1.0, 1.0],
            },
        },
    );
    colors[0] = [255, 0, 255, 255];
    publish(&mut owner, &engine, &references);
    pixels(adapter, &names, &colors);
    let replaced = control.allocations_snapshot();
    assert_eq!(replaced.plan.live, initial.plan.live);
    assert_eq!(
        replaced.plan.created, initial.plan.created,
        "unchanged target storage is reused on live replacement"
    );
    assert!(control.snapshot().reused_textures > before.reused_textures);
    let resources = owner.renderer().snapshot().resources.unwrap();
    assert!(resources.peak_bytes <= admission);
    let (control, retirement, retirement_stages) = owner.finish();
    let final_inventory = control.allocations_snapshot();
    assert_eq!(final_inventory.plan.live, 0);
    assert_eq!(final_inventory.plan.created, final_inventory.plan.released);
    assert!(final_inventory.outputs.is_empty() && final_inventory.sources.is_empty());
    json!({"adapter":adapter,"initial_memory":memory,"admitted_ceiling":admission,
        "independent_4k_outputs":8,"reference_alias_outputs":2,"full_pixel_checks":20,
        "first_tracked_bytes":first_resources.bytes_used,"peak_tracked_bytes":resources.peak_bytes,
        "initial_native_allocations":initial,"samples":samples,"steady_submitted_frames":steady_frames,"replacement_allocations":replaced,
        "created_textures":control.snapshot().created_textures,"reused_textures":control.snapshot().reused_textures,
        "retirement_ms":retirement.as_secs_f64()*1000.0,"retirement_stages":retirement_stages,
        "retirement_within_two_seconds":retirement<Duration::from_secs(2),"final_inventory":final_inventory})
}

// A deliberately stricter workload ceiling exercises real native rejection/recovery without
// allocating to exhaustion of the user's GPU. Production still uses half the measured budget.
fn qualify_pressure(adapter: &AdapterInfo) -> serde_json::Value {
    let memory = query_adapter_memory(adapter.id).unwrap();
    let ceiling = memory.admission_limit(2048 * MIB).unwrap();
    assert_eq!(
        ceiling,
        2048 * MIB,
        "bounded pressure workload requires a safely admitted 2 GiB"
    );
    let config = mapgyver::preview::config::configuration(ceiling, None).video;
    let (engine, references, colors) = scene();
    let names = references
        .iter()
        .map(|_| format!("MapGyverCapacityPressure-{}", uuid::Uuid::new_v4()))
        .collect::<Vec<_>>();
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(4096, 4096).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    };
    let runtime = VideoRuntime::spawn_d3d11(
        config.clone(),
        D3d11Config {
            output_rates: Vec::new(),
            adapter: adapter.id,
            diagnostics: None,
            inputs: InputLimits {
                max_sources: 8,
                max_storage_bytes: 256 * MIB,
            },
            outputs: names
                .iter()
                .enumerate()
                .map(|(index, name)| OutputBinding::Spout {
                    id: OutputId::new(index as u64 + 1).unwrap(),
                    name: name.clone(),
                    descriptor,
                })
                .collect(),
        },
    )
    .unwrap();
    let mut owner = Owner(Some(ChannelRenderer::new(runtime, config.plan_limits)));
    let startup_deadline = Instant::now() + WAIT;
    while !owner
        .renderer()
        .snapshot()
        .resources
        .is_some_and(|resources| resources.initialized)
    {
        assert!(Instant::now() < startup_deadline, "native pressure startup deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
    let first = request(&mut owner, &engine, &references[..4]);
    wait_publication(owner.renderer(), first, 4);
    let before = owner.renderer().snapshot().resources.unwrap();
    let rejected = request(&mut owner, &engine, &references);
    let deadline = Instant::now() + WAIT;
    let rejection = loop {
        let mut outcomes = Vec::new();
        owner
            .renderer()
            .drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        if let Some(outcome) = outcomes.into_iter().find(|outcome| outcome.request == rejected) {
            let OutcomeKind::Rejected(error) = outcome.result else {
                panic!("overload must reject before allocation")
            };
            assert_eq!(error.code(), golden_video::runtime::FailureCode::ResourcePressure);
            break error.to_string();
        }
        assert!(Instant::now() < deadline, "native pressure outcome deadline");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(
        owner.renderer().snapshot().resources.unwrap().bytes_used,
        before.bytes_used
    );
    pixels(adapter, &names[..4], &colors[..4]);
    let recovery = request(&mut owner, &engine, &references[..4]);
    wait_publication(owner.renderer(), recovery, 4);
    pixels(adapter, &names[..4], &colors[..4]);
    let resources = owner.renderer().snapshot().resources.unwrap();
    assert!(resources.peak_bytes <= ceiling);
    let (control, retirement, retirement_stages) = owner.finish();
    assert!(retirement < Duration::from_secs(2));
    let final_inventory = control.allocations_snapshot();
    assert_eq!(final_inventory.plan.live, 0);
    assert!(final_inventory.outputs.is_empty() && final_inventory.sources.is_empty());
    json!({"workload_ceiling":ceiling,"rejection":rejection,"healthy_output_pixel_checks":8,
        "tracked_before":before.bytes_used,"peak_tracked_bytes":resources.peak_bytes,
        "retirement_ms":retirement.as_secs_f64()*1000.0,"retirement_stages":retirement_stages,"final_inventory":final_inventory})
}

#[test]
#[ignore = "explicit M11/2 full 4K native capacity, pixel, memory and retirement qualification"]
fn eight_independent_4k_outputs_and_reference_aliases_fit_measured_adapter_admission() {
    let (done, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            wait.recv_timeout(Duration::from_secs(180)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("4K capacity/retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let directory = std::env::var_os("MAPGYVER_CAPACITY_NATIVE_DIRECTORY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/m11-capacity-native")
        });
    std::fs::create_dir_all(&directory).unwrap();
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    let selected = std::env::var("MAPGYVER_CAPACITY_ADAPTER_INDEX")
        .ok()
        .map(|value| value.parse::<usize>().unwrap());
    if let Some(index) = selected {
        assert!(index < hardware.len(), "selected adapter is absent");
    }
    for (index, adapter) in hardware.into_iter().enumerate() {
        if selected.is_some_and(|selected| selected != index) {
            continue;
        }
        let capacity = qualify(&adapter);
        let file = directory.join(format!("adapter-{index}.json"));
        assert!(!file.exists(), "qualification requires a fresh artifact path");
        std::fs::write(
            &file,
            serde_json::to_string_pretty(&json!({"capacity":capacity})).unwrap(),
        )
        .unwrap();
        assert_eq!(
            capacity["retirement_within_two_seconds"], true,
            "retirement must retain its two-second bound; actual stages: {}",
            capacity["retirement_stages"]
        );
        let report = json!({"capacity":capacity,"pressure":qualify_pressure(&adapter)});
        eprintln!(
            "4K capacity adapter {index}: {}",
            json!({"admitted_ceiling":report["capacity"]["admitted_ceiling"],"retirement_ms":report["capacity"]["retirement_ms"],"pressure_retirement_ms":report["pressure"]["retirement_ms"],"artifact":file})
        );
        std::fs::write(file, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    done.send(()).unwrap();
}
