//! CPU interpretation of the diagnostic card specification. No GPU/shader code is reused.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CardInput {
    pub color: [f32; 4],
    pub frame: u32,
    pub channel: u32,
    pub stamp: u64,
}

impl CardInput {
    pub fn fixed(channel: u32) -> Result<Self, String> {
        match channel {
            0 => Ok(Self {
                color: [1.0, 0.25, 0.0, 1.0],
                frame: 123456,
                channel,
                stamp: 0x0123_4567_89ab_cdef,
            }),
            1 => Ok(Self {
                color: [0.0, 0.5, 1.0, 1.0],
                frame: 789012,
                channel,
                stamp: 0xfedc_ba98_7654_3210,
            }),
            _ => Err("procedural channel must be 0 or 1".into()),
        }
    }
    pub fn marker_words(self) -> [u32; 5] {
        let low = self.stamp as u32;
        let high = (self.stamp >> 32) as u32;
        let magic = 0xa671_c300 | self.channel;
        [
            self.frame,
            magic,
            low,
            high,
            self.frame.rotate_left(7) ^ low ^ high.rotate_left(13) ^ magic ^ 0x9e37_79b9,
        ]
    }
}

fn digit(x: f64, y: f64, number: u32) -> bool {
    // Segment geometry is expressed as CPU rectangle predicates rather than shader execution.
    let rectangles = [
        (0.15, 0.02, 0.85, 0.14),
        (0.80, 0.10, 0.94, 0.48),
        (0.80, 0.52, 0.94, 0.90),
        (0.15, 0.86, 0.85, 0.98),
        (0.06, 0.52, 0.20, 0.90),
        (0.06, 0.10, 0.20, 0.48),
        (0.15, 0.44, 0.85, 0.56),
    ];
    let enabled = match number % 10 {
        0 => [true, true, true, true, true, true, false],
        1 => [false, true, true, false, false, false, false],
        2 => [true, true, false, true, true, false, true],
        3 => [true, true, true, true, false, false, true],
        4 => [false, true, true, false, false, true, true],
        5 => [true, false, true, true, false, true, true],
        6 => [true, false, true, true, true, true, true],
        7 => [true, true, true, false, false, false, false],
        8 => [true; 7],
        _ => [true, true, true, true, false, true, true],
    };
    rectangles
        .iter()
        .zip(enabled)
        .any(|(&(left, top, right, bottom), on)| on && x >= left && x <= right && y >= top && y <= bottom)
}

pub(super) fn pixel(extent: [u32; 2], x: u32, y: u32, input: CardInput) -> [u8; 4] {
    let u = (f64::from(x) + 0.5) / f64::from(extent[0]);
    let v = (f64::from(y) + 0.5) / f64::from(extent[1]);
    if v > 0.25 && v < 0.28 && (0.02..0.12).contains(&u) {
        let word = input.marker_words()[(((u - 0.02) / 0.02).floor() as usize).min(4)].to_le_bytes();
        return [word[2], word[1], word[0], word[3]];
    }
    let color = input.color.map(f64::from);
    let rgba = if v > 0.76 {
        [
            color[0],
            color[1],
            color[2],
            ((u.min(0.999) * 4.0).floor() + 1.0) * 0.25,
        ]
    } else if v > 0.35 && v < 0.65 {
        color
    } else if v > 0.04 && v < 0.23 && u > 0.05 && u < 0.95 {
        let px = (u - 0.05) / 0.90 * 7.0;
        let py = (v - 0.04) / 0.19;
        let column = px.floor() as u32;
        let number = if column == 0 {
            input.channel
        } else {
            input.frame / 10_u32.pow(6 - column.min(6))
        };
        let value = if digit(px.fract(), py.fract(), number) {
            1.0
        } else {
            0.02
        };
        [value, value, value, 1.0]
    } else {
        let checker = if ((u * 16.0).floor() as u32 + (v * 9.0).floor() as u32) & 1 == 1 {
            0.22
        } else {
            0.05
        };
        if (u * 16.0).fract() < 0.025 || (v * 9.0).fract() < 0.025 {
            [1.0; 4]
        } else {
            [
                checker * 0.55 + color[0] * 0.45,
                checker * 0.55 + color[1] * 0.45,
                checker * 0.55 + color[2] * 0.45,
                1.0,
            ]
        }
    };
    let rgba = rgba.map(|c| (c * 255.0).round().clamp(0.0, 255.0) as u8);
    [rgba[2], rgba[1], rgba[0], rgba[3]]
}

#[cfg(test)]
mod tests;
