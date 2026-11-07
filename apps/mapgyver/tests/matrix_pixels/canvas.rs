use super::*;
use golden_io::PayloadPool;
use golden_video::{native::SpoutProbe, preview::protocol::CssRect};

fn pixels(fixture: &Fixture, adapter: AdapterId, output: usize) -> Vec<[u8; 4]> {
    // Receiving texture, staging image and CPU observation each hold one bounded credit.
    let budget = PayloadPool::new("canvas-readback", 3, 1024 * 1024).unwrap();
    let mut probe = SpoutProbe::new(fixture.sender(output), adapter, budget.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let image = loop {
        if let Some(image) = probe.capture_pixels().unwrap() {
            break image;
        }
        assert!(Instant::now() < deadline, "canvas native readback timeout");
        std::thread::sleep(Duration::from_millis(5));
    };
    image.pixels().to_vec()
}

fn encoded_linear_byte(value: u8) -> u8 {
    // The existing preview target is linear RGBA8: include its explicit quantization.
    let srgb = f64::from(value) / 255.;
    let linear = if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    };
    let linear = (linear * 255.).round() / 255.;
    let encoded = if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1. / 2.4) - 0.055
    };
    (encoded * 255.).round() as u8
}

#[test]
#[ignore = "requires explicit hardware D3D11 and native Spout output availability"]
fn native_canvas_surround_tracks_content_geometry_theme_and_alpha_on_both_gpus() {
    let adapters = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|a| !a.software)
        .collect::<Vec<_>>();
    assert_eq!(
        adapters.len(),
        2,
        "qualification expects both installed hardware adapters"
    );
    for adapter in adapters {
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let source = color(&mut engine, [1., 0., 0., 0.5]);
        let mut fixture = Fixture::new(adapter.id);
        let mut baseline = None;
        for (index, (x, y, width, height, theme)) in [
            (8., 6., 16., 12., [31, 32, 33, 255]),
            (2., 4., 24., 16., [72, 74, 80, 255]),
            (-8., -6., 24., 18., [120, 122, 124, 255]),
            (20., 16., 24., 18., [31, 32, 33, 255]),
        ]
        .into_iter()
        .enumerate()
        {
            fixture.set_canvas(CssRect { x, y, width, height }, theme);
            fixture.publish_channels(&engine, &[source, source], source, None);
            let output = pixels(&fixture, adapter.id, 0);
            if let Some(baseline) = &baseline {
                assert_eq!(
                    &output, baseline,
                    "canvas geometry/theme must not change authored output"
                );
            } else {
                baseline = Some(output);
            }
            let preview = pixels(&fixture, adapter.id, 2);
            let background = [
                encoded_linear_byte(theme[0]),
                encoded_linear_byte(theme[1]),
                encoded_linear_byte(theme[2]),
                255,
            ];
            for py in 0..24 {
                for px in 0..32 {
                    let inside = f64::from(px) + 0.5 >= x
                        && f64::from(px) + 0.5 < x + width
                        && f64::from(py) + 0.5 >= y
                        && f64::from(py) + 0.5 < y + height;
                    let expected = if inside { [188, 0, 0, 255] } else { background };
                    let actual = preview[py as usize * 32 + px as usize];
                    assert!(
                        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                        "canvas {index} at {px},{py}: {actual:?} expected {expected:?}"
                    );
                }
            }
            eprintln!(
                "Canvas adapter={:?} case={index} complete_texels={} output_unchanged=true",
                adapter.id,
                preview.len()
            );
        }
    }
}
