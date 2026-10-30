//! Startup uses the same detached-validation boundary as project replacement.

use std::io::{Error, Result};

use golden_engine::app::{
    ProjectLifecycle, activate_engine_for_runtime, prepare_engine_candidate_for_runtime,
    validate_engine_project_candidate,
};
use golden_engine::engine::Engine;

pub(super) fn prepare_engine_for_host_startup<T: ProjectLifecycle>(engine: &mut Engine<T>) -> Result<()> {
    T::project_opened(engine).map_err(Error::other)?;
    prepare_engine_candidate_for_runtime(engine)?;
    validate_engine_project_candidate(engine).map_err(Error::other)?;
    T::prepare_project_candidate(engine).map_err(Error::other)?;
    // The app hook can enqueue authored edits; settle them before activating resources.
    prepare_engine_candidate_for_runtime(engine)?;
    validate_engine_project_candidate(engine).map_err(Error::other)?;
    activate_engine_for_runtime(engine)
}
