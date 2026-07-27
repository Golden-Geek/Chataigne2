use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use cpal::{
    BufferSize, Data, Device, ErrorKind, I24, InputCallbackInfo, OutputCallbackInfo, Sample, SampleFormat,
    StreamConfig, U24,
    traits::{DeviceTrait, StreamTrait},
};

use crate::{
    AudioCallbackTimestamp, AudioDeviceDescriptor, AudioDeviceReadiness, AudioDirection, AudioError,
    AudioErrorCategory, AudioInspectorError, AudioPermissionState, AudioRecoveryPolicy, AudioStream,
    AudioStreamHandler, AudioStreamStatus, InterleavedInput, InterleavedOutput, NegotiatedStreamFormat, StreamRequest,
};

use super::{discovery::sample_format_to_cpal, error::map_cpal_error};

pub(super) fn open_cpal_stream(
    device: Device,
    request: &StreamRequest,
    descriptor: AudioDeviceDescriptor,
    format: NegotiatedStreamFormat,
    handler: Box<dyn AudioStreamHandler>,
) -> Result<Box<dyn AudioStream>, AudioError> {
    let sample_format = sample_format_to_cpal(format.sample_format).ok_or_else(|| {
        AudioError::new(
            AudioErrorCategory::UnsupportedFormat,
            "the negotiated sample format has no safe CPAL callback representation",
        )
    })?;
    let config = StreamConfig {
        channels: format.channels,
        sample_rate: format.sample_rate,
        buffer_size: cpal_buffer_size(request, &format),
    };
    let callback_sample_capacity = usize::from(format.channels)
        .checked_mul(format.buffer_frames as usize)
        .ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::CapacityExceeded,
                "audio callback sample capacity overflowed",
            )
        })?;
    let runtime_error = Arc::new(AtomicU8::new(RuntimeErrorCode::None as u8));
    let error_sink = Arc::clone(&runtime_error);
    let data_error = Arc::clone(&runtime_error);
    let backend = request.target.backend().clone();
    let stream = match request.direction {
        AudioDirection::Input => device.build_input_stream_raw(
            config,
            sample_format,
            input_callback(handler, sample_format, callback_sample_capacity, data_error),
            move |error| store_runtime_error(&error_sink, error.kind()),
            None,
        ),
        AudioDirection::Output => device.build_output_stream_raw(
            config,
            sample_format,
            output_callback(handler, sample_format, callback_sample_capacity, data_error),
            move |error| store_runtime_error(&error_sink, error.kind()),
            None,
        ),
    }
    .map_err(|error| map_cpal_error(error, "open audio stream", Some(backend)))?;
    Ok(Box::new(CpalStream {
        stream,
        runtime_error,
        status: AudioStreamStatus {
            direction: request.direction,
            enabled: false,
            selected_target: Some(request.target.clone()),
            selected_label: Some(descriptor.label),
            profile_key: Some(descriptor.profile_key),
            active_target: Some(descriptor.target),
            readiness: AudioDeviceReadiness::Primed,
            permission: AudioPermissionState::Granted,
            recovery_policy: AudioRecoveryPolicy::WaitForSelected,
            retry_attempt: 0,
            next_retry_ms: None,
            format: Some(format),
            error: None,
        },
    }))
}

pub(super) fn cpal_buffer_size(request: &StreamRequest, format: &NegotiatedStreamFormat) -> BufferSize {
    match request.buffer_policy {
        crate::AudioBufferPolicy::Automatic => BufferSize::Default,
        crate::AudioBufferPolicy::Fixed(_) => BufferSize::Fixed(format.buffer_frames),
    }
}

