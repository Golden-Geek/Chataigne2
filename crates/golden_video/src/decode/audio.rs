//! Stereo PCM from an embedded stream, with sample timestamps relative to the Video origin.
use std::path::Path;

use super::{DecodeError, ffmpeg};
use ffmpeg::{ChannelLayout, Rational, codec, format, frame, software::resampling};

const PACKET_LIMIT: usize = 4096;
const PACKET_BYTES: usize = 64 * 1024 * 1024;
const FRAME_LIMIT: usize = 65_536;
const PACKED_F32: format::Sample = format::Sample::F32(format::sample::Type::Packed);

/// Stereo frames at the reader's chosen output sample rate and an exact signed sample index.
#[derive(Debug)]
pub struct AudioChunk {
    /// Index of the first frame relative to the source's Video start.
    pub start_frame: i64,
    /// Interleaved left/right sample pairs at the rate requested when opening the reader.
    pub samples: Vec<(f32, f32)>,
}

/// A local-file decoder to be opened and used on an I/O worker.
pub struct AudioReader {
    input: format::context::Input,
    decoder: codec::decoder::Audio,
    index: usize,
    time_base: Rational,
    origin_micros: i64,
    origin_frames: i64,
    sample_rate: u32,
    converter: Option<resampling::Context>,
    next_frame: Option<i64>,
    expected_source_frame: Option<i64>,
    draining: bool,
    ended: bool,
}

impl AudioReader {
    /// Open the best embedded audio stream. Files without audio return `Ok(None)`.
    pub fn open(
        path: &Path,
        sample_rate: u32,
        interrupt: impl FnMut() -> bool + Send + 'static,
    ) -> Result<Option<Self>, DecodeError> {
        if !path.is_file() {
            return Err(DecodeError::NotAFile);
        }
        if !(8_000..=192_000).contains(&sample_rate) {
            return Err(DecodeError::InvalidLimits);
        }
        ffmpeg::init()?;
        let input = format::input_with_interrupt(&path, interrupt)?;
        let Some(stream) = input.streams().best(ffmpeg::media::Type::Audio) else {
            return Ok(None);
        };
        let (index, time_base) = (stream.index(), stream.time_base());
        let video = input.streams().best(ffmpeg::media::Type::Video);
        let reference = video.as_ref().unwrap_or(&stream);
        let (origin, origin_base) = (reference.start_time(), reference.time_base());
        let origin = if origin == ffmpeg::ffi::AV_NOPTS_VALUE {
            0
        } else {
            origin
        };
        let origin_micros = from_ticks(origin, origin_base, 1_000_000)?;
        let origin_frames = from_ticks(origin, origin_base, sample_rate)?;
        let mut context = codec::context::Context::from_parameters(stream.parameters())?;
        context.set_threading(codec::threading::Config::count(1));
        let decoder = context.decoder().audio()?;
        Ok(Some(Self {
            input,
            decoder,
            index,
            time_base,
            origin_micros,
            origin_frames,
            sample_rate,
            converter: None,
            next_frame: None,
            expected_source_frame: None,
            draining: false,
            ended: false,
        }))
    }

    /// Restart from an earlier demux position. Read timestamps to discard PCM before the target.
    pub fn seek(&mut self, frame: u64) -> Result<(), DecodeError> {
        let micros = i128::from(frame) * 1_000_000 / i128::from(self.sample_rate) + i128::from(self.origin_micros);
        let micros = i64::try_from(micros).map_err(|_| DecodeError::OutsideStream)?;
        self.input.seek(micros, ..micros)?;
        self.decoder.flush();
        self.converter = None;
        self.next_frame = None;
        self.expected_source_frame = None;
        self.draining = false;
        self.ended = false;
        Ok(())
    }

