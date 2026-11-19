use super::*;
use crate::native::tests::pixels::readback;

#[test]
#[ignore = "requires explicitly available Windows hardware D3D11"]
fn hardware_straight_srgb_output_matches_generic_color_and_alpha() {
    let extent = Extent2D::new(8, 8).unwrap();
    let descriptor = TextureDescriptor {
        extent,
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let levels = [0, 1, 5, 17, 69, 113, 210, 255];
    let pixels = levels
        .into_iter()
        .flat_map(|alpha| {
            (0..levels.len()).flat_map(move |x| {
                let channel =
                    |offset| ((u16::from(levels[(x + offset) % levels.len()]) * u16::from(alpha)) / 255) as u8;
                [channel(0), channel(3), channel(5), alpha]
            })
        })
        .collect();
    let image = crate::image::ImageData::new(descriptor, pixels).unwrap();
    for adapter in adapters().unwrap().into_iter().filter(|adapter| !adapter.software) {
        let gpu = Device::new(adapter.id).unwrap();
        let source = Texture::from_image(&gpu.device, &image).unwrap();
        let mut pipeline = Pipeline::new(&gpu.device).unwrap();
        let specialized = pipeline.straight_srgb_pixel.clone();
        for format in [PixelFormat::Rgba8Unorm, PixelFormat::Bgra8Unorm] {
            let target = Texture::new(
                &gpu.device,
                TextureDescriptor {
                    format,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: AlphaMode::Straight,
                    ..descriptor
                },
            )
            .unwrap();
            let draw = |pipeline: &Pipeline| {
                pipeline.draw(
                    &gpu.context,
                    &source,
                    None,
                    (&target).into(),
                    1.0,
                    BlendMode::Normal,
                    SampleFilter::Nearest,
                );
                readback(&gpu, &target)
            };
            pipeline.straight_srgb_pixel = pipeline.pixel.clone();
            let expected = draw(&pipeline);
            pipeline.straight_srgb_pixel = specialized.clone();
            let actual = draw(&pipeline);
            assert!(
                actual == expected,
                "{}: straight sRGB {format:?} color and alpha",
                adapter.description
            );
        }
        eprintln!("Exact straight sRGB output on {}", adapter.description);
    }
}
