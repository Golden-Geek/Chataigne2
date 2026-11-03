use super::*;
use crate::runtime::{CompletionToken, DeviceGeneration};

#[test]
fn live_placement_is_checked_and_coherent_across_passes_in_one_frame() {
    let placement = RegionPlacement::new(UnitRect::FULL, UnitRect::FULL).unwrap();
    let device = DeviceGeneration::new(uuid::Uuid::new_v4(), 1);
    let first = CompletionToken { device, serial: 1 };
    let region = BlitRegion {
        source: texture(1),
        destination: UnitRect::FULL,
        source_rect: UnitRect::FULL,
        filter: SampleFilter::Linear,
        visibility: None,
        placement: Some(placement.clone()),
    };
    assert!(!placement.presented_through(first));
    assert_eq!(region.for_frame(first).destination, UnitRect::FULL);
    let moved = UnitRect {
        x: 0.25,
        y: 0.25,
        width: 0.5,
        height: 0.5,
    };
    assert!(placement.update(moved, UnitRect::FULL));
    assert!(!placement.presented_through(first));
    assert_eq!(
        region.for_frame(first).destination,
        UnitRect::FULL,
        "two passes cannot tear within a batch"
    );
    let second = CompletionToken { device, serial: 2 };
    assert_eq!(region.for_frame(second).destination, moved);
    assert!(!placement.presented_through(first));
    assert!(placement.presented_through(second));
    assert!(!placement.update(UnitRect { x: f32::NAN, ..moved }, UnitRect::FULL));
    assert_eq!(
        region.for_frame(CompletionToken { device, serial: 3 }).destination,
        moved
    );
}

#[test]
fn decorations_and_image_share_one_batch_sample_and_invalid_updates_are_atomic() {
    let placement = RegionPlacement::with_overlay_capacity(UnitRect::FULL, UnitRect::FULL, MAX_VIEW_OVERLAYS).unwrap();
    let device = DeviceGeneration::new(uuid::Uuid::new_v4(), 1);
    let token = CompletionToken { device, serial: 1 };
    let overlay = OverlayPrimitive {
        shape: OverlayShape::Quad {
            corners: [[-1.0, 0.0], [0.5, 0.0], [0.5, 1.0], [-1.0, 1.0]],
        },
        fill: [0.0; 4],
        stroke: [0.0, 0.5, 1.0, 1.0],
        width: 1.6,
        dash: None,
    };
    assert!(placement.update_decorated(UnitRect::FULL, UnitRect::FULL, UnitRect::FULL, 1.25, &[overlay]));
    let first = placement.sample(token);
    let moved = UnitRect {
        x: 0.25,
        width: 0.75,
        ..UnitRect::FULL
    };
    assert!(placement.update_decorated(moved, UnitRect::FULL, UnitRect::FULL, 2.0, &[]));
    assert_eq!(
        placement.sample(token),
        first,
        "image and decorations cannot tear between passes"
    );
    let second = placement.sample(CompletionToken { device, serial: 2 });
    assert_eq!(second.destination, moved);
    assert!(second.overlays.is_empty());
    assert!(!placement.update_decorated(UnitRect::FULL, UnitRect::FULL, UnitRect::FULL, f32::NAN, &[overlay]));
    assert!(!placement.update_decorated(
        UnitRect::FULL,
        UnitRect::FULL,
        UnitRect::FULL,
        1.0,
        &vec![overlay; MAX_VIEW_OVERLAYS + 1]
    ));
    let invalid = OverlayPrimitive { width: -1.0, ..overlay };
    assert!(!placement.update_decorated(UnitRect::FULL, UnitRect::FULL, UnitRect::FULL, 1.0, &[invalid]));
    assert_eq!(placement.sample(CompletionToken { device, serial: 3 }), second);
}