    /// Read the next timestamped PCM chunk, including any resampler tail before clean EOF.
    pub fn next_chunk(&mut self) -> Result<Option<AudioChunk>, DecodeError> {
        let mut packets = 0;
        let mut decoded = frame::Audio::empty();
        while !self.ended {
            match self.decoder.receive_frame(&mut decoded) {
                Ok(()) => {
                    if let Some(chunk) = self.convert(&mut decoded)? {
                        return Ok(Some(chunk));
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    self.ended = true;
                }
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN && !self.draining => {
                    self.send_packet(&mut packets)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        let Some(converter) = &mut self.converter else {
            return Ok(None);
        };
        let Some(delay) = converter.delay() else {
            return Ok(None);
        };
        let mut output = allocate_output(delay.output.max(1) as usize)?;
        converter.flush(&mut output)?;
        self.take_output(&output)
    }

    fn send_packet(&mut self, packets: &mut usize) -> Result<(), DecodeError> {
        let mut packet = ffmpeg::Packet::empty();
        while *packets < PACKET_LIMIT {
            *packets += 1;
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.size() > PACKET_BYTES {
                        return Err(DecodeError::LimitExceeded);
                    }
                    if packet.stream() == self.index {
                        return Ok(self.decoder.send_packet(&packet)?);
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    self.decoder.send_eof()?;
                    self.draining = true;
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(DecodeError::LimitExceeded)
    }

    fn convert(&mut self, decoded: &mut frame::Audio) -> Result<Option<AudioChunk>, DecodeError> {
        if decoded.rate() == 0 || !(1..=32).contains(&decoded.channels()) || decoded.samples() > FRAME_LIMIT {
            return Err(DecodeError::LimitExceeded);
        }
        if decoded.channel_layout().is_empty() {
            decoded.set_channel_layout(ChannelLayout::default(i32::from(decoded.channels())));
        }
        let source_frame = decoded
            .timestamp()
            .map(|value| {
                from_ticks(value, self.time_base, self.sample_rate)
                    .map(|frame| frame.saturating_sub(self.origin_frames))
            })
            .transpose()?
            .or(self.expected_source_frame)
            .ok_or(DecodeError::MissingTimestamp)?;
        let duration = (decoded.samples() as u64 * u64::from(self.sample_rate)).div_ceil(u64::from(decoded.rate()));

        // Container timestamps may be quantized more coarsely than the PCM clock.
        let tolerance = from_ticks(1, self.time_base, self.sample_rate)?.unsigned_abs().max(2);
        let continuous = self
            .expected_source_frame
            .is_some_and(|expected| expected.abs_diff(source_frame) <= tolerance);
        if !continuous
            || self.converter.as_ref().is_none_or(|converter| {
                let definition = converter.input();
                definition.rate != decoded.rate()
                    || definition.channel_layout != decoded.channel_layout()
                    || definition.format != decoded.format()
            })
        {
            self.converter = Some(resampling::Context::get(
                decoded.format(),
                decoded.channel_layout(),
                decoded.rate(),
                PACKED_F32,
                ChannelLayout::STEREO,
                self.sample_rate,
            )?);
            self.next_frame = Some(source_frame);
        }
        self.expected_source_frame = Some(source_frame.saturating_add(duration as i64));

        let converter = self
            .converter
            .as_mut()
            .expect("converter initialized before resampling");
        let delayed = converter.delay().map_or(0, |delay| delay.output.max(0) as u64);
        let mut output = allocate_output((duration + delayed + 1) as usize)?;
        converter.run(decoded, &mut output)?;
        self.take_output(&output)
    }

    fn take_output(&mut self, output: &frame::Audio) -> Result<Option<AudioChunk>, DecodeError> {
        if output.samples() == 0 {
            return Ok(None);
        }
        if output.planes() != 1 || output.format() != PACKED_F32 || output.channels() != 2 {
            return Err(ffmpeg::Error::InvalidData.into());
        }
        let start_frame = self.next_frame.ok_or(DecodeError::MissingTimestamp)?;
        let samples = output.plane::<(f32, f32)>(0).to_vec();
        self.next_frame = Some(start_frame.saturating_add(samples.len() as i64));
        Ok(Some(AudioChunk { start_frame, samples }))
    }
}

fn allocate_output(frames: usize) -> Result<frame::Audio, DecodeError> {
    if frames > FRAME_LIMIT {
        return Err(DecodeError::LimitExceeded);
    }
    Ok(frame::Audio::new(PACKED_F32, frames, ChannelLayout::STEREO))
}

fn from_ticks(ticks: i64, time_base: Rational, unit_hz: u32) -> Result<i64, DecodeError> {
    if time_base.numerator() <= 0 || time_base.denominator() <= 0 {
        return Err(ffmpeg::Error::InvalidData.into());
    }
    let value = i128::from(ticks) * i128::from(time_base.numerator()) * i128::from(unit_hz);
    i64::try_from(value.div_euclid(i128::from(time_base.denominator()))).map_err(|_| DecodeError::OutsideStream)
}

#[cfg(test)]
mod tests;
