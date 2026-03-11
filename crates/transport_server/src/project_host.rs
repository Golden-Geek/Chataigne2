use std::sync::{Arc, Mutex};

use golden_engine::app::{
    ProjectLifecycle, configure_loaded_engine, create_new_project_engine, prepare_engine_for_runtime,
};
use golden_engine::engine::Engine;

fn normalize_project_path(raw_path: &str) -> Option<String> {
    let path = raw_path.trim();
    if path.is_empty() {
        return None;
    }
    Some(path.to_string())
}

fn replace_live_engine<T: ProjectLifecycle>(
    engine: &Arc<Mutex<Engine<T>>>,
    mut next_engine: Engine<T>,
    reason: &str,
) -> Result<(), String> {
    prepare_engine_for_runtime(&mut next_engine).map_err(|err| err.to_string())?;
    next_engine.clear_ui_event_log();
    next_engine.push_ui_custom_event(
        "__transport.resync_required",
        None,
        serde_json::json!({ "reason": reason }),
    );

    let mut guard = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = next_engine;
    Ok(())
}

pub(crate) fn create_new_project<T: ProjectLifecycle>(engine: &Arc<Mutex<Engine<T>>>) -> Result<(), String> {
    let next_engine = create_new_project_engine::<T>()?;
    replace_live_engine(engine, next_engine, "project_new")
}

pub(crate) fn save_project<T: ProjectLifecycle>(engine: &Arc<Mutex<Engine<T>>>, raw_path: &str) -> Result<(), String> {
    let path = normalize_project_path(raw_path).ok_or_else(|| "project-save path cannot be empty".to_string())?;

    let guard = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard
        .save_project_file_with(path.as_str(), |node| node.project_encode_data())
        .map_err(|err| err.to_string())
}

pub(crate) fn load_project<T: ProjectLifecycle>(engine: &Arc<Mutex<Engine<T>>>, raw_path: &str) -> Result<(), String> {
    let path = normalize_project_path(raw_path).ok_or_else(|| "project-load path cannot be empty".to_string())?;

    let mut next_engine =
        Engine::<T>::load_project_file_with(path.as_str(), T::project_decode_node).map_err(|err| err.to_string())?;
    configure_loaded_engine(&mut next_engine)?;
    replace_live_engine(engine, next_engine, "project_loaded")
}
