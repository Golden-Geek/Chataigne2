use std::io::{BufReader, Read, Seek};

use golden_assets::{AssetDecoder, AssetInput, PreparationFailure};
use golden_video::image::ImageData;
use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};
use image::{ColorType, ImageDecoder, ImageFormat, metadata::Orientation};

const MAX_DIMENSION: u32 = 16_384;
const FIXED_WORKSPACE: usize = 64 * 1024;

/// Static SDR Picture preparation. Untagged input uses the app's sRGB convention; explicit
/// unsupported color descriptions fail rather than being silently relabelled. CPU codec work
/// runs only inside the admitted asset worker, never on the engine/UI owner.
#[derive(Clone, Copy, Debug, Default)]
pub struct PictureDecoder;

impl AssetDecoder for PictureDecoder {
    type Output = ImageData;

    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<Self::Output, PreparationFailure> {
        input.check()?;
        if maximum_bytes <= FIXED_WORKSPACE {
            return Err(PreparationFailure::ResourcePressure);
        }
        let mut signature = [0; 16];
        let count = input.read(&mut signature).map_err(PreparationFailure::from)?;
        input.rewind().map_err(PreparationFailure::from)?;
        let format = image::guess_format(&signature[..count]).map_err(image_failure)?;
        let (width, height, pixels, orientation) = match format {
            ImageFormat::Png => decode_png(input, maximum_bytes)?,
            ImageFormat::Jpeg => decode_jpeg(input, maximum_bytes)?,
            _ => return Err(PreparationFailure::Decode),
        };
        input.check()?;
        let rgba = image::RgbaImage::from_raw(width, height, pixels).ok_or(PreparationFailure::Decode)?;
        let mut oriented = image::DynamicImage::ImageRgba8(rgba);
        oriented.apply_orientation(orientation);
        input.check()?;
        let descriptor = TextureDescriptor {
            extent: Extent2D::new(oriented.width(), oriented.height()).map_err(|_| PreparationFailure::Decode)?,
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        };
        ImageData::new(descriptor, oriented.into_rgba8().into_raw()).map_err(|_| PreparationFailure::Decode)
    }

    fn retained_bytes(&self, value: &Self::Output) -> usize {
        value.retained_bytes()
    }
}

type Decoded = (u32, u32, Vec<u8>, Orientation);

fn pixel_count(width: u32, height: u32) -> Result<usize, PreparationFailure> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(PreparationFailure::Decode);
    }
    usize::try_from(u64::from(width) * u64::from(height)).map_err(|_| PreparationFailure::ResourcePressure)
}

fn allocate(bytes: usize) -> Result<Vec<u8>, PreparationFailure> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(bytes)
        .map_err(|_| PreparationFailure::ResourcePressure)?;
    buffer.resize(bytes, 0);
    Ok(buffer)
}

fn image_failure(error: image::ImageError) -> PreparationFailure {
    match error {
        image::ImageError::Limits(_) => PreparationFailure::ResourcePressure,
        _ => PreparationFailure::Decode,
    }
}

fn png_failure(error: png::DecodingError) -> PreparationFailure {
    match error {
        png::DecodingError::LimitsExceeded => PreparationFailure::ResourcePressure,
        _ => PreparationFailure::Decode,
    }
}

fn decode_png(input: &mut AssetInput, maximum_bytes: usize) -> Result<Decoded, PreparationFailure> {
    let available = maximum_bytes - FIXED_WORKSPACE;
    // The decoder's own quota and our raw/RGBA/orientation buffers share the reservation.
    let codec_bytes = available / 2;
    let mut decoder = png::Decoder::new_with_limits(BufReader::new(&mut *input), png::Limits { bytes: codec_bytes });
    decoder.set_ignore_text_chunk(true);
    decoder.set_transformations(png::Transformations::EXPAND);
    let header = decoder.read_header_info().map_err(png_failure)?;
    let pixels = pixel_count(header.width, header.height)?;
    if header.bit_depth == png::BitDepth::Sixteen {
        return Err(PreparationFailure::Decode);
    }
    // The reader/codec is dropped before conversion; conversion's raw buffer is dropped
    // before orientation. Charge the peak live phase, rather than adding disjoint buffers.
    let raw_work = pixels.checked_mul(4).ok_or(PreparationFailure::ResourcePressure)?;
    let conversion_work = pixels.checked_mul(8).ok_or(PreparationFailure::ResourcePressure)?;
    if raw_work > available - codec_bytes || conversion_work > available {
        return Err(PreparationFailure::ResourcePressure);
    }
    let mut reader = decoder.read_info().map_err(png_failure)?;
    let info = reader.info();
    // These accessors reflect parsed gAMA/cHRM; source_* are encoder convenience fields.
    if info.animation_control.is_some()
        || info.icc_profile.is_some()
        || info.coding_independent_code_points.is_some()
        || info.mastering_display_color_volume.is_some()
        || info.content_light_level.is_some()
        || (info.srgb.is_none() && (info.gamma().is_some() || info.chromaticities().is_some()))
    {
        return Err(PreparationFailure::Decode);
    }
    let orientation = info
        .exif_metadata
        .as_ref()
        .and_then(|bytes| Orientation::from_exif_chunk(bytes))
        .unwrap_or(Orientation::NoTransforms);
    let (width, height) = (info.width, info.height);
    let raw_bytes = reader
        .output_buffer_size()
        .ok_or(PreparationFailure::ResourcePressure)?;
    if raw_bytes > pixels.checked_mul(4).ok_or(PreparationFailure::ResourcePressure)? {
        return Err(PreparationFailure::Decode);
    }
    let mut raw = allocate(raw_bytes)?;
    let output = reader.next_frame(&mut raw).map_err(png_failure)?;
    if output.bit_depth != png::BitDepth::Eight || output.width != width || output.height != height {
        return Err(PreparationFailure::Decode);
    }
    let color = match output.color_type {
        png::ColorType::Grayscale => ColorType::L8,
        png::ColorType::GrayscaleAlpha => ColorType::La8,
        png::ColorType::Rgb => ColorType::Rgb8,
        png::ColorType::Rgba => ColorType::Rgba8,
        png::ColorType::Indexed => return Err(PreparationFailure::Decode),
    };
    drop(reader);
    input.check()?;
    Ok((width, height, rgba(raw, color, pixels, input)?, orientation))
}

