//! Two bounded VP8 patch decoders with one ordered YUVA canvas on the file owner.
use super::{
    DecodeError, ffmpeg,
    packets::{MAX_PATCH_BYTES, PacketWithOmission, Rect, independent_patch},
    set_color_metadata,
};
use crate::{
    decode::{DecodedFrame, FrameStorage, software_pool::Allocator},
    image::ImageStorageBudget,
};
use golden_io::WorkerTask;
use std::{
    collections::VecDeque,
    sync::mpsc::{self, Receiver, Sender},
};

pub(in crate::decode) const WORKERS: usize = 2;
const WEBP_HEADER_BYTES: usize = 30;
pub(in crate::decode) const PACKET_RESERVATION: u64 = (WORKERS * 2 * (MAX_PATCH_BYTES + WEBP_HEADER_BYTES)) as u64;

struct Job {
    packet: ffmpeg::Packet,
    completion: Sender<Result<ffmpeg::frame::Video, DecodeError>>,
}

struct Pending {
    worker: usize,
    rect: Rect,
    pts: i64,
    omitted: Option<(i64, i64)>,
    completion: Receiver<Result<ffmpeg::frame::Video, DecodeError>>,
}

struct PatchWorker(Option<WorkerTask<Job>>);

impl PatchWorker {
    fn new(budget: ImageStorageBudget, max_pixels: u64) -> Result<Self, DecodeError> {
        let task = WorkerTask::spawn_with_capacity("golden-webp-patch", 1, move |jobs| {
            let allocator = Allocator::with_budget(budget);
            let mut decoder = open_patch_decoder(&allocator, max_pixels);
            for job in jobs {
                let Job { packet, completion } = job;
                let frame = match &mut decoder {
                    Ok(decoder) => decode_patch(decoder, &packet).map_err(|error| allocator.failure(error)),
                    Err(_) => Err(DecodeError::UnsupportedCodec),
                };
                drop(packet);
                let _ = completion.send(frame);
            }
        })?;
        Ok(Self(Some(task)))
    }
}

impl Drop for PatchWorker {
    fn drop(&mut self) {
        if let Some(thread) = self.0.take().and_then(WorkerTask::into_join_handle) {
            let _ = thread.join();
        }
    }
}

pub(in crate::decode) struct Parallel {
    workers: [PatchWorker; WORKERS],
    pending: VecDeque<Pending>,
    canvas: Option<ffmpeg::frame::Video>,
    allocator: Allocator,
    extent: (u32, u32),
}

impl Parallel {
    pub(in crate::decode) fn new(
        extent: (u32, u32),
        budget: ImageStorageBudget,
        max_pixels: u64,
    ) -> Result<Self, DecodeError> {
        Ok(Self {
            workers: [
                PatchWorker::new(budget.clone(), max_pixels)?,
                PatchWorker::new(budget.clone(), max_pixels)?,
            ],
            pending: VecDeque::with_capacity(WORKERS),
            canvas: None,
            allocator: Allocator::with_budget(budget),
            extent,
        })
    }

    pub(in crate::decode) fn reset(&mut self) {
        for pending in self.pending.drain(..) {
            let _ = pending.completion.recv();
        }
        self.canvas = None;
    }

    pub(in crate::decode) fn next(
        &mut self,
        mut read: impl FnMut() -> Result<Option<PacketWithOmission>, DecodeError>,
    ) -> Result<Option<DecodedFrame>, DecodeError> {
        for (index, worker) in self.workers.iter().enumerate() {
            if self.pending.iter().any(|pending| pending.worker == index) {
                continue;
            }
            let Some((packet, omitted)) = read()? else {
                break;
            };
            let rect = independent_patch(&packet, self.extent).ok_or(ffmpeg::Error::InvalidData)?;
            let pts = packet.pts().ok_or(DecodeError::MissingTimestamp)?;
            let standalone = standalone_file(&packet, rect)?;
            let (completion, receiver) = mpsc::channel();
            worker
                .0
                .as_ref()
                .unwrap()
                .send(Job {
                    packet: standalone,
                    completion,
                })
                .map_err(|_| DecodeError::ReopenRequired)?;
            self.pending.push_back(Pending {
                worker: index,
                rect,
                pts,
                omitted,
                completion: receiver,
            });
        }
        let Some(pending) = self.pending.pop_front() else {
            return Ok(None);
        };
        let patch = pending.completion.recv().map_err(|_| DecodeError::ReopenRequired)??;
        let mut canvas = self.apply(pending.rect, patch)?;
        set_color_metadata(&mut canvas);
        Ok(Some(DecodedFrame {
            timestamp: pending.pts,
            omitted_before: pending.omitted,
            storage: FrameStorage::Software(canvas),
        }))
    }

