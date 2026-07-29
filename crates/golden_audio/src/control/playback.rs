use std::{
    collections::HashMap,
    sync::{Arc, RwLock, mpsc::SyncSender},
};

use crate::{
    AudioEngineConfig, AudioError, AudioErrorCategory, AudioEvent, AudioObservationSnapshot, CommandSequence,
    EngineLimits, PlayFileRequest, PlaybackCommandIgnored, PlaybackFailure, PlaybackId, PlaybackInfo,
    PlaybackPreparation, PlaybackPreparationResult, PlaybackRenderEvent, PlaybackScheduler, PlaybackSchedulerRequest,
    PlaybackStopInfo, PlaybackStopReason, PlaybackVoice, PlaybackVoiceController, PlaybackVoiceSource,
    default_playback_routes,
};

use super::engine::{publish_event, update_observation};

#[derive(Clone, Debug)]
pub(super) enum PlaybackLifecycle {
    Pending {
        sequence: CommandSequence,
    },
    Playing {
        sequence: CommandSequence,
        info: PlaybackInfo,
    },
}

pub(super) type PlaybackLifecycles = HashMap<PlaybackId, PlaybackLifecycle>;

pub(super) fn schedule_playback(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    scheduler: &mut PlaybackScheduler,
    voices: &mut PlaybackVoiceController,
    lifecycle: &mut PlaybackLifecycles,
    sequence: CommandSequence,
    request: PlayFileRequest,
) {
    if lifecycle.contains_key(&request.playback_id) {
        if !request.force_restart {
            publish_event(
                event_sender,
                observation,
                AudioEvent::PlaybackCommandIgnored(PlaybackCommandIgnored::play_file(
                    sequence,
                    request.playback_id,
                    request.path,
                )),
            );
            return;
        }
        let _ = stop_playback(
            event_sender,
            observation,
            scheduler,
            voices,
            lifecycle,
            &request.playback_id,
            PlaybackStopReason::Replaced,
        );
    }
    let playback_id = request.playback_id.clone();
    let path = request.path.clone();
    match scheduler.try_schedule(PlaybackSchedulerRequest {
        sequence,
        request: request.clone(),
    }) {
        Ok(()) => {
            lifecycle.insert(playback_id, PlaybackLifecycle::Pending { sequence });
        }
        Err(error) => publish_event(
            event_sender,
            observation,
            AudioEvent::PlaybackFailed(PlaybackFailure {
                playback_id,
                path,
                error,
            }),
        ),
    }
}

pub(super) fn stop_playback(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    scheduler: &mut PlaybackScheduler,
    voices: &mut PlaybackVoiceController,
    lifecycle: &mut PlaybackLifecycles,
    playback_id: &PlaybackId,
    reason: PlaybackStopReason,
) -> bool {
    let Some(current) = lifecycle.remove(playback_id) else {
        return false;
    };
    match current {
        PlaybackLifecycle::Pending { .. } => {
            scheduler.stop(playback_id);
            publish_event(
                event_sender,
                observation,
                AudioEvent::PlaybackStopped(PlaybackStopInfo {
                    playback_id: playback_id.clone(),
                    voice: None,
                    reason,
                }),
            );
        }
        PlaybackLifecycle::Playing { sequence, info } => {
            if let Err(error) = voices.try_stop(info.voice, reason) {
                lifecycle.insert(
                    playback_id.clone(),
                    PlaybackLifecycle::Playing {
                        sequence,
                        info: info.clone(),
                    },
                );
                publish_event(
                    event_sender,
                    observation,
                    AudioEvent::PlaybackFailed(PlaybackFailure {
                        playback_id: playback_id.clone(),
                        path: info.path,
                        error,
                    }),
                );
                return true;
            }
            scheduler.stop(playback_id);
        }
    }
    true
}

