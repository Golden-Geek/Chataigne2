use crate::{AudioError, SampleRate, assert_not_realtime};

pub(crate) fn resample_planar(
    input: &[f32],
    channels: usize,
    input_frames: usize,
    source_rate: SampleRate,
    destination_rate: SampleRate,
) -> Result<(Vec<f32>, usize), AudioError> {
    assert_not_realtime("playback sample-rate conversion");
    if channels == 0 || input_frames == 0 || input.len() != channels.saturating_mul(input_frames) {
        return Err(AudioError::invalid_configuration(
            "playback resampler input shape is invalid",
        ));
    }
    if source_rate == destination_rate {
        return Ok((input.to_vec(), input_frames));
    }
    let output_frames_u128 = (input_frames as u128)
        .saturating_mul(u128::from(destination_rate.get()))
        .saturating_add(u128::from(source_rate.get()) / 2)
        / u128::from(source_rate.get());
    let output_frames = usize::try_from(output_frames_u128)
        .map_err(|_| AudioError::capacity_exceeded("resampled playback frame count overflowed"))?
        .max(1);
    let sample_count = channels
        .checked_mul(output_frames)
        .ok_or_else(|| AudioError::capacity_exceeded("resampled playback sample count overflowed"))?;
    let mut output = vec![0.0; sample_count];
    let source_per_destination = f64::from(source_rate.get()) / f64::from(destination_rate.get());
    for channel in 0..channels {
        let input_channel = &input[channel * input_frames..(channel + 1) * input_frames];
        let output_channel = &mut output[channel * output_frames..(channel + 1) * output_frames];
        for (frame, sample) in output_channel.iter_mut().enumerate() {
            let position = frame as f64 * source_per_destination;
            let before = (position.floor() as usize).min(input_frames - 1);
            let after = before.saturating_add(1).min(input_frames - 1);
            let fraction = (position - before as f64) as f32;
            *sample = input_channel[before] + (input_channel[after] - input_channel[before]) * fraction;
        }
    }
    Ok((output, output_frames))
}

#[derive(Debug)]
pub(crate) struct StreamingResampler {
    channels: usize,
    passthrough: bool,
    source_frames_per_output: f64,
    next_position: f64,
    carry: Vec<f32>,
}

impl StreamingResampler {
    pub fn new(channels: u16, source_rate: SampleRate, destination_rate: SampleRate) -> Self {
        Self {
            channels: usize::from(channels),
            passthrough: source_rate == destination_rate,
            source_frames_per_output: f64::from(source_rate.get()) / f64::from(destination_rate.get()),
            next_position: 0.0,
            carry: Vec::with_capacity(usize::from(channels)),
        }
    }

    pub fn process(&mut self, planes: &[Vec<f32>], output: &mut Vec<f32>) -> Result<(), AudioError> {
        output.clear();
        let frames = planes.first().map_or(0, Vec::len);
        if planes.len() != self.channels || frames == 0 || planes.iter().any(|plane| plane.len() != frames) {
            return Err(AudioError::invalid_configuration(
                "streaming resampler received inconsistent audio planes",
            ));
        }
        if self.passthrough {
            output.reserve(frames.saturating_mul(self.channels));
            for frame in 0..frames {
                for plane in planes {
                    output.push(plane[frame]);
                }
            }
            return Ok(());
        }

        let has_carry = !self.carry.is_empty();
        let combined_frames = frames + usize::from(has_carry);
        output.reserve(
            ((combined_frames as f64 / self.source_frames_per_output).ceil() as usize).saturating_mul(self.channels),
        );
        while self.next_position < (combined_frames - 1) as f64 {
            let before = self.next_position.floor() as usize;
            let after = before + 1;
            let fraction = (self.next_position - before as f64) as f32;
            for (channel, plane) in planes.iter().enumerate() {
                let before_sample = combined_sample(&self.carry, plane, channel, before, has_carry);
                let after_sample = combined_sample(&self.carry, plane, channel, after, has_carry);
                output.push(before_sample + (after_sample - before_sample) * fraction);
            }
            self.next_position += self.source_frames_per_output;
        }
        self.next_position -= (combined_frames - 1) as f64;
        self.carry.clear();
        self.carry.extend(planes.iter().map(|plane| plane[frames - 1]));
        Ok(())
    }

    pub fn finish(&mut self, output: &mut Vec<f32>) {
        output.clear();
        if !self.passthrough && !self.carry.is_empty() && self.next_position < 0.5 {
            output.extend_from_slice(&self.carry);
        }
        self.carry.clear();
    }
}

fn combined_sample(carry: &[f32], plane: &[f32], channel: usize, frame: usize, has_carry: bool) -> f32 {
    if has_carry {
        if frame == 0 { carry[channel] } else { plane[frame - 1] }
    } else {
        plane[frame]
    }
}
