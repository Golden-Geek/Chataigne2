use super::*;

#[test]
fn virtual_sender_names_leave_room_for_the_native_frame_counter_namespace() {
    for (length, valid) in [(239, true), (240, false)] {
        let definition = ScreenDefinition {
            enabled: true,
            target: Some(ScreenTarget::Spout {
                sender: "a".repeat(length),
                extent: golden_video::Extent2D::new(64, 64).unwrap(),
            }),
        };
        assert_eq!(definition.validate().is_ok(), valid);
    }
}

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
        UnitRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 1.0,
        },
        UnitRect {
            x: -0.1,
            ..UnitRect::default()
        },
        UnitRect {
            width: f32::INFINITY,
            ..UnitRect::default()
        },
        UnitRect {
            x: 0.5,
            width: 0.6,
            ..UnitRect::default()
        },
    ] {
        assert!(!crop.is_valid());
    }
}
