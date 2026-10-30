use super::super::{Point, ProjectiveMapping, Quad, UnitRect};

fn close(actual: [f32; 2], expected: [f32; 2]) {
    assert!(
        (actual[0] - expected[0]).abs() < 1e-5 && (actual[1] - expected[1]).abs() < 1e-5,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn projective_inverse_matches_an_independently_defined_forward_transform_then_crop() {
    // Forward: x=(.25+.75u)/(1+.5v), y=(.125+.875v)/(1+.5v).
    let mapping = ProjectiveMapping::new(
        UnitRect {
            x: 0.25,
            y: 0.1,
            width: 0.5,
            height: 0.8,
        },
        Quad {
            corners: [
                Point { x: 0.25, y: 0.125 },
                Point { x: 1.0, y: 0.125 },
                Point {
                    x: 2.0 / 3.0,
                    y: 2.0 / 3.0,
                },
                Point {
                    x: 1.0 / 6.0,
                    y: 2.0 / 3.0,
                },
            ],
        },
    )
    .unwrap();
    for v in [0.1, 0.3, 0.6, 0.9] {
        for u in [0.1, 0.25, 0.6, 0.9] {
            let point = Point {
                x: (0.25 + 0.75 * u) / (1.0 + 0.5 * v),
                y: (0.125 + 0.875 * v) / (1.0 + 0.5 * v),
            };
            close(
                mapping.source_coordinate(point).unwrap(),
                [0.25 + 0.5 * u, 0.1 + 0.8 * v],
            );
        }
    }
    assert!(mapping.source_coordinate(Point { x: 0.0, y: 0.0 }).is_none());
}

#[test]
fn mirrored_off_target_quads_are_valid_and_singular_or_unstable_maps_are_rejected() {
    let mapping = ProjectiveMapping::new(
        UnitRect::FULL,
        Quad {
            corners: [
                Point { x: 1.5, y: -0.5 },
                Point { x: -0.5, y: -0.5 },
                Point { x: -0.5, y: 1.5 },
                Point { x: 1.5, y: 1.5 },
            ],
        },
    )
    .unwrap();
    close(
        mapping.source_coordinate(Point { x: 0.0, y: 0.5 }).unwrap(),
        [0.75, 0.5],
    );
    for quad in [
        Quad {
            corners: [Point { x: 0.0, y: 0.0 }; 4],
        },
        Quad {
            corners: [
                Point { x: 0.0, y: 0.0 },
                Point { x: 1.0, y: 1.0 },
                Point { x: 1.0, y: 0.0 },
                Point { x: 0.0, y: 1.0 },
            ],
        },
        Quad {
            corners: [Point { x: f32::NAN, y: 0.0 }; 4],
        },
    ] {
        assert!(ProjectiveMapping::new(UnitRect::FULL, quad).is_err());
    }
    assert!(
        ProjectiveMapping::new(
            UnitRect {
                width: 0.0,
                ..UnitRect::FULL
            },
            Quad::default()
        )
        .is_err()
    );
}