pub(super) fn stop_all_playback(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    scheduler: &mut PlaybackScheduler,
    voices: &mut PlaybackVoiceController,
    lifecycle: &mut PlaybackLifecycles,
    reason: PlaybackStopReason,
) -> bool {
    if lifecycle.is_empty() {
        return false;
    }
    let render_stop = voices.try_stop_all(reason);
    if render_stop.is_ok() {
        scheduler.stop_all();
    }
    let stopped = std::mem::take(lifecycle);
    let mut still_playing = HashMap::new();
    for (playback_id, current) in stopped {
        match current {
            PlaybackLifecycle::Pending { .. } => {
                scheduler.stop(&playback_id);
                publish_event(
                    event_sender,
                    observation,
                    AudioEvent::PlaybackStopped(PlaybackStopInfo {
                        playback_id,
                        voice: None,
                        reason,
                    }),
                );
            }
            PlaybackLifecycle::Playing { sequence, info } => {
                if let Err(error) = &render_stop {
                    publish_event(
                        event_sender,
                        observation,
                        AudioEvent::PlaybackFailed(PlaybackFailure {
                            playback_id: playback_id.clone(),
                            path: info.path.clone(),
                            error: error.clone(),
                        }),
                    );
                    still_playing.insert(playback_id, PlaybackLifecycle::Playing { sequence, info });
                }
            }
        }
    }
    lifecycle.extend(still_playing);
    true
}

pub(super) fn drain_playback_work(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    engine_config: &AudioEngineConfig,
    limits: &EngineLimits,
    scheduler: &mut PlaybackScheduler,
    voices: &mut PlaybackVoiceController,
    lifecycle: &mut PlaybackLifecycles,
) {
    let mut retired = Vec::new();
    voices.reclaim(|event| retired.push(event));
    for event in retired {
        publish_render_event(event_sender, observation, scheduler, lifecycle, event);
    }
    while let Some(result) = scheduler.try_recv() {
        let sequence = result.sequence();
        let playback_id = result.playback_id().clone();
        let is_current = lifecycle.get(&playback_id).is_some_and(
            |state| matches!(state, PlaybackLifecycle::Pending { sequence: active, .. } if *active == sequence),
        );
        if !is_current {
            continue;
        }
        match result {
            PlaybackPreparationResult::Failed(failure) => {
                lifecycle.remove(&failure.playback_id);
                publish_event(
                    event_sender,
                    observation,
                    AudioEvent::PlaybackFailed(PlaybackFailure {
                        playback_id: failure.playback_id,
                        path: failure.path,
                        error: failure.error,
                    }),
                );
            }
            PlaybackPreparationResult::Prepared {
                sequence,
                request,
                preparation,
            } => {
                let (channels, source, start_frame) = match preparation {
                    PlaybackPreparation::Resident(asset) => {
                        let start_frame =
                            crate::playback::frame_at_or_after(request.start_offset, engine_config.sample_rate)
                                .and_then(|frame| {
                                    usize::try_from(frame).map_err(|_| {
                                        AudioError::capacity_exceeded("playback start frame exceeds limits")
                                    })
                                });
                        (asset.channels(), PlaybackVoiceSource::Resident(asset), start_frame)
                    }
                    PlaybackPreparation::Stream { probe, reader } => {
                        let channels = probe.channels;
                        let scratch_len =
                            usize::from(channels).saturating_mul(engine_config.internal_block_frames.get() as usize);
                        (
                            channels,
                            PlaybackVoiceSource::Stream {
                                reader,
                                channels,
                                scratch: vec![0.0; scratch_len].into_boxed_slice(),
                            },
                            Ok(0),
                        )
                    }
                };
                let routes = default_playback_routes(channels, limits.max_virtual_outputs);
                let ramp_frames = (f64::from(engine_config.sample_rate.get()) * f64::from(engine_config.gain_ramp_ms)
                    / 1_000.0)
                    .round()
                    .max(1.0) as u32;
                let voice = start_frame.and_then(|start_frame| {
                    PlaybackVoice::new(
                        request.playback_id.clone(),
                        request.path.clone(),
                        source,
                        request.gain,
                        routes,
                        ramp_frames,
                        engine_config.internal_block_frames.get() as usize,
                    )
                    .and_then(|voice| voice.with_start_frame(start_frame))
                });
                let admitted = voice.and_then(|voice| voices.try_activate(voice));
                match admitted {
                    Ok(voice) => {
                        let info = PlaybackInfo {
                            playback_id: request.playback_id.clone(),
                            path: request.path,
                            voice,
                        };
                        lifecycle.insert(
                            request.playback_id,
                            PlaybackLifecycle::Playing {
                                sequence,
                                info: info.clone(),
                            },
                        );
                        publish_event(event_sender, observation, AudioEvent::PlaybackStarted(info));
                    }
                    Err(error) => {
                        scheduler.complete(&request.playback_id, sequence);
                        lifecycle.remove(&request.playback_id);
                        publish_event(
                            event_sender,
                            observation,
                            AudioEvent::PlaybackFailed(PlaybackFailure {
                                playback_id: request.playback_id,
                                path: request.path,
                                error,
                            }),
                        );
                    }
                }
            }
        }
    }
    publish_playback_observation(observation, scheduler, voices, lifecycle);
}

