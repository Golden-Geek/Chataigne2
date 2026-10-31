#![cfg(all(windows, feature = "native-qualification"))]
//! Fixed M04 Solid Color acceptance through authored app channels and independent native consumers.
use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};
use golden_io::PayloadPool;
use golden_video::native::{AdapterId, D3d11Config, InputLimits, OutputBinding, SpoutProbe, adapters};
use golden_video::plan::PlanLimits;
use golden_video::runtime::{OutcomeKind, ResourceLimits, RuntimeConfig, VideoRuntime};
use golden_video::{AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor};
use mapgyver::app::{AppEngine, AppNode};
use mapgyver::media::{
    capture_channels, channel,
    protocol::*,
    rendering::{ChannelRenderer, ChannelRoute},
};
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

const WAIT: Duration = Duration::from_secs(8);
const MIB: usize = 1024 * 1024;

fn edit(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

fn configure(engine: &mut AppEngine, reference: ChannelReference, size: SizePolicy, rgba: [f32; 4]) {
    let mut definition = channel(engine, reference).unwrap().definition;
    definition.size = size;
    definition.source = ChannelSource::SolidColor { rgba };
    edit(engine, MediaEdit::ConfigureChannel { reference, definition });
}

struct OwnedRenderer(Option<ChannelRenderer>);
impl Drop for OwnedRenderer {
    fn drop(&mut self) {
        if let Some(mut renderer) = self.0.take() {
            let retired = renderer.retirement();
            renderer.close();
            drop(renderer);
            while !retired.wait(Duration::from_millis(20)) {}
        }
    }
}

fn renderer(adapter: AdapterId, extents: [Extent2D; 2], names: &[String; 3]) -> OwnedRenderer {
    let limits = PlanLimits {
        max_passes: 4,
        max_external_textures: 0,
        max_outputs: 3,
        max_dependencies: 16,
        max_texture_dimension: 4096,
        max_texture_bytes: 64 * MIB as u64,
    };
    let config = RuntimeConfig {
        plan_limits: limits,
        resource_limits: ResourceLimits {
            max_bytes: 64 * MIB as u64,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: golden_time::RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(16).unwrap(),
        max_input_bytes: NonZeroUsize::new(65536).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(262144).unwrap(),
        recovery_delay: Duration::from_millis(50),
        max_recovery_delay: Duration::from_secs(1),
        notifier: None,
    };
    let outputs = names
        .iter()
        .zip([extents[0], extents[0], extents[1]])
        .enumerate()
        .map(|(index, (name, extent))| OutputBinding::Spout {
            id: OutputId::new(index as u64 + 1).unwrap(),
            name: name.clone(),
            descriptor: TextureDescriptor {
                extent,
                format: PixelFormat::Bgra8Unorm,
                color_encoding: ColorEncoding::Srgb,
                alpha_mode: AlphaMode::Straight,
            },
        })
        .collect();
    let renderer = ChannelRenderer::new(
        VideoRuntime::spawn_d3d11(
            config,
            D3d11Config {
                diagnostics: None,
                adapter,
                inputs: InputLimits {
                    max_sources: 0,
                    max_storage_bytes: 0,
                },
                outputs,
            },
        )
        .unwrap(),
        limits,
    );
    let deadline = Instant::now() + WAIT;
    while renderer.snapshot().resources.is_none() {
        assert!(Instant::now() < deadline, "native color renderer startup timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    OwnedRenderer(Some(renderer))
}

fn publish(
    renderer: &mut ChannelRenderer,
    engine: &AppEngine,
    references: [ChannelReference; 2],
    extents: [Extent2D; 2],
) {
    let request = renderer
        .publish(
            capture_channels(engine, &references, 4).unwrap(),
            [references[0], references[0], references[1]]
                .into_iter()
                .enumerate()
                .map(|(index, channel)| ChannelRoute {
                    output: OutputId::new(index as u64 + 1).unwrap(),
                    channel,
                })
                .collect(),
        )
        .unwrap();
    let deadline = Instant::now() + WAIT;
    let mut installed = None;
    let mut first_frame = None;
    loop {
        renderer.refresh().unwrap();
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        for outcome in outcomes {
            match outcome.result {
                OutcomeKind::Installed { generation, .. } if outcome.request == request => {
                    installed = Some(generation.identity)
                }
                OutcomeKind::Rejected(error) if outcome.request == request => {
                    panic!("native Solid Color plan rejected: {error:?}")
                }
                _ => {}
            }
        }
        if installed.is_some()
            && let Some(frame) = renderer.snapshot().last_frame
            && Some(frame.identity) == installed
        {
            let token = *first_frame.get_or_insert(frame.completion);
            let outputs = renderer.native_control().unwrap().output_snapshots();
            if outputs.len() == 3
                && outputs.iter().all(|output| {
                    output
                        .last_accepted
                        .is_some_and(|accepted| accepted.device == token.device && accepted.serial >= token.serial)
                })
            {
                break;
            }
        }
        assert!(Instant::now() < deadline, "Solid Color publication timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        renderer.channels().len(),
        2,
        "three routes share exactly two authored channel productions"
    );
    for (channel, extent) in renderer.channels().iter().zip(extents) {
        assert_eq!(channel.resolution.extent, Some(extent));
    }
    assert_ne!(renderer.channels()[0].target, renderer.channels()[1].target);
    assert!(renderer.native_control().unwrap().sources().is_empty());
    assert_eq!(renderer.native_control().unwrap().snapshot().uploaded_images, 0);
}

fn expected(rgba: [f32; 4]) -> [u8; 4] {
    // Independent sRGB specification oracle; alpha is linear coverage, source RGBA is straight linear.
    let encode = |linear: f32| {
        let value = if linear <= 0.0031308 {
            12.92 * linear
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (value * 255.0).round() as u8
    };
    let rgb = if rgba[3] == 0.0 {
        [0.0; 3]
    } else {
        [rgba[0], rgba[1], rgba[2]]
    };
    [
        encode(rgb[0]),
        encode(rgb[1]),
        encode(rgb[2]),
        (rgba[3] * 255.0).round() as u8,
    ]
}

fn pixels(adapter: AdapterId, names: &[String; 3], extents: [Extent2D; 2], colors: [[f32; 4]; 2]) {
    let budget = PayloadPool::new("solid-color-pixel-budget", 9, 64 * MIB).unwrap();
    let mut probes = names
        .iter()
        .map(|name| SpoutProbe::new(name, adapter, budget.clone()).unwrap())
        .collect::<Vec<_>>();
    for (index, probe) in probes.iter_mut().enumerate() {
        let deadline = Instant::now() + WAIT;
        let image = loop {
            if let Some(image) = probe.capture_pixels().unwrap() {
                break image;
            }
            assert!(
                Instant::now() < deadline,
                "native color receive/readback timed out: {index}"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        let channel = usize::from(index == 2);
        assert_eq!(image.extent(), extents[channel]);
        assert_eq!(
            image.pixels().len(),
            (extents[channel].width() * extents[channel].height()) as usize
        );
        let expected = expected(colors[channel]);
        assert!(
            image.pixels().iter().all(|pixel| pixel
                .iter()
                .zip(expected)
                .all(|(actual, wanted)| actual.abs_diff(wanted) <= 2)),
            "native output {index}: sample {:?}, expected {expected:?} within 2/255",
            image.pixels()[0]
        );
        eprintln!(
            "color output={index} extent={:?} pixels={} sample={:?} expected={expected:?} tolerance=2/255",
            image.extent(),
            image.pixels().len(),
            image.pixels()[0]
        );
    }
    drop(probes);
    assert!(budget.wait_for_idle(Duration::from_secs(1)));
}

#[test]
#[ignore = "explicit M04 hardware color/alpha/extent/shared-consumer qualification"]
fn authored_solid_color_channels_qualify_pixels_sizing_sharing_and_recreation_on_hardware() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if completion.recv_timeout(Duration::from_secs(90)).is_err() {
            eprintln!("Solid Color qualification/retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let MediaEditResult::Created { reference: first } = edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::SolidColor,
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
        let automatic = mapgyver::media::resolution::default_canvas();
        let fixed = Extent2D::new(7, 3).unwrap();
        let initial = [[0.25, 0.5, 0.75, 0.5], [0.75, 0.5, 0.25, 0.25]];
        configure(&mut engine, first, SizePolicy::Automatic, initial[0]);
        configure(&mut engine, second, SizePolicy::Fixed { extent: fixed }, initial[1]);
        let names = std::array::from_fn(|index| format!("MapGyverSolid-{}-{index}", uuid::Uuid::new_v4()));
        let mut owned = renderer(adapter.id, [automatic, fixed], &names);
        publish(owned.0.as_mut().unwrap(), &engine, references, [automatic, fixed]);
        pixels(adapter.id, &names, [automatic, fixed], initial);
        let colors = [[0.9, 0.6, 0.3, 0.0], [0.25, 0.5, 0.75, 1.0]];
        configure(&mut engine, first, SizePolicy::Automatic, colors[0]);
        configure(&mut engine, second, SizePolicy::Fixed { extent: fixed }, colors[1]);
        publish(owned.0.as_mut().unwrap(), &engine, references, [automatic, fixed]);
        pixels(adapter.id, &names, [automatic, fixed], colors);
        drop(owned);
        configure(&mut engine, first, SizePolicy::Fixed { extent: fixed }, initial[0]);
        configure(&mut engine, second, SizePolicy::Automatic, initial[1]);
        let document = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
        let mut restored = golden_core::app::from_sparse_project_json::<AppNode>(&document).unwrap();
        restored.apply_edits().unwrap();
        let names = std::array::from_fn(|index| format!("MapGyverSolidReload-{}-{index}", uuid::Uuid::new_v4()));
        let mut recreated = renderer(adapter.id, [fixed, automatic], &names);
        publish(recreated.0.as_mut().unwrap(), &restored, references, [fixed, automatic]);
        pixels(adapter.id, &names, [fixed, automatic], initial);
        drop(recreated);
        eprintln!(
            "M04 Solid Color qualified adapter={:?} name={}",
            adapter.id, adapter.description
        );
    }
    done.send(()).unwrap();
}
