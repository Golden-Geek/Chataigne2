use super::*;
use crate::plan::{Point, Quad};

#[test]
fn four_k_four_density_stays_inside_original_dimension_budget() {
    let output = Extent2D::new(3840, 2160).unwrap();
    let tiles = supersample_tiles(output, 4, 8192, 8).unwrap();
    assert_eq!(tiles.len(), 4);
    for tile in tiles {
        assert_eq!(tile.extent, Extent2D::new(1920, 1080).unwrap());
        assert_eq!(tile.raster_extent(4).unwrap(), Extent2D::new(7680, 4320).unwrap());
        assert!(tile.viewport(output).is_valid());
    }
    assert!(supersample_tiles(output, 4, 8192, 3).is_none());
    assert!(supersample_tiles(output, 0, 8192, 8).is_none());
    assert!(supersample_tiles(output, 4, 3, 8).is_none());
}

#[test]
fn odd_tiles_cover_every_pixel_once_and_preserve_projective_coordinates() {
    let output = Extent2D::new(67, 35).unwrap();
    let tiles = supersample_tiles(output, 4, 128, 12).unwrap();
    let mapping = ProjectiveMapping::new(
        UnitRect {
            x: 0.1,
            y: 0.2,
            width: 0.7,
            height: 0.6,
        },
        Quad::default(),
    )
    .unwrap();
    let mut hits = vec![0; 67 * 35];
    for tile in tiles {
        assert!(tile.raster_extent(4).unwrap().width() <= 128);
        let viewport = tile.viewport(output);
        let local = mapping.in_viewport(viewport).unwrap();
        for y in 0..tile.extent.height() {
            for x in 0..tile.extent.width() {
                let global_x = tile.origin[0] + x;
                let global_y = tile.origin[1] + y;
                hits[(global_y * 67 + global_x) as usize] += 1;
                let point = Point {
                    x: (x as f32 + 0.5) / tile.extent.width() as f32,
                    y: (y as f32 + 0.5) / tile.extent.height() as f32,
                };
                let actual = local.source_coordinate(point).unwrap();
                let expected = mapping
                    .source_coordinate(Point {
                        x: (global_x as f32 + 0.5) / 67.,
                        y: (global_y as f32 + 0.5) / 35.,
                    })
                    .unwrap();
                assert!((actual[0] - expected[0]).abs() < 1e-6);
                assert!((actual[1] - expected[1]).abs() < 1e-6);
            }
        }
    }
    assert!(hits.into_iter().all(|count| count == 1));
}
