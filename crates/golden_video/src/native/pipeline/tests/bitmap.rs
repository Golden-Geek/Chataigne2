use super::*;
use crate::{
    Opacity,
    image::ImageData,
    plan::{CanvasPaint, CanvasPattern, CanvasPrimitive, Layer, LayerSource, Pattern, Point, Quad},
};
use std::sync::Arc;

fn render(
    gpu: &Device,
    pipeline: &Pipeline,
    target: &Texture,
    canvas: CanvasPattern,
    filter: SampleFilter,
    mapping: Option<ProjectiveMapping>,
) -> Vec<[u8; 4]> {
    let source = Texture::from_canvas_image(&gpu.device, canvas.image().unwrap()).unwrap();
    pipeline.draw_layer(
        &gpu.context,
        Some(&source),
        None,
        target.into(),
        &Layer {
            source: LayerSource::Pattern(Pattern::Canvas(canvas)),
            opacity: Opacity::OPAQUE,
            opacity_envelope: None,
            blend: BlendMode::Normal,
            filter,
            mapping,
        },
    );
    crate::native::tests::pixels::readback(gpu, target)
}
fn close(actual: [u8; 4], expected: [u8; 4]) {
    assert!(
        actual.into_iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
        "{actual:?} != {expected:?}"
    );
}

#[test]
#[ignore = "requires explicitly available hardware D3D11 adapters"]
fn hardware_canvas_bitmaps_preserve_alpha_color_filtering_opacity_and_mapping() {
    let adapters = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!adapters.is_empty());
    for adapter in adapters {
        let gpu = Device::new(adapter.id).unwrap();
        let pipeline = Pipeline::new(&gpu.device).unwrap();
        let target = Texture::new(
            &gpu.device,
            TextureDescriptor {
                extent: Extent2D::new(8, 8).unwrap(),
                format: PixelFormat::Rgba8Unorm,
                color_encoding: ColorEncoding::Linear,
                alpha_mode: AlphaMode::Premultiplied,
            },
        )
        .unwrap();
        let image = Arc::new(
            ImageData::new(
                TextureDescriptor {
                    extent: Extent2D::new(2, 2).unwrap(),
                    format: PixelFormat::Rgba8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: AlphaMode::Straight,
                },
                vec![255, 0, 0, 0, 0, 255, 0, 128, 128, 128, 128, 255, 255, 255, 255, 255],
            )
            .unwrap(),
        );
        let bounds = UnitRect {
            x: 0.25,
            y: 0.25,
            width: 0.5,
            height: 0.5,
        };
        let scene = |opacity| {
            CanvasPattern::with_image(
                vec![
                    CanvasPrimitive::rectangle(
                        UnitRect::FULL,
                        CanvasPaint::solid(LinearColor::from_straight(0., 0., 1., 1.).unwrap()),
                    )
                    .unwrap(),
                    CanvasPrimitive::image(bounds, opacity).unwrap(),
                ],
                image.clone(),
            )
            .unwrap()
        };
        let pixels = render(
            &gpu,
            &pipeline,
            &target,
            scene(Opacity::OPAQUE),
            SampleFilter::Nearest,
            None,
        );
        for (x, y, value) in [
            (0, 0, [0, 0, 255, 255]),
            (2, 2, [0, 0, 255, 255]),
            (4, 2, [0, 128, 127, 255]),
            (2, 4, [55, 55, 55, 255]),
            (4, 4, [255; 4]),
        ] {
            close(pixels[y * 8 + x], value);
        }
        let pixels = render(
            &gpu,
            &pipeline,
            &target,
            scene(Opacity::OPAQUE),
            SampleFilter::Linear,
            None,
        );
        close(pixels[3 * 8 + 3], [26, 50, 194, 255]);
        let pixels = render(
            &gpu,
            &pipeline,
            &target,
            scene(Opacity::new(0.5).unwrap()),
            SampleFilter::Nearest,
            None,
        );
        close(pixels[4 * 8 + 4], [128, 128, 255, 255]);
        let mapping = ProjectiveMapping::new(
            UnitRect::FULL,
            Quad {
                corners: [
                    Point { x: 0.125, y: 0.125 },
                    Point { x: 0.875, y: 0.125 },
                    Point { x: 0.875, y: 0.875 },
                    Point { x: 0.125, y: 0.875 },
                ],
            },
        )
        .unwrap();
        let pixels = render(
            &gpu,
            &pipeline,
            &target,
            scene(Opacity::OPAQUE),
            SampleFilter::Nearest,
            Some(mapping),
        );
        close(pixels[0], [0; 4]);
        close(pixels[8 + 1], [0, 0, 255, 255]);
        close(pixels[2 * 8 + 4], [0, 128, 127, 255]);
        eprintln!(
            "canvas bitmap {:?} {}: alpha, sRGB-before-filter, placement, opacity and mapped pixels pass",
            adapter.id, adapter.description
        );
    }
}
