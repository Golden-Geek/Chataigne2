//! Timing and rewind for FFmpeg's standalone animated WebP demuxer.
use super::{DecodeError, ffmpeg, metadata::open_local};
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

mod packets;
mod parallel;
use packets::{MAX_PATCH_BYTES, matches_vp8_header, overwrite_rect};
pub(super) use parallel::{PACKET_RESERVATION, Parallel, WORKERS};

const ANIMATION_FORMAT: &str = "webp_anim";
const MAX_CHUNKS: usize = 1_000_000;
type Interrupt = Arc<Mutex<dyn FnMut() -> bool + Send>>;

pub(super) fn set_color_metadata(frame: &mut ffmpeg::frame::Video) {
    use ffmpeg::util::color::{Range, Space, TransferCharacteristic};

    // RFC 9649 assumes sRGB for unprofiled WebP and recommends Rec. 601 for VP8 YCbCr.
    if matches!(
        frame.format(),
        ffmpeg::format::Pixel::YUV420P | ffmpeg::format::Pixel::YUVA420P
    ) {
        frame.set_color_space(Space::SMPTE170M);
        frame.set_color_range(Range::MPEG);
    } else {
        frame.set_color_space(Space::RGB);
        frame.set_color_range(Range::JPEG);
    }
    frame.set_color_transfer_characteristic(TransferCharacteristic::IEC61966_2_1);
}

pub(super) struct Animation {
    path: PathBuf,
    interrupt: Interrupt,
    pub duration_ms: i64,
    pub(super) lookahead: VecDeque<ffmpeg::Packet>,
    pub(super) independent_patches: bool,
    pub(super) parallel: Option<Parallel>,
    pub(super) started: bool,
}

pub(super) fn open_video(
    path: &Path,
    interrupt: impl FnMut() -> bool + Send + 'static,
) -> Result<(ffmpeg::format::context::Input, Option<Animation>), DecodeError> {
    let interrupt: Interrupt = Arc::new(Mutex::new(interrupt));
    let input = open_local(path, {
        let interrupt = interrupt.clone();
        move || cancelled(&interrupt)
    })?;
    let animation = if input.format().name() == ANIMATION_FORMAT {
        let inspected = inspect(path, || cancelled(&interrupt))?;
        Some(Animation {
            path: path.to_owned(),
            interrupt,
            duration_ms: inspected.duration_ms,
            lookahead: VecDeque::new(),
            independent_patches: inspected.independent_patches,
            parallel: None,
            started: false,
        })
    } else {
        None
    };
    Ok((input, animation))
}

fn cancelled(interrupt: &Interrupt) -> bool {
    (interrupt.lock().unwrap())()
}

impl Animation {
    pub fn rewind(&self) -> Result<ffmpeg::format::context::Input, DecodeError> {
        // Later frames can depend on the first canvas. The demuxer's generic seek does
        // not restore its animation state or guarantee it lands on that canvas.
        let interrupt = self.interrupt.clone();
        let input = open_local(&self.path, move || cancelled(&interrupt))?;
        if input.format().name() != ANIMATION_FORMAT {
            return Err(DecodeError::CompanionMismatch);
        }
        Ok(input)
    }
}

struct Inspection {
    duration_ms: i64,
    independent_patches: bool,
}

fn inspect(path: &Path, mut interrupt: impl FnMut() -> bool) -> Result<Inspection, DecodeError> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut header = [0u8; 12];
    reader.read_exact(&mut header)?;
    let end = u64::from(u32::from_le_bytes(header[4..8].try_into().unwrap())) + 8;
    if &header[..4] != b"RIFF" || &header[8..] != b"WEBP" || end != file_len || end < 12 {
        return Err(ffmpeg::Error::InvalidData.into());
    }

    let mut remaining = end - 12;
    let mut duration = 0i64;
    let mut chunks = 0;
    let mut canvas = None;
    let mut independent = false;
    let mut first = true;
    while remaining != 0 {
        if interrupt() {
            return Err(ffmpeg::Error::Exit.into());
        }
        if chunks == MAX_CHUNKS {
            return Err(DecodeError::LimitExceeded);
        }
        chunks += 1;
        let mut chunk = [0u8; 8];
        remaining = remaining.checked_sub(8).ok_or(ffmpeg::Error::InvalidData)?;
        reader.read_exact(&mut chunk)?;
        let size = u64::from(u32::from_le_bytes(chunk[4..].try_into().unwrap()));
        let skip = size.next_multiple_of(2);
        remaining = remaining.checked_sub(skip).ok_or(ffmpeg::Error::InvalidData)?;
        let chunk_end = end - remaining;
        if chunks == 1 && &chunk[..4] == b"VP8X" && size == 10 {
            let mut header = [0u8; 10];
            reader.read_exact(&mut header)?;
            let dimension = |i| 1 + u32::from_le_bytes([header[i], header[i + 1], header[i + 2], 0]);
            canvas = Some((dimension(4), dimension(7)));
            // Keep mixed, color-profiled or non-alpha files on FFmpeg's canvas path.
            independent = header[0] & 0x32 == 0x12;
        }
        if &chunk[..4] == b"ANMF" {
            if size < 16 {
                return Err(ffmpeg::Error::InvalidData.into());
            }
            let mut frame = [0u8; 16];
            reader.read_exact(&mut frame)?;
            let delay = u32::from_le_bytes([frame[12], frame[13], frame[14], 0]);
            // Match the pinned demuxer's delay normalization so the last frame's
            // declared end agrees with the packet timestamps delivered to the decoder.
            duration += if delay <= 10 { 100 } else { i64::from(delay) };
            if independent {
                let shape = canvas.unwrap();
                independent = if let Some(rect) = overwrite_rect(&frame, shape)
                    && (!first || rect.is_full(shape))
                    && size <= MAX_PATCH_BYTES as u64
                {
                    independent_payload(&mut reader, size - 16, rect.extent())?
                } else {
                    false
                };
            }
            first = false;
        }
        reader.seek(SeekFrom::Start(chunk_end))?;
    }
    if duration == 0 {
        return Err(ffmpeg::Error::InvalidData.into());
    }
    Ok(Inspection {
        duration_ms: duration,
        independent_patches: independent && !first,
    })
}

fn independent_payload(reader: &mut BufReader<File>, len: u64, extent: (u32, u32)) -> Result<bool, DecodeError> {
    if len < 28 {
        return Ok(false);
    }
    let mut header = [0u8; 9];
    reader.read_exact(&mut header)?;
    let alpha = u64::from(u32::from_le_bytes(header[4..8].try_into().unwrap()));
    let padded_alpha = alpha.next_multiple_of(2);
    if &header[..4] != b"ALPH" || alpha <= 1 || header[8] & 3 > 1 || padded_alpha + 26 > len {
        return Ok(false);
    }
    reader.seek_relative((padded_alpha - 1) as i64)?;
    let mut vp8 = [0u8; 18];
    reader.read_exact(&mut vp8)?;
    let size = u64::from(u32::from_le_bytes(vp8[4..8].try_into().unwrap()));
    Ok(&vp8[..4] == b"VP8 "
        && size >= 10
        && 16 + padded_alpha + size.next_multiple_of(2) == len
        && matches_vp8_header(&vp8[8..], extent))
}

#[cfg(test)]
mod tests;
