use super::*;

fn canvas() -> Extent2D {
    Extent2D::new(400, 240).unwrap()
}

fn style() -> SubtitleStyle {
    SubtitleStyle {
        size: 0.10,
        position: [0.50, 0.85],
    }
}

fn bounds(image: &ImageData) -> [usize; 4] {
    let width = image.descriptor().extent.width() as usize;
    let mut result = [usize::MAX, usize::MAX, 0, 0];
    for (index, rgba) in image.pixels().chunks_exact(4).enumerate() {
        if rgba[3] > 0 {
            let (x, y) = (index % width, index / width);
            result[0] = result[0].min(x);
            result[1] = result[1].min(y);
            result[2] = result[2].max(x);
            result[3] = result[3].max(y);
        }
    }
    assert!(result[0] < result[2] && result[1] < result[3]);
    result
}

#[test]
fn transparent_canvas_keeps_unicode_text_size_and_normalized_placement() {
    let rasterizer = SubtitleRasterizer::new("Segoe UI");
    let text = "Résumé & café\nSecond line";
    let normal = rasterizer.rasterize(text, canvas(), style()).unwrap().unwrap();
    assert_eq!(
        normal.descriptor(),
        TextureDescriptor {
            extent: canvas(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        }
    );
    assert_eq!(&normal.pixels()[..4], [0; 4]);
    assert!(normal.pixels().chunks_exact(4).any(|rgba| rgba == [255; 4]));
    let [left, top, right, bottom] = bounds(&normal);
    assert!((left + right).abs_diff(canvas().width() as usize) < 12);
    assert!(bottom <= 205 && bottom > 175, "bottom={bottom}");
    assert!(bottom - top > 24, "both text lines render");

    let moved = rasterizer
        .rasterize(
            text,
            canvas(),
            SubtitleStyle {
                position: [0.25, 0.50],
                ..style()
            },
        )
        .unwrap()
        .unwrap();
    let [left_moved, top_moved, right_moved, bottom_moved] = bounds(&moved);
    assert!(left_moved < left && top_moved < top && right_moved < right && bottom_moved < bottom);
    assert_eq!(bottom - top, bottom_moved - top_moved);
    let smaller = rasterizer
        .rasterize(text, canvas(), SubtitleStyle { size: 0.05, ..style() })
        .unwrap()
        .unwrap();
    let [left_small, top_small, right_small, bottom_small] = bounds(&smaller);
    assert!(right_small - left_small < right - left);
    assert!(bottom_small - top_small < bottom - top);
}

#[test]
fn unknown_font_uses_the_same_fallback_and_installed_font_changes_the_witness() {
    let text = "MMMM iiii";
    let render = |name| {
        SubtitleRasterizer::new(name)
            .rasterize(text, canvas(), style())
            .unwrap()
            .unwrap()
    };
    let fallback = render("");
    assert_eq!(render("MissingSubtitleFace3072").pixels(), fallback.pixels());
    let long_family = "a".repeat(33);
    assert_eq!(render(&long_family).pixels(), fallback.pixels());
    let mut device = RasterTarget::new(1, 1, 1).unwrap();
    device.select_font(w!("Courier New"), 24).unwrap();
    if device.selected_family("courier new") {
        assert_ne!(render("Courier New").pixels(), fallback.pixels());
    }
}

#[test]
fn subtitle_raster_checks_bounds_before_native_allocation() {
    let rasterizer = SubtitleRasterizer::new("");
    assert!(rasterizer.rasterize(" \r\n  ", canvas(), style()).unwrap().is_none());
    assert!(matches!(
        rasterizer.rasterize("x\0y", canvas(), style()),
        Err(SubtitleRasterError::Text)
    ));
    assert!(matches!(
        rasterizer.rasterize(&"x".repeat(MAX_TEXT_BYTES + 1), canvas(), style()),
        Err(SubtitleRasterError::Text)
    ));
    assert!(matches!(
        rasterizer.rasterize("one", Extent2D::new(16_384, 16_384).unwrap(), style()),
        Err(SubtitleRasterError::Canvas)
    ));
    assert!(matches!(
        rasterizer.rasterize(
            "one",
            canvas(),
            SubtitleStyle {
                position: [f32::NAN, 0.5],
                ..style()
            }
        ),
        Err(SubtitleRasterError::Style)
    ));
}

#[test]
fn coverage_preserves_white_straight_rgb_and_composites_the_shadow_in_linear_light() {
    let pixels = compose_coverage(&[0, 255, 128, 128, 0, 0], 2).unwrap();
    assert_eq!(&pixels[..4], [0; 4]);
    assert_eq!(&pixels[4..8], [255; 4]);
    assert_eq!(&pixels[8..12], [255, 255, 255, 128]);
    let combined = crate::LinearColor::from_srgba8(pixels[12..16].try_into().unwrap()).components();
    assert!((combined[0] - 128.0 / 255.0).abs() < 0.005);
    assert!((combined[3] - (128.0 / 255.0 + 0.75 * (1.0 - 128.0 / 255.0))).abs() < 0.005);
    assert_eq!(&pixels[16..20], [0, 0, 0, 96]);
}
