#[cfg(feature = "desktop")]
use mapgyver_native_probe::workload::Scenario;
use mapgyver_native_probe::{allocation::Operations, sample_window::Window, workload::ConsumerMode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositePhase {
    Initial,
    Resized,
    SourceAbsent,
    Recovered,
}

impl CompositePhase {
    pub const ALL: [Self; 4] = [Self::Initial, Self::Resized, Self::SourceAbsent, Self::Recovered];

    #[cfg(feature = "desktop")]
    pub fn matches_source(self, extent: Option<[u32; 2]>, scenario: Scenario) -> bool {
        let [width, height] = scenario.extent();
        match self {
            Self::Initial | Self::Recovered => extent == Some([width, height]),
            Self::Resized => extent == Some([width / 2, height / 2]),
            Self::SourceAbsent => extent.is_none(),
        }
    }

    pub fn expected_pixels(self) -> [[u8; 4]; 2] {
        let left = match self {
            Self::Initial => [0, 0, 255, 255],
            Self::Resized => [0, 255, 0, 255],
            Self::SourceAbsent => [13, 13, 13, 255],
            Self::Recovered => [255, 0, 0, 255],
        };
        [left, [0, 255, 255, 255]]
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Request {
    BeginObservation { window: Window },
    Sample { phase: CompositePhase },
    VerifyGeneration { generation: u32 },
    Finish,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Response {
    Ready {
        process_id: u32,
        adapter_luid: String,
        consumer_mode: ConsumerMode,
    },
    ObservationReady {
        ready: ObservationReady,
    },
    Checkpoint {
        result: Checkpoint,
    },
    Generation {
        generation: u32,
        result: Checkpoint,
    },
    FrameAgeChunk {
        offset: usize,
        samples: Vec<mapgyver_native_probe::frame_age::Sample>,
    },
    Resources {
        message: mapgyver_native_probe::resource_log::Message,
    },
    Finished {
        report: Box<ObserverReport>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub phase: CompositePhase,
    pub extent: [u32; 2],
    pub sampled_bgra: Vec<[[u8; 4]; 2]>,
    pub sampled_alpha: Vec<[u8; 2]>,
    pub sampled_generation_markers: Vec<[u8; 4]>,
    pub elapsed_ms: f64,
}

pub fn expected_alpha(phase: CompositePhase) -> [u8; 2] {
    [
        if phase == CompositePhase::SourceAbsent {
            255
        } else {
            191
        },
        191,
    ]
}

pub fn generation_marker(generation: u32) -> [u8; 4] {
    [generation as u8, (generation >> 8) as u8, 165, 255]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservationReady {
    pub window: Window,
    pub consumer_mode: ConsumerMode,
    pub completed_qpc: u64,
    pub operations_at_fence: Operations,
    pub completion_markers: u32,
    pub skipped_completion_markers: u32,
}

impl ObservationReady {
    pub fn validate(&self, window: Window, consumer_mode: ConsumerMode, received_qpc: u64) -> Result<(), String> {
        window.validate()?;
        if self.window != window || self.consumer_mode != consumer_mode || window.warmup_seconds != 30 {
            return Err("observer readiness mode/window differs from the requested observation".into());
        }
        if self.completed_qpc < window.start_qpc
            || self.completed_qpc > received_qpc
            || received_qpc >= window.sample_begin_qpc()
        {
            return Err("observer readiness did not complete and arrive before sampling".into());
        }
        Ok(())
    }

    #[cfg(any(feature = "desktop", test))]
    pub fn same_as(&self, other: &Self) -> bool {
        self.window == other.window
            && self.consumer_mode == other.consumer_mode
            && self.completed_qpc == other.completed_qpc
            && self.completion_markers == other.completion_markers
            && self.skipped_completion_markers == other.skipped_completion_markers
            && self.operations_unchanged(&other.operations_at_fence)
    }

    pub fn operations_unchanged(&self, operations: &Operations) -> bool {
        operation_counts(&self.operations_at_fence) == operation_counts(operations)
    }

    pub fn resource_activity_unchanged(
        &self,
        operations: &Operations,
        completion_markers: u32,
        skipped_completion_markers: u32,
    ) -> bool {
        self.operations_unchanged(operations)
            && self.completion_markers == completion_markers
            && self.skipped_completion_markers == skipped_completion_markers
    }
}

fn operation_counts(operations: &Operations) -> [u64; 10] {
    let Operations {
        clears,
        shader_draws,
        uniform_upload_bytes,
        diagnostic_readback_pixels,
        sdk_send_calls,
        sdk_receive_calls,
        sdk_texture_queries,
        sdk_texture_observations,
        explicit_gpu_copy_calls,
        explicit_gpu_copy_bytes,
    } = operations;
    [
        *clears,
        *shader_draws,
        *uniform_upload_bytes,
        *diagnostic_readback_pixels,
        *sdk_send_calls,
        *sdk_receive_calls,
        *sdk_texture_queries,
        *sdk_texture_observations,
        *explicit_gpu_copy_calls,
        *explicit_gpu_copy_bytes,
    ]
}

pub(super) fn checkpoint_before_sample(window: Option<Window>, qpc: u64) -> Result<(), String> {
    if window.is_some_and(|window| window.warmup_seconds == 30 && qpc >= window.sample_begin_qpc()) {
        return Err("observer content checkpoint overlapped the sample window".into());
    }
    Ok(())
}

pub(super) fn validate_observation_window(window: Window, consumer_mode: ConsumerMode) -> Result<(), String> {
    window.validate()?;
    if consumer_mode == ConsumerMode::Paused && window.warmup_seconds != 30 {
        return Err("paused observer requires a thirty-second warmup".into());
    }
    Ok(())
}

pub(super) fn validate_observation_finish(
    window: Option<Window>,
    consumer_mode: ConsumerMode,
    finish_qpc: Option<u64>,
) -> Result<(), String> {
    match (window, finish_qpc) {
        (Some(window), Some(finish_qpc)) => {
            validate_observation_window(window, consumer_mode)?;
            let end_qpc = window.start_qpc + window.frequency * u64::from(window.maximum_seconds);
            if finish_qpc < window.start_qpc || (consumer_mode == ConsumerMode::Paused && finish_qpc < end_qpc) {
                return Err("observer cleanup began before its required observation window ended".into());
            }
            Ok(())
        }
        (None, None) if consumer_mode == ConsumerMode::Active => Ok(()),
        _ => Err("observer Finish clock/window is missing or inconsistent".into()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObserverReport {
    pub process_id: u32,
    pub adapter_luid: String,
    pub consumer_mode: ConsumerMode,
    pub observation_ready: Option<ObservationReady>,
    pub observation_ready_received_qpc: Option<u64>,
    pub observation_finish_qpc: Option<u64>,
    pub checkpoints: Vec<Checkpoint>,
    pub received_observations: u32,
    pub diagnostic_readbacks: u32,
    pub verified_generations: u32,
    pub frame_age: Option<mapgyver_native_probe::frame_age::Report>,
    pub resources: Option<mapgyver_native_probe::resource_log::Report>,
}

#[cfg(any(feature = "desktop", test))]
pub fn request_line(request: &Request) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(request)?;
    bytes.push(b'\n');
    Ok(bytes)
}
