//! Fixed qualification inputs; product authoring/default policy does not live in this harness.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    Diagnostic,
    Q01,
    Q02,
}

impl std::str::FromStr for Scenario {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "diagnostic" => Ok(Self::Diagnostic),
            "q01" => Ok(Self::Q01),
            "q02" => Ok(Self::Q02),
            _ => Err("scenario must be diagnostic, q01 or q02".into()),
        }
    }
}

impl Scenario {
    pub fn name(self) -> &'static str {
        match self {
            Self::Diagnostic => "diagnostic",
            Self::Q01 => "q01",
            Self::Q02 => "q02",
        }
    }
    pub fn extent(self) -> [u32; 2] {
        if self == Self::Q02 { [3840, 2160] } else { [1920, 1080] }
    }
    pub fn cards(self) -> bool {
        self != Self::Diagnostic
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct RunSpec {
    pub scenario: Scenario,
    presentation_policy: PresentationPolicy,
    gpu_stages: bool,
    consumer_mode: ConsumerMode,
    #[serde(flatten)]
    exercise: Exercise,
}

/// A qualification control for native output; editor presentation keeps the baseline interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationPolicy {
    Latest,
    OutputVblank,
}

impl std::str::FromStr for PresentationPolicy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "latest" => Ok(Self::Latest),
            "output-vblank" => Ok(Self::OutputVblank),
            _ => Err("presentation policy must be latest or output-vblank".into()),
        }
    }
}

/// A diagnostic control for shared-output consumption during the sample window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerMode {
    Active,
    Paused,
}

impl ConsumerMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
        }
    }
}

impl std::str::FromStr for ConsumerMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            _ => Err("consumer mode must be active or paused".into()),
        }
    }
}

/// Why rendering stopped, recorded before teardown can add time to an incomplete run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationEnd {
    DeadlineReached,
    Recreated,
    FinalGenerationComplete,
    WindowClosed,
    EditorIsolationComplete,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "exercise", rename_all = "snake_case")]
enum Exercise {
    Timed { sample_seconds: u32, warmup_seconds: u32 },
    Recreate { cycles: u32 },
    EditorIsolation,
}

impl RunSpec {
    pub fn new(scenario: Scenario, sample_seconds: u32, warmup_seconds: u32) -> Result<Self, String> {
        let total = sample_seconds.checked_add(warmup_seconds).ok_or("duration overflow")?;
        if sample_seconds < 8 || total > 3600 {
            return Err("sample must be at least 8 seconds; total must not exceed 3600".into());
        }
        Ok(Self {
            scenario,
            presentation_policy: PresentationPolicy::Latest,
            gpu_stages: false,
            consumer_mode: ConsumerMode::Active,
            exercise: Exercise::Timed {
                sample_seconds,
                warmup_seconds,
            },
        })
    }

    /// Keep lifecycle and editor-isolation exercises on the baseline while comparing one
    /// presentation setting in an explicitly selected timed fixed scene.
    pub fn with_presentation_policy(mut self, policy: PresentationPolicy) -> Result<Self, String> {
        if !matches!(self.exercise, Exercise::Timed { .. }) || !self.scenario.cards() {
            return Err("presentation comparison requires timed q01/q02 qualification".into());
        }
        if self.consumer_mode == ConsumerMode::Paused && policy != PresentationPolicy::Latest {
            return Err("paused-consumer control requires the latest presentation policy".into());
        }
        if self.gpu_stages && policy != PresentationPolicy::Latest {
            return Err("GPU stage timing requires the latest presentation policy".into());
        }
        self.presentation_policy = policy;
        Ok(self)
    }

    /// Additional timestamp queries are an explicit diagnostic in the baseline fixed scene.
    pub fn with_gpu_stages(mut self) -> Result<Self, String> {
        if !matches!(self.exercise, Exercise::Timed { .. }) || !self.scenario.cards() {
            return Err("GPU stage timing requires timed q01/q02 qualification".into());
        }
        if self.presentation_policy != PresentationPolicy::Latest {
            return Err("GPU stage timing requires the latest presentation policy".into());
        }
        self.gpu_stages = true;
        Ok(self)
    }

    pub fn gpu_stages(self) -> bool {
        self.gpu_stages
    }

