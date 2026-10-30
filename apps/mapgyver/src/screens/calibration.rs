//! Product calibration choices expressed through Golden's generic procedural layer contract.

use super::protocol::Calibration;
use golden_video::{
    LinearColor,
    plan::{GridPattern, Pattern},
};

pub(super) fn pattern(mode: Calibration) -> Option<Pattern> {
    match mode {
        Calibration::Off => None,
        Calibration::Grid => Some(Pattern::Grid(
            GridPattern::new(
                [16, 9],
                0.025,
                LinearColor::from_straight(1.0, 1.0, 1.0, 1.0).unwrap(),
                LinearColor::from_straight(0.0, 0.0, 0.0, 1.0).unwrap(),
            )
            .expect("fixed calibration grid"),
        )),
        Calibration::ColorBars => Some(Pattern::Bars(
            [
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 0.0],
            ]
            .map(|[r, g, b]| LinearColor::from_straight(r, g, b, 1.0).unwrap()),
        )),
    }
}
