use std::{path::PathBuf, sync::Arc};

use allocation_counter::measure;

use crate::{
    ClockSource, FrameCount, GainDb, PlanarBuffer, PlaybackId, PlaybackStopReason, RenderClockCoordinator, SampleRate,
};

use super::super::{
    AudioSourceFingerprint, PlaybackRenderEvent, PlaybackVoice, PlaybackVoiceSource, ResidentAssetKey,
    ResidentAudioAsset, default_playback_routes, playback_voice_pool, streaming_playback_ring,
};

#[test]
fn mono_stereo_and_multichannel_default_routes_are_explicit() {
    let mono_stereo = default_playback_routes(1, 2);
    assert_eq!(mono_stereo.len(), 2);
    assert!((mono_stereo[0].gain - 0.707_106_77).abs() < f32::EPSILON);
    assert_eq!(mono_stereo[1].destination_channel, 1);

    let mono_single = default_playback_routes(1, 1);
    assert_eq!(mono_single[0].gain, 1.0);
    let stereo = default_playback_routes(2, 2);
    assert_eq!((stereo[0].source_channel, stereo[0].destination_channel), (0, 0));
    assert_eq!((stereo[1].source_channel, stereo[1].destination_channel), (1, 1));
    let surround = default_playback_routes(6, 4);
    assert_eq!(surround.len(), 4);
}

#[test]
fn resident_render_is_allocation_free_and_final_asset_drop_returns_to_control() {
    let asset = asset(1, 4, 1.0);
    let mut source_owner = Some(Arc::clone(&asset));
    let voice = voice(
        PlaybackVoiceSource::Resident(source_owner.take().unwrap()),
        default_playback_routes(1, 2),
        1,
        4,
    );
    let (mut controller, mut renderer) = playback_voice_pool(1, 2, 4).unwrap();
    let id = controller.try_activate(voice).unwrap();
    let mut destination = PlanarBuffer::new(2, 4).unwrap();

    let allocation = measure(|| renderer.render(&mut destination, 4).unwrap());
    assert_eq!(allocation.count_total, 0);
    assert_eq!(allocation.bytes_total, 0);
    assert!((destination.sample(0, 0) - 0.707_106_77).abs() < 1.0e-6);
    assert!((destination.sample(1, 0) - 0.707_106_77).abs() < 1.0e-6);
    assert_eq!(Arc::strong_count(&asset), 2, "retirement queue still owns the asset");

    let mut event = None;
    assert_eq!(controller.reclaim(|received| event = Some(received)), 1);
    assert_eq!(Arc::strong_count(&asset), 1);
    assert!(matches!(
        event,
        Some(PlaybackRenderEvent::Finished {
            voice,
            rendered_frames: 4,
            ..
        }) if voice == id
    ));
}

#[test]
fn queued_activation_is_reclaimed_explicitly_off_callback() {
    let asset = asset(1, 16, 0.5);
    let voice = voice(
        PlaybackVoiceSource::Resident(Arc::clone(&asset)),
        default_playback_routes(1, 1),
        1,
        4,
    );
    let (mut controller, renderer) = playback_voice_pool(1, 1, 4).unwrap();
    controller.try_activate(voice).unwrap();
    assert_eq!(Arc::strong_count(&asset), 2);
    renderer.into_retirement().reclaim();
    assert_eq!(Arc::strong_count(&asset), 1);
}

#[test]
fn requested_stop_uses_a_bounded_ramp_and_reports_the_authored_reason() {
    let voice = voice(
        PlaybackVoiceSource::Resident(asset(1, 64, 1.0)),
        default_playback_routes(1, 1),
        4,
        4,
    );
    let (mut controller, mut renderer) = playback_voice_pool(1, 1, 4).unwrap();
    let id = controller.try_activate(voice).unwrap();
    let mut destination = PlanarBuffer::new(1, 4).unwrap();
    renderer.render(&mut destination, 4).unwrap();
    controller.try_stop(id, PlaybackStopReason::Replaced).unwrap();
    renderer.render(&mut destination, 4).unwrap();
    assert!(destination.sample(0, 0) > destination.sample(0, 3));

    let mut event = None;
    controller.reclaim(|received| event = Some(received));
    assert!(matches!(
        event,
        Some(PlaybackRenderEvent::Stopped {
            voice,
            reason: PlaybackStopReason::Replaced,
            ..
        }) if voice == id
    ));
}

