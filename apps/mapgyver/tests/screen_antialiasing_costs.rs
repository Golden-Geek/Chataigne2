#![cfg(all(windows, feature = "native-qualification"))]
//! Finite Screen-only costs; retained Q01/Q02 source/physical qualification stays separate.

use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor,
    native::{
        D3d11Config, D3d11Control, DiagnosticsConfig, InputLimits, OutputBinding, adapters, query_adapter_memory,
        query_process_resources,
    },
    runtime::{OutcomeKind, VideoRuntime},
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
    fs::File,
    io::{BufWriter, Write},
    num::NonZeroUsize,
    path::Path,
    time::{Duration, Instant},
};
use windows::Win32::System::Performance::QueryPerformanceCounter;

fn qpc() -> u64 {
    let mut value = 0;
    // SAFETY: writable stack output, no native ownership or input mutation.
    unsafe { QueryPerformanceCounter(&mut value) }.unwrap();
    u64::try_from(value).unwrap()
}
fn write(writer: &mut BufWriter<File>, value: Value) {
    serde_json::to_writer(&mut *writer, &value).unwrap();
    writer.write_all(b"\n").unwrap();
}
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
fn scene(extent: Extent2D, mode: ScreenAntiAliasing) -> (AppEngine, ChannelReference, ScreenId) {
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let MediaEditResult::Created { reference } = serde_json::from_value(edit(
        &mut engine,
        MEDIA_NAMESPACE,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::SolidColor,
            label: None,
        },
    ))
    .unwrap() else {
        panic!()
    };
    edit(
        &mut engine,
        MEDIA_NAMESPACE,
        MediaEdit::ConfigureMediaResolution {
            media: reference.media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(8, 8).unwrap(),
            },
        },
    );
    let ScreenEditResult::ScreenCreated { screen, surface } = serde_json::from_value(edit(
        &mut engine,
        SCREEN_NAMESPACE,
        ScreenEdit::CreateScreen { label: None },
    ))
    .unwrap() else {
        panic!()
    };
    edit(
        &mut engine,
        SCREEN_NAMESPACE,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                enabled: true,
                anti_aliasing: mode,
                target: Some(ScreenTarget::Spout {
                    sender: "Authored Screen cost".into(),
                    extent,
                }),
            },
        },
    );
    edit(
        &mut engine,
        SCREEN_NAMESPACE,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                source: Some(reference),
                calibration: Calibration::Grid,
                ..Default::default()
            },
        },
    );
    let ScreenEditResult::SurfaceCreated { reference: second } = serde_json::from_value(edit(
        &mut engine,
        SCREEN_NAMESPACE,
        ScreenEdit::AddSurface { screen, label: None },
    ))
    .unwrap() else {
        panic!()
    };
    edit(
        &mut engine,
        SCREEN_NAMESPACE,
        ScreenEdit::ConfigureSurface {
            reference: second,
            definition: SurfaceDefinition {
                source: Some(reference),
                opacity: 0.25,
                ..Default::default()
            },
        },
    );
    (engine, reference, screen)
}
fn drain(control: &D3d11Control, writer: &mut BufWriter<File>) {
    let mut rows = Vec::with_capacity(4096);
    control.drain_diagnostics(&mut rows, NonZeroUsize::new(4096).unwrap());
    for event in rows {
        write(writer, json!({"kind":"diagnostic","event":event}));
    }
}
fn capture(
    adapter: &golden_video::native::AdapterInfo,
    extent: Extent2D,
    mode: ScreenAntiAliasing,
    directory: &Path,
    seconds: u64,
) {
    std::fs::create_dir_all(directory).unwrap();
    let mut writer = BufWriter::new(File::create(directory.join("renderer.jsonl")).unwrap());
    let ceiling = if extent.width() == 1920 {
        512 * 1024 * 1024
    } else {
        1024 * 1024 * 1024
    };
    let memory = query_adapter_memory(adapter.id).unwrap();
    let ceiling = memory.admission_limit(ceiling).unwrap();
    let config = mapgyver::preview::config::configuration(ceiling, None).video;
    let runtime = VideoRuntime::spawn_d3d11(
        config.clone(),
        D3d11Config {
            adapter: adapter.id,
            diagnostics: Some(DiagnosticsConfig {
                capacity: NonZeroUsize::new(65536).unwrap(),
            }),
            inputs: InputLimits {
                max_sources: 0,
                max_storage_bytes: 0,
            },
            outputs: vec![OutputBinding::Spout {
                id: OutputId::new(1).unwrap(),
                name: format!("MapGyver-AA-Cost-{}", uuid::Uuid::new_v4()),
                descriptor: TextureDescriptor {
                    extent,
                    format: PixelFormat::Bgra8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: AlphaMode::Straight,
                },
            }],
        },
    )
    .unwrap();
    let mut renderer = ChannelRenderer::new(runtime, config.plan_limits);
    let control = renderer.native_control().unwrap();
    let (engine, reference, screen) = scene(extent, mode);
    let request = renderer
        .publish_screen_targets(
            capture_channels(&engine, &[reference], 8).unwrap(),
            Vec::new(),
            vec![ScreenRoute {
                output: Some(OutputId::new(1).unwrap()),
                extent,
                screen: capture_screen(&engine, screen, 8).unwrap(),
            }],
            None,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut installed = None;
    loop {
        renderer.refresh().unwrap();
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(8).unwrap());
        for outcome in outcomes.into_iter().filter(|outcome| outcome.request == request) {
            match outcome.result {
                OutcomeKind::Installed { generation, .. } => installed = Some(generation.identity),
                OutcomeKind::Rejected(error) => panic!("Screen cost publication rejected: {error:?}"),
                _ => {}
            }
        }
        if installed.is_some()
            && renderer
                .snapshot()
                .last_frame
                .is_some_and(|frame| Some(frame.identity) == installed)
            && control
                .output_snapshots()
                .iter()
                .any(|output| output.last_accepted.is_some())
        {
            break;
        }
        assert!(Instant::now() < deadline, "Screen cost startup timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
    let frequency = control.diagnostics_snapshot().qpc_frequency.unwrap();
    let diagnostic = seconds != 120;
    write(
        &mut writer,
        json!({"kind":"capture","qpc_frequency":frequency,"process_id":std::process::id(),
        "adapter":adapter,"adapter_memory":memory,"ceiling":ceiling,"extent":extent,"mode":mode,
        "scope":"Screen-only official calibration + Solid Color, two Surfaces, one SDK output; not Q01/Q02",
        "diagnostic":diagnostic,"requested_warmup_seconds":if diagnostic {0} else {30},"requested_sample_seconds":seconds}),
    );
    let phases = if diagnostic {
        vec![("sample-1", seconds)]
    } else {
        vec![("warmup", 30), ("sample-1", 120), ("sample-2", 120), ("sample-3", 120)]
    };
    for (name, duration) in phases {
        let begin = qpc();
        let end = begin + duration * frequency;
        write(
            &mut writer,
            json!({"kind":"phase","name":name,"begin_qpc":begin,"end_qpc":end}),
        );
        let mut next_resource = Instant::now();
        while qpc() < end {
            drain(&control, &mut writer);
            if Instant::now() >= next_resource {
                let resources = renderer.snapshot().resources.unwrap();
                assert!(resources.peak_bytes <= ceiling);
                write(
                    &mut writer,
                    json!({"kind":"resources","qpc":qpc(),"reserved_bytes":resources.bytes_used,
                    "peak_reserved_bytes":resources.peak_bytes,"in_flight":resources.in_flight,"retiring":resources.retiring_generations,
                    "allocations":control.allocations_snapshot(),"process":query_process_resources(std::process::id()).unwrap(),
                    "adapter_memory":query_adapter_memory(adapter.id).unwrap()}),
                );
                next_resource = Instant::now() + Duration::from_secs(1);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        writer.flush().unwrap();
        eprintln!(
            "Screen cost {}x{} {mode:?} {name} finished on {}",
            extent.width(),
            extent.height(),
            adapter.description
        );
    }
    let retirement = renderer.retirement();
    let begin = Instant::now();
    renderer.close();
    drop(renderer);
    assert!(retirement.wait(Duration::from_secs(2)));
    drain(&control, &mut writer);
    write(
        &mut writer,
        json!({"kind":"finish","diagnostics":control.diagnostics_snapshot(),"allocations":control.allocations_snapshot(),"retirement_ms":begin.elapsed().as_secs_f64()*1000.0}),
    );
    writer.flush().unwrap();
}

#[test]
#[ignore = "explicit optimized M12/8 Screen-only native cost series; up to 26 minutes per GPU"]
fn screen_raw_smooth_native_costs() {
    let directory = std::path::PathBuf::from(
        std::env::var_os("MAPGYVER_SCREEN_COST_DIRECTORY").expect("owned output directory required"),
    );
    let seconds = std::env::var("MAPGYVER_SCREEN_COST_SECONDS")
        .unwrap_or_else(|_| "120".into())
        .parse::<u64>()
        .unwrap();
    assert!((1..=120).contains(&seconds));
    let selected = std::env::var("MAPGYVER_SCREEN_COST_GPU").ok();
    let selected_case = std::env::var("MAPGYVER_SCREEN_COST_CASE").ok();
    let (done, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            wait.recv_timeout(Duration::from_secs(1800)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Screen cost watchdog expired");
            std::process::exit(1);
        }
    });
    let mut count = 0;
    for (index, adapter) in adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .enumerate()
    {
        if selected.as_ref().is_some_and(|selected| selected != &index.to_string()) {
            continue;
        }
        assert!(
            seconds != 120 || selected.is_some(),
            "sustained series requires one explicitly selected GPU"
        );
        for (width, height) in [(1920, 1080), (3840, 2160)] {
            for mode in [ScreenAntiAliasing::Raw, ScreenAntiAliasing::Smooth2x] {
                if selected_case
                    .as_ref()
                    .is_some_and(|selected| selected != &format!("{width}-{mode:?}"))
                {
                    continue;
                }
                capture(
                    &adapter,
                    Extent2D::new(width, height).unwrap(),
                    mode,
                    &directory.join(format!("gpu-{index}-{width}-{mode:?}")),
                    seconds,
                );
                count += 1;
            }
        }
    }
    assert!(count > 0);
    done.send(()).unwrap();
}
