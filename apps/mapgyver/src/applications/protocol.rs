use golden_core::model::NodeUuid;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct ApplicationId(pub NodeUuid);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ApplicationWindow {
    #[default]
    Hidden,
    Normal,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ApplicationReadiness {
    /// Process readiness only; never proof that a texture/sender has produced a frame.
    #[default]
    ProcessStarted,
    Spout {
        sender: String,
        first_frame: bool,
        timeout_ms: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct ApplicationStop {
    pub graceful_ms: u32,
    pub retirement_ms: u32,
}
impl Default for ApplicationStop {
    fn default() -> Self {
        Self {
            graceful_ms: 2000,
            retirement_ms: 5000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct ApplicationRestart {
    pub enabled: bool,
    pub initial_ms: u32,
    pub maximum_ms: u32,
}
impl Default for ApplicationRestart {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_ms: 500,
            maximum_ms: 30_000,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct ApplicationDefinition {
    pub executable: Option<String>,
    pub arguments: Vec<String>,
    pub directory: Option<String>,
    pub environment: Option<Vec<(String, String)>>,
    pub window: ApplicationWindow,
    pub readiness: ApplicationReadiness,
    pub stop: ApplicationStop,
    pub restart: ApplicationRestart,
}
impl ApplicationDefinition {
    pub fn process_spec(&self) -> Result<Option<golden_io::owned_process::ProcessSpec>, String> {
        use golden_io::owned_process::{ProcessSpec, ProcessWindow};
        let Some(executable) = &self.executable else {
            return Ok(None);
        };
        let mut spec = ProcessSpec::new(executable);
        spec.arguments = self.arguments.iter().map(Into::into).collect();
        spec.directory = self.directory.as_ref().map(Into::into);
        spec.environment = self.environment.as_ref().map(|entries| {
            entries
                .iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect()
        });
        spec.window = match self.window {
            ApplicationWindow::Hidden => ProcessWindow::Hidden,
            ApplicationWindow::Normal => ProcessWindow::Normal,
        };
        spec.validate().map_err(|error| error.to_string())?;
        Ok(Some(spec))
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.arguments.len() > 256
            || self.environment.as_ref().is_some_and(|entries| entries.len() > 128)
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 32 * 1024
        {
            return Err("application definition exceeds 256 arguments, 128 environment entries or 32 KiB".into());
        }
        if self.arguments.iter().any(|value| value.contains('\0'))
            || self.directory.as_ref().is_some_and(|value| value.contains('\0'))
            || self
                .environment
                .iter()
                .flatten()
                .any(|(name, value)| name.is_empty() || name.contains(['\0', '=']) || value.contains('\0'))
        {
            return Err("application strings must be NUL-free and environment names nonempty without '='".into());
        }
        if let ApplicationReadiness::Spout { sender, timeout_ms, .. } = &self.readiness
            && (sender.is_empty()
                || sender.len() > 255
                || sender.contains('\0')
                || *timeout_ms == 0
                || *timeout_ms > 600_000)
        {
            return Err("Spout readiness requires a sender (1..255 bytes) and timeout 1..600000 ms".into());
        }
        if self.stop.graceful_ms > 600_000
            || self.stop.retirement_ms == 0
            || self.stop.retirement_ms > 600_000
            || self.restart.initial_ms == 0
            || self.restart.maximum_ms < self.restart.initial_ms
            || self.restart.maximum_ms > 600_000
        {
            return Err("application stop/restart deadlines are invalid or exceed 600000 ms".into());
        }
        self.process_spec()?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ColumnApplication {
    pub application: ApplicationId,
    pub pre: u16,
    pub post: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ApplicationSnapshot {
    pub id: ApplicationId,
    pub label: String,
    pub enabled: bool,
    pub revision: u64,
    /// Ephemeral node lifetime; Undo/reload can recreate the same persistent application ID.
    pub definition_instance: uuid::Uuid,
    pub definition: ApplicationDefinition,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ApplicationSummary {
    pub id: ApplicationId,
    pub label: String,
    pub enabled: bool,
    pub configured: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ApplicationReference {
    pub application: ApplicationId,
    /// None means the saved reference has no definition in this project's catalog.
    pub summary: Option<ApplicationSummary>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ApplicationPage {
    pub applications: Vec<ApplicationSummary>,
    pub total: u32,
    pub start: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ApplicationPhase {
    Idle,
    Starting,
    WaitingForSender,
    Ready,
    Backoff,
    Stopping,
    RetirementUnverified,
    Failed,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ApplicationConfigurationState {
    NotRunning,
    Current,
    Pending,
    Retained,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ApplicationRuntimeStatus {
    pub application: ApplicationId,
    pub launch: Option<uuid::Uuid>,
    pub phase: ApplicationPhase,
    pub process_id: Option<u32>,
    pub exit_code: Option<u32>,
    pub failures: u32,
    pub next_action_ms: Option<u64>,
    pub message: Option<String>,
    pub configuration: ApplicationConfigurationState,
    /// Saved definition revision against which this observation was reconciled.
    pub definition_revision: Option<u64>,
    pub definition_instance: Option<uuid::Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ApplicationRuntimeSnapshot {
    pub project: uuid::Uuid,
    pub revision: u64,
    /// Monotonic milliseconds since this runtime started; deadlines use the same origin.
    pub elapsed_ms: u64,
    pub applications: Vec<ApplicationRuntimeStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ApplicationEdit {
    InspectReferences {
        applications: Vec<ApplicationId>,
    },
    InspectRuntime {},
    Create {
        label: Option<String>,
    },
    Configure {
        application: ApplicationId,
        definition: ApplicationDefinition,
    },
    Rename {
        application: ApplicationId,
        label: String,
    },
    Remove {
        application: ApplicationId,
    },
    Duplicate {
        application: ApplicationId,
    },
    Inspect {
        application: ApplicationId,
    },
    InspectCatalog {
        search: String,
        start: u32,
        count: u16,
    },
}
