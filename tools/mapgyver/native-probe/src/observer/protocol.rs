#[cfg(feature = "desktop")]
use mapgyver_native_probe::workload::Scenario;
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
    BeginFrameAge {
        config: mapgyver_native_probe::frame_age::Config,
    },
    Sample {
        phase: CompositePhase,
    },
    VerifyGeneration {
        generation: u32,
    },
    Finish,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Response {
    Ready {
        process_id: u32,
        adapter_luid: String,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ObserverReport {
    pub process_id: u32,
    pub adapter_luid: String,
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
