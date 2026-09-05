use super::*;

#[test]
fn force_restart_false_ignores_pending_and_active_duplicate_ids_without_replacement() {
    let first = sine_wave_file(1, 48_000, 32_768);
    let second = sine_wave_file(2, 48_000, 4_096);
    let third = sine_wave_file(1, 48_000, 4_096);
    let playback_id = PlaybackId::new("keep-current").unwrap();
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
    let pending_ignored_sequence = control
        .submit(AudioCommand::PlayFile(
            PlayFileRequest::new(second.path(), playback_id.clone()).with_force_restart(false),
        ))
        .unwrap();
    let (pending_ignored, first_started) =
        wait_for_ignored_and_started(&events, &playback_id, pending_ignored_sequence);
    assert_eq!(
        pending_ignored,
        PlaybackCommandIgnored::play_file(
            pending_ignored_sequence,
            playback_id.clone(),
            second.path().to_path_buf(),
        ),
    );
    assert_eq!(
        std::fs::canonicalize(&first_started.path).unwrap(),
        std::fs::canonicalize(first.path()).unwrap(),
    );

    let active_ignored_sequence = control
        .submit(AudioCommand::PlayFile(
            PlayFileRequest::new(third.path(), playback_id.clone()).with_force_restart(false),
        ))
        .unwrap();
    let active_ignored = wait_event(&events, |event| {
        matches!(
            event,
            AudioEvent::PlaybackCommandIgnored(ignored)
                if ignored.sequence() == active_ignored_sequence
        )
    });
    assert_eq!(
        active_ignored,
        AudioEvent::PlaybackCommandIgnored(PlaybackCommandIgnored::play_file(
            active_ignored_sequence,
            playback_id.clone(),
            third.path().to_path_buf(),
        )),
    );

    control
        .submit(AudioCommand::StopFile {
            playback_id: playback_id.clone(),
        })
        .unwrap();
    let stopped = wait_event_while_rendering(&events, &mut renderer, |event| {
        matches!(
            event,
            AudioEvent::PlaybackStopped(info)
                if info.playback_id == playback_id
                    && info.voice == Some(first_started.voice)
                    && info.reason == PlaybackStopReason::Requested
        )
    });
    assert!(matches!(stopped, AudioEvent::PlaybackStopped(_)));
    engine.shutdown().unwrap();
}

#[test]
fn missing_stop_file_and_empty_stop_all_publish_typed_ignored_events() {
    let missing_id = PlaybackId::new("missing").unwrap();
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();

    let stop_sequence = control
        .submit(AudioCommand::StopFile {
            playback_id: missing_id.clone(),
        })
        .unwrap();
    let stopped = wait_event(&events, |event| {
        matches!(
            event,
            AudioEvent::PlaybackCommandIgnored(ignored) if ignored.sequence() == stop_sequence
        )
    });
    assert_eq!(
        stopped,
        AudioEvent::PlaybackCommandIgnored(PlaybackCommandIgnored::stop_file(stop_sequence, missing_id,)),
    );
    let AudioEvent::PlaybackCommandIgnored(stopped) = stopped else {
        unreachable!();
    };
    assert_eq!(stopped.command(), PlaybackCommandKind::StopFile);
    assert_eq!(stopped.reason(), PlaybackCommandIgnoredReason::PlaybackIdNotFound,);

    let stop_all_sequence = control.submit(AudioCommand::StopAllFiles).unwrap();
    let stopped_all = wait_event(&events, |event| {
        matches!(
            event,
            AudioEvent::PlaybackCommandIgnored(ignored) if ignored.sequence() == stop_all_sequence
        )
    });
    assert_eq!(
        stopped_all,
        AudioEvent::PlaybackCommandIgnored(PlaybackCommandIgnored::stop_all_files(stop_all_sequence,)),
    );
    let AudioEvent::PlaybackCommandIgnored(stopped_all) = stopped_all else {
        unreachable!();
    };
    assert_eq!(stopped_all.command(), PlaybackCommandKind::StopAllFiles);
    assert_eq!(stopped_all.reason(), PlaybackCommandIgnoredReason::NoPlaybacks);
    engine.shutdown().unwrap();
}

