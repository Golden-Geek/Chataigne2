use std::sync::atomic::Ordering;
#[cfg(test)]
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::app::{
    ProjectGeneration, ProjectLifecycle, activate_engine_for_runtime, prepare_engine_candidate_for_runtime,
    prepare_engine_candidate_for_runtime_recovering, shutdown_engine_for_runtime, validate_engine_project_candidate,
};
use crate::engine::{Engine, ProjectLoadRecoveryReport};
use crate::ui_read_model::{RetiredUiReadModelState, UiReadModel, UiReadModelReplaceReason};
use crate::ui_sync::UiProjectFileSpec;

use super::ProductionRuntime;

/// Request to replace the live project with a decoded engine.
pub struct ProjectReplacement<T: ProjectLifecycle> {
    /// Decoded replacement engine.
    pub engine: Engine<T>,
    /// Host-owned project-file metadata for the new observation snapshot.
    pub project_file: UiProjectFileSpec,
    /// Stable replacement reason used by resynchronization diagnostics.
    pub reason: String,
    /// Whether recoverable runtime-startup failures may be retained in the report.
    pub recover: bool,
}

/// Timing and recovery evidence from replacing the live engine.
#[derive(Clone, Debug, Default)]
pub struct ProjectReplacementResult {
    /// Project generation installed by this replacement.
    pub project_generation: ProjectGeneration,
    /// Recoverable load/startup problems.
    pub recovery: ProjectLoadRecoveryReport,
    /// Number of nodes in the replacement project.
    pub node_count: usize,
    /// Time spent waiting for the production adapter lock.
    pub lock_wait: Duration,
    /// Time spent shutting down the previous project.
    pub shutdown: Duration,
    /// Time spent dropping the previous project.
    pub drop_previous: Duration,
    /// Time spent preparing the replacement for runtime use.
    pub prepare: Duration,
    /// Non-fatal failure reported while retiring the already-detached previous project.
    pub retirement_error: Option<String>,
    /// Total replacement time.
    pub total: Duration,
}

/// Live-resource state of the currently authored project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectRuntimeStatus {
    /// The project owns its activated runtime resources and may tick.
    Active {
        /// Authoritative project identity.
        generation: ProjectGeneration,
    },
    /// The authored project remains coherent for observation/save, but must not tick or edit.
    Paused {
        /// Authoritative project identity, unchanged by the rejected replacement.
        generation: ProjectGeneration,
        /// Actionable handoff failure.
        error: String,
    },
}

/// Fallible stages surrounding one project replacement transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectReplacementStage {
    /// Detached graph stabilization and validation.
    CandidatePreparation,
    /// App-owned script/domain preflight.
    ScriptPreparation,
    /// Detached runtime-generation compilation before resource handoff.
    CandidateCompilation,
    /// Deferred callbacks that may acquire devices or external endpoints.
    DeviceActivation,
    /// Runtime-generation compilation after activation-time graph changes.
    ActivatedCompilation,
    /// Full immutable UI projection construction before commit.
    PublicationPreparation,
    /// Cleanup and disposal of the already-detached previous engine.
    Retirement,
}

#[cfg(test)]
pub(crate) type ProjectReplacementFaultCallback =
    Arc<dyn Fn(ProjectReplacementStage) -> Result<(), String> + Send + Sync + 'static>;

#[derive(Clone, Default)]
pub(super) struct ProjectReplacementFaultHook {
    #[cfg(test)]
    callback: Arc<Mutex<Option<ProjectReplacementFaultCallback>>>,
}

impl ProjectReplacementFaultHook {
    pub(super) fn check(&self, stage: ProjectReplacementStage) -> Result<(), String> {
        #[cfg(test)]
        {
            let callback = self
                .callback
                .lock()
                .expect("project-replacement fault hook poisoned")
                .clone();
            if let Some(callback) = callback {
                callback(stage)?;
            }
        }
        #[cfg(not(test))]
        let _ = stage;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn set(&self, callback: Option<ProjectReplacementFaultCallback>) {
        *self.callback.lock().expect("project-replacement fault hook poisoned") = callback;
    }
}

struct CommittedProjectReplacement<T: ProjectLifecycle> {
    previous: Engine<T>,
    retired_read_model: RetiredUiReadModelState,
    recovery: ProjectLoadRecoveryReport,
    node_count: usize,
    shutdown: Duration,
    prepare: Duration,
}

enum ProjectReplacementActorOutcome<T: ProjectLifecycle> {
    Committed(Box<CommittedProjectReplacement<T>>),
    Rejected { candidate: Box<Engine<T>>, error: String },
}

fn reject_project_candidate<T: ProjectLifecycle>(
    mut candidate: Engine<T>,
    error: String,
) -> ProjectReplacementActorOutcome<T> {
    shutdown_engine_for_runtime(&mut candidate);
    ProjectReplacementActorOutcome::Rejected {
        candidate: Box::new(candidate),
        error,
    }
}

impl<T: ProjectLifecycle> ProductionRuntime<T> {
    /// Returns the generation of the project currently published to observers.
    pub fn current_project_generation(&self) -> ProjectGeneration {
        self.inner.read_model.current_project_generation()
    }

