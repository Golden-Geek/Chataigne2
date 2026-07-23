use std::{env, thread, time::Duration};

use golden_audio::{
    AudioBufferPolicy, AudioDeviceTargetId, AudioDirection, AudioSampleFormat, SampleRate, StreamRequest,
    compiled_cpal_backends,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let requested_backend = env::args().nth(1).unwrap_or_else(|| native_backend_id().to_owned());
    let backend = compiled_cpal_backends()
        .into_iter()
        .find(|backend| backend.descriptor().id.as_str() == requested_backend)
        .ok_or_else(|| format!("backend {requested_backend:?} is not compiled"))?;
    let backend_id = backend.descriptor().id;
    let devices = backend.discover()?;
    let device = devices
        .iter()
        .find(|device| device.is_system_default_output && device.supports(AudioDirection::Output))
        .ok_or("the backend has no system default output device")?;
    let supported = device
        .supported_configurations
        .iter()
        .filter(|configuration| configuration.direction == AudioDirection::Output)
        .min_by_key(|configuration| {
            (
                configuration.sample_format != AudioSampleFormat::F32,
                configuration.channels.abs_diff(2),
                configuration.min_sample_rate.abs_diff(48_000),
            )
        })
        .ok_or("the system default output has no supported stream configuration")?;
    let sample_rate = 48_000_u32.clamp(supported.min_sample_rate, supported.max_sample_rate);
    let request = StreamRequest {
        direction: AudioDirection::Output,
        target: AudioDeviceTargetId::SystemDefault { backend: backend_id },
        engine_sample_rate: SampleRate::new(sample_rate)?,
        channels: supported.channels,
        buffer_policy: AudioBufferPolicy::Automatic,
    };
    let mut stream = backend.open_stream(&request)?;
    stream.start()?;
    thread::sleep(Duration::from_millis(100));
    stream.stop()?;
    println!(
        "{} output smoke passed: {} channels at {} Hz",
        requested_backend, supported.channels, sample_rate
    );
    Ok(())
}

fn native_backend_id() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "wasapi"
    }
    #[cfg(target_os = "macos")]
    {
        "coreaudio"
    }
    #[cfg(target_os = "linux")]
    {
        "alsa"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "cpal-null"
    }
}
