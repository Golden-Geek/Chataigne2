use std::path::PathBuf;
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
        let path = runtime_path().ok_or_else(|| {
            NdiError::Runtime(format!(
                "{DLL} was not found in NDI_RUNTIME_DIR_V6 or the standard installation folders"
            ))
        })?;
        // Use an absolute installed path and exclude working-directory DLL lookup.
        let library = unsafe {
            Library::load_with_flags(
                &path,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        }
        .map_err(|error| NdiError::Runtime(format!("{}: {error}", path.display())))?;
        type Load = unsafe extern "C" fn() -> *const sys::NDIlib_v5;
        // NDI documents the v5 compatibility table as its dynamic loading entry point on NDI 6.
        let load = unsafe { library.get::<Load>(b"NDIlib_v5_load\0") }
            .map_err(|error| NdiError::Runtime(format!("{}: {error}", path.display())))?;
        let api = unsafe { load().as_ref() }
            .copied()
            .ok_or_else(|| NdiError::Runtime("the NDI function table is missing".into()))?;
        let initialize = api
            .initialize
            .ok_or_else(|| NdiError::Runtime("the NDI initialize function is missing".into()))?;
        if api.find_create_v2.is_none()
            || api.find_destroy.is_none()
            || api.find_get_current_sources.is_none()
            || api.recv_create_v3.is_none()
            || api.recv_destroy.is_none()
            || api.recv_capture_v3.is_none()
            || api.recv_free_video_v2.is_none()
            || api.recv_get_no_connections.is_none()
        {
            return Err(NdiError::Runtime(
                "the NDI discovery and receive APIs are incomplete".into(),
            ));
        }
        let images = ImageStorageBudget::new(512 * 1024 * 1024)?;
        // Serialized startup owns the SDK until process exit. Failures are left retryable.
        if !unsafe { initialize() } {
            return Err(NdiError::Runtime("the SDK rejected initialization".into()));
        }
        Ok(Self {
            _library: library,
            api,
            images,
        })
    }
}

fn runtime_path() -> Option<PathBuf> {
    let configured = std::env::var_os("NDI_RUNTIME_DIR_V6").map(PathBuf::from);
    let installed = std::env::var_os("ProgramFiles").map(PathBuf::from);
    configured
        .into_iter()
        .chain(installed.iter().map(|path| path.join("NDI/NDI 6 Runtime")))
        .chain(installed.iter().map(|path| path.join("NDI/NDI 6 Tools/Runtime")))
        .filter(|path| path.is_absolute())
        .map(|path| path.join(DLL))
        .find(|path| path.is_file())
}