    /// Returns whether the authoritative project currently owns live runtime resources.
    pub fn project_runtime_status(&self) -> ProjectRuntimeStatus {
        self.inner
            .control
            .call(|state| match state.project_pause_error() {
                Some(error) => ProjectRuntimeStatus::Paused {
                    generation: state.project_generation(),
                    error: error.to_string(),
                },
                None => ProjectRuntimeStatus::Active {
                    generation: state.project_generation(),
                },
            })
            .expect("production control actor disconnected")
            .output
    }

    /// Replaces the live project after the caller decodes and configures it.
    pub fn replace_project(&self, mut request: ProjectReplacement<T>) -> Result<ProjectReplacementResult, String> {
        let started = Instant::now();
        let retirement_permit = self
            .inner
            .project_retirements
            .try_reserve()
            .map_err(|error| format!("project replacement rejected: {error}; retry after cleanup completes"))?;
        let expected_generation = self.current_project_generation();
        let generation = ProjectGeneration::new(self.inner.next_project_generation.fetch_add(1, Ordering::Relaxed));
        self.inner
            .latest_requested_project_generation
            .fetch_max(generation.get(), Ordering::AcqRel);

        let prepare_started = Instant::now();
        let detached_result = (|| -> Result<ProjectLoadRecoveryReport, String> {
            self.inner
                .project_replacement_fault_hook
                .check(ProjectReplacementStage::CandidatePreparation)?;
            let mut recovery = if request.recover {
                prepare_engine_candidate_for_runtime_recovering(&mut request.engine)
            } else {
                prepare_engine_candidate_for_runtime(&mut request.engine).map_err(|error| error.to_string())?;
                ProjectLoadRecoveryReport::default()
            };
            self.inner
                .project_replacement_fault_hook
                .check(ProjectReplacementStage::ScriptPreparation)?;
            validate_engine_project_candidate(&request.engine)?;
            T::prepare_project_candidate(&mut request.engine)?;
            if request.recover {
                let additional = prepare_engine_candidate_for_runtime_recovering(&mut request.engine);
                recovery.problems.extend(additional.problems);
            } else {
                prepare_engine_candidate_for_runtime(&mut request.engine).map_err(|error| error.to_string())?;
            }
            validate_engine_project_candidate(&request.engine)?;
            Ok(recovery)
        })();
        let recovery = match detached_result {
            Ok(recovery) => recovery,
            Err(error) => {
                shutdown_engine_for_runtime(&mut request.engine);
                return Err(error);
            }
        };
        let detached_prepare = prepare_started.elapsed();
        let replacement_fence = match self
            .inner
            .persistence_coordinator
            .begin_generation_replacement(expected_generation.get())
        {
            Ok(fence) => fence,
            Err(error) => {
                shutdown_engine_for_runtime(&mut request.engine);
                return Err(error.to_string());
            }
        };

        let read_model = self.inner.read_model.clone();
        let publication_hook = self.inner.read_model_publication_hook.clone();
        let replacement_fault_hook = self.inner.project_replacement_fault_hook.clone();
        let latest_requested_project_generation = self.inner.latest_requested_project_generation.clone();
        let receipt = self
            .inner
            .control
            .call(move |state| {
                let candidate_prepare_started = Instant::now();
                let mut candidate = request.engine;
                let node_count = candidate.nodes.iter().count();
                let stale = || latest_requested_project_generation.load(Ordering::Acquire) != generation.get();
                if stale() {
                    return reject_project_candidate(
                        candidate,
                        format!("project replacement generation {} was superseded", generation.get()),
                    );
                }
                if let Err(error) = replacement_fault_hook.check(ProjectReplacementStage::CandidateCompilation) {
                    return reject_project_candidate(candidate, error);
                }
                if let Err(error) = state.compile_project_candidate(&candidate) {
                    return reject_project_candidate(candidate, error);
                }
                if stale() {
                    return reject_project_candidate(
                        candidate,
                        format!("project replacement generation {} was superseded", generation.get()),
                    );
                }

                let shutdown_started = Instant::now();
                shutdown_engine_for_runtime(&mut state.engine);
                let shutdown = shutdown_started.elapsed();
                state.pause_project(format!(
                    "project replacement generation {} is awaiting resource activation",
                    generation.get()
                ));

                let activation = replacement_fault_hook
                    .check(ProjectReplacementStage::DeviceActivation)
                    .and_then(|()| activate_engine_for_runtime(&mut candidate).map_err(|error| error.to_string()));
                if let Err(error) = activation {
                    state.pause_project(format!("replacement device activation failed: {error}"));
                    return reject_project_candidate(candidate, error);
                }
                if let Err(error) = replacement_fault_hook.check(ProjectReplacementStage::ActivatedCompilation) {
                    state.pause_project(format!("replacement post-activation compilation failed: {error}"));
                    return reject_project_candidate(candidate, error);
                }
                let compiled = match state.compile_project_candidate(&candidate) {
                    Ok(compiled) => compiled,
                    Err(error) => {
                        state.pause_project(format!("replacement post-activation compilation failed: {error}"));
                        return reject_project_candidate(candidate, error);
                    }
                };

                candidate.clear_ui_event_log();
                candidate.push_ui_custom_event(
                    "__transport.resync_required",
                    None,
                    serde_json::json!({ "reason": request.reason }),
                );
                if let Err(error) = replacement_fault_hook.check(ProjectReplacementStage::PublicationPreparation) {
                    state.pause_project(format!("replacement publication preparation failed: {error}"));
                    return reject_project_candidate(candidate, error);
                }
                let project_was_saved = request.project_file.current_path.is_some();
                let prepared_read_model =
                    UiReadModel::prepare_project_replacement(&candidate, request.project_file, generation);
                if stale() {
                    let error = format!("project replacement generation {} was superseded", generation.get());
                    state.pause_project(error.clone());
                    return reject_project_candidate(candidate, error);
                }

                let (previous, retired_read_model) =
                    match state.commit_project(candidate, compiled, generation, project_was_saved, move |engine| {
                        publication_hook.invoke();
                        let retired = read_model
                            .commit_project_replacement(prepared_read_model, UiReadModelReplaceReason::ProjectReplaced);
                        read_model.publish_engine_events_since(engine, None);
                        retired
                    }) {
                        Ok(previous) => previous,
                        Err(rejected) => {
                            let error = rejected.error;
                            let candidate = *rejected.candidate;
                            state.pause_project(format!("replacement commit preparation failed: {error}"));
                            return reject_project_candidate(candidate, error);
                        }
                    };

                ProjectReplacementActorOutcome::Committed(Box::new(CommittedProjectReplacement {
                    previous,
                    retired_read_model,
                    recovery,
                    node_count,
                    shutdown,
                    prepare: detached_prepare.saturating_add(candidate_prepare_started.elapsed()),
                }))
            })
            .map_err(|error| error.to_string())?;
        let committed = match receipt.output {
            ProjectReplacementActorOutcome::Committed(committed) => {
                replacement_fence
                    .commit(generation.get())
                    .expect("reserved project generation must advance the persistence fence");
                committed
            }
            ProjectReplacementActorOutcome::Rejected { candidate, error } => {
                drop(replacement_fence);
                drop(candidate);
                return Err(error);
            }
        };

        let retirement_error = self
            .inner
            .project_replacement_fault_hook
            .check(ProjectReplacementStage::Retirement)
            .err();
        let drop_started = Instant::now();
        drop((committed.previous, committed.retired_read_model));
        let drop_previous = drop_started.elapsed();
        drop(retirement_permit);
        Ok(ProjectReplacementResult {
            project_generation: generation,
            recovery: committed.recovery,
            node_count: committed.node_count,
            lock_wait: receipt.queue_wait,
            shutdown: committed.shutdown,
            drop_previous,
            prepare: committed.prepare,
            retirement_error,
            total: started.elapsed(),
        })
    }
}
