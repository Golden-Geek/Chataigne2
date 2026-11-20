use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use libloading::os::windows::{LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, Library};
use ndi_sdk::sys;

use crate::image::ImageStorageBudget;

use super::NdiError;

const DLL: &str = "Processing.NDI.Lib.x64.dll";

pub(super) struct Runtime {
    // The process-global SDK remains initialized while any worker can create an endpoint.
    // Each finder, receiver and frame still has independent RAII retirement.
    _library: Library,
    pub(super) api: sys::NDIlib_v5,
    pub(super) images: ImageStorageBudget,
}

impl Runtime {
    pub(super) fn get() -> Result<&'static Self, NdiError> {
        static RUNTIME: OnceLock<Runtime> = OnceLock::new();
        static INITIALIZE: Mutex<()> = Mutex::new(());
        if let Some(runtime) = RUNTIME.get() {
            return Ok(runtime);
        }
        let _initialize = INITIALIZE.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(runtime) = RUNTIME.get() {
            return Ok(runtime);
        }
        let runtime = Self::load()?;
        Ok(RUNTIME.get_or_init(|| runtime))
    }

    fn load() -> Result<Self, NdiError> {
        let mut failures = Vec::new();
        for path in runtime_paths().filter(|path| path.is_file()) {
            match Self::load_path(&path) {
                Ok(runtime) => return Ok(runtime),
                Err(error) => failures.push(format!("{}: {error}", path.display())),
            }
        }
        Err(NdiError::Runtime(if failures.is_empty() {
            format!(
                "{DLL} was not found beside the app in ndi/, in NDI_RUNTIME_DIR_V6, or in the standard installation folders. Reinstall the app or install the NDI 6 Runtime."
            )
        } else {
            format!("No compatible NDI runtime could be loaded. {}", failures.join("; "))
        }))
    }

    fn load_path(path: &Path) -> Result<Self, String> {
        // Use an absolute installed path and exclude working-directory DLL lookup.
        let library = unsafe {
            Library::load_with_flags(
                path,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        }
        .map_err(|error| error.to_string())?;
        type Load = unsafe extern "C" fn() -> *const sys::NDIlib_v5;
        // NDI documents the v5 compatibility table as its dynamic loading entry point on NDI 6.
        let load = unsafe { library.get::<Load>(b"NDIlib_v5_load\0") }.map_err(|error| error.to_string())?;
        let api = unsafe { load().as_ref() }
            .copied()
            .ok_or("the NDI function table is missing")?;
        let initialize = api.initialize.ok_or("the NDI initialize function is missing")?;
        if api.find_create_v2.is_none()
            || api.find_destroy.is_none()
            || api.find_get_current_sources.is_none()
            || api.recv_create_v3.is_none()
            || api.recv_destroy.is_none()
            || api.recv_capture_v3.is_none()
            || api.recv_free_video_v2.is_none()
            || api.recv_free_audio_v3.is_none()
            || api.recv_get_no_connections.is_none()
        {
            return Err("the NDI discovery and receive APIs are incomplete".into());
        }
        if api.send_create.is_none()
            || api.send_get_source_name.is_none()
            || api.send_destroy.is_none()
            || api.send_send_video_v2.is_none()
            || api.send_send_audio_v3.is_none()
        {
            return Err("the NDI send API is incomplete".into());
        }
        let images = ImageStorageBudget::new(512 * 1024 * 1024).map_err(|error| error.to_string())?;
        // Serialized startup owns the SDK until process exit. Failures are left retryable.
        if !unsafe { initialize() } {
            return Err("the SDK rejected initialization".into());
        }
        Ok(Self {
            _library: library,
            api,
            images,
        })
    }
}

fn runtime_paths() -> impl Iterator<Item = PathBuf> {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|directory| directory.join("ndi")));
    let configured = std::env::var_os("NDI_RUNTIME_DIR_V6").map(PathBuf::from);
    let installed = std::env::var_os("ProgramFiles").map(PathBuf::from);
    [
        bundled,
        configured,
        installed.as_ref().map(|path| path.join("NDI/NDI 6 Runtime")),
        installed.map(|path| path.join("NDI/NDI 6 Tools/Runtime")),
    ]
    .into_iter()
    .flatten()
    .filter(|path| path.is_absolute())
    .map(|path| path.join(DLL))
}