#[test]
fn streamed_voice_starves_to_silence_then_recovers() {
    let (mut writer, reader) = streaming_playback_ring(1, 16).unwrap();
    let voice = voice(
        PlaybackVoiceSource::Stream {
            reader,
            channels: 1,
            scratch: vec![0.0; 4].into_boxed_slice(),
        },
        default_playback_routes(1, 1),
        1,
        4,
    );
    let (mut controller, mut renderer) = playback_voice_pool(1, 1, 4).unwrap();
    controller.try_activate(voice).unwrap();
    let mut destination = PlanarBuffer::new(1, 4).unwrap();
    renderer.render(&mut destination, 4).unwrap();
    assert_eq!(destination.channel(0), &[0.0; 4]);

    writer.write_interleaved(&[0.25; 4]).unwrap();
    renderer.render(&mut destination, 4).unwrap();
    assert_eq!(destination.channel(0), &[0.25; 4]);
    writer.finish();
    renderer.render(&mut destination, 4).unwrap();
    assert_eq!(controller.reclaim(|_| {}), 1);
}

#[test]
fn null_clock_handoff_continues_the_same_voice_playhead() {
    let block = FrameCount::new(4).unwrap();
    let mut clock = RenderClockCoordinator::new(SampleRate::default(), block).unwrap();
    clock.prime_output(1).unwrap();
    let voice = voice(
        PlaybackVoiceSource::Resident(asset(1, 16, 0.5)),
        default_playback_routes(1, 1),
        1,
        4,
    );
    let (mut controller, mut renderer) = playback_voice_pool(1, 1, 4).unwrap();
    controller.try_activate(voice).unwrap();
    let mut destination = PlanarBuffer::new(1, 4).unwrap();

    for source in [
        ClockSource::Output(1),
        ClockSource::Null,
        ClockSource::Null,
        ClockSource::Output(2),
    ] {
        match source {
            ClockSource::Null => clock.output_lost(1),
            ClockSource::Output(2) => clock.prime_output(2).unwrap(),
            ClockSource::Output(1) => {}
            ClockSource::Output(_) => unreachable!(),
        }
        let rendered = clock.advance(source, block).unwrap();
        assert!(rendered.render);
        renderer.render(&mut destination, 4).unwrap();
    }
    let mut event = None;
    controller.reclaim(|received| event = Some(received));
    assert!(matches!(
        event,
        Some(PlaybackRenderEvent::Finished {
            rendered_frames: 16,
            ..
        })
    ));
    assert_eq!(clock.frame(), 16);
}

fn voice(
    source: PlaybackVoiceSource,
    routes: Vec<super::super::DefaultPlaybackRoute>,
    ramp_frames: u32,
    block_frames: usize,
) -> PlaybackVoice {
    PlaybackVoice::new(
        PlaybackId::new("fixture").unwrap(),
        PathBuf::from("fixture.wav"),
        source,
        GainDb::UNITY,
        routes,
        ramp_frames,
        block_frames,
    )
    .unwrap()
}

fn asset(channels: u16, frames: usize, sample: f32) -> Arc<ResidentAudioAsset> {
    Arc::new(
        ResidentAudioAsset::new(
            ResidentAssetKey {
                source: AudioSourceFingerprint {
                    canonical_path: PathBuf::from("fixture.wav"),
                    length_bytes: 1,
                    modified_nanos: 1,
                },
                track: 0,
                engine_sample_rate: SampleRate::default(),
            },
            channels,
            frames,
            vec![sample; usize::from(channels) * frames],
        )
        .unwrap(),
    )
}
