use super::*;

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.00001, "{actual} != {expected}");
}

#[test]
fn contain_uses_actual_source_and_target_pixel_aspects_with_centered_margins() {
    let tile = RegionTile {
        destination: UnitRect::FULL,
        source_rect: UnitRect::FULL,
    };
    for (source, target, wanted) in [
        ([100, 200], [200, 100], [0.375, 0., 0.25, 1.]),
        ([200, 100], [100, 200], [0., 0.375, 1., 0.25]),
        ([100, 100], [200, 100], [0.25, 0., 0.5, 1.]),
        ([200, 100], [200, 100], [0., 0., 1., 1.]),
    ] {
        let source = Extent2D::new(source[0], source[1]).unwrap();
        let target = Extent2D::new(target[0], target[1]).unwrap();
        let fitted = tile.fitted(RegionFit::Contain, source, target).unwrap();
        for (actual, wanted) in [
            fitted.destination.x,
            fitted.destination.y,
            fitted.destination.width,
            fitted.destination.height,
        ]
        .into_iter()
        .zip(wanted)
        {
            close(actual, wanted);
        }
        assert_eq!(fitted.source_rect, UnitRect::FULL);
        assert_eq!(tile.fitted(RegionFit::Stretch, source, target), Some(tile));
    }
}

#[test]
fn contain_fits_before_scroll_clipping_and_skips_margin_only_visible_tiles() {
    let source = Extent2D::new(100, 200).unwrap();
    let target = Extent2D::new(200, 100).unwrap();
    // The left half of an original square aperture remains after horizontal scrolling.
    let tile = RegionTile {
        destination: UnitRect {
            x: 0.,
            y: 0.,
            width: 0.25,
            height: 1.,
        },
        source_rect: UnitRect {
            x: 0.5,
            y: 0.,
            width: 0.5,
            height: 1.,
        },
    };
    let fitted = tile.fitted(RegionFit::Contain, source, target).unwrap();
    close(fitted.destination.x, 0.);
    close(fitted.destination.width, 0.125);
    close(fitted.source_rect.x, 0.5);
    close(fitted.source_rect.width, 0.5);
    let margin = RegionTile {
        source_rect: UnitRect {
            x: 0.9,
            y: 0.,
            width: 0.1,
            height: 1.,
        },
        ..tile
    };
    assert!(margin.fitted(RegionFit::Contain, source, target).is_none());
}

#[test]
fn live_dimensions_refit_one_placement_without_retaining_previous_source_metadata() {
    let tile = RegionTile {
        destination: UnitRect::FULL,
        source_rect: UnitRect::FULL,
    };
    let target = Extent2D::new(200, 100).unwrap();
    let portrait = tile
        .fitted(RegionFit::Contain, Extent2D::new(100, 200).unwrap(), target)
        .unwrap();
    let landscape = tile
        .fitted(RegionFit::Contain, Extent2D::new(200, 100).unwrap(), target)
        .unwrap();
    assert_ne!(portrait.destination, landscape.destination);
    assert_eq!(landscape.destination, UnitRect::FULL);
    assert_eq!(tile.destination, UnitRect::FULL);
}
