//! Independent inverse-affine raster oracle using premultiplied linear-light source-over.
use mapgyver::media::composition::protocol::{BlendMode, InstanceDefinition};

pub type Pixel = [f64; 4];

pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Pixel>,
}

fn quantize(pixel: Pixel) -> Pixel {
    pixel.map(|value| (value.clamp(0.0, 1.0) * 255.0).round() / 255.0)
}

impl Image {
    pub fn solid(width: usize, height: usize, straight: Pixel) -> Self {
        let [r, g, b, a] = straight;
        Self {
            width,
            height,
            pixels: vec![quantize([r * a, g * a, b * a, a]); width * height],
        }
    }

    fn at(&self, x: isize, y: isize) -> Pixel {
        let x = x.clamp(0, self.width as isize - 1) as usize;
        let y = y.clamp(0, self.height as isize - 1) as usize;
        self.pixels[y * self.width + x]
    }

    pub fn draw(&mut self, source: &Self, definition: &InstanceDefinition) {
        if !definition.enabled {
            return;
        }
        let t = definition.transform;
        let angle = f64::from(t.rotation);
        for y in 0..self.height {
            for x in 0..self.width {
                let dx = x as f64 + 0.5 - f64::from(t.position[0]);
                let dy = y as f64 + 0.5 - f64::from(t.position[1]);
                let u = (angle.cos() * dx + angle.sin() * dy) / f64::from(t.scale[0])
                    + f64::from(t.pivot[0]) * source.width as f64;
                let v = (-angle.sin() * dx + angle.cos() * dy) / f64::from(t.scale[1])
                    + f64::from(t.pivot[1]) * source.height as f64;
                if u < 0.0 || v < 0.0 || u >= source.width as f64 || v >= source.height as f64 {
                    continue;
                }
                let src = source
                    .at(u.floor() as isize, v.floor() as isize)
                    .map(|value| value * f64::from(definition.opacity));
                let target = &mut self.pixels[y * self.width + x];
                *target = quantize(over(src, *target, definition.blend));
            }
        }
    }

    pub fn resized(&self, width: usize, height: usize) -> Self {
        let mut result = Self::solid(width, height, [0.0; 4]);
        for y in 0..height {
            for x in 0..width {
                let u = (x as f64 + 0.5) * self.width as f64 / width as f64 - 0.5;
                let v = (y as f64 + 0.5) * self.height as f64 / height as f64 - 0.5;
                let (left, top) = (u.floor() as isize, v.floor() as isize);
                let (a, b) = (u - u.floor(), v - v.floor());
                result.pixels[y * width + x] = std::array::from_fn(|c| {
                    self.at(left, top)[c] * (1.0 - a) * (1.0 - b)
                        + self.at(left + 1, top)[c] * a * (1.0 - b)
                        + self.at(left, top + 1)[c] * (1.0 - a) * b
                        + self.at(left + 1, top + 1)[c] * a * b
                });
            }
        }
        result
    }

    pub fn output(&self) -> Vec<[u8; 4]> {
        self.pixels
            .iter()
            .map(|p| {
                let rgb = std::array::from_fn::<_, 3, _>(|c| if p[3] > 0.0 { p[c] / p[3] } else { 0.0 });
                let encode = |v: f64| {
                    if v <= 0.0031308 {
                        12.92 * v
                    } else {
                        1.055 * v.powf(1.0 / 2.4) - 0.055
                    }
                };
                [
                    byte(encode(rgb[0])),
                    byte(encode(rgb[1])),
                    byte(encode(rgb[2])),
                    byte(p[3]),
                ]
            })
            .collect()
    }

    pub fn preview(&self) -> Vec<[u8; 4]> {
        let background = quantize([0.0075, 0.0075, 0.0075, 1.0]);
        let image = Self {
            width: self.width,
            height: self.height,
            pixels: self
                .pixels
                .iter()
                .map(|p| quantize(over(quantize(*p), background, BlendMode::Normal)))
                .collect(),
        };
        image.output()
    }

