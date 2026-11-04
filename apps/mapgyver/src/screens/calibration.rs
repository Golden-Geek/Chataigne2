//! Product calibration choices expressed through Golden's generic procedural layer contract.

use super::protocol::Calibration;
use golden_video::{
    Extent2D, LinearColor, Opacity,
    plan::{CanvasPaint, CanvasPattern, CanvasPrimitive, Pattern, UnitRect},
};
use std::time::Duration;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(super) struct CalibrationLayout {
    pub cell_pixels: f32,
    pub cells: [f32; 2],
    pub brightness: Option<UnitRect>,
    pub colors: Option<UnitRect>,
    pub circle: UnitRect,
}

pub(super) fn layout(extent: Extent2D) -> CalibrationLayout {
    let [width, height] = [extent.width() as f32, extent.height() as f32];
    let diameter = width.min(height);
    let cell_pixels = (diameter / 9.).max(width.max(height) / 32.).max(1.);
    let row = cell_pixels / height;
    CalibrationLayout {
        cell_pixels,
        cells: [width / cell_pixels, height / cell_pixels],
        brightness: (row <= 0.25).then_some(UnitRect {
            x: 0.,
            y: row,
            width: 1.,
            height: row,
        }),
        colors: (row <= 0.25).then_some(UnitRect {
            x: 0.,
            y: 1. - 2. * row,
            width: 1.,
            height: row,
        }),
        circle: UnitRect {
            x: (width - diameter) / (2. * width),
            y: (height - diameter) / (2. * height),
            width: diameter / width,
            height: diameter / height,
        },
    }
}

fn color(r: f32, g: f32, b: f32) -> LinearColor {
    LinearColor::from_straight(r, g, b, 1.).expect("fixed calibration color")
}

pub(super) fn work_count(mode: Calibration, extent: Extent2D) -> usize {
    if mode != Calibration::Grid {
        return 1;
    }
    let layout = layout(extent);
    5 + usize::from(layout.brightness.is_some()) + usize::from(layout.colors.is_some())
}

fn grid(extent: Extent2D) -> Pattern {
    let layout = layout(extent);
    let [width, height] = [extent.width() as f32, extent.height() as f32];
    let line_pixels = (layout.cell_pixels / 160.).clamp(0.5, 1.5);
    let mut primitives = vec![
        CanvasPrimitive::checkerboard(
            UnitRect::FULL,
            layout.cells,
            [color(0.1, 0.1, 0.1), color(0.2, 0.2, 0.2)],
        )
        .unwrap(),
    ];
    if let Some(bounds) = layout.brightness {
        primitives.push(
            CanvasPrimitive::bars(
                bounds,
                std::array::from_fn(|index| {
                    let value = index as f32 / 7.;
                    color(value, value, value)
                }),
            )
            .unwrap(),
        );
    }
    if let Some(bounds) = layout.colors {
        primitives.push(CanvasPrimitive::bars(bounds, palette()).unwrap());
    }
    let stroke = CanvasPaint::solid(color(0.65, 0.65, 0.65));
    primitives.push(
        CanvasPrimitive::ellipse(layout.circle, (2. * line_pixels / width.min(height)).min(0.25), stroke).unwrap(),
    );
    for (start, end) in [([0., 0.], [1., 1.]), ([0., 1.], [1., 0.])] {
        primitives.push(
            CanvasPrimitive::line(
                UnitRect::FULL,
                start,
                end,
                (line_pixels / height).min(0.25),
                width / height,
                stroke,
            )
            .unwrap(),
        );
    }
    let rainbow = CanvasPaint::rainbow(Duration::from_secs(6), [1., 1.], Opacity::OPAQUE).unwrap();
    primitives.push(
        CanvasPrimitive::grid(
            UnitRect::FULL,
            layout.cells,
            [(line_pixels / width).min(0.25), (line_pixels / height).min(0.25)],
            rainbow,
        )
        .unwrap(),
    );
    Pattern::Canvas(CanvasPattern::new(primitives).expect("bounded calibration geometry"))
}

fn palette() -> [LinearColor; 8] {
    [
        [1., 1., 1.],
        [1., 1., 0.],
        [0., 1., 1.],
        [0., 1., 0.],
        [1., 0., 1.],
        [1., 0., 0.],
        [0., 0., 1.],
        [0., 0., 0.],
    ]
    .map(|[r, g, b]| color(r, g, b))
}

pub(super) fn pattern(mode: Calibration, extent: Extent2D) -> Option<Pattern> {
    match mode {
        Calibration::Off => None,
        Calibration::Grid => Some(grid(extent)),
        Calibration::ColorBars => Some(Pattern::Bars(palette())),
    }
}
