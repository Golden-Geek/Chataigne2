use super::*;
use golden_video::plan::CanvasShape;

#[test]
fn geometry_preserves_square_cells_circle_and_second_palette_rows_across_aspects_and_resize() {
    for [width, height] in [[1920, 1080], [1080, 1920], [1024, 1024], [3840, 2160], [1919, 1079]] {
        let extent = Extent2D::new(width, height).unwrap();
        let shape = layout(extent);
        assert!((width as f32 / shape.cells[0] - height as f32 / shape.cells[1]).abs() < 0.001);
        assert!(shape.cells.iter().all(|count| *count <= 32.));
        let circle = shape.circle;
        assert!((circle.width * width as f32 - circle.height * height as f32).abs() < 0.001);
        assert!((circle.x + circle.width / 2. - 0.5).abs() < 0.00001);
        assert!((circle.y + circle.height / 2. - 0.5).abs() < 0.00001);
        assert!((circle.width.max(circle.height) - 1.).abs() < 0.00001);
        let top = shape.brightness.unwrap();
        let bottom = shape.colors.unwrap();
        assert!((top.y * height as f32 - shape.cell_pixels).abs() < 0.001);
        assert!(((1. - bottom.y) * height as f32 - 2. * shape.cell_pixels).abs() < 0.001);
        let Some(Pattern::Canvas(canvas)) = pattern(Calibration::Grid, extent) else {
            panic!()
        };
        assert_eq!(canvas.primitives().len(), 7);
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
fn calibration_geometry_is_bounded_at_extreme_sizes_and_off_restores_source_routing() {
    for [width, height] in [[1, 1], [16384, 1], [1, 16384], [16384, 16384], [127, 63]] {
        let extent = Extent2D::new(width, height).unwrap();
        assert!(pattern(Calibration::Off, extent).is_none());
        let Some(Pattern::Canvas(canvas)) = pattern(Calibration::Grid, extent) else {
            panic!()
        };
        assert!(canvas.primitives().len() <= 7);
        assert_eq!(canvas.primitives().len(), work_count(Calibration::Grid, extent));
        assert!(
            canvas
                .primitives()
                .iter()
                .all(|primitive| primitive.bounds().is_valid())
        );
        assert!(matches!(
            pattern(Calibration::ColorBars, extent),
            Some(Pattern::Bars(_))
        ));
    }
}