    pub fn preview_bounds(&self) -> Vec<([u8; 4], [u8; 4])> {
        let background = quantize([0.0075, 0.0075, 0.0075, 1.0]);
        self.pixels
            .iter()
            .map(|pixel| {
                let value = over(quantize(*pixel), background, BlendMode::Normal);
                // D3D11 FLOAT -> UNORM permits either integer within 0.6 ULP, not only
                // CPU round-to-nearest. Bound this conversion before nonlinear sRGB encoding.
                // https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm#3.2.3.6%20FLOAT%20-%3E%20UNORM
                let low = value.map(|v| (v * 255.0 - 0.6).ceil().clamp(0.0, 255.0) / 255.0);
                let high = value.map(|v| (v * 255.0 + 0.6).floor().clamp(0.0, 255.0) / 255.0);
                let encode = |p| {
                    Image {
                        width: 1,
                        height: 1,
                        pixels: vec![p],
                    }
                    .output()[0]
                };
                (encode(low), encode(high))
            })
            .collect()
    }
}

fn byte(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn over(source: Pixel, destination: Pixel, blend: BlendMode) -> Pixel {
    if blend == BlendMode::Add {
        return std::array::from_fn(|c| (source[c] + destination[c]).min(1.0));
    }
    let (sa, da) = (source[3], destination[3]);
    let mut result = [0.0; 4];
    result[3] = sa + da * (1.0 - sa);
    for c in 0..3 {
        let s = if sa > 0.0 { source[c] / sa } else { 0.0 };
        let d = if da > 0.0 { destination[c] / da } else { 0.0 };
        let mixed = match blend {
            BlendMode::Normal => s,
            BlendMode::Multiply => s * d,
            BlendMode::Screen => 1.0 - (1.0 - s) * (1.0 - d),
            BlendMode::Add => unreachable!(),
        };
        result[c] = sa * (1.0 - da) * s + sa * da * mixed + (1.0 - sa) * da * d;
    }
    result
}

#[test]
fn reference_checks_known_coverage_order_and_srgb_values() {
    assert_eq!(
        Image::solid(1, 1, [1.0, 0.0, 0.0, 0.5]).output(),
        vec![[255, 0, 0, 128]]
    );
    assert_eq!(
        over([0.5, 0.0, 0.0, 0.5], [0.0, 0.5, 0.0, 0.5], BlendMode::Normal),
        [0.5, 0.25, 0.0, 0.75]
    );
    assert_eq!(
        over([0.5, 0.0, 0.0, 0.5], [0.0, 0.5, 0.0, 0.5], BlendMode::Multiply),
        [0.25, 0.25, 0.0, 0.75]
    );
}

#[test]
fn reference_checks_asymmetric_rotation_and_mirror() {
    use mapgyver::media::composition::protocol::Transform2D;
    let source = Image {
        width: 2,
        height: 1,
        pixels: vec![[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]],
    };
    let mut target = Image::solid(2, 2, [0.0; 4]);
    target.draw(
        &source,
        &InstanceDefinition {
            transform: Transform2D {
                position: [1.0, 2.0],
                scale: [-1.0, 1.0],
                rotation: std::f32::consts::FRAC_PI_2,
                pivot: [0.0, 0.0],
            },
            ..Default::default()
        },
    );
    assert_eq!(
        target.output(),
        vec![[0, 255, 0, 255], [0; 4], [255, 0, 0, 255], [0; 4]]
    );
}

#[test]
fn preview_unorm_bound_covers_both_legal_near_black_conversions_only() {
    let image = Image::solid(1, 1, [0.0, 1.0, 0.0, 0.75]);
    assert_eq!(image.preview(), vec![[13, 225, 13, 255]]);
    assert_eq!(image.preview_bounds(), vec![([0, 224, 0, 255], [13, 225, 13, 255])]);
    // At 0.25 ULP, only zero is legal; this is not a blanket extra output-byte tolerance.
    let low = (0.25_f64 - 0.6).ceil().max(0.0);
    let high = (0.25_f64 + 0.6).floor();
    assert_eq!((low, high), (0.0, 0.0));
}
