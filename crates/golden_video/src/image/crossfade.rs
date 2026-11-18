use super::{ImageData, ImageError, ImageStorageBudget};
use crate::{AlphaMode, ColorEncoding, LinearColor, Opacity, PixelFormat};
use std::sync::OnceLock;

/// Blend two software-decoded SDR images in linear premultiplied space, then encode back
/// to their shared straight-alpha sRGB RGBA8 descriptor. Reserve the result in the caller's
/// storage budget before allocating it. Run on the decode worker, never the engine loop.
///
/// Callers with shared input ownership can reuse the corresponding image at either endpoint.
pub fn crossfade_srgba8(
    outgoing: &ImageData,
    incoming: &ImageData,
    incoming_weight: Opacity,
    budget: &ImageStorageBudget,
) -> Result<ImageData, ImageError> {
    let descriptor = outgoing.descriptor();
    if descriptor != incoming.descriptor()
        || descriptor.format != PixelFormat::Rgba8Unorm
        || descriptor.color_encoding != ColorEncoding::Srgb
        || descriptor.alpha_mode != AlphaMode::Straight
    {
        return Err(ImageError::CrossfadeDescriptor);
    }
    let bytes = outgoing.pixels().len();
    let storage = budget.reserve(bytes).ok_or(ImageError::Capacity)?;
    let mut output = Vec::new();
    output.try_reserve_exact(bytes).map_err(|_| ImageError::Capacity)?;
    let incoming_weight = incoming_weight.get();
    if incoming_weight == 0.0 {
        output.extend_from_slice(outgoing.pixels());
    } else if incoming_weight == 1.0 {
        output.extend_from_slice(incoming.pixels());
    } else {
        let tables = srgb_tables();
        let outgoing_weight = 1.0 - incoming_weight;
        for (tail, head) in outgoing.pixels().chunks_exact(4).zip(incoming.pixels().chunks_exact(4)) {
            // Weights include straight coverage before the mix. Unpremultiply using its
            // unquantized coverage so transparent pixels cannot leak their hidden color.
            let tail_alpha = (f32::from(tail[3]) / 255.0) * outgoing_weight;
            let head_alpha = (f32::from(head[3]) / 255.0) * incoming_weight;
            let alpha = tail_alpha + head_alpha;
            let alpha_byte = (alpha * 255.0).round() as u8;
            if alpha_byte == 0 {
                output.extend_from_slice(&[0; 4]);
                continue;
            }
            let inverse_alpha = alpha.recip();
            for index in 0..3 {
                let premultiplied = tables.linear[usize::from(tail[index])] * tail_alpha
                    + tables.linear[usize::from(head[index])] * head_alpha;
                let value = premultiplied * inverse_alpha;
                output.push(tables.thresholds.partition_point(|threshold| *threshold <= value) as u8);
            }
            output.push(alpha_byte);
        }
    }
    ImageData::new(descriptor, output)?.with_storage(storage)
}

struct SrgbTables {
    linear: [f32; 256],
    thresholds: [f32; 255],
}

fn srgb_tables() -> &'static SrgbTables {
    static TABLES: OnceLock<SrgbTables> = OnceLock::new();
    TABLES.get_or_init(|| SrgbTables {
        linear: std::array::from_fn(|byte| LinearColor::from_srgba8([byte as u8, 0, 0, 255]).components()[0]),
        // A rounded sRGB byte changes at the linear image of each half-byte threshold.
        // Searching this table avoids a transfer-function powf for every output component.
        thresholds: std::array::from_fn(|byte| {
            let encoded = (byte as f32 + 0.5) / 255.0;
            if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            }
        }),
    })
}

#[cfg(test)]
mod tests;
