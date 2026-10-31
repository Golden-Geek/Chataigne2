use super::super::*;
use crate::native::{InputLimits, OutputBinding, adapters};
use crate::{
    AlphaMode, ColorEncoding, Extent2D, Opacity, PixelFormat,
    plan::{InterpolationCurve, Layer, OpacityEnvelope, PlanLimits, PlanOutput, RenderPass, RenderPlan},
};
use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires an explicitly available hardware D3D11 adapter and Spout output"]
fn hardware_shared_time_fades_preserve_premultiplied_pixels_group_opacity_and_one_plan() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(2, 2).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let texture = |id| TextureId::new(id).unwrap();
    for curve in [InterpolationCurve::Linear, InterpolationCurve::SmoothStep] {
        let mut authority =
            TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
        let anchor = authority.sample().unwrap();
        let envelope = |from, to| {
            Some(
                OpacityEnvelope::new(
                    anchor,
                    Duration::from_secs(1),
                    Opacity::new(from).unwrap(),
                    Opacity::new(to).unwrap(),
                    curve,
                )
                .unwrap(),
            )
        };
        let layer = |source, opacity, opacity_envelope, blend| Layer {
            source: texture(source).into(),
            opacity: Opacity::new(opacity).unwrap(),
            opacity_envelope,
            blend,
            filter: SampleFilter::Nearest,
            mapping: None,
        };
        let plan = RenderPlan {
            external_textures: vec![],
            passes: vec![
                RenderPass {
                    id: PassId::new(1).unwrap(),
                    output: texture(1),
                    descriptor,
                    operation: RenderOperation::Clear(LinearColor::from_straight(1., 0., 0., 0.5).unwrap()),
                },
                RenderPass {
                    id: PassId::new(2).unwrap(),
                    output: texture(2),
                    descriptor,
                    operation: RenderOperation::Clear(LinearColor::from_straight(0., 1., 0., 0.25).unwrap()),
                },
                RenderPass {
                    id: PassId::new(3).unwrap(),
                    output: texture(3),
                    descriptor,
                    operation: RenderOperation::Composite {
                        layers: vec![
                            layer(1, 1., envelope(1., 0.), BlendMode::Add),
                            layer(2, 1., envelope(0., 1.), BlendMode::Add),
                        ],
                    },
                },
                RenderPass {
                    id: PassId::new(4).unwrap(),
                    output: texture(4),
                    descriptor,
                    operation: RenderOperation::Composite {
                        layers: vec![layer(3, 0.4, None, BlendMode::Normal)],
                    },
                },
            ],
            outputs: vec![
                PlanOutput {
                    id: OutputId::new(1).unwrap(),
                    source: texture(3),
                },
                PlanOutput {
                    id: OutputId::new(2).unwrap(),
                    source: texture(4),
                },
            ],
        }
        .compile(&PlanLimits {
            max_passes: 4,
            max_external_textures: 0,
            max_outputs: 2,
            max_dependencies: 3,
            max_texture_dimension: 2,
            max_texture_bytes: 64,
        })
        .unwrap();
        let device = DeviceGeneration::new(uuid::Uuid::new_v4(), 1);
        let mut backend = D3d11Backend::new(
            D3d11Config {
                adapter: adapter.id,
                diagnostics: None,
                inputs: InputLimits {
                    max_sources: 0,
                    max_storage_bytes: 0,
                },
                outputs: (1..=2)
                    .map(|id| OutputBinding::Spout {
                        id: OutputId::new(id).unwrap(),
                        name: format!("GoldenFade-{}-{id}", uuid::Uuid::new_v4()),
                        descriptor,
                    })
                    .collect(),
            },
            Default::default(),
            device,
        );
        backend.initialize().unwrap();
        let resources = backend
            .prepare(&plan, super::generation(&backend, &plan, device))
            .unwrap();
        let created = backend.control.snapshot().created_textures;
        for (index, millis) in [0, 250, 500, 750, 1000, 1500].into_iter().enumerate() {
            let time = anchor
                .at_monotonic(anchor.monotonic().checked_add(Duration::from_millis(millis)).unwrap())
                .unwrap();
            let serial = index as u64 + 1;
            backend
                .submit(&resources, &plan, CompletionToken { device, serial }, time)
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while backend.completion().unwrap().serial < serial {
                if Instant::now() >= deadline {
                    std::mem::forget(resources);
                    panic!("fade GPU completion deadline");
                }
                backend.wait_for_completion(Duration::from_millis(1)).unwrap();
            }
            let t = (millis as f64 / 1000.0).min(1.0);
            let t = match curve {
                InterpolationCurve::Linear => t,
                InterpolationCurve::SmoothStep => t * t * (3.0 - 2.0 * t),
            };
            for (target, factor) in [(3, 1.0), (4, 0.4)] {
                let expected = [
                    0.5 * (1.0 - t) * factor,
                    0.25 * t * factor,
                    0.0,
                    (0.5 * (1.0 - t) + 0.25 * t) * factor,
                ]
                .map(|v| (v * 255.0).round() as u8);
                let pixels = crate::native::tests::pixels::readback(
                    &backend.native.as_ref().unwrap().device,
                    &resources.textures[&texture(target)],
                );
                assert!(
                    pixels.iter().all(|pixel| pixel
                        .iter()
                        .zip(expected)
                        .all(|(a, b)| (i16::from(*a) - i16::from(b)).abs() <= 2)),
                    "{curve:?} at {millis}ms target {target}: {pixels:?} expected {expected:?}"
                );
            }
            assert_eq!(backend.control.snapshot().created_textures, created);
        }
        assert!(backend.control.sources().is_empty());
        drop(resources);
    }
}
