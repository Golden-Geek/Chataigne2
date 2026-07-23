#![cfg(feature = "playback")]

use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

use golden_audio::{
    AudioCommand, AudioEngineBuilder, AudioEvent, PlanarBuffer, PlayFileRequest, PlaybackId, PlaybackStopReason,
};
use tempfile::{Builder, NamedTempFile};

#[test]
fn public_engine_runs_ordered_async_playback_through_the_callback_renderer() {
    let file = sine_wave_file(2, 48_000, 256);
    let playback_id = PlaybackId::new("ordered").unwrap();
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let mut renderer = engine.take_playback_renderer().unwrap();
    let events = engine.take_event_receiver().unwrap();
    engine
        .control()
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            file.path(),
            playback_id.clone(),
        )))
        .unwrap();
    let started = wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackStarted(info) if info.playback_id == playback_id),
    );
    let voice = match started {
        AudioEvent::PlaybackStarted(info) => info.voice,
        _ => unreachable!(),
    };

    let mut destination = PlanarBuffer::new(256, 128).unwrap();
    renderer.render(&mut destination, 128).unwrap();
    renderer.render(&mut destination, 128).unwrap();
    let finished = wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackFinished(info) if info.voice == voice),
    );
    assert!(matches!(finished, AudioEvent::PlaybackFinished(_)));
    engine.shutdown().unwrap();
}

#[test]
fn same_id_replacement_and_pending_stop_never_publish_a_stale_start() {
    let first = sine_wave_file(1, 48_000, 32_768);
    let second = sine_wave_file(2, 48_000, 128);
    let playback_id = PlaybackId::new("replace").unwrap();
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();
    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            first.path(),
            playback_id.clone(),
        )))
        .unwrap();
    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            second.path(),
            playback_id.clone(),
        )))
        .unwrap();

    let started = wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackStarted(info) if info.playback_id == playback_id),
    );
    match started {
        AudioEvent::PlaybackStarted(info) => assert_eq!(
            std::fs::canonicalize(info.path).unwrap(),
            std::fs::canonicalize(second.path()).unwrap()
        ),
        _ => unreachable!(),
    }

    let pending_id = PlaybackId::new("pending-stop").unwrap();
    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            first.path(),
            pending_id.clone(),
        )))
        .unwrap();
    control
        .submit(AudioCommand::StopFile {
            playback_id: pending_id.clone(),
        })
        .unwrap();
    let stopped = wait_event(&events, |event| {
        matches!(
            event,
            AudioEvent::PlaybackStopped(info)
                if info.playback_id == pending_id && info.voice.is_none()
        )
    });
    assert!(matches!(
        stopped,
        AudioEvent::PlaybackStopped(info) if info.reason == PlaybackStopReason::Requested
    ));
    engine.shutdown().unwrap();
}

#[test]
fn replacement_while_playing_fades_old_voice_and_starts_new_generation() {
    let first = sine_wave_file(1, 48_000, 4_096);
    let second = sine_wave_file(1, 48_000, 4_096);
    let playback_id = PlaybackId::new("playing-replace").unwrap();
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let mut renderer = engine.take_playback_renderer().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();
    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            first.path(),
            playback_id.clone(),
        )))
        .unwrap();
    let first_started = wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackStarted(info) if info.playback_id == playback_id),
    );
    let first_voice = match first_started {
        AudioEvent::PlaybackStarted(info) => info.voice,
        _ => unreachable!(),
    };
    let mut destination = PlanarBuffer::new(256, 128).unwrap();
    renderer.render(&mut destination, 128).unwrap();

    control
        .submit(AudioCommand::PlayFile(PlayFileRequest::new(
            second.path(),
            playback_id.clone(),
        )))
        .unwrap();
    let second_started = wait_event(&events, |event| {
        matches!(
            event,
            AudioEvent::PlaybackStarted(info)
                if info.playback_id == playback_id && info.voice != first_voice
        )
    });
    assert!(matches!(second_started, AudioEvent::PlaybackStarted(_)));
    for _ in 0..4 {
        renderer.render(&mut destination, 128).unwrap();
    }
    let stopped = wait_event(&events, |event| {
        matches!(
            event,
            AudioEvent::PlaybackStopped(info)
                if info.voice == Some(first_voice) && info.reason == PlaybackStopReason::Replaced
        )
    });
    assert!(matches!(stopped, AudioEvent::PlaybackStopped(_)));
    engine.shutdown().unwrap();
}

fn wait_event(events: &golden_audio::AudioEventReceiver, predicate: impl Fn(&AudioEvent) -> bool) -> AudioEvent {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        while let Some(event) = events.try_recv() {
            if predicate(&event) {
                return event;
            }
        }
        assert!(Instant::now() < deadline, "timed out waiting for playback event");
        thread::sleep(Duration::from_millis(2));
    }
}

fn sine_wave_file(channels: u16, sample_rate: u32, frames: u32) -> NamedTempFile {
    let mut file = Builder::new().suffix(".wav").tempfile().unwrap();
    write_sine_wave(file.as_file_mut(), channels, sample_rate, frames);
    file
}

fn write_sine_wave(writer: &mut impl Write, channels: u16, sample_rate: u32, frames: u32) {
    let data_bytes = frames.saturating_mul(u32::from(channels)).saturating_mul(2);
    writer.write_all(b"RIFF").unwrap();
    writer.write_all(&(36 + data_bytes).to_le_bytes()).unwrap();
    writer.write_all(b"WAVEfmt ").unwrap();
    writer.write_all(&16_u32.to_le_bytes()).unwrap();
    writer.write_all(&1_u16.to_le_bytes()).unwrap();
    writer.write_all(&channels.to_le_bytes()).unwrap();
    writer.write_all(&sample_rate.to_le_bytes()).unwrap();
    writer
        .write_all(&(sample_rate * u32::from(channels) * 2).to_le_bytes())
        .unwrap();
    writer.write_all(&(channels * 2).to_le_bytes()).unwrap();
    writer.write_all(&16_u16.to_le_bytes()).unwrap();
    writer.write_all(b"data").unwrap();
    writer.write_all(&data_bytes.to_le_bytes()).unwrap();
    for frame in 0..frames {
        let sample = (((frame as f32 * 440.0 * std::f32::consts::TAU / sample_rate as f32).sin()) * 12_000.0) as i16;
        for _ in 0..channels {
            writer.write_all(&sample.to_le_bytes()).unwrap();
        }
    }
    writer.flush().unwrap();
}
