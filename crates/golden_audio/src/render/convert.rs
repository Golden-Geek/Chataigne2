use crate::{AudioError, AudioErrorCategory};

use super::PlanarBuffer;

#[derive(Debug)]
pub enum InterleavedInput<'a> {
    I8(&'a [i8]),
    F32(&'a [f32]),
    F64(&'a [f64]),
    I16(&'a [i16]),
    I24(&'a [i32]),
    I32(&'a [i32]),
    I64(&'a [i64]),
    U8(&'a [u8]),
    U16(&'a [u16]),
    U24(&'a [u32]),
    U32(&'a [u32]),
    U64(&'a [u64]),
}

impl InterleavedInput<'_> {
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::I8(samples) => samples.len(),
            Self::F32(samples) => samples.len(),
            Self::F64(samples) => samples.len(),
            Self::I16(samples) => samples.len(),
            Self::I24(samples) | Self::I32(samples) => samples.len(),
            Self::I64(samples) => samples.len(),
            Self::U8(samples) => samples.len(),
            Self::U16(samples) => samples.len(),
            Self::U24(samples) | Self::U32(samples) => samples.len(),
            Self::U64(samples) => samples.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    fn normalized(&self, index: usize) -> f32 {
        match self {
            Self::I8(samples) => signed_to_f32(i64::from(samples[index]), 128, 127),
            Self::F32(samples) => finite_or_zero(samples[index]),
            Self::F64(samples) => finite_or_zero_f64(samples[index]),
            Self::I16(samples) => signed_to_f32(i64::from(samples[index]), 32_768, 32_767),
            Self::I24(samples) => signed_to_f32(
                i64::from(samples[index].clamp(-8_388_608, 8_388_607)),
                8_388_608,
                8_388_607,
            ),
            Self::I32(samples) => signed_to_f32(i64::from(samples[index]), 2_147_483_648, 2_147_483_647),
            Self::I64(samples) => signed_to_f32_i128(i128::from(samples[index]), 1_i128 << 63, (1_i128 << 63) - 1),
            Self::U8(samples) => unsigned_to_f32(u64::from(samples[index]), 255),
            Self::U16(samples) => unsigned_to_f32(u64::from(samples[index]), 65_535),
            Self::U24(samples) => unsigned_to_f32(u64::from(samples[index].min(16_777_215)), 16_777_215),
            Self::U32(samples) => unsigned_to_f32(u64::from(samples[index]), 4_294_967_295),
            Self::U64(samples) => unsigned_to_f32_u128(u128::from(samples[index]), u128::from(u64::MAX)),
        }
    }

    #[must_use]
    fn is_non_finite(&self, index: usize) -> bool {
        match self {
            Self::F32(samples) => !samples[index].is_finite(),
            Self::F64(samples) => !samples[index].is_finite(),
            Self::I8(_)
            | Self::I16(_)
            | Self::I24(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U24(_)
            | Self::U32(_)
            | Self::U64(_) => false,
        }
    }
}

#[derive(Debug)]
pub enum InterleavedOutput<'a> {
    I8(&'a mut [i8]),
    F32(&'a mut [f32]),
    F64(&'a mut [f64]),
    I16(&'a mut [i16]),
    I24(&'a mut [i32]),
    I32(&'a mut [i32]),
    I64(&'a mut [i64]),
    U8(&'a mut [u8]),
    U16(&'a mut [u16]),
    U24(&'a mut [u32]),
    U32(&'a mut [u32]),
    U64(&'a mut [u64]),
}

impl InterleavedOutput<'_> {
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::I8(samples) => samples.len(),
            Self::F32(samples) => samples.len(),
            Self::F64(samples) => samples.len(),
            Self::I16(samples) => samples.len(),
            Self::I24(samples) | Self::I32(samples) => samples.len(),
            Self::I64(samples) => samples.len(),
            Self::U8(samples) => samples.len(),
            Self::U16(samples) => samples.len(),
            Self::U24(samples) | Self::U32(samples) => samples.len(),
            Self::U64(samples) => samples.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn fill_silence(&mut self) {
        match self {
            Self::I8(samples) => samples.fill(0),
            Self::F32(samples) => samples.fill(0.0),
            Self::F64(samples) => samples.fill(0.0),
            Self::I16(samples) => samples.fill(0),
            Self::I24(samples) | Self::I32(samples) => samples.fill(0),
            Self::I64(samples) => samples.fill(0),
            Self::U8(samples) => samples.fill(1 << 7),
            Self::U16(samples) => samples.fill(1 << 15),
            Self::U24(samples) => samples.fill(1 << 23),
            Self::U32(samples) => samples.fill(1 << 31),
            Self::U64(samples) => samples.fill(1 << 63),
        }
    }

    fn set_normalized(&mut self, index: usize, sample: f32) {
        match self {
            Self::I8(samples) => samples[index] = f32_to_signed(sample, 128, 127) as i8,
            Self::F32(samples) => samples[index] = sample,
            Self::F64(samples) => samples[index] = f64::from(sample),
            Self::I16(samples) => samples[index] = f32_to_signed(sample, 32_768, 32_767) as i16,
            Self::I24(samples) => {
                samples[index] = f32_to_signed(sample, 8_388_608, 8_388_607) as i32;
            }
            Self::I32(samples) => {
                samples[index] = f32_to_signed(sample, 2_147_483_648, 2_147_483_647) as i32;
            }
            Self::I64(samples) => {
                samples[index] = f32_to_signed_i128(sample, 1_i128 << 63, (1_i128 << 63) - 1) as i64;
            }
            Self::U8(samples) => samples[index] = f32_to_unsigned(sample, 255) as u8,
            Self::U16(samples) => samples[index] = f32_to_unsigned(sample, 65_535) as u16,
            Self::U24(samples) => {
                samples[index] = f32_to_unsigned(sample, 16_777_215) as u32;
            }
            Self::U32(samples) => {
                samples[index] = f32_to_unsigned(sample, 4_294_967_295) as u32;
            }
            Self::U64(samples) => {
                samples[index] = f32_to_unsigned_u128(sample, u128::from(u64::MAX)) as u64;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ConversionStats {
    pub clipped_samples: u64,
    pub non_finite_samples: u64,
}

pub fn deinterleave(
    input: InterleavedInput<'_>,
    channels: usize,
    frames: usize,
    destination: &mut PlanarBuffer,
    destination_frame_offset: usize,
) -> Result<ConversionStats, AudioError> {
    validate_shape(input.len(), channels, frames, destination, destination_frame_offset)?;
    let mut stats = ConversionStats::default();
    for frame in 0..frames {
        for channel in 0..channels {
            let sample_index = frame * channels + channel;
            if input.is_non_finite(sample_index) {
                stats.non_finite_samples += 1;
            }
            let sample = input.normalized(sample_index);
            destination.set_sample(channel, destination_frame_offset + frame, sample);
        }
    }
    Ok(stats)
}

pub fn interleave(
    source: &PlanarBuffer,
    source_frame_offset: usize,
    channels: usize,
    frames: usize,
    mut output: InterleavedOutput<'_>,
) -> Result<ConversionStats, AudioError> {
    validate_shape(output.len(), channels, frames, source, source_frame_offset)?;
    let mut stats = ConversionStats::default();
    for frame in 0..frames {
        for channel in 0..channels {
            let raw = source.sample(channel, source_frame_offset + frame);
            let sample = if raw.is_finite() {
                if !(-1.0..=1.0).contains(&raw) {
                    stats.clipped_samples += 1;
                }
                raw.clamp(-1.0, 1.0)
            } else {
                stats.non_finite_samples += 1;
                0.0
            };
            output.set_normalized(frame * channels + channel, sample);
        }
    }
    Ok(stats)
}

fn validate_shape(
    interleaved_len: usize,
    channels: usize,
    frames: usize,
    planar: &PlanarBuffer,
    planar_frame_offset: usize,
) -> Result<(), AudioError> {
    if planar.channels() < channels {
        return Err(AudioError::invalid_configuration(format!(
            "planar buffer has {} channels but conversion requires {channels}",
            planar.channels()
        )));
    }
    planar.validate_range(planar_frame_offset, frames)?;
    let required = channels.checked_mul(frames).ok_or_else(|| {
        AudioError::new(
            AudioErrorCategory::CapacityExceeded,
            "interleaved sample count overflowed",
        )
    })?;
    if interleaved_len < required {
        return Err(AudioError::invalid_configuration(format!(
            "interleaved buffer has {interleaved_len} samples but conversion requires {required}"
        )));
    }
    Ok(())
}

fn finite_or_zero(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

fn finite_or_zero_f64(sample: f64) -> f32 {
    if sample.is_finite() { sample as f32 } else { 0.0 }
}

fn signed_to_f32(sample: i64, negative_scale: i64, positive_scale: i64) -> f32 {
    if sample < 0 {
        sample as f32 / negative_scale as f32
    } else {
        sample as f32 / positive_scale as f32
    }
}

fn signed_to_f32_i128(sample: i128, negative_scale: i128, positive_scale: i128) -> f32 {
    if sample < 0 {
        (sample as f64 / negative_scale as f64) as f32
    } else {
        (sample as f64 / positive_scale as f64) as f32
    }
}

fn unsigned_to_f32(sample: u64, maximum: u64) -> f32 {
    (sample as f64 / maximum as f64 * 2.0 - 1.0) as f32
}

fn unsigned_to_f32_u128(sample: u128, maximum: u128) -> f32 {
    (sample as f64 / maximum as f64 * 2.0 - 1.0) as f32
}

fn f32_to_signed(sample: f32, negative_scale: i64, positive_scale: i64) -> i64 {
    if sample < 0.0 {
        (f64::from(sample) * negative_scale as f64).round() as i64
    } else {
        (f64::from(sample) * positive_scale as f64).round() as i64
    }
}

fn f32_to_signed_i128(sample: f32, negative_scale: i128, positive_scale: i128) -> i128 {
    let scaled = if sample < 0.0 {
        (f64::from(sample) * negative_scale as f64).round() as i128
    } else {
        (f64::from(sample) * positive_scale as f64).round() as i128
    };
    scaled.clamp(-negative_scale, positive_scale)
}

fn f32_to_unsigned(sample: f32, maximum: u64) -> u64 {
    ((f64::from(sample) * 0.5 + 0.5) * maximum as f64).round() as u64
}

fn f32_to_unsigned_u128(sample: f32, maximum: u128) -> u128 {
    let scaled = ((f64::from(sample) * 0.5 + 0.5) * maximum as f64).round() as u128;
    scaled.min(maximum)
}
