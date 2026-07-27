use std::sync::{Arc, RwLock, mpsc::SyncSender};

use crate::{
    AudioBackend, AudioConfiguration, AudioEngineConfig, AudioError, AudioEvent, AudioObservationSnapshot,
    BackendPolicy, ConfigGeneration, EngineLimits, NullBackend, RenderCompileContext, RenderPlanCompiler,
};

#[cfg(all(feature = "analysis", feature = "playback"))]
use super::render_runtime::ManagedRenderRuntime;
use super::{
    device_runtime::DeviceRuntime,
    engine::{publish_event, update_observation},
};

pub(super) struct ApplyConfigurationContext<'a> {
    pub(super) event_sender: &'a SyncSender<AudioEvent>,
    pub(super) observation: &'a Arc<RwLock<AudioObservationSnapshot>>,
    pub(super) engine_config: &'a AudioEngineConfig,
    pub(super) limits: &'a EngineLimits,
    pub(super) backends: &'a [Arc<dyn AudioBackend>],
    pub(super) devices: &'a mut DeviceRuntime,
    #[cfg(all(feature = "analysis", feature = "playback"))]
    pub(super) managed_render_runtime: Option<&'a mut ManagedRenderRuntime>,
}

pub(super) fn validate_backends(backends: &[Arc<dyn AudioBackend>], policy: &BackendPolicy) -> Result<(), AudioError> {
    let mut ids = std::collections::HashSet::with_capacity(backends.len());
    for backend in backends {
        let id = backend.id();
        if !ids.insert(id.clone()) {
            return Err(AudioError::invalid_configuration(format!(
                "duplicate audio backend ID {id}"
            )));
        }
    }
    for preferred in &policy.preferred {
        if !ids.contains(preferred) && !(policy.allow_null_fallback && preferred == &NullBackend::backend_id()) {
            return Err(AudioError::invalid_configuration(format!(
                "preferred audio backend {preferred} was not registered"
            )));
        }
    }
    if backends.is_empty() && !policy.allow_null_fallback {
        return Err(AudioError::invalid_configuration(
            "audio engine has no registered backend and null fallback is disabled",
        ));
    }
    Ok(())
}

#[cfg_attr(not(all(feature = "analysis", feature = "playback")), allow(unused_mut))]
pub(super) fn apply_configuration(
    mut runtime: ApplyConfigurationContext<'_>,
    generation: ConfigGeneration,
    config: AudioConfiguration,
) {
    let active_generation = runtime
        .observation
        .read()
        .map(|snapshot| snapshot.generation)
        .unwrap_or(ConfigGeneration::INITIAL);
    if generation <= active_generation {
        publish_event(
            runtime.event_sender,
            runtime.observation,
            AudioEvent::ConfigurationRejected {
                generation,
                error: AudioError::invalid_configuration(format!(
                    "configuration generation {generation} is not newer than active generation {active_generation}"
                )),
            },
        );
        return;
    }
    let context = RenderCompileContext::derive_from_configuration(&config);
    match RenderPlanCompiler::new(runtime.engine_config.clone(), runtime.limits.clone()).compile(&config, &context) {
        Ok(compilation) => {
            let inputs = config
                .virtual_inputs
                .iter()
                .map(|channel| crate::ChannelObservation {
                    channel: channel.id,
                    rms_linear: 0.0,
                    rms_dbfs: crate::GainDb::SILENCE_DB,
                    peak_dbfs: crate::GainDb::SILENCE_DB,
                    clipped: false,
                })
                .collect();
            let outputs = config
                .virtual_outputs
                .iter()
                .map(|channel| crate::ChannelObservation {
                    channel: channel.id,
                    rms_linear: 0.0,
                    rms_dbfs: crate::GainDb::SILENCE_DB,
                    peak_dbfs: crate::GainDb::SILENCE_DB,
                    clipped: false,
                })
                .collect();
            let analysis_taps = config
                .analysis_taps
                .iter()
                .map(|tap| crate::AnalysisTapObservation {
                    tap: tap.id,
                    source: tap.source,
                    enabled: tap.enabled,
                    result: None,
                })
                .collect();
            #[cfg(all(feature = "analysis", feature = "playback"))]
            if let Some(managed) = runtime.managed_render_runtime.as_deref_mut()
                && let Err(error) = managed.publish_plan(generation, compilation.plan.clone())
            {
                publish_event(
                    runtime.event_sender,
                    runtime.observation,
                    AudioEvent::ConfigurationRejected { generation, error },
                );
                return;
            }
            runtime.devices.configure(
                runtime.event_sender,
                runtime.observation,
                runtime.backends,
                &config,
                compilation.plan,
                #[cfg(all(feature = "analysis", feature = "playback"))]
                runtime.managed_render_runtime.as_deref_mut(),
            );
            update_observation(runtime.observation, |snapshot| {
                snapshot.generation = generation;
                snapshot.enabled = config.enabled;
                snapshot.inputs = inputs;
                snapshot.outputs = outputs;
                snapshot.analysis = crate::AnalysisObservationSnapshot {
                    generation,
                    taps: analysis_taps,
                    ..crate::AnalysisObservationSnapshot::default()
                };
            });
            publish_event(
                runtime.event_sender,
                runtime.observation,
                AudioEvent::ConfigurationApplied { generation },
            );
        }
        Err(error) => publish_event(
            runtime.event_sender,
            runtime.observation,
            AudioEvent::ConfigurationRejected { generation, error },
        ),
    }
}