fn decode_jpeg(input: &mut AssetInput, maximum_bytes: usize) -> Result<Decoded, PreparationFailure> {
    // image's JPEG adapter copies the encoded source before reading headers. Charge that copy,
    // metadata and capacity headroom before construction, separately from asset source staging.
    let encoded = usize::try_from(input.length()).map_err(|_| PreparationFailure::ResourcePressure)?;
    let source_work = encoded
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(FIXED_WORKSPACE))
        .ok_or(PreparationFailure::ResourcePressure)?;
    if source_work >= maximum_bytes {
        return Err(PreparationFailure::ResourcePressure);
    }
    let mut decoder = image::codecs::jpeg::JpegDecoder::new(BufReader::new(&mut *input)).map_err(image_failure)?;
    let (width, height) = decoder.dimensions();
    let pixels = pixel_count(width, height)?;
    // The pinned zune backend has no strict total-allocation limit. This conservative padded
    // charge includes progressive coefficients, component/row scratch, raw/RGBA and orientation.
    // It is declared codec admission, not an allocator/OS-memory guarantee.
    let padded_pixels = usize::try_from(u64::from(width.div_ceil(16) * 16) * u64::from(height.div_ceil(16) * 16))
        .map_err(|_| PreparationFailure::ResourcePressure)?;
    let pixel_work = padded_pixels
        .checked_mul(64)
        .ok_or(PreparationFailure::ResourcePressure)?;
    if pixel_work > maximum_bytes - source_work {
        return Err(PreparationFailure::ResourcePressure);
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some((maximum_bytes - source_work) as u64);
    decoder.set_limits(limits).map_err(image_failure)?;
    if decoder.icc_profile().map_err(image_failure)?.is_some() {
        return Err(PreparationFailure::Decode);
    }
    let orientation = decoder.orientation().map_err(image_failure)?;
    let color = decoder.color_type();
    if !matches!(
        color,
        ColorType::L8 | ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8
    ) {
        return Err(PreparationFailure::Decode);
    }
    let raw_bytes = usize::try_from(decoder.total_bytes()).map_err(|_| PreparationFailure::ResourcePressure)?;
    if raw_bytes > pixels.checked_mul(4).ok_or(PreparationFailure::ResourcePressure)? {
        return Err(PreparationFailure::Decode);
    }
    let mut raw = allocate(raw_bytes)?;
    decoder.read_image(&mut raw).map_err(image_failure)?;
    input.check()?;
    Ok((width, height, rgba(raw, color, pixels, input)?, orientation))
}

fn rgba(raw: Vec<u8>, color: ColorType, pixels: usize, input: &AssetInput) -> Result<Vec<u8>, PreparationFailure> {
    let channels = usize::from(color.channel_count());
    if raw.len()
        != pixels
            .checked_mul(channels)
            .ok_or(PreparationFailure::ResourcePressure)?
    {
        return Err(PreparationFailure::Decode);
    }
    if color == ColorType::Rgba8 {
        return Ok(raw);
    }
    let mut output = allocate(pixels.checked_mul(4).ok_or(PreparationFailure::ResourcePressure)?)?;
    for (index, (source, target)) in raw.chunks_exact(channels).zip(output.chunks_exact_mut(4)).enumerate() {
        if index % 4096 == 0 {
            input.check()?;
        }
        match color {
            ColorType::L8 => target.copy_from_slice(&[source[0], source[0], source[0], 255]),
            ColorType::La8 => target.copy_from_slice(&[source[0], source[0], source[0], source[1]]),
            ColorType::Rgb8 => target.copy_from_slice(&[source[0], source[1], source[2], 255]),
            _ => return Err(PreparationFailure::Decode),
        }
    }
    Ok(output)
}
