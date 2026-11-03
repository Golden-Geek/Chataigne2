//! Independent inverse-affine raster oracle using premultiplied linear-light source-over.
use mapgyver::media::composition::protocol::{BlendMode, InstanceDefinition};

pub type Pixel = [f64; 4];

#[derive(Clone, Copy)]
struct Bounds {
    low: Pixel,
    high: Pixel,
}

impl Bounds {
    fn exact(pixel: Pixel) -> Self {
        Self {
            low: pixel,
            high: pixel,
        }
    }
    fn quantized(self) -> Self {
        // D3D11 §3.2.3.6 permits any integer within 0.6 ULP. Propagate those
        // conversions at each pass; normalizing translucent RGB magnifies their difference.
        Self {
            low: self
                .low
                .map(|v| (v.clamp(0.0, 1.0) * 255.0 - 0.6).ceil().clamp(0.0, 255.0) / 255.0),
            high: self
                .high
                .map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.6).floor().clamp(0.0, 255.0) / 255.0),
        }
    }
    fn scaled(self, opacity: f64) -> Self {
        Self {
            low: self.low.map(|v| v * opacity),
            high: self.high.map(|v| v * opacity),
        }
    }
    fn over(self, d: Self, blend: BlendMode) -> Self {
        if blend == BlendMode::Add {
            return Self {
                low: std::array::from_fn(|c| (self.low[c] + d.low[c]).min(1.0)),
                high: std::array::from_fn(|c| (self.high[c] + d.high[c]).min(1.0)),
            };
        }
        let mut result = Self::exact([0.0; 4]);
        result.low[3] = self.low[3] + d.low[3] * (1.0 - self.low[3]);
        result.high[3] = self.high[3] + d.high[3] * (1.0 - self.high[3]);
        for c in 0..3 {
            let evaluate = |s: f64, t: f64, sa: f64, da: f64| match blend {
                BlendMode::Normal => s + t * (1.0 - sa),
                BlendMode::Multiply => s * (1.0 - da) + t * (1.0 - sa) + s * t,
                BlendMode::Screen => s + t - s * t,
                BlendMode::Add => unreachable!(),
            };
            result.low[c] = evaluate(self.low[c], d.low[c], self.high[3], d.high[3]);
            result.high[c] = evaluate(self.high[c], d.high[c], self.low[3], d.low[3]);
        }
        result
    }
}

pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Pixel>,
    bounds: Vec<Bounds>,
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
            bounds: vec![Bounds::exact([r * a, g * a, b * a, a]).quantized(); width * height],
        }
    }

    fn at(&self, x: isize, y: isize) -> Pixel {
        self.pixels[self.index(x, y)]
    }
    fn index(&self, x: isize, y: isize) -> usize {
        let x = x.clamp(0, self.width as isize - 1) as usize;
        let y = y.clamp(0, self.height as isize - 1) as usize;
        y * self.width + x
    }

    pub fn draw(&mut self, source: &Self, definition: &InstanceDefinition) {
        if !definition.enabled {
            return;
        }
        let t = definition.transform;
        let angle = f64::from(t.rotation_degrees).to_radians();
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
                let source_bounds = source.bounds[source.index(u.floor() as isize, v.floor() as isize)]
                    .scaled(f64::from(definition.opacity));
                let index = y * self.width + x;
                self.bounds[index] = source_bounds.over(self.bounds[index], definition.blend).quantized();
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
                let corners = [
                    (left, top, (1.0 - a) * (1.0 - b)),
                    (left + 1, top, a * (1.0 - b)),
                    (left, top + 1, (1.0 - a) * b),
                    (left + 1, top + 1, a * b),
                ];
                result.bounds[y * width + x] = Bounds {
                    low: std::array::from_fn(|c| {
                        corners
                            .iter()
                            .map(|(x, y, w)| self.bounds[self.index(*x, *y)].low[c] * w)
                            .sum()
                    }),
                    high: std::array::from_fn(|c| {
                        corners
                            .iter()
                            .map(|(x, y, w)| self.bounds[self.index(*x, *y)].high[c] * w)
                            .sum()
                    }),
                };
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
        self.preview_image().output()
    }

    pub fn preview_bounds(&self) -> Vec<([u8; 4], [u8; 4])> {
        self.preview_image().output_bounds()
    }
    fn preview_image(&self) -> Self {
        // This fixture fills the active aperture. The workbench theme is outside that aperture;
        // authored transparent content composites over the app's black canvas backing.
        let mut result = Self::solid(self.width, self.height, [0.0, 0.0, 0.0, 1.0]);
        for index in 0..self.pixels.len() {
            result.pixels[index] = quantize(over(
                quantize(self.pixels[index]),
                result.pixels[index],
                BlendMode::Normal,
            ));
            result.bounds[index] = self.bounds[index]
                .quantized()
                .over(result.bounds[index], BlendMode::Normal)
                .quantized();
        }
        result
    }
    pub fn output_bounds(&self) -> Vec<([u8; 4], [u8; 4])> {
        let encode = |v: f64| {
            if v <= 0.0031308 {
                12.92 * v
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        };
        self.bounds
            .iter()
            .map(|b| {
                let low = std::array::from_fn(|c| {
                    if c == 3 {
                        byte(b.low[3])
                    } else {
                        byte(encode(if b.high[3] > 0.0 { b.low[c] / b.high[3] } else { 0.0 }))
                    }
                });
                let high = std::array::from_fn(|c| {
                    if c == 3 {
                        byte(b.high[3])
                    } else {
                        byte(encode(if b.low[3] > 0.0 {
                            b.high[c] / b.low[3]
                        } else if b.high[c] > 0.0 {
                            1.0
                        } else {
                            0.0
                        }))
                    }
                });
                (low, high)
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
        bounds: vec![Bounds::exact([1.0, 0.0, 0.0, 1.0]), Bounds::exact([0.0, 1.0, 0.0, 1.0])],
    };
    let mut target = Image::solid(2, 2, [0.0; 4]);
    target.draw(
        &source,
        &InstanceDefinition {
            transform: Transform2D {
                position: [1.0, 2.0],
                scale: [-1.0, 1.0],
                rotation_degrees: 90.0,
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
fn preview_uses_black_active_area_without_workbench_theme_bleed() {
    let image = Image::solid(1, 1, [0.0, 1.0, 0.0, 0.75]);
    assert_eq!(image.preview(), vec![[0, 224, 0, 255]]);
    assert_eq!(image.preview_bounds(), vec![([0, 224, 0, 255], [0, 224, 0, 255])]);
}

#[test]
fn unorm_bounds_keep_legal_half_steps_without_a_blanket_output_tolerance() {
    let bounds = Bounds::exact([0.5 / 255.0, 0.25 / 255.0, 0.0, 1.0]).quantized();
    assert_eq!(bounds.low, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(bounds.high, [1.0 / 255.0, 0.0, 0.0, 1.0]);
    // At 0.25 ULP, only zero is legal; this is not a blanket extra output-byte tolerance.
    let low = (0.25_f64 - 0.6).ceil().max(0.0);
    let high = (0.25_f64 + 0.6).floor();
    assert_eq!((low, high), (0.0, 0.0));
}

#[test]
fn group_opacity_bounds_include_legal_half_steps_but_reject_a_missing_contributor() {
    use mapgyver::media::composition::protocol::Transform2D;
    let definition = |opacity| InstanceDefinition {
        opacity,
        transform: Transform2D {
            pivot: [0.0, 0.0],
            ..Default::default()
        },
        ..Default::default()
    };
    let mut red = Image::solid(1, 1, [0.0; 4]);
    red.draw(&Image::solid(1, 1, [1.0, 0.0, 0.0, 0.5]), &definition(0.75));
    let mut green = Image::solid(1, 1, [0.0; 4]);
    green.draw(&Image::solid(1, 1, [0.0, 1.0, 0.0, 0.75]), &definition(0.6));
    let mut combined = Image::solid(1, 1, [0.0; 4]);
    combined.draw(&red, &definition(1.0));
    combined.draw(&green, &definition(1.0));
    let mut group = Image::solid(1, 1, [0.0; 4]);
    group.draw(&combined, &definition(0.5));
    assert_eq!(group.output(), vec![[154, 217, 0, 84]]);
    let (low, high) = group.output_bounds()[0];
    let legal = [151, 215, 0, 84];
    assert!((0..4).all(|c| low[c] <= legal[c] && legal[c] <= high[c]));
    assert!(low[0] > 100, "a lost red contributor must remain outside the interval");
    assert!(
        high[0] - low[0] < 16 && high[1] - low[1] < 16,
        "bounds must remain discriminating"
    );
}
