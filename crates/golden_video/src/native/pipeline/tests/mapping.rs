use super::*;
use crate::plan::{Layer, Point, ProjectiveMapping, Quad};
use crate::{Opacity, TextureId};

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_pipeline_projective_crop_matches_independent_texel_and_alpha_oracles() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let gpu = Device::new(adapter.id).unwrap();
    let pipeline = Pipeline::new(&gpu.device).unwrap();
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(8, 8).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let pixels = (0..8)
        .flat_map(|y| (0..8).flat_map(move |x| [x * 30, y * 30, 0, 255]))
        .collect();
    let image = crate::image::ImageData::new(descriptor, pixels).unwrap();
    let source = Texture::from_image(&gpu.device, &image).unwrap();
    let previous = Texture::new(&gpu.device, descriptor).unwrap();
    let target = Texture::new(&gpu.device, descriptor).unwrap();
    pipeline.clear(
        &gpu.context,
        &previous,
        LinearColor::from_straight(0.0, 0.0, 1.0, 0.5).unwrap(),
    );
    let crop = UnitRect {
        x: 0.25,
        y: 0.25,
        width: 0.5,
        height: 0.5,
    };
    let quads = [
        Quad::default(),
        Quad {
            corners: [
                Point { x: 0.25, y: 0.125 },
                Point { x: 1.0, y: 0.125 },
                Point {
                    x: 2.0 / 3.0,
                    y: 2.0 / 3.0,
                },
                Point {
                    x: 1.0 / 6.0,
                    y: 2.0 / 3.0,
                },
            ],
        },
        Quad {
            corners: [
                Point { x: 1.5, y: -0.5 },
                Point { x: -0.5, y: -0.5 },
                Point { x: -0.5, y: 1.5 },
                Point { x: 1.5, y: 1.5 },
            ],
        },
    ];
    for (case, quad) in quads.into_iter().enumerate() {
        let layer = Layer {
            source: TextureId::new(1).unwrap(),
            opacity: Opacity::new(0.5).unwrap(),
            blend: BlendMode::Normal,
            filter: SampleFilter::Nearest,
            mapping: Some(ProjectiveMapping::new(crop, quad).unwrap()),
        };
        pipeline.draw_layer(&gpu.context, &source, Some(&previous), (&target).into(), &layer);
        let actual = crate::native::tests::pixels::readback(&gpu, &target);
        let mut checked = 0;
        for (index, pixel) in actual.into_iter().enumerate() {
            let x = (index % 8) as f64 / 8.0 + 1.0 / 16.0;
            let y = (index / 8) as f64 / 8.0 + 1.0 / 16.0;
            // Independent analytic inverses, not the production homography implementation.
            let (u, v) = match case {
                0 => (x, y),
                1 => {
                    let v = (y - 0.125) / (0.875 - 0.5 * y);
                    ((x * (1.0 + 0.5 * v) - 0.25) / 0.75, v)
                }
                _ => ((1.5 - x) / 2.0, (y + 0.5) / 2.0),
            };
            let inside = (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v);
            let tx = (0.25 + u * 0.5) * 8.0;
            let ty = (0.25 + v * 0.5) * 8.0;
            // Texel/coverage boundaries have two legitimate rounding choices at f32 precision.
            if [u, v].iter().any(|v| v.abs() < 1e-5 || (v - 1.0).abs() < 1e-5)
                || (inside && [tx, ty].iter().any(|v| (v - v.round()).abs() < 1e-5))
            {
                continue;
            }
            let expected = if inside {
                [tx.floor() as u8 * 15, ty.floor() as u8 * 15, 64, 191]
            } else {
                [0, 0, 128, 128]
            };
            assert!(
                pixel
                    .into_iter()
                    .zip(expected)
                    .all(|(a, b)| (i16::from(a) - i16::from(b)).abs() <= 2),
                "mapping case {case} pixel {index}: {pixel:?} != {expected:?}"
            );
            checked += 1;
        }
        assert!(checked >= 48, "insufficient independent reference coverage: {checked}");
        eprintln!("projective case {case}: {checked}/64 independently checked pixels, tolerance 2/255");
    }
}
