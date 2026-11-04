use super::*;
use golden_video::plan::CanvasShape;

#[test]
fn text_labels_follow_cells_and_bracket_the_logo_across_output_aspects() {
    let names = cell_names([32., 32.]);
    assert_eq!(names.len(), 1024);
    assert_eq!(
        (&names[0], &names[25], &names[26], &names[1023]),
        (
            &"A1".to_string(),
            &"Z1".to_string(),
            &"AA1".to_string(),
            &"AF32".to_string()
        )
    );
    assert_eq!(cell_names([2.1, 1.1]), ["A1", "B1", "C1", "A2", "B2", "C2"]);
    let mut assets = CalibrationAssets::default();
    for [width, height] in [[1920, 1080], [1080, 1920], [1024, 1024], [3840, 2160], [1919, 1079]] {
        let extent = Extent2D::new(width, height).unwrap();
        let shape = layout(extent);
        let Some(Pattern::Canvas(canvas)) = pattern(Calibration::Grid, extent, &mut assets).unwrap() else {
            panic!()
        };
        let text: Vec<_> = canvas
            .primitives()
            .iter()
            .filter(|p| p.shape() == CanvasShape::Text)
            .collect();
        assert_eq!(text.len(), 4);
        assert!(text[1].bounds().y + text[1].bounds().height < shape.logo.y);
        assert!(text[2].bounds().y > shape.logo.y + shape.logo.height);
        assert!(text[3].bounds().y >= text[2].bounds().y + text[2].bounds().height - 0.00001);
        for line in &text[1..] {
            assert!((line.bounds().x + line.bounds().width / 2. - 0.5).abs() < 0.00001);
        }
    }
}

#[test]
fn geometry_preserves_square_cells_circle_and_second_palette_rows_across_aspects_and_resize() {
    let mut assets = CalibrationAssets::default();
    for [width, height] in [[1920, 1080], [1080, 1920], [1024, 1024], [3840, 2160], [1919, 1079]] {
        let extent = Extent2D::new(width, height).unwrap();
        let shape = layout(extent);
        assert!((width as f32 / shape.cells[0] - height as f32 / shape.cells[1]).abs() < 0.001);
        assert!(shape.cells.iter().all(|count| *count <= 32.));
        let circle = shape.circle;
        assert!((shape.logo.width * width as f32 - shape.logo.height * height as f32).abs() < 0.001);
        assert!((shape.logo.x + shape.logo.width / 2. - 0.5).abs() < 0.00001);
        assert!((shape.logo.y + shape.logo.height / 2. - 0.5).abs() < 0.00001);
        assert!((circle.width * width as f32 - circle.height * height as f32).abs() < 0.001);
        assert!((circle.x + circle.width / 2. - 0.5).abs() < 0.00001);
        assert!((circle.y + circle.height / 2. - 0.5).abs() < 0.00001);
        assert!((circle.width.max(circle.height) - 1.).abs() < 0.00001);
        let top = shape.brightness.unwrap();
        let bottom = shape.colors.unwrap();
        assert!((top.y * height as f32 - shape.cell_pixels).abs() < 0.001);
        assert!(((1. - bottom.y) * height as f32 - 2. * shape.cell_pixels).abs() < 0.001);
        let Some(Pattern::Canvas(canvas)) = pattern(Calibration::Grid, extent, &mut assets).unwrap() else {
            panic!()
        };
        assert_eq!(canvas.primitives().len(), 12);
        assert_eq!(
            canvas.image().unwrap().descriptor().extent,
            Extent2D::new(512, 512).unwrap()
        );
        assert_eq!(
            canvas
                .primitives()
                .iter()
                .filter(|primitive| primitive.shape() == CanvasShape::Image)
                .count(),
            1
        );
        assert_eq!(canvas.primitives().len(), work_count(Calibration::Grid, extent));
        assert_eq!(
            canvas
                .primitives()
                .iter()
                .filter(|primitive| primitive.shape() == CanvasShape::Line)
                .count(),
            2
        );
        assert!(
            canvas
                .primitives()
                .iter()
                .all(|primitive| primitive.bounds().is_valid())
        );
    }
}

#[test]
fn logo_cache_shares_live_pixels_without_retaining_the_last_plan_or_native_owner() {
    let mut assets = CalibrationAssets::default();
    let first = assets.logo().unwrap();
    let second = assets.logo().unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    let weak = std::sync::Arc::downgrade(&first);
    drop(first);
    drop(second);
    assert!(
        weak.upgrade().is_none(),
        "weak memoization cannot keep closed image storage alive"
    );
    assert_eq!(assets.logo().unwrap().pixels().len(), 512 * 512 * 4);
}

#[test]
fn calibration_geometry_is_bounded_at_extreme_sizes_and_off_restores_source_routing() {
    let mut assets = CalibrationAssets::default();
    for [width, height] in [[1, 1], [16384, 1], [1, 16384], [16384, 16384], [127, 63]] {
        let extent = Extent2D::new(width, height).unwrap();
        assert!(pattern(Calibration::Off, extent, &mut assets).unwrap().is_none());
        let Some(Pattern::Canvas(canvas)) = pattern(Calibration::Grid, extent, &mut assets).unwrap() else {
            panic!()
        };
        assert!(canvas.primitives().len() <= 12);
        assert_eq!(canvas.primitives().len(), work_count(Calibration::Grid, extent));
        assert!(
            canvas
                .primitives()
                .iter()
                .all(|primitive| primitive.bounds().is_valid())
        );
        assert!(matches!(
            pattern(Calibration::ColorBars, extent, &mut assets).unwrap(),
            Some(Pattern::Bars(_))
        ));
    }
}