    pub fn with_paused_consumer(mut self) -> Result<Self, String> {
        if !matches!(self.exercise, Exercise::Timed { warmup_seconds: 30, .. })
            || !self.scenario.cards()
            || !self.gpu_stages
            || self.presentation_policy != PresentationPolicy::Latest
        {
            return Err("paused-consumer control requires timed q01/q02, exactly 30 seconds warmup, GPU stages and latest presentation".into());
        }
        self.consumer_mode = ConsumerMode::Paused;
        Ok(self)
    }

    pub fn consumer_mode(self) -> ConsumerMode {
        self.consumer_mode
    }

    pub fn presentation_policy(self) -> PresentationPolicy {
        self.presentation_policy
    }

    pub fn total_seconds(self) -> u32 {
        match self.exercise {
            Exercise::Timed {
                sample_seconds,
                warmup_seconds,
            } => sample_seconds + warmup_seconds,
            // Recreation has an overall watchdog, not a fixed measurement window. Budget
            // operating time, retirement fences and final live-scene settling after the last ack.
            Exercise::Recreate { cycles } => 8 + cycles * 4 + self.settling_seconds(),
            Exercise::EditorIsolation => 185,
        }
    }

    pub fn warmup_seconds(self) -> u32 {
        match self.exercise {
            Exercise::Timed { warmup_seconds, .. } => warmup_seconds,
            Exercise::Recreate { .. } | Exercise::EditorIsolation => 0,
        }
    }

    pub fn device_recreations(self) -> u32 {
        match self.exercise {
            Exercise::Recreate { cycles } => cycles,
            Exercise::Timed { .. } | Exercise::EditorIsolation => 0,
        }
    }

    /// Observe retirement of older devices with the final scene and ownership set still live.
    pub fn settling_seconds(self) -> u32 {
        match self.exercise {
            Exercise::Recreate { .. } => 10,
            Exercise::Timed { .. } | Exercise::EditorIsolation => 0,
        }
    }

    pub fn recreation(scenario: Scenario, cycles: u32) -> Result<Self, String> {
        if !(1..=100).contains(&cycles) || !scenario.cards() {
            return Err("recreation requires q01/q02 and 1..=100 cycles".into());
        }
        Ok(Self {
            scenario,
            presentation_policy: PresentationPolicy::Latest,
            gpu_stages: false,
            consumer_mode: ConsumerMode::Active,
            exercise: Exercise::Recreate { cycles },
        })
    }

    pub fn editor_isolation(scenario: Scenario) -> Result<Self, String> {
        if !scenario.cards() {
            return Err("editor isolation requires q01/q02 fixtures".into());
        }
        Ok(Self {
            scenario,
            presentation_policy: PresentationPolicy::Latest,
            gpu_stages: false,
            consumer_mode: ConsumerMode::Active,
            exercise: Exercise::EditorIsolation,
        })
    }

    pub fn isolates_editor(self) -> bool {
        matches!(self.exercise, Exercise::EditorIsolation)
    }

    pub fn validate_generation_end(self, generation: u32, end: GenerationEnd) -> Result<(), String> {
        match (self.exercise, generation, end) {
            (Exercise::EditorIsolation, 0, GenerationEnd::EditorIsolationComplete) => Ok(()),
            (Exercise::Timed { .. }, 0, GenerationEnd::DeadlineReached) => Ok(()),
            (Exercise::Timed { .. }, 0, GenerationEnd::WindowClosed) if !self.scenario.cards() => Ok(()),
            (Exercise::Recreate { cycles }, generation, GenerationEnd::Recreated) if generation < cycles => Ok(()),
            (Exercise::Recreate { cycles }, generation, GenerationEnd::FinalGenerationComplete)
                if generation == cycles =>
            {
                Ok(())
            }
            (_, _, GenerationEnd::WindowClosed) => {
                Err("qualification cancelled by window close before completion".into())
            }
            (Exercise::Recreate { .. }, _, GenerationEnd::DeadlineReached) => {
                Err("device recreation watchdog expired before the final operating interval completed".into())
            }
            _ => Err(format!("invalid generation termination: {generation}, {end:?}")),
        }
    }
}

#[cfg(test)]
mod tests;