    fn apply(&mut self, rect: Rect, patch: ffmpeg::frame::Video) -> Result<ffmpeg::frame::Video, DecodeError> {
        if patch.format() != ffmpeg::format::Pixel::YUVA420P || (patch.width(), patch.height()) != rect.extent() {
            return Err(ffmpeg::Error::InvalidData.into());
        }
        if rect.is_full(self.extent) {
            self.canvas = Some(patch);
        } else {
            let canvas = self.canvas.as_mut().ok_or(ffmpeg::Error::InvalidData)?;
            // SAFETY: this owned frame is live. Refcounted aliases include old displayed images.
            if unsafe { ffmpeg::ffi::av_frame_is_writable(canvas.as_ptr().cast_mut()) } == 0 {
                let mut unique = self.allocator.yuva420_canvas(self.extent.0, self.extent.1)?;
                copy_planes(&mut unique, canvas, 0, 0);
                *canvas = unique;
            }
            copy_planes(canvas, &patch, rect.x, rect.y);
        }
        let mut output = ffmpeg::frame::Video::empty();
        // SAFETY: output is empty and the canvas owns refcounted planes for this copy of refs.
        let result = unsafe { ffmpeg::ffi::av_frame_ref(output.as_mut_ptr(), self.canvas.as_ref().unwrap().as_ptr()) };
        if result < 0 {
            return Err(ffmpeg::Error::from(result).into());
        }
        Ok(output)
    }
}

fn copy_planes(target: &mut ffmpeg::frame::Video, patch: &ffmpeg::frame::Video, x: u32, y: u32) {
    for plane in 0..4 {
        let shift = u32::from(plane == 1 || plane == 2);
        let columns = patch.width().div_ceil(1 << shift) as usize;
        let rows = patch.height().div_ceil(1 << shift) as usize;
        let left = (x >> shift) as usize;
        let top = (y >> shift) as usize;
        let stride = target.stride(plane);
        let input = patch.data(plane).chunks(patch.stride(plane)).take(rows);
        let output = target.data_mut(plane).chunks_mut(stride).skip(top);
        for (source, destination) in input.zip(output) {
            destination[left..left + columns].copy_from_slice(&source[..columns]);
        }
    }
}

fn standalone_file(packet: &ffmpeg::Packet, rect: Rect) -> Result<ffmpeg::Packet, DecodeError> {
    let chunks = &packet.data().ok_or(ffmpeg::Error::InvalidData)?[16..];
    let total = WEBP_HEADER_BYTES + chunks.len();
    let mut standalone = ffmpeg::Packet::new(total);
    let bytes = standalone.data_mut().ok_or(ffmpeg::Error::Other {
        errno: ffmpeg::error::ENOMEM,
    })?;
    let (width, height) = rect.extent();
    bytes[..4].copy_from_slice(b"RIFF");
    bytes[4..8].copy_from_slice(&((total - 8) as u32).to_le_bytes());
    bytes[8..24].copy_from_slice(b"WEBPVP8X\x0a\x00\x00\x00\x10\x00\x00\x00");
    bytes[24..27].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
    bytes[27..30].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
    bytes[WEBP_HEADER_BYTES..].copy_from_slice(chunks);
    Ok(standalone)
}

fn open_patch_decoder(allocator: &Allocator, max_pixels: u64) -> Result<ffmpeg::decoder::Video, DecodeError> {
    let codec = ffmpeg::decoder::find(ffmpeg::codec::Id::WEBP).ok_or(DecodeError::UnsupportedCodec)?;
    let mut context = ffmpeg::codec::context::Context::new_with_codec(codec);
    context.set_threading(ffmpeg::codec::threading::Config::count(1));
    allocator.configure(&mut context)?;
    // SAFETY: this unopened context is uniquely owned. The file admission bounds were validated.
    unsafe {
        (*context.as_mut_ptr()).max_pixels = max_pixels as i64;
    }
    Ok(context.decoder().video()?)
}

fn decode_patch(
    decoder: &mut ffmpeg::decoder::Video,
    packet: &ffmpeg::Packet,
) -> Result<ffmpeg::frame::Video, DecodeError> {
    let mut frame = ffmpeg::frame::Video::empty();
    decoder.send_packet(packet)?;
    decoder.receive_frame(&mut frame)?;
    Ok(frame)
}