#[test]
fn resident_start_offset_begins_at_the_first_expected_sample() {
    const SAMPLE_RATE: u32 = 48_000;
    const OFFSET_FRAMES: u32 = 480;
    const RENDERED_FRAME: usize = 288;
    let file = ramp_wave_file(SAMPLE_RATE, 4_096);
    let playback_id = PlaybackId::new("resident-offset").unwrap();
    let mut builder = AudioEngineBuilder::default();
    builder.config.gain_ramp_ms = 5.0;
    let mut engine = builder.build().unwrap();
    let mut renderer = engine.take_playback_renderer().unwrap();
    let events = engine.take_event_receiver().unwrap();
    engine
        .control()
        .submit(AudioCommand::PlayFile(
            PlayFileRequest::new(file.path(), playback_id.clone()).with_start_offset(Duration::from_millis(10)),
        ))
        .unwrap();
    wait_event(
        &events,
        |event| matches!(event, AudioEvent::PlaybackStarted(info) if info.playback_id == playback_id),
    );

    let mut destination = PlanarBuffer::new(256, 128).unwrap();
    for _ in 0..3 {
        renderer.render(&mut destination, 128).unwrap();
    }
    let local_frame = RENDERED_FRAME - 2 * 128;
    let source_frame = OFFSET_FRAMES as usize + RENDERED_FRAME;
    let expected = source_frame as f32 / 32_768.0 * 0.707_106_77;
    let actual = destination.sample(0, local_frame);
    assert!(
        (actual - expected).abs() < 1.0e-5,
        "resident playback produced {actual}, expected source frame {source_frame} ({expected})",
    );
    engine.shutdown().unwrap();
}

#[test]
fn offsets_at_and_beyond_eof_fail_identically_for_resident_and_streamed_sources() {
    let file = ramp_wave_file(8_000, 8_000);
    for (mode, threshold) in [("resident", 1024 * 1024), ("streamed", 1)] {
        let mut builder = AudioEngineBuilder::default();
        builder.limits.resident_asset_threshold_bytes = threshold;
        let mut engine = builder.build().unwrap();
        let events = engine.take_event_receiver().unwrap();
        for (suffix, offset) in [("at-eof", Duration::from_secs(1)), ("past-eof", Duration::from_secs(2))] {
            let playback_id = PlaybackId::new(format!("{mode}-{suffix}")).unwrap();
            engine
                .control()
                .submit(AudioCommand::PlayFile(
                    PlayFileRequest::new(file.path(), playback_id.clone()).with_start_offset(offset),
                ))
                .unwrap();
            let failure = wait_event(&events, |event| {
                matches!(
                    event,
                    AudioEvent::PlaybackFailed(failure) if failure.playback_id == playback_id
                )
            });
            let AudioEvent::PlaybackFailed(failure) = failure else {
                unreachable!();
            };
            assert_eq!(
                failure.error.category,
                golden_audio::AudioErrorCategory::InvalidConfiguration,
                "{mode} {suffix}",
            );
            assert_eq!(
                failure.error.message, "playback start offset must be before the end of the audio source",
                "{mode} {suffix}",
            );
        }
        engine.shutdown().unwrap();
    }
}

fn wait_for_ignored_and_started(
    events: &golden_audio::AudioEventReceiver,
    playback_id: &PlaybackId,
    ignored_sequence: golden_audio::CommandSequence,
) -> (PlaybackCommandIgnored, golden_audio::PlaybackInfo) {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut ignored = None;
    let mut started = None;
    while ignored.is_none() || started.is_none() {
        while let Some(event) = events.try_recv() {
            match event {
                AudioEvent::PlaybackCommandIgnored(value) if value.sequence() == ignored_sequence => {
                    ignored = Some(value);
                }
                AudioEvent::PlaybackStarted(info) if &info.playback_id == playback_id => {
                    started = Some(info);
                }
                _ => {}
            }
        }
        assert!(Instant::now() < deadline, "timed out waiting for playback outcomes");
        thread::sleep(Duration::from_millis(2));
    }
    (ignored.unwrap(), started.unwrap())
}

fn ramp_wave_file(sample_rate: u32, frames: u32) -> NamedTempFile {
    let mut file = Builder::new().suffix(".wav").tempfile().unwrap();
    write_ramp_wave(file.as_file_mut(), sample_rate, frames);
    file
}

fn write_ramp_wave(writer: &mut impl Write, sample_rate: u32, frames: u32) {
    let data_bytes = frames.saturating_mul(2);
    writer.write_all(b"RIFF").unwrap();
    writer.write_all(&(36 + data_bytes).to_le_bytes()).unwrap();
    writer.write_all(b"WAVEfmt ").unwrap();
    writer.write_all(&16_u32.to_le_bytes()).unwrap();
    writer.write_all(&1_u16.to_le_bytes()).unwrap();
    writer.write_all(&1_u16.to_le_bytes()).unwrap();
    writer.write_all(&sample_rate.to_le_bytes()).unwrap();
    writer.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
    writer.write_all(&2_u16.to_le_bytes()).unwrap();
    writer.write_all(&16_u16.to_le_bytes()).unwrap();
    writer.write_all(b"data").unwrap();
    writer.write_all(&data_bytes.to_le_bytes()).unwrap();
    for frame in 0..frames {
        writer.write_all(&i16::try_from(frame).unwrap().to_le_bytes()).unwrap();
    }
    writer.flush().unwrap();
}
