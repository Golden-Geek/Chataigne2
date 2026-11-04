use super::*;
use crate::{
    Opacity,
    plan::{CanvasPaint, CanvasPattern, CanvasPrimitive, Layer, LayerSource, Pattern},
};
use windows::Win32::Graphics::Direct3D11::D3D11_BOX;

fn color(rgb: [f32; 3]) -> LinearColor {
    LinearColor::from_straight(rgb[0], rgb[1], rgb[2], 1.).unwrap()
}

fn check(gpu: &Device, pipeline: &Pipeline, target: &Texture, canvas: CanvasPattern, samples: &[([u32; 2], [u8; 4])]) {
    assert!(!samples.is_empty() && samples.len() <= 64);
    let layer = Layer {
        source: LayerSource::Pattern(Pattern::Canvas(canvas)),
        opacity: Opacity::OPAQUE,
        opacity_envelope: None,
        blend: BlendMode::Normal,
        filter: SampleFilter::Nearest,
        mapping: None,
    };
    pipeline.draw_layer(&gpu.context, None, None, target.into(), &layer);
    // Gather explicit landmarks into the existing finite 8x8 readback contract.
    let staging_source = Texture::new(
        &gpu.device,
        TextureDescriptor {
            extent: Extent2D::new(8, 8).unwrap(),
            ..target.descriptor
        },
    )
    .unwrap();
    for (index, ([x, y], _)) in samples.iter().enumerate() {
        assert!(*x < target.descriptor.extent.width() && *y < target.descriptor.extent.height());
        let area = D3D11_BOX {
            left: *x,
            top: *y,
            right: *x + 1,
            bottom: *y + 1,
            front: 0,
            back: 1,
        };
        // SAFETY: matching formats/device and checked one-pixel source/destination rectangles.
        unsafe {
            gpu.context.CopySubresourceRegion(
                &staging_source.texture,
                0,
                index as u32 % 8,
                index as u32 / 8,
                0,
                &target.texture,
                0,
                Some(&area),
            );
        }
    }
    let pixels = crate::native::tests::pixels::readback(gpu, &staging_source);
    for (actual, (point, expected)) in pixels.into_iter().zip(samples) {
        assert!(
            actual.into_iter().zip(*expected).all(|(a, b)| a.abs_diff(b) <= 1),
            "at {point:?}: {actual:?} != {expected:?}"
        );
    }
}

#[test]
#[ignore = "requires explicitly available hardware D3D11 adapters"]
fn hardware_batched_canvas_geometry_palettes_and_shared_clock_match_landmarks() {
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let gpu = Device::new(adapter.id).unwrap();
        let pipeline = Pipeline::new(&gpu.device).unwrap();
        for [width, height] in [[1920, 1080], [1080, 1920], [1024, 1024], [3840, 2160]] {
            let target = Texture::new(
                &gpu.device,
                TextureDescriptor {
                    extent: Extent2D::new(width, height).unwrap(),
                    format: PixelFormat::Rgba8Unorm,
                    color_encoding: ColorEncoding::Linear,
                    alpha_mode: AlphaMode::Premultiplied,
                },
            )
            .unwrap();
            let [w, h] = [width as f32, height as f32];
            let diameter = w.min(h);
            let cell = diameter / 9.;
            let white = CanvasPaint::solid(color([1.; 3]));
            let palette_rgb = [
                [1., 1., 1.],
                [1., 1., 0.],
                [0., 1., 1.],
                [0., 1., 0.],
                [1., 0., 1.],
                [1., 0., 0.],
                [0., 0., 1.],
                [0., 0., 0.],
            ];
            let checker = CanvasPrimitive::checkerboard(
                UnitRect::FULL,
                [w / cell, h / cell],
                [color([0.25; 3]), color([0.5; 3])],
            )
            .unwrap();
            let scene = CanvasPattern::new(vec![
                checker,
                CanvasPrimitive::bars(
                    UnitRect {
                        x: 0.,
                        y: cell / h,
                        width: 1.,
                        height: cell / h,
                    },
                    std::array::from_fn(|index| color([index as f32 / 7.; 3])),
                )
                .unwrap(),
                CanvasPrimitive::bars(
                    UnitRect {
                        x: 0.,
                        y: 1. - 2. * cell / h,
                        width: 1.,
                        height: cell / h,
                    },
                    palette_rgb.map(color),
                )
                .unwrap(),
            ])
            .unwrap();
            let mut samples = vec![
                ([(0.5 * cell) as u32, (0.5 * cell) as u32], [64, 64, 64, 255]),
                ([(1.5 * cell) as u32, (0.5 * cell) as u32], [128, 128, 128, 255]),
            ];
            for (index, rgb) in palette_rgb.into_iter().enumerate() {
                let x = ((index as f32 + 0.5) * w / 8.) as u32;
                let grey = (index as f32 * 255. / 7.).round() as u8;
                samples.push(([x, (1.5 * cell) as u32], [grey, grey, grey, 255]));
                samples.push((
                    [x, (h - 1.5 * cell) as u32],
                    [(rgb[0] * 255.) as u8, (rgb[1] * 255.) as u8, (rgb[2] * 255.) as u8, 255],
                ));
            }
            check(&gpu, &pipeline, &target, scene, &samples);
            let circle = UnitRect {
                x: (w - diameter) / (2. * w),
                y: (h - diameter) / (2. * h),
                width: diameter / w,
                height: diameter / h,
            };
            let mut samples = [
                [((w - diameter) / 2.) as u32, height / 2],
                [((w + diameter) / 2.) as u32 - 1, height / 2],
                [width / 2, ((h - diameter) / 2.) as u32],
                [width / 2, ((h + diameter) / 2.) as u32 - 1],
            ]
            .map(|point| (point, [255; 4]))
            .to_vec();
            samples.push(([width / 2, height / 2], [0; 4]));
            check(
                &gpu,
                &pipeline,
                &target,
                CanvasPattern::new(vec![CanvasPrimitive::ellipse(circle, 1.5 / diameter, white).unwrap()]).unwrap(),
                &samples,
            );
            let mut samples = [
                [0, 0],
                [width - 1, 0],
                [0, height - 1],
                [width - 1, height - 1],
                [width / 2, height / 2],
            ]
            .map(|point| (point, [255; 4]))
            .to_vec();
            samples.push(([width / 2, height / 4], [0; 4]));
            check(
                &gpu,
                &pipeline,
                &target,
                CanvasPattern::new(vec![
                    CanvasPrimitive::line(UnitRect::FULL, [0., 0.], [1., 1.], 0.75 / h, w / h, white).unwrap(),
                    CanvasPrimitive::line(UnitRect::FULL, [0., 1.], [1., 0.], 0.75 / h, w / h, white).unwrap(),
                ])
                .unwrap(),
                &samples,
            );
            let paint = CanvasPaint::rainbow(Duration::from_secs(6), [0., 0.], Opacity::OPAQUE).unwrap();
            let mut scene = CanvasPattern::new(vec![
                CanvasPrimitive::grid(UnitRect::FULL, [4., 4.], [0.01, 0.01], paint).unwrap(),
            ])
            .unwrap();
            check(&gpu, &pipeline, &target, scene.clone(), &[([0, 0], [255, 0, 0, 255])]);
            scene.monotonic_nanos = 3_000_000_000;
            check(&gpu, &pipeline, &target, scene, &[([0, 0], [0, 255, 255, 255])]);
            eprintln!(
                "canvas hardware {:?} {}: {}x{} checker/palette/circle/diagonal/clock landmarks pass",
                adapter.id, adapter.description, width, height
            );
        }
    }
}
