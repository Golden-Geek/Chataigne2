use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

static SHARED_FORMULA_DIR_ENV_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static SHARED_FORMULA_DIR_OVERRIDE: RefCell<Option<Option<PathBuf>>> =
        const { RefCell::new(None) };
}

pub(crate) struct ScopedSharedFormulaDir {
    previous: Option<Option<PathBuf>>,
    _lock: MutexGuard<'static, ()>,
    _temporary_directory: Option<tempfile::TempDir>,
}

impl Drop for ScopedSharedFormulaDir {
    fn drop(&mut self) {
        SHARED_FORMULA_DIR_OVERRIDE.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

pub(crate) fn scoped_shared_formula_dir(path: Option<&Path>) -> ScopedSharedFormulaDir {
    let lock = SHARED_FORMULA_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = SHARED_FORMULA_DIR_OVERRIDE.with(|slot| slot.replace(Some(path.map(Path::to_path_buf))));
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
    let previous =
        SHARED_FORMULA_DIR_OVERRIDE.with(|slot| slot.replace(Some(Some(temporary_directory.path().to_path_buf()))));
    ScopedSharedFormulaDir {
        previous,
        _lock: lock,
        _temporary_directory: Some(temporary_directory),
    }
}

pub(crate) fn shared_formula_dir_override() -> Option<Option<PathBuf>> {
    SHARED_FORMULA_DIR_OVERRIDE.with(|slot| slot.borrow().clone())
}
