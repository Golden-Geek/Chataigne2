use super::*;

#[test]
fn crops_and_quads_admit_offscreen_mirrored_mapping_but_reject_invalid_geometry() {
    let mut quad = Quad::default();
    quad.corners[0].x = -0.2;
    quad.corners[3].x = -0.1;
    quad.validate().unwrap();
    quad.corners.reverse();
    quad.validate().unwrap();
    for corners in [
        [Point { x: 0.0, y: 0.0 }; 4],
        [
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 1.0 },
            Point { x: 1.0, y: 0.0 },
            Point { x: 0.0, y: 1.0 },
        ],
        [
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 0.0 },
            Point { x: 0.2, y: 0.2 },
            Point { x: 0.0, y: 1.0 },
        ],
        [Point { x: f32::NAN, y: 0.0 }; 4],
    ] {
        assert!(Quad { corners }.validate().is_err());
    }
    for crop in [
        Crop {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 1.0,
        },
        Crop {
            x: -0.1,
            ..Crop::default()
        },
        Crop {
            width: f32::INFINITY,
            ..Crop::default()
        },
        Crop {
            x: 0.5,
            width: 0.6,
            ..Crop::default()
        },
    ] {
        assert!(crop.validate().is_err());
    }
}
