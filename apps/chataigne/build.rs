use std::ffi::OsStr;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

use golden_codegen_support::ui_bundle::{self, UiBundleConfig};
use golden_codegen_support::{generate_app_nodes_from_roots, AppNodeSourceRoot};

const GC_KINECT20_DLL: &str = "GC_KINECT20_DLL";
const LEAPSDK_LIB_PATH: &str = "LEAPSDK_LIB_PATH";

struct BuildPaths {
    out_dir: PathBuf,
    app_nodes_rs: PathBuf,
    builtin_formulas_dir: PathBuf,
    builtin_formulas_rs: PathBuf,
}

impl BuildPaths {
    fn from_env() -> Self {
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is not set"));
        Self {
            app_nodes_rs: out_dir.join("app_nodes.rs"),
            builtin_formulas_dir: PathBuf::from("resources/formulas/builtin"),
            builtin_formulas_rs: out_dir.join("builtin_formulas.rs"),
            out_dir,
        }
    }

    #[cfg(windows)]
    fn cargo_profile_dir(&self) -> Option<&Path> {
        self.out_dir.ancestors().nth(3)
    }
}

fn main() -> std::io::Result<()> {
    let paths = BuildPaths::from_env();
    let workspace_root = Path::new("../..");
    let shared_input_roots = ["golden-ui", "golden-graph-ui", "golden-audio-ui"]
        .map(|package| workspace_root.join("packages").join(package));

    emit_rerun_tracking(&paths)?;
    ui_bundle::prepare(&UiBundleConfig {
        workspace_root,
        ui_root: Path::new("ui"),
        npm_workspace: "chataigne-ui",
        out_dir: &paths.out_dir,
        shared_input_roots: &shared_input_roots,
        dependency_cache_namespace: "chataigne2",
    })?;
    prepare_native_sidecars(&paths)?;
    run_tauri_build();
    generate_rust_code(&paths)?;

    Ok(())
}

fn emit_rerun_tracking(paths: &BuildPaths) -> std::io::Result<()> {
    println!("cargo:rerun-if-env-changed={GC_KINECT20_DLL}");
    println!("cargo:rerun-if-env-changed={LEAPSDK_LIB_PATH}");
    emit_rerun_if_changed_for_dir(&paths.builtin_formulas_dir)
}

fn prepare_native_sidecars(paths: &BuildPaths) -> std::io::Result<()> {
    configure_ultraleap_sdk(paths)?;

    #[cfg(windows)]
    {
        sidecar_kinect_runtime(paths)?;
    }

    #[cfg(not(windows))]
    {
        let _ = paths;
    }

    Ok(())
}

fn configure_ultraleap_sdk(paths: &BuildPaths) -> std::io::Result<()> {
    let Some(link_dir) = find_ultraleap_link_dir() else {
        println!(
            "cargo:warning=LeapC runtime library directory was not found; Ultraleap support still compiles, but running the module will require the Ultraleap Tracking Software installed or {LEAPSDK_LIB_PATH} set."
        );
        return Ok(());
    };

    #[cfg(windows)]
    sidecar_ultraleap_runtime(paths, &link_dir)?;

    #[cfg(not(windows))]
    let _ = (paths, link_dir);

    Ok(())
}

fn find_ultraleap_link_dir() -> Option<PathBuf> {
    ultraleap_sdk_candidates()
        .into_iter()
        .find(|candidate| ultraleap_link_artifact(candidate).is_file())
}

fn ultraleap_sdk_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(value) = std::env::var_os(LEAPSDK_LIB_PATH) {
        candidates.push(PathBuf::from(value));
    }

    #[cfg(windows)]
    {
        candidates.push(PathBuf::from(r"C:\Program Files\Ultraleap\LeapSDK\lib\x64"));
    }

    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from(
            r"/Applications/Ultraleap Hand Tracking Service.app/Contents/LeapSDK/lib",
        ));
        candidates.push(PathBuf::from(
            r"/Applications/Ultraleap Hand Tracking.app/Contents/LeapSDK/lib",
        ));
    }

    #[cfg(target_os = "linux")]
    {
        candidates.push(PathBuf::from("/usr/lib/ultraleap-hand-tracking-service"));
        candidates.push(PathBuf::from("/usr/share/doc/ultraleap-hand-tracking-service"));
    }

    candidates
}

#[cfg(windows)]
fn ultraleap_link_artifact(candidate: &Path) -> PathBuf {
    candidate.join("LeapC.lib")
}

#[cfg(target_os = "macos")]
fn ultraleap_link_artifact(candidate: &Path) -> PathBuf {
    candidate.join("libLeapC.dylib")
}