fn input_callback(
    mut handler: Box<dyn AudioStreamHandler>,
    sample_format: SampleFormat,
    sample_capacity: usize,
    runtime_error: Arc<AtomicU8>,
) -> impl FnMut(&Data, &InputCallbackInfo) + Send + 'static {
    let mut packed_scratch = vec![0.0_f32; sample_capacity];
    move |data, info| {
        let timestamp = input_timestamp(info);
        match sample_format {
            SampleFormat::I8 => process_input::<i8>(data, timestamp, &mut handler, InterleavedInput::I8),
            SampleFormat::I16 => process_input::<i16>(data, timestamp, &mut handler, InterleavedInput::I16),
            SampleFormat::I32 => process_input::<i32>(data, timestamp, &mut handler, InterleavedInput::I32),
            SampleFormat::I64 => process_input::<i64>(data, timestamp, &mut handler, InterleavedInput::I64),
            SampleFormat::U8 => process_input::<u8>(data, timestamp, &mut handler, InterleavedInput::U8),
            SampleFormat::U16 => process_input::<u16>(data, timestamp, &mut handler, InterleavedInput::U16),
            SampleFormat::U32 => process_input::<u32>(data, timestamp, &mut handler, InterleavedInput::U32),
            SampleFormat::U64 => process_input::<u64>(data, timestamp, &mut handler, InterleavedInput::U64),
            SampleFormat::F32 => process_input::<f32>(data, timestamp, &mut handler, InterleavedInput::F32),
            SampleFormat::F64 => process_input::<f64>(data, timestamp, &mut handler, InterleavedInput::F64),
            SampleFormat::I24 => {
                process_packed_i24_input(data, timestamp, &mut handler, &mut packed_scratch, &runtime_error)
            }
            SampleFormat::U24 => {
                process_packed_u24_input(data, timestamp, &mut handler, &mut packed_scratch, &runtime_error)
            }
            _ => {}
        }
    }
}

fn output_callback(
    mut handler: Box<dyn AudioStreamHandler>,
    sample_format: SampleFormat,
    sample_capacity: usize,
    runtime_error: Arc<AtomicU8>,
) -> impl FnMut(&mut Data, &OutputCallbackInfo) + Send + 'static {
    let mut packed_scratch = vec![0.0_f32; sample_capacity];
    move |data, info| {
        let timestamp = output_timestamp(info);
        match sample_format {
            SampleFormat::I8 => process_output::<i8>(data, timestamp, &mut handler, InterleavedOutput::I8),
            SampleFormat::I16 => process_output::<i16>(data, timestamp, &mut handler, InterleavedOutput::I16),
            SampleFormat::I32 => process_output::<i32>(data, timestamp, &mut handler, InterleavedOutput::I32),
            SampleFormat::I64 => process_output::<i64>(data, timestamp, &mut handler, InterleavedOutput::I64),
            SampleFormat::U8 => process_output::<u8>(data, timestamp, &mut handler, InterleavedOutput::U8),
            SampleFormat::U16 => process_output::<u16>(data, timestamp, &mut handler, InterleavedOutput::U16),
            SampleFormat::U32 => process_output::<u32>(data, timestamp, &mut handler, InterleavedOutput::U32),
            SampleFormat::U64 => process_output::<u64>(data, timestamp, &mut handler, InterleavedOutput::U64),
            SampleFormat::F32 => process_output::<f32>(data, timestamp, &mut handler, InterleavedOutput::F32),
            SampleFormat::F64 => process_output::<f64>(data, timestamp, &mut handler, InterleavedOutput::F64),
            SampleFormat::I24 => {
                process_packed_i24_output(data, timestamp, &mut handler, &mut packed_scratch, &runtime_error)
            }
            SampleFormat::U24 => {
                process_packed_u24_output(data, timestamp, &mut handler, &mut packed_scratch, &runtime_error)
            }
            _ => {}
        }
    }
}

