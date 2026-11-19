use std::io::SeekFrom;

use image_webp::{DecodingError, WebPDecoder};

use super::*;

pub(super) fn decode(input: &mut AssetInput, maximum_bytes: usize) -> Result<Decoded, PreparationFailure> {
    let source = usize::try_from(input.length()).map_err(|_| PreparationFailure::ResourcePressure)?;
    let coded_extent = check_container(input)?;
    let mut decoder = WebPDecoder::new(BufReader::new(&mut *input)).map_err(failure)?;
    let (width, height) = decoder.dimensions();
    let pixels = pixel_count(width, height)?;
    if decoder.is_animated() || coded_extent.is_some_and(|extent| extent != (width, height)) {
        return Err(PreparationFailure::Decode);
    }
    // Reserve padded codec work plus packed output. The VP8 backend can allocate one
    // remaining partition from an untrusted 24-bit length before a short read rejects it.
    let padded = width.div_ceil(16) as usize * height.div_ceil(16) as usize * 256;
    let partition_slack = if coded_extent.is_some() { 1 << 24 } else { 0 };
    let work = padded
        .checked_mul(12)
        .and_then(|pixels| source.checked_mul(3).and_then(|source| pixels.checked_add(source)))
        .and_then(|bytes| bytes.checked_add(partition_slack + FIXED_WORKSPACE))
        .ok_or(PreparationFailure::ResourcePressure)?;
    if work > maximum_bytes {
        return Err(PreparationFailure::ResourcePressure);
    }
    decoder.set_memory_limit(maximum_bytes - FIXED_WORKSPACE);
    let exif = decoder.exif_metadata().map_err(failure)?;
    let orientation = exif
        .as_deref()
        .and_then(Orientation::from_exif_chunk)
        .unwrap_or(Orientation::NoTransforms);
    drop(exif);
    let color = if decoder.has_alpha() {
        ColorType::Rgba8
    } else {
        ColorType::Rgb8
    };
    let mut raw = allocate(
        decoder
            .output_buffer_size()
            .ok_or(PreparationFailure::ResourcePressure)?,
    )?;
    decoder.read_image(&mut raw).map_err(failure)?;
    drop(decoder);
    input.check()?;
    Ok((width, height, rgba(raw, color, pixels, input)?, orientation))
}

/// Reject animated or explicitly profiled data before allocating codec storage. VP8X
/// declares a canvas separately from its VP8 keyframe, whose size drives native allocations.
fn check_container(input: &mut AssetInput) -> Result<Option<(u32, u32)>, PreparationFailure> {
    let mut file = [0; 12];
    input.read_exact(&mut file)?;
    let end = u64::from(u32::from_le_bytes(file[4..8].try_into().unwrap())) + 8;
    if &file[..4] != b"RIFF" || &file[8..] != b"WEBP" || end != input.length() {
        return Err(PreparationFailure::Decode);
    }

    let mut coded = None;
    let mut offset = 12;
    while offset < end {
        if end - offset < 8 {
            return Err(PreparationFailure::Decode);
        }
        input.seek(SeekFrom::Start(offset))?;
        let mut chunk = [0; 8];
        input.read_exact(&mut chunk)?;
        let size = u64::from(u32::from_le_bytes(chunk[4..].try_into().unwrap()));
        offset += 8 + size.next_multiple_of(2);
        if offset > end || matches!(&chunk[..4], b"ICCP" | b"ANIM" | b"ANMF") {
            return Err(PreparationFailure::Decode);
        }
        if &chunk[..4] == b"VP8 " {
            if size < 10 || coded.is_some() {
                return Err(PreparationFailure::Decode);
            }
            let mut frame = [0; 10];
            input.read_exact(&mut frame)?;
            let width = u16::from_le_bytes([frame[6], frame[7]]) & 0x3fff;
            let height = u16::from_le_bytes([frame[8], frame[9]]) & 0x3fff;
            coded = Some((u32::from(width), u32::from(height)));
        }
    }
    input.rewind()?;
    Ok(coded)
}

fn failure(error: DecodingError) -> PreparationFailure {
    match error {
        DecodingError::IoError(error) => error.into(),
        DecodingError::MemoryLimitExceeded => PreparationFailure::ResourcePressure,
        _ => PreparationFailure::Decode,
    }
}