#[cfg(target_os = "linux")]
fn ultraleap_link_artifact(candidate: &Path) -> PathBuf {
    candidate.join("libLeapC.so")
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn ultraleap_link_artifact(candidate: &Path) -> PathBuf {
    candidate.join("LeapC")
}

#[cfg(windows)]
fn sidecar_ultraleap_runtime(paths: &BuildPaths, link_dir: &Path) -> std::io::Result<()> {
    let source = link_dir.join("LeapC.dll");
    if !source.is_file() {
        println!(
            "cargo:warning=LeapC.dll was not found in {}; Ultraleap support will need the runtime DLL available on PATH or next to the executable.",
            link_dir.display()
        );
        return Ok(());
    }

    println!("cargo:rerun-if-changed={}", source.display());

    let Some(profile_dir) = paths.cargo_profile_dir() else {
        return Err(Error::other(format!(
            "failed to resolve Cargo profile output directory from OUT_DIR {}",
            paths.out_dir.display()
        )));
    };

    for destination_dir in [profile_dir.to_path_buf(), profile_dir.join("deps")] {
        if !destination_dir.exists() {
            fs::create_dir_all(&destination_dir)?;
        }
        copy_sidecar_if_needed(&source, &destination_dir.join("LeapC.dll"))?;
    }

    Ok(())
}

#[cfg(windows)]
fn sidecar_kinect_runtime(paths: &BuildPaths) -> std::io::Result<()> {
    let Some(source) = find_kinect20_dll() else {
        println!(
            "cargo:warning=Kinect20.dll was not found on this Windows build machine; Kinect 2 support will compile, but deployment must sidecar the runtime DLL manually."
        );
        return Ok(());
    };

    println!("cargo:rerun-if-changed={}", source.display());

    let Some(profile_dir) = paths.cargo_profile_dir() else {
        return Err(Error::other(format!(
            "failed to resolve Cargo profile output directory from OUT_DIR {}",
            paths.out_dir.display()
        )));
    };

    for destination_dir in [profile_dir.to_path_buf(), profile_dir.join("deps")] {
        if !destination_dir.exists() {
            fs::create_dir_all(&destination_dir)?;
        }
        copy_sidecar_if_needed(&source, &destination_dir.join("Kinect20.dll"))?;
    }

    Ok(())
}

#[cfg(windows)]
fn find_kinect20_dll() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(value) = std::env::var_os(GC_KINECT20_DLL) {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
        candidates.push(path);
    }

    if let Some(system_root) = std::env::var_os("SystemRoot") {
        candidates.push(PathBuf::from(system_root).join("System32").join("Kinect20.dll"));
    }

    if let Some(sdk_dir) = std::env::var_os("KINECTSDK20_DIR") {
        candidates.push(PathBuf::from(&sdk_dir).join("Redist").join("Kinect20.dll"));
        candidates.push(PathBuf::from(&sdk_dir).join("Kinect20.dll"));
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}

#[cfg(windows)]
fn copy_sidecar_if_needed(source: &Path, destination: &Path) -> std::io::Result<()> {
    let should_copy = match fs::metadata(destination) {
        Ok(existing) => {
            let source_meta = fs::metadata(source)?;
            existing.len() != source_meta.len() || existing.modified().ok() != source_meta.modified().ok()
        }
        Err(_) => true,
    };

    if should_copy {
        fs::copy(source, destination)?;
    }

    Ok(())
}

fn run_tauri_build() {
    tauri_build::build();
}

fn generate_rust_code(paths: &BuildPaths) -> std::io::Result<()> {
    let source_roots = [
        AppNodeSourceRoot::new(Path::new("src"), ""),
        AppNodeSourceRoot::new(Path::new("systems/alchemist/integration"), "systems_alchemist"),
        AppNodeSourceRoot::new(Path::new("systems/state_machine/integration"), "systems_state_machine"),
    ];
    generate_app_nodes_from_roots(&source_roots, &paths.app_nodes_rs);
    generate_builtin_formulas_module(&paths.builtin_formulas_dir, &paths.builtin_formulas_rs)
}

fn generate_builtin_formulas_module(formulas_dir: &Path, out_file: &Path) -> std::io::Result<()> {
    let mut formula_paths = fs::read_dir(formulas_dir)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().and_then(OsStr::to_str) == Some("json"))
        .collect::<Vec<_>>();
    formula_paths.sort();

    let mut generated = String::from(
        "// @generated by apps/chataigne/build.rs. Do not edit.\n\
         const BUNDLED_BUILTIN_FORMULAS: &[BundledBuiltinFormula] = &[\n",
    );
    for formula_path in formula_paths {
        let file_name = formula_path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "non-UTF-8 formula filename"))?
            .to_owned();
        let formula_path = fs::canonicalize(&formula_path)?;
        let formula_source_path = formula_path
            .to_str()
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "non-UTF-8 built-in formula path"))?;
        let icon = [("svg", "image/svg+xml"), ("png", "image/png")]
            .into_iter()
            .find_map(|(extension, mime)| {
                let path = formula_path.with_extension(extension);
                path.is_file().then_some((path, mime))
            });
        let icon_source = match icon {
            Some((path, mime)) => {
                let path = fs::canonicalize(path)?;
                let path = path
                    .to_str()
                    .ok_or_else(|| Error::new(ErrorKind::InvalidData, "non-UTF-8 built-in icon path"))?;
                format!("Some(({mime:?}, include_bytes!({path:?}) as &[u8]))")
            }
            None => "None".to_owned(),
        };
        generated.push_str(&format!(
            "    BundledBuiltinFormula {{ file_name: {file_name:?}, source: include_str!({formula_source_path:?}), icon: {icon_source} }},\n"
        ));
    }
    generated.push_str("];\n");
    fs::write(out_file, generated)
}

fn emit_rerun_if_changed_for_dir(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    println!("cargo:rerun-if-changed={}", dir.display());

    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| {
                ["node_modules", "build", "dist", ".svelte-kit", "docs", "tests"]
                    .contains(&name.to_string_lossy().as_ref())
            }) {
                continue;
            }
            emit_rerun_if_changed_for_dir(&path)?;
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    Ok(())
}
