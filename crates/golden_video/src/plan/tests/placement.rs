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
