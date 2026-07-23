use crate::{
    AudioBackendState, AudioBackendStatus, AudioBufferPolicy, AudioDeviceDescriptor, AudioDeviceInspectorState,
    AudioDeviceMatch, AudioDeviceReadiness, AudioDeviceSelection, AudioDirection, AudioError, AudioErrorCategory,
    AudioInspectorError, AudioPermissionState, AudioRecoveryPolicy, AudioStreamStatus, BackendId,
    NegotiatedStreamFormat, SampleRate, assert_not_realtime, match_device_selection,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryBackoff {
    pub initial_ms: u64,
    pub maximum_ms: u64,
    pub jitter_percent: u8,
    pub seed: u64,
}

impl RetryBackoff {
    pub fn validate(self) -> Result<(), AudioError> {
        if self.initial_ms == 0 || self.maximum_ms < self.initial_ms {
            return Err(AudioError::invalid_configuration(
                "retry backoff requires a nonzero initial delay not exceeding its maximum",
            ));
        }
        if self.jitter_percent > 50 {
            return Err(AudioError::invalid_configuration(
                "retry backoff jitter must not exceed 50 percent",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn delay_ms(self, attempt: u32, direction: AudioDirection) -> u64 {
        let exponent = attempt.saturating_sub(1).min(31);
        let base = self.initial_ms.saturating_mul(1_u64 << exponent).min(self.maximum_ms);
        let span = base.saturating_mul(u64::from(self.jitter_percent)) / 100;
        if span == 0 {
            return base;
        }
        let direction_salt = match direction {
            AudioDirection::Input => 0x9e37_79b9_7f4a_7c15,
            AudioDirection::Output => 0xd1b5_4a32_d192_ed03,
        };
        let mixed = splitmix64(self.seed ^ direction_salt ^ u64::from(attempt));
        let width = span.saturating_mul(2).saturating_add(1);
        let jitter = (mixed % width) as i128 - i128::from(span);
        (i128::from(base) + jitter).max(1) as u64
    }
}

impl Default for RetryBackoff {
    fn default() -> Self {
        Self {
            initial_ms: 100,
            maximum_ms: 10_000,
            jitter_percent: 20,
            seed: 0x676f_6c64_656e_6175,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeviceSupervisorConfig {
    pub retry: RetryBackoff,
}

impl DeviceSupervisorConfig {
    pub fn validate(self) -> Result<(), AudioError> {
        self.retry.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceSwitchPhase {
    Disabled,
    Discovering,
    Missing,
    RetryWaiting,
    Preparing,
    Primed,
    Switching,
    Stable,
    Failed,
}

#[derive(Clone, Debug)]
pub struct SupervisorDirection {
    direction: AudioDirection,
    selection: Option<AudioDeviceSelection>,
    status: AudioStreamStatus,
    phase: DeviceSwitchPhase,
    prepared_device: Option<AudioDeviceDescriptor>,
    active_device: Option<AudioDeviceDescriptor>,
    retry: RetryBackoff,
}

impl SupervisorDirection {
    fn new(direction: AudioDirection, retry: RetryBackoff) -> Self {
        Self {
            direction,
            selection: None,
            status: AudioStreamStatus::disabled(direction),
            phase: DeviceSwitchPhase::Disabled,
            prepared_device: None,
            active_device: None,
            retry,
        }
    }

    pub fn configure(
        &mut self,
        enabled: bool,
        selection: Option<AudioDeviceSelection>,
        recovery_policy: AudioRecoveryPolicy,
    ) {
        assert_not_realtime("audio device supervisor configuration");
        self.selection = selection;
        self.status.enabled = enabled;
        self.status.recovery_policy = recovery_policy;
        self.status.selected_target = self.selection.as_ref().map(|value| value.target.clone());
        self.status.selected_label = self.selection.as_ref().map(|value| value.last_known_label.clone());
        self.status.profile_key = self.selection.as_ref().map(|value| value.profile_key.clone());
        self.status.retry_attempt = 0;
        self.status.next_retry_ms = None;
        self.status.error = None;
        self.prepared_device = None;
        if enabled {
            self.phase = DeviceSwitchPhase::Discovering;
            self.status.readiness = AudioDeviceReadiness::Discovering;
        } else {
            self.phase = DeviceSwitchPhase::Disabled;
            self.status.readiness = AudioDeviceReadiness::Disabled;
            self.status.active_target = None;
            self.status.format = None;
            self.active_device = None;
        }
    }

    pub fn observe_devices(&mut self, now_ms: u64, devices: &[AudioDeviceDescriptor]) {
        assert_not_realtime("audio device discovery application");
        if !self.status.enabled {
            return;
        }
        let Some(selection) = &self.selection else {
            self.fail(
                AudioDeviceReadiness::Failed,
                AudioError::invalid_configuration("enabled audio direction has no device selection"),
            );
            return;
        };
        let effective = if self.status.recovery_policy == AudioRecoveryPolicy::FollowSystemDefault {
            AudioDeviceSelection::follow_system_default(selection.target.backend().clone(), self.direction)
        } else {
            selection.clone()
        };
        match match_device_selection(&effective, self.direction, devices) {
            AudioDeviceMatch::Matched(device) => {
                if self.active_device.as_ref() == Some(device.as_ref()) {
                    self.phase = DeviceSwitchPhase::Stable;
                    self.status.readiness = AudioDeviceReadiness::Ready;
                    self.status.retry_attempt = 0;
                    self.status.next_retry_ms = None;
                    self.status.error = None;
                    return;
                }
                self.prepared_device = Some(*device);
                self.phase = DeviceSwitchPhase::Preparing;
                self.status.readiness = AudioDeviceReadiness::Preparing;
                self.status.next_retry_ms = None;
                self.status.error = None;
            }
            AudioDeviceMatch::Missing => {
                self.prepared_device = None;
                self.active_device = None;
                self.status.active_target = None;
                self.status.format = None;
                self.phase = DeviceSwitchPhase::Missing;
                self.status.readiness = AudioDeviceReadiness::Missing;
                self.schedule_retry(now_ms);
            }
            AudioDeviceMatch::Ambiguous(candidates) => {
                self.prepared_device = None;
                self.fail(
                    AudioDeviceReadiness::Failed,
                    AudioError::new(
                        AudioErrorCategory::DeviceMissing,
                        "fallback device fingerprint matches multiple devices; selection remains unresolved",
                    )
                    .with_context(
                        "candidates",
                        candidates
                            .iter()
                            .map(|target| format!("{target:?}"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                );
            }
        }
    }

    pub fn mark_primed(&mut self, format: NegotiatedStreamFormat) -> Result<(), AudioError> {
        assert_not_realtime("audio stream priming");
        if self.phase != DeviceSwitchPhase::Preparing || self.prepared_device.is_none() {
            return Err(AudioError::new(
                AudioErrorCategory::InternalInvariant,
                "audio stream can only be primed after device preparation",
            ));
        }
        format.validate()?;
        self.status.format = Some(format);
        self.phase = DeviceSwitchPhase::Primed;
        self.status.readiness = AudioDeviceReadiness::Primed;
        Ok(())
    }

    pub fn begin_switch(&mut self) -> Result<(), AudioError> {
        assert_not_realtime("audio stream switch");
        if self.phase != DeviceSwitchPhase::Primed {
            return Err(AudioError::new(
                AudioErrorCategory::InternalInvariant,
                "audio stream switch requires a primed stream",
            ));
        }
        self.phase = DeviceSwitchPhase::Switching;
        self.status.readiness = AudioDeviceReadiness::Switching;
        Ok(())
    }

    pub fn commit_switch(&mut self) -> Result<(), AudioError> {
        assert_not_realtime("audio stream switch commit");
        if self.phase != DeviceSwitchPhase::Switching {
            return Err(AudioError::new(
                AudioErrorCategory::InternalInvariant,
                "audio stream switch commit requires the switching phase",
            ));
        }
        let device = self.prepared_device.take().ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::InternalInvariant,
                "prepared audio device disappeared before switch commit",
            )
        })?;
        self.status.active_target = Some(device.target.clone());
        self.status.readiness = AudioDeviceReadiness::Ready;
        self.status.permission = AudioPermissionState::Granted;
        self.status.retry_attempt = 0;
        self.status.next_retry_ms = None;
        self.status.error = None;
        self.active_device = Some(device);
        self.phase = DeviceSwitchPhase::Stable;
        Ok(())
    }

    pub fn report_open_error(&mut self, now_ms: u64, error: AudioError) {
        assert_not_realtime("audio stream failure reporting");
        let readiness = match error.category {
            AudioErrorCategory::DeviceBusy => AudioDeviceReadiness::Busy,
            AudioErrorCategory::PermissionDenied => {
                self.status.permission = AudioPermissionState::Denied;
                AudioDeviceReadiness::PermissionDenied
            }
            AudioErrorCategory::DeviceMissing => AudioDeviceReadiness::Missing,
            AudioErrorCategory::BackendUnavailable => AudioDeviceReadiness::Unavailable,
            _ => AudioDeviceReadiness::Failed,
        };
        self.fail(readiness, error);
        self.schedule_retry(now_ms);
    }

    pub fn report_backend_unavailable(&mut self, now_ms: u64, backend: &AudioBackendStatus) {
        if !self.status.enabled {
            return;
        }
        self.active_device = None;
        self.prepared_device = None;
        self.status.active_target = None;
        self.status.format = None;
        self.fail(
            AudioDeviceReadiness::Unavailable,
            AudioError::new(
                AudioErrorCategory::BackendUnavailable,
                backend
                    .detail
                    .clone()
                    .unwrap_or_else(|| format!("audio backend {} is unavailable", backend.backend)),
            ),
        );
        self.schedule_retry(now_ms);
    }

    #[must_use]
    pub fn tick(&mut self, now_ms: u64) -> bool {
        if self.phase == DeviceSwitchPhase::RetryWaiting
            && self.status.next_retry_ms.is_some_and(|deadline| now_ms >= deadline)
        {
            self.phase = DeviceSwitchPhase::Discovering;
            self.status.readiness = AudioDeviceReadiness::Recovering;
            self.status.next_retry_ms = None;
            return true;
        }
        false
    }

    #[must_use]
    pub const fn phase(&self) -> DeviceSwitchPhase {
        self.phase
    }

    #[must_use]
    pub fn status(&self) -> &AudioStreamStatus {
        &self.status
    }

    fn fail(&mut self, readiness: AudioDeviceReadiness, error: AudioError) {
        self.phase = DeviceSwitchPhase::Failed;
        self.status.readiness = readiness;
        self.status.error = Some(AudioInspectorError::from(&error));
    }

    fn schedule_retry(&mut self, now_ms: u64) {
        self.status.retry_attempt = self.status.retry_attempt.saturating_add(1);
        let delay = self.retry.delay_ms(self.status.retry_attempt, self.direction);
        self.status.next_retry_ms = Some(now_ms.saturating_add(delay));
        self.phase = DeviceSwitchPhase::RetryWaiting;
    }
}

#[derive(Clone, Debug)]
pub struct DeviceSupervisor {
    config: DeviceSupervisorConfig,
    backends: Vec<AudioBackendStatus>,
    devices: Vec<AudioDeviceDescriptor>,
    pub input: SupervisorDirection,
    pub output: SupervisorDirection,
    engine_sample_rate: SampleRate,
    buffer_policy: AudioBufferPolicy,
}

impl DeviceSupervisor {
    pub fn new(
        config: DeviceSupervisorConfig,
        engine_sample_rate: SampleRate,
        buffer_policy: AudioBufferPolicy,
    ) -> Result<Self, AudioError> {
        config.validate()?;
        buffer_policy.validate()?;
        Ok(Self {
            config,
            backends: Vec::new(),
            devices: Vec::new(),
            input: SupervisorDirection::new(AudioDirection::Input, config.retry),
            output: SupervisorDirection::new(AudioDirection::Output, config.retry),
            engine_sample_rate,
            buffer_policy,
        })
    }

    pub fn observe_discovery(
        &mut self,
        now_ms: u64,
        backends: Vec<AudioBackendStatus>,
        devices: Vec<AudioDeviceDescriptor>,
    ) {
        assert_not_realtime("audio device discovery");
        self.backends = backends;
        self.devices = devices;
        observe_direction(now_ms, &self.backends, &self.devices, &mut self.input);
        observe_direction(now_ms, &self.backends, &self.devices, &mut self.output);
    }

    #[must_use]
    pub fn tick(&mut self, now_ms: u64) -> bool {
        self.input.tick(now_ms) | self.output.tick(now_ms)
    }

    #[must_use]
    pub fn inspector_state(&self) -> AudioDeviceInspectorState {
        AudioDeviceInspectorState {
            discovery_in_progress: matches!(
                (self.input.phase(), self.output.phase()),
                (DeviceSwitchPhase::Discovering, _) | (_, DeviceSwitchPhase::Discovering)
            ),
            backends: self.backends.clone(),
            devices: self.devices.clone(),
            input: self.input.status.clone(),
            output: self.output.status.clone(),
            engine_sample_rate: self.engine_sample_rate.get(),
            buffer_policy: self.buffer_policy,
        }
    }

    #[must_use]
    pub const fn config(&self) -> DeviceSupervisorConfig {
        self.config
    }
}

fn observe_direction(
    now_ms: u64,
    backends: &[AudioBackendStatus],
    devices: &[AudioDeviceDescriptor],
    direction: &mut SupervisorDirection,
) {
    let Some(selection) = &direction.selection else {
        direction.observe_devices(now_ms, devices);
        return;
    };
    if let Some(backend) = find_unavailable_backend(selection.target.backend(), backends) {
        direction.report_backend_unavailable(now_ms, backend);
    } else {
        direction.observe_devices(now_ms, devices);
    }
}

fn find_unavailable_backend<'a>(
    selected: &BackendId,
    backends: &'a [AudioBackendStatus],
) -> Option<&'a AudioBackendStatus> {
    backends
        .iter()
        .find(|backend| &backend.backend == selected && backend.state != AudioBackendState::Available)
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
