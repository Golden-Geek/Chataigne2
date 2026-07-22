use std::{
    ffi::OsString,
    path::Path,
    sync::{Mutex, MutexGuard},
};

const SHARED_FORMULA_DIR_ENV: &str = "CHATAIGNE_SHARED_FORMULAS_DIR";
static SHARED_FORMULA_DIR_ENV_LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct ScopedSharedFormulaDir {
    previous: Option<OsString>,
    _lock: MutexGuard<'static, ()>,
    _temporary_directory: Option<tempfile::TempDir>,
}

impl Drop for ScopedSharedFormulaDir {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { std::env::set_var(SHARED_FORMULA_DIR_ENV, value) },
            None => unsafe { std::env::remove_var(SHARED_FORMULA_DIR_ENV) },
        }
    }
}

pub(crate) fn scoped_shared_formula_dir(path: Option<&Path>) -> ScopedSharedFormulaDir {
    let lock = SHARED_FORMULA_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = std::env::var_os(SHARED_FORMULA_DIR_ENV);
    match path {
        Some(path) => unsafe { std::env::set_var(SHARED_FORMULA_DIR_ENV, path) },
        None => unsafe { std::env::remove_var(SHARED_FORMULA_DIR_ENV) },
    }
    ScopedSharedFormulaDir {
        previous,
        _lock: lock,
        _temporary_directory: None,
    }
}

pub(crate) fn scoped_empty_shared_formula_dir() -> ScopedSharedFormulaDir {
    let temporary_directory = tempfile::tempdir().expect("temporary shared-formula directory should be creatable");
    let lock = SHARED_FORMULA_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = std::env::var_os(SHARED_FORMULA_DIR_ENV);
    unsafe { std::env::set_var(SHARED_FORMULA_DIR_ENV, temporary_directory.path()) };
    ScopedSharedFormulaDir {
        previous,
        _lock: lock,
        _temporary_directory: Some(temporary_directory),
    }
}