fn publish_playback_observation(
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    scheduler: &PlaybackScheduler,
    voices: &PlaybackVoiceController,
    lifecycle: &PlaybackLifecycles,
) {
    let cache = scheduler.cache_observation();
    let loading_voices = lifecycle
        .values()
        .filter(|state| matches!(state, PlaybackLifecycle::Pending { .. }))
        .count();
    let active_voices = lifecycle
        .values()
        .filter(|state| matches!(state, PlaybackLifecycle::Playing { .. }))
        .count();
    update_observation(observation, |snapshot| {
        snapshot.playback = crate::PlaybackObservation {
            loading_voices: u16::try_from(loading_voices).unwrap_or(u16::MAX),
            active_voices: u16::try_from(active_voices).unwrap_or(u16::MAX),
            command_queue_pressure_count: voices.command_queue_full(),
            cache_entries: u64::try_from(cache.entries).unwrap_or(u64::MAX),
            resident_bytes: cache.resident_bytes,
            cache_hits: cache.hits,
            cache_misses: cache.misses,
            cache_invalidations: cache.invalidations,
            cache_evictions: cache.evictions,
        };
    });
}

fn publish_render_event(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    scheduler: &mut PlaybackScheduler,
    lifecycle: &mut PlaybackLifecycles,
    event: PlaybackRenderEvent,
) {
    let (playback_id, voice) = match &event {
        PlaybackRenderEvent::Finished { playback_id, voice, .. }
        | PlaybackRenderEvent::Stopped { playback_id, voice, .. }
        | PlaybackRenderEvent::Failed { playback_id, voice, .. } => (playback_id.clone(), *voice),
    };
    if let Some(PlaybackLifecycle::Playing { sequence, info }) = lifecycle.get(&playback_id)
        && info.voice == voice
    {
        let sequence = *sequence;
        lifecycle.remove(&playback_id);
        scheduler.complete(&playback_id, sequence);
    }
    let audio_event = match event {
        PlaybackRenderEvent::Finished {
            playback_id,
            path,
            voice,
            ..
        } => AudioEvent::PlaybackFinished(PlaybackInfo {
            playback_id,
            path,
            voice,
        }),
        PlaybackRenderEvent::Stopped {
            playback_id,
            voice,
            reason,
            ..
        } => AudioEvent::PlaybackStopped(PlaybackStopInfo {
            playback_id,
            voice: Some(voice),
            reason,
        }),
        PlaybackRenderEvent::Failed { playback_id, path, .. } => AudioEvent::PlaybackFailed(PlaybackFailure {
            playback_id,
            path,
            error: AudioError::new(
                AudioErrorCategory::DecodeFailed,
                "streamed playback decoder failed after the voice started",
            ),
        }),
    };
    publish_event(event_sender, observation, audio_event);
}
