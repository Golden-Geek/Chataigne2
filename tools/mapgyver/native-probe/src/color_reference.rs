//! Finite authored image fixtures and a CPU bilinear oracle, independent of the GPU shader.

use serde::{Deserialize, Serialize};
mod procedural;
pub use procedural::CardInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fixture {
    Authored,
    Procedural,
}

impl Fixture {
    pub fn name(self) -> &'static str {
        match self {
            Self::Authored => "authored",
            Self::Procedural => "procedural",
        }
    }
    pub fn reference(self, extent: [u32; 2], channel: u32) -> Result<Image, String> {
        match self {
            Self::Authored => Image::card(extent, channel),
            Self::Procedural => Image::procedural(extent, channel),
        }
    }
}
impl std::str::FromStr for Fixture {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "authored" => Ok(Self::Authored),
            "procedural" => Ok(Self::Procedural),
            _ => Err("invalid accuracy fixture".into()),
        }
    }
}

pub const TOLERANCE: u8 = 2;
const MAX_PIXELS: usize = 3840 * 2160;

pub struct Image {
    extent: [u32; 2],
    pixels: Vec<[u8; 4]>,
}

impl Image {
    pub fn procedural(extent: [u32; 2], channel: u32) -> Result<Self, String> {
        let [width, height] = extent;
        let count = (width as usize).checked_mul(height as usize).ok_or("image overflow")?;
        if width < 16 || height < 16 || count > MAX_PIXELS || channel > 1 {
            return Err("invalid procedural reference card".into());
        }
        let input = CardInput::fixed(channel)?;
        let pixels = (0..height)
            .flat_map(|y| (0..width).map(move |x| procedural::pixel(extent, x, y, input)))
            .collect();
        Ok(Self { extent, pixels })
    }
    pub fn card(extent: [u32; 2], channel: u32) -> Result<Self, String> {
        let [width, height] = extent;
        let count = (width as usize).checked_mul(height as usize).ok_or("image overflow")?;
        if width < 16 || height < 16 || count > MAX_PIXELS || channel > 1 {
            return Err("invalid finite reference card".into());
        }
        let pixels = (0..height)
            .flat_map(|y| {
                (0..width).map(move |x| {
                    // Asymmetric ramps, checker edges, channel identity and straight-alpha stripes.
                    // Values are authored bytes; no GPU procedural-card implementation is reused.
                    let red = (x * 255 / (width - 1)) as u8;
                    let green = (y * 255 / (height - 1)) as u8;
                    let blue = if ((x / 17 + y / 11 + channel) & 1) == 0 {
                        23
                    } else {
                        237
                    };
                    let alpha = [0, 64, 128, 191, 255][(x * 5 / width) as usize];
                    if x < 7 && y < 13 {
                        [255, (channel * 255) as u8, 0, 255]
                    } else if x >= width - 9 && y >= height - 5 {
                        [0, 255, (channel * 255) as u8, 64]
                    } else {
                        [blue, green, red, alpha]
                    }
                })
            })
            .collect();
        Ok(Self { extent, pixels })
    }

    pub fn extent(&self) -> [u32; 2] {
        self.extent
    }

    pub fn pixels(&self) -> &[[u8; 4]] {
        &self.pixels
    }

    /// D3D normalized linear sampling: pixel centers, texel-center offset, clamp address mode.
    fn sample(&self, x: u32, y: u32, viewport: [u32; 2]) -> [u8; 4] {
        let [width, height] = self.extent;
        let sx = (f64::from(x) + 0.5) * f64::from(width) / f64::from(viewport[0]) - 0.5;
        let sy = (f64::from(y) + 0.5) * f64::from(height) / f64::from(viewport[1]) - 0.5;
        let base = [sx.floor() as i64, sy.floor() as i64];
        let weight = [sx - sx.floor(), sy - sy.floor()];
        let mut value = [0.0; 4];
        for dy in 0..2 {
            for dx in 0..2 {
                let tx = (base[0] + dx).clamp(0, i64::from(width - 1)) as usize;
                let ty = (base[1] + dy).clamp(0, i64::from(height - 1)) as usize;
                let w = if dx == 0 { 1.0 - weight[0] } else { weight[0] }
                    * if dy == 0 { 1.0 - weight[1] } else { weight[1] };
                for (c, component) in value.iter_mut().enumerate() {
                    *component += f64::from(self.pixels[ty * width as usize + tx][c]) * w;
                }
            }
        }
        value.map(|v| v.round().clamp(0.0, 255.0) as u8)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Difference {
    pub position: [u32; 2],
    pub expected_bgra: [u8; 4],
    pub actual_bgra: [u8; 4],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Comparison {
    pub extent: [u32; 2],
    pub compared_pixels: u64,
    pub tolerance_per_component: u8,
    pub maximum_error_bgra: [u8; 4],
    pub differing_components_bgra: [u64; 4],
    pub mismatched_pixels: u64,
    pub first_differences: Vec<Difference>,
}

fn expected(images: &[Image; 2], extent: [u32; 2], x: u32, y: u32) -> [u8; 4] {
    let half = extent[0] / 2;
    images[usize::from(x >= half)].sample(x % half, y, [half, extent[1]])
}

pub fn compare(images: &[Image; 2], extent: [u32; 2], pixels: &[[u8; 4]]) -> Result<Comparison, String> {
    let [width, height] = extent;
    let count = (width as usize)
        .checked_mul(height as usize)
        .ok_or("comparison overflow")?;
    if width < 2 || width % 2 != 0 || height == 0 || count > MAX_PIXELS || pixels.len() != count {
        return Err("invalid full-image comparison".into());
    }
    let mut result = Comparison {
        extent,
        compared_pixels: count as u64,
        tolerance_per_component: TOLERANCE,
        maximum_error_bgra: [0; 4],
        differing_components_bgra: [0; 4],
        mismatched_pixels: 0,
        first_differences: Vec::with_capacity(16),
    };
    for y in 0..height {
        for x in 0..width {
            let expected = expected(images, extent, x, y);
            let actual = pixels[(y * width + x) as usize];
            let mut mismatch = false;
            for c in 0..4 {
                let error = expected[c].abs_diff(actual[c]);
                result.maximum_error_bgra[c] = result.maximum_error_bgra[c].max(error);
                if error > TOLERANCE {
                    result.differing_components_bgra[c] += 1;
                    mismatch = true;
                }
            }
            if mismatch {
                result.mismatched_pixels += 1;
                if result.first_differences.len() < 16 {
                    result.first_differences.push(Difference {
                        position: [x, y],
                        expected_bgra: expected,
                        actual_bgra: actual,
                    });
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