fn process_input<'a, T: cpal::SizedSample + 'a>(
    data: &'a Data,
    timestamp: AudioCallbackTimestamp,
    handler: &mut Box<dyn AudioStreamHandler>,
    wrap: impl FnOnce(&'a [T]) -> InterleavedInput<'a>,
) {
    if let Some(samples) = data.as_slice::<T>() {
        handler.process_input(wrap(samples), timestamp);
    }
}

fn process_output<'a, T: cpal::SizedSample + 'a>(
    data: &'a mut Data,
    timestamp: AudioCallbackTimestamp,
    handler: &mut Box<dyn AudioStreamHandler>,
    wrap: impl FnOnce(&'a mut [T]) -> InterleavedOutput<'a>,
) {
    if let Some(samples) = data.as_slice_mut::<T>() {
        let mut samples = wrap(samples);
        samples.fill_silence();
        handler.process_output(samples, timestamp);
    }
}

fn process_packed_i24_input(
    data: &Data,
    timestamp: AudioCallbackTimestamp,
    handler: &mut Box<dyn AudioStreamHandler>,
    scratch: &mut [f32],
    runtime_error: &AtomicU8,
) {
    let Some(samples) = data.as_slice::<I24>() else {
        store_runtime_error(runtime_error, ErrorKind::UnsupportedConfig);
        return;
    };
    let Some(destination) = scratch.get_mut(..samples.len()) else {
        store_runtime_error(runtime_error, ErrorKind::ResourceExhausted);
        return;
    };
    for (destination, sample) in destination.iter_mut().zip(samples) {
        let sample = i64::from(sample.inner());
        *destination = if sample < 0 {
            sample as f32 / 8_388_608.0
        } else {
            sample as f32 / 8_388_607.0
        };
    }
    handler.process_input(InterleavedInput::F32(destination), timestamp);
}

fn process_packed_u24_input(
    data: &Data,
    timestamp: AudioCallbackTimestamp,
    handler: &mut Box<dyn AudioStreamHandler>,
    scratch: &mut [f32],
    runtime_error: &AtomicU8,
) {
    let Some(samples) = data.as_slice::<U24>() else {
        store_runtime_error(runtime_error, ErrorKind::UnsupportedConfig);
        return;
    };
    let Some(destination) = scratch.get_mut(..samples.len()) else {
        store_runtime_error(runtime_error, ErrorKind::ResourceExhausted);
        return;
    };
    for (destination, sample) in destination.iter_mut().zip(samples) {
        *destination = (f64::from(sample.inner()) / 16_777_215.0 * 2.0 - 1.0) as f32;
    }
    handler.process_input(InterleavedInput::F32(destination), timestamp);
}

fn process_packed_i24_output(
    data: &mut Data,
    timestamp: AudioCallbackTimestamp,
    handler: &mut Box<dyn AudioStreamHandler>,
    scratch: &mut [f32],
    runtime_error: &AtomicU8,
) {
    let Some(samples) = data.as_slice_mut::<I24>() else {
        store_runtime_error(runtime_error, ErrorKind::UnsupportedConfig);
        return;
    };
    let Some(source) = scratch.get_mut(..samples.len()) else {
        samples.fill(I24::EQUILIBRIUM);
        store_runtime_error(runtime_error, ErrorKind::ResourceExhausted);
        return;
    };
    source.fill(0.0);
    handler.process_output(InterleavedOutput::F32(source), timestamp);
    for (sample, source) in samples.iter_mut().zip(source) {
        let source = source.clamp(-1.0, 1.0);
        let scaled = if source < 0.0 {
            (source * 8_388_608.0).round() as i32
        } else {
            (source * 8_388_607.0).round() as i32
        };
        *sample = I24::new_unchecked(scaled.clamp(-8_388_608, 8_388_607));
    }
}

fn process_packed_u24_output(
    data: &mut Data,
    timestamp: AudioCallbackTimestamp,
    handler: &mut Box<dyn AudioStreamHandler>,
    scratch: &mut [f32],
    runtime_error: &AtomicU8,
) {
    let Some(samples) = data.as_slice_mut::<U24>() else {
        store_runtime_error(runtime_error, ErrorKind::UnsupportedConfig);
        return;
    };
    let Some(source) = scratch.get_mut(..samples.len()) else {
        samples.fill(U24::EQUILIBRIUM);
        store_runtime_error(runtime_error, ErrorKind::ResourceExhausted);
        return;
    };
    source.fill(0.0);
    handler.process_output(InterleavedOutput::F32(source), timestamp);
    for (sample, source) in samples.iter_mut().zip(source) {
        let source = source.clamp(-1.0, 1.0);
        let scaled = ((f64::from(source) * 0.5 + 0.5) * 16_777_215.0).round() as i32;
        *sample = U24::new_unchecked(scaled.clamp(0, 16_777_215));
    }
}

fn input_timestamp(info: &InputCallbackInfo) -> AudioCallbackTimestamp {
    let timestamp = info.timestamp();
    AudioCallbackTimestamp {
        callback_nanos: timestamp.callback.as_nanos(),
        device_nanos: timestamp.capture.as_nanos(),
    }
}

fn output_timestamp(info: &OutputCallbackInfo) -> AudioCallbackTimestamp {
    let timestamp = info.timestamp();
    AudioCallbackTimestamp {
        callback_nanos: timestamp.callback.as_nanos(),
        device_nanos: timestamp.playback.as_nanos(),
    }
}

struct CpalStream {
    stream: cpal::Stream,
    runtime_error: Arc<AtomicU8>,
    status: AudioStreamStatus,
}

impl std::fmt::Debug for CpalStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CpalStream")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl AudioStream for CpalStream {
    fn status(&self) -> AudioStreamStatus {
        let mut status = self.status.clone();
        if let Some((readiness, error)) = load_runtime_error(&self.runtime_error) {
            status.readiness = readiness;
            status.error = Some(error);
        }
        status
    }

    fn start(&mut self) -> Result<(), AudioError> {
        self.stream
            .play()
            .map_err(|error| map_cpal_error(error, "start audio stream", None))?;
        self.status.enabled = true;
        self.status.readiness = AudioDeviceReadiness::Ready;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.stream
            .pause()
            .map_err(|error| map_cpal_error(error, "stop audio stream", None))?;
        self.status.enabled = false;
        self.status.readiness = AudioDeviceReadiness::Disabled;
        Ok(())
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeErrorCode {
    None,
    DeviceChanged,
    DeviceMissing,
    HostUnavailable,
    PermissionDenied,
    RealtimeDenied,
    Xrun,
    Invalidated,
    Failed,
}

fn store_runtime_error(target: &AtomicU8, kind: ErrorKind) {
    let code = match kind {
        ErrorKind::DeviceChanged => RuntimeErrorCode::DeviceChanged,
        ErrorKind::DeviceNotAvailable => RuntimeErrorCode::DeviceMissing,
        ErrorKind::HostUnavailable => RuntimeErrorCode::HostUnavailable,
        ErrorKind::PermissionDenied => RuntimeErrorCode::PermissionDenied,
        ErrorKind::RealtimeDenied => RuntimeErrorCode::RealtimeDenied,
        ErrorKind::Xrun => RuntimeErrorCode::Xrun,
        ErrorKind::StreamInvalidated => RuntimeErrorCode::Invalidated,
        ErrorKind::DeviceBusy
        | ErrorKind::InvalidInput
        | ErrorKind::ResourceExhausted
        | ErrorKind::UnsupportedConfig
        | ErrorKind::UnsupportedOperation
        | ErrorKind::BackendError
        | ErrorKind::Other => RuntimeErrorCode::Failed,
        _ => RuntimeErrorCode::Failed,
    };
    target.store(code as u8, Ordering::Release);
}

fn load_runtime_error(source: &AtomicU8) -> Option<(AudioDeviceReadiness, AudioInspectorError)> {
    let code = source.swap(RuntimeErrorCode::None as u8, Ordering::AcqRel);
    let (readiness, category, message) = match code {
        value if value == RuntimeErrorCode::None as u8 => return None,
        value if value == RuntimeErrorCode::DeviceChanged as u8 => (
            AudioDeviceReadiness::Recovering,
            AudioErrorCategory::DeviceMissing,
            "The audio route changed; the stream may need to be rebuilt.",
        ),
        value if value == RuntimeErrorCode::DeviceMissing as u8 => (
            AudioDeviceReadiness::Missing,
            AudioErrorCategory::DeviceMissing,
            "The active audio device was disconnected.",
        ),
        value if value == RuntimeErrorCode::HostUnavailable as u8 => (
            AudioDeviceReadiness::Recovering,
            AudioErrorCategory::BackendUnavailable,
            "The audio host became unavailable.",
        ),
        value if value == RuntimeErrorCode::PermissionDenied as u8 => (
            AudioDeviceReadiness::PermissionDenied,
            AudioErrorCategory::PermissionDenied,
            "Audio device permission was denied.",
        ),
        value if value == RuntimeErrorCode::RealtimeDenied as u8 => (
            AudioDeviceReadiness::Ready,
            AudioErrorCategory::StreamNegotiationFailed,
            "Real-time thread priority was denied.",
        ),
        value if value == RuntimeErrorCode::Xrun as u8 => (
            AudioDeviceReadiness::Ready,
            AudioErrorCategory::StreamNegotiationFailed,
            "The audio stream reported a buffer underrun or overrun.",
        ),
        value if value == RuntimeErrorCode::Invalidated as u8 => (
            AudioDeviceReadiness::Recovering,
            AudioErrorCategory::StreamNegotiationFailed,
            "The audio stream configuration was invalidated.",
        ),
        _ => (
            AudioDeviceReadiness::Failed,
            AudioErrorCategory::StreamNegotiationFailed,
            "The audio stream reported a backend failure.",
        ),
    };
    Some((
        readiness,
        AudioInspectorError {
            category,
            message: message.to_owned(),
            technical_detail: None,
        },
    ))
}
