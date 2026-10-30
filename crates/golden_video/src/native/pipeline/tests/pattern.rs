use super::*;
use crate::{
    Opacity,
    plan::{GridPattern, Layer, LayerSource, Pattern, Point, ProjectiveMapping, Quad},
};

fn color(r: f32, g: f32, b: f32) -> LinearColor {
    LinearColor::from_straight(r, g, b, 1.0).unwrap()
}
fn layer(pattern: Pattern, opacity: f32, mapping: Option<ProjectiveMapping>) -> Layer {
    Layer {
        source: LayerSource::Pattern(pattern),
        opacity: Opacity::new(opacity).unwrap(),
        blend: BlendMode::Normal,
        filter: SampleFilter::Nearest,
        mapping,
    }
}
fn pixels(gpu: &Device, target: &Texture, expected: impl Fn(usize, usize) -> [u8; 4]) {
    for (index, pixel) in crate::native::tests::pixels::readback(gpu, target)
        .into_iter()
        .enumerate()
    {
        let value = expected(index % 8, index / 8);
        assert!(
            pixel
                .into_iter()
                .zip(value)
                .all(|(a, b)| (i16::from(a) - i16::from(b)).abs() <= 2),
            "pattern pixel {index}: {pixel:?} != {value:?}"
        );
    }
}

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_procedural_patterns_mapping_order_and_alpha_match_pixel_oracles() {
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
    let target = Texture::new(&gpu.device, descriptor).unwrap();
    let scratch = Texture::new(&gpu.device, descriptor).unwrap();
    let backdrop = Texture::new(&gpu.device, descriptor).unwrap();
    pipeline.clear(
        &gpu.context,
        &backdrop,
        LinearColor::from_straight(0., 0., 1., 0.5).unwrap(),
    );
    let palette = [
        [1., 1., 1.],
        [1., 1., 0.],
        [0., 1., 1.],
        [0., 1., 0.],
        [1., 0., 1.],
        [1., 0., 0.],
        [0., 0., 1.],
        [0., 0., 0.],
    ];
    let bars = Pattern::Bars(palette.map(|[r, g, b]| color(r, g, b)));
    pipeline.draw_layer(&gpu.context, None, None, (&target).into(), &layer(bars, 1.0, None));
    pixels(&gpu, &target, |x, _| {
        [
            (palette[x][0] * 255.) as u8,
            (palette[x][1] * 255.) as u8,
            (palette[x][2] * 255.) as u8,
            255,
        ]
    });
    let grid = Pattern::Grid(GridPattern::new([2, 2], 0.2, color(1., 1., 1.), color(0., 0., 0.)).unwrap());
    let cropped = ProjectiveMapping::new(
        UnitRect {
            x: 0.25,
            y: 0.,
            width: 0.5,
            height: 1.,
        },
        Quad::default(),
    )
    .unwrap();
    pipeline.draw_layer(
        &gpu.context,
        None,
        None,
        (&target).into(),
        &layer(bars, 1., Some(cropped)),
    );
    pixels(&gpu, &target, |x, _| {
        let rgb = palette[x / 2 + 2];
        [(rgb[0] * 255.) as u8, (rgb[1] * 255.) as u8, (rgb[2] * 255.) as u8, 255]
    });
    pipeline.draw_layer(&gpu.context, None, None, (&target).into(), &layer(grid, 1.0, None));
    pixels(&gpu, &target, |x, y| {
        let value = if x % 4 == 0 || x % 4 == 3 || y % 4 == 0 || y % 4 == 3 {
            255
        } else {
            0
        };
        [value, value, value, 255]
    });
    let mapping = ProjectiveMapping::new(
        UnitRect::FULL,
        Quad {
            corners: [
                Point { x: 0., y: 0. },
                Point { x: 0.5, y: 0. },
                Point { x: 0.5, y: 1. },
                Point { x: 0., y: 1. },
            ],
        },
    )
    .unwrap();
    pipeline.draw_layer(
        &gpu.context,
        None,
        Some(&backdrop),
        (&target).into(),
        &layer(bars, 0.5, Some(mapping)),
    );
    pixels(&gpu, &target, |x, _| {
        if x < 4 {
            let rgb = palette[x * 2 + 1];
            [
                (rgb[0] * 127.5).round() as u8,
                (rgb[1] * 127.5).round() as u8,
                (64. + rgb[2] * 127.5).round() as u8,
                191,
            ]
        } else {
            [0, 0, 128, 128]
        }
    });
    let red = layer(Pattern::Bars([color(1., 0., 0.); 8]), 1., None);
    let green = layer(Pattern::Bars([color(0., 1., 0.); 8]), 0.5, Some(mapping));
    pipeline.draw_layer(&gpu.context, None, None, (&scratch).into(), &red);
    pipeline.draw_layer(&gpu.context, None, Some(&scratch), (&target).into(), &green);
    pixels(
        &gpu,
        &target,
        |x, _| if x < 4 { [128, 128, 0, 255] } else { [255, 0, 0, 255] },
    );
    pipeline.draw_layer(&gpu.context, None, None, (&scratch).into(), &green);
    pipeline.draw_layer(&gpu.context, None, Some(&scratch), (&target).into(), &red);
    pixels(&gpu, &target, |_, _| [255, 0, 0, 255]);
    // Texture sampling still uses the ordinary path after procedural constant-buffer use.
    pipeline.draw(
        &gpu.context,
        &backdrop,
        None,
        (&target).into(),
        1.0,
        BlendMode::Normal,
        SampleFilter::Nearest,
    );
    pixels(&gpu, &target, |_, _| [0, 0, 128, 128]);
    eprintln!("448/448 finite pattern/crop/mapping/order/alpha/texture pixels matched, tolerance 2/255");
}
