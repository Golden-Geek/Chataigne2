//! DXV high-quality software planes use centered 4:2:0 YCgCo and an optional straight alpha plane.
use super::{ConversionError, ffmpeg, scratch};
use crate::image::ImageStorageBudget;
use ffmpeg::{
    format::Pixel,
    software::scaling::{Context, Flags},
    util::color::Range,
};

pub(super) struct Scaler(Option<Resize>);

struct Resize {
    rgba: ffmpeg::frame::Video,
    scaler: Context,
}

impl Scaler {
    pub fn new(
        input: &ffmpeg::frame::Video,
        width: u32,
        height: u32,
        budget: &ImageStorageBudget,
    ) -> Result<Self, ConversionError> {
        if !matches!(input.format(), Pixel::YUV420P | Pixel::YUVA420P) {
            return Err(ConversionError::UnsupportedMatrix);
        }
        if (input.width(), input.height()) == (width, height) {
            return Ok(Self(None));
        }
        let scaler = Context::get(
            Pixel::RGBA,
            input.width(),
            input.height(),
            Pixel::RGBA,
            width,
            height,
            Flags::BILINEAR,
        )
        .map_err(|error| ConversionError::Failed(error.to_string()))?;
        Ok(Self(Some(Resize {
            rgba: scratch::create(input.width(), input.height(), budget)?,
            scaler,
        })))
    }

    pub fn run(
        &mut self,
        input: &ffmpeg::frame::Video,
        output: &mut ffmpeg::frame::Video,
    ) -> Result<(), ConversionError> {
        if let Some(resize) = &mut self.0 {
            convert(input, &mut resize.rgba);
            resize
                .scaler
                .run(&resize.rgba, output)
                .map_err(|error| ConversionError::Failed(error.to_string()))
        } else {
            convert(input, output);
            Ok(())
        }
    }
}

fn chroma_axis(pixel: usize, count: usize) -> (usize, usize, f32) {
    let cell = pixel / 2;
    // Chroma samples sit between adjacent luma pixels. Extend the edge sample.
    if pixel.is_multiple_of(2) {
        (cell.saturating_sub(1), cell, 0.75)
    } else {
        (cell, (cell + 1).min(count - 1), 0.25)
    }
}

fn row(frame: &ffmpeg::frame::Video, plane: usize, index: usize) -> &[u8] {
    let stride = frame.stride(plane);
    &frame.data(plane)[index * stride..(index + 1) * stride]
}

fn convert(input: &ffmpeg::frame::Video, output: &mut ffmpeg::frame::Video) {
    let width = input.width() as usize;
    let height = input.height() as usize;
    let alpha = input.format() == Pixel::YUVA420P;
    let (y_offset, y_scale, chroma_scale) = if input.color_range() == Range::JPEG {
        (0.0, 1.0, 1.0)
    } else {
        (16.0, 255.0 / 219.0, 255.0 / 224.0)
    };
    let stride = output.stride(0);
    let bytes = output.data_mut(0);
    for (y, destination) in bytes.chunks_exact_mut(stride).take(height).enumerate() {
        let (top, bottom, vertical) = chroma_axis(y, height.div_ceil(2));
        let luma = row(input, 0, y);
        let coverage = alpha.then(|| row(input, 3, y));
        let cg = (row(input, 1, top), row(input, 1, bottom));
        let co = (row(input, 2, top), row(input, 2, bottom));
        for (x, pixel) in destination[..width * 4].chunks_exact_mut(4).enumerate() {
            let (left, right, horizontal) = chroma_axis(x, width.div_ceil(2));
            let chroma = |(top, bottom): (&[u8], &[u8])| {
                let top = f32::from(top[left]) * (1.0 - horizontal) + f32::from(top[right]) * horizontal;
                let bottom = f32::from(bottom[left]) * (1.0 - horizontal) + f32::from(bottom[right]) * horizontal;
                (top * (1.0 - vertical) + bottom * vertical - 128.0) * chroma_scale
            };
            let cg = chroma(cg);
            let co = chroma(co);
            let y = (f32::from(luma[x]) - y_offset) * y_scale;
            pixel[0] = (y - cg + co).round() as u8;
            pixel[1] = (y + cg).round() as u8;
            pixel[2] = (y - cg - co).round() as u8;
            pixel[3] = coverage.map_or(255, |row| row[x]);
        }
    }
}
