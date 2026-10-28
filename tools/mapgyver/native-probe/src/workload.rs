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
    #[serde(flatten)]
    exercise: Exercise,
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
            exercise: Exercise::Timed {
                sample_seconds,
                warmup_seconds,
            },
        })
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
            exercise: Exercise::Recreate { cycles },
        })
    }

    pub fn editor_isolation(scenario: Scenario) -> Result<Self, String> {
        if !scenario.cards() {
            return Err("editor isolation requires q01/q02 fixtures".into());
        }
        Ok(Self {
            scenario,
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
