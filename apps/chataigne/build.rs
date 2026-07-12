use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use golden_codegen_support::generate_app_nodes;

const GC_FORCE_NPM_CI: &str = "GC_FORCE_NPM_CI";
const GC_KINECT20_DLL: &str = "GC_KINECT20_DLL";
const GC_SKIP_UI_BUILD: &str = "GC_SKIP_UI_BUILD";
const GC_UI_ASSUME_BUILT: &str = "GC_UI_ASSUME_BUILT";
const LEAPSDK_LIB_PATH: &str = "LEAPSDK_LIB_PATH";
const REQUIRED_NODE_RANGE: &str = "Node.js 20.19+ or 22.12+";

struct BuildPaths {
    #[cfg(windows)]
    out_dir: PathBuf,
    bundled_ui_dir: PathBuf,
    ui_assets_rs: PathBuf,
    app_nodes_rs: PathBuf,
    ui_root: PathBuf,
    workspace_root: PathBuf,
    node_modules: PathBuf,
    package_lock: PathBuf,
    npm_deps_stamp: PathBuf,
}

impl BuildPaths {
    fn from_env() -> Self {
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is not set"));
        let ui_root = PathBuf::from("ui");
        let workspace_root = PathBuf::from("../..");
        let node_modules = workspace_root.join("node_modules");
        let package_lock = workspace_root.join("package-lock.json");
        let npm_deps_stamp = node_modules.join(".cache").join("chataigne2").join("package-lock.hash");
        Self {
            #[cfg(windows)]
            out_dir: out_dir.clone(),
            bundled_ui_dir: out_dir.join("ui-dist"),
            ui_assets_rs: out_dir.join("ui_assets.rs"),
            app_nodes_rs: out_dir.join("app_nodes.rs"),
            ui_root,
            workspace_root,
            node_modules,
            package_lock,
            npm_deps_stamp,
        }
    }

    #[cfg(windows)]
    fn cargo_profile_dir(&self) -> Option<&Path> {
        self.out_dir.ancestors().nth(3)
    }
}

fn main() -> std::io::Result<()> {
    let paths = BuildPaths::from_env();

    emit_rerun_tracking(&paths)?;
    prepare_ui_assets(&paths)?;
    prepare_native_sidecars(&paths)?;
    run_tauri_build();
    generate_rust_code(&paths);

    Ok(())
}

fn emit_rerun_tracking(paths: &BuildPaths) -> std::io::Result<()> {
    println!("cargo:rerun-if-env-changed={GC_FORCE_NPM_CI}");
    println!("cargo:rerun-if-env-changed={GC_KINECT20_DLL}");
    println!("cargo:rerun-if-env-changed={GC_SKIP_UI_BUILD}");
    println!("cargo:rerun-if-env-changed={GC_UI_ASSUME_BUILT}");
    println!("cargo:rerun-if-env-changed={LEAPSDK_LIB_PATH}");

    track_ui_inputs(&paths.ui_root, &paths.package_lock)?;
    Ok(())
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

fn prepare_ui_assets(paths: &BuildPaths) -> std::io::Result<()> {
    if env_flag(GC_SKIP_UI_BUILD) {
        println!("cargo:warning={GC_SKIP_UI_BUILD}=1; compiling without bundled UI assets");
        return generate_empty_ui_assets_module(&paths.ui_assets_rs);
    }

    let asset_root = if env_flag(GC_UI_ASSUME_BUILT) {
        let assumed_dist = paths.ui_root.join("build");
        println!(
            "cargo:warning={GC_UI_ASSUME_BUILT}=1; using prebuilt UI at {}",
            assumed_dist.display()
        );
        assumed_dist
    } else {
        ensure_ui_dependencies(paths)?;
        build_ui_bundle(paths)?;
        paths.bundled_ui_dir.clone()
    };

    generate_ui_assets_module(&asset_root, &paths.ui_assets_rs)
}

fn ensure_ui_dependencies(paths: &BuildPaths) -> std::io::Result<()> {
    ensure_frontend_toolchain()?;

    let current_hash = file_hash(&paths.package_lock)?;
    let previous_hash = fs::read_to_string(&paths.npm_deps_stamp)
        .ok()
        .map(|value| value.trim().to_string());

    let force_ci = env_flag(GC_FORCE_NPM_CI);
    let has_node_modules = paths.node_modules.exists();
    let node_modules_ready = ui_node_modules_ready(paths);

    if force_ci {
        println!("cargo:warning={GC_FORCE_NPM_CI}=1; running npm ci");
        run_npm_command(&paths.workspace_root, &["ci", "--ignore-scripts"], &[])?;
        write_npm_deps_stamp(paths, &current_hash)?;
        return Ok(());
    }

    if !has_node_modules {
        println!("cargo:warning=ui/node_modules is missing; running npm ci");
        run_npm_command(&paths.workspace_root, &["ci", "--ignore-scripts"], &[])?;
        write_npm_deps_stamp(paths, &current_hash)?;
        return Ok(());
    }

    if !node_modules_ready {
        println!("cargo:warning=ui/node_modules is incomplete; running npm install");
        run_npm_command(&paths.workspace_root, &["install", "--ignore-scripts"], &[])?;
        write_npm_deps_stamp(paths, &current_hash)?;
        return Ok(());
    }

    match previous_hash.as_deref() {
        Some(previous) if previous == current_hash => {}
        None => {
            println!("cargo:warning=ui dependency stamp is missing; trusting existing node_modules");
            write_npm_deps_stamp(paths, &current_hash)?;
        }
        Some(_) => {
            println!("cargo:warning=package-lock.json changed; running npm install");
            run_npm_command(&paths.workspace_root, &["install", "--ignore-scripts"], &[])?;
            write_npm_deps_stamp(paths, &current_hash)?;
        }
    }

    Ok(())
}

fn ui_node_modules_ready(paths: &BuildPaths) -> bool {
    paths.node_modules.join(".bin").join(vite_bin_name()).exists()
}

fn vite_bin_name() -> &'static str {
    if cfg!(windows) {
        "vite.cmd"
    } else {
        "vite"
    }
}

fn write_npm_deps_stamp(paths: &BuildPaths, hash: &str) -> std::io::Result<()> {
    if let Some(parent) = paths.npm_deps_stamp.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&paths.npm_deps_stamp, hash)
}

fn build_ui_bundle(paths: &BuildPaths) -> std::io::Result<()> {
    if paths.bundled_ui_dir.exists() {
        fs::remove_dir_all(&paths.bundled_ui_dir)?;
    }
    fs::create_dir_all(&paths.bundled_ui_dir)?;

    run_npm_command(
        &paths.workspace_root,
        &["run", "build", "--workspace", "chataigne-ui"],
        &[("GC_UI_OUT_DIR", paths.bundled_ui_dir.as_os_str())],
    )
}

fn run_tauri_build() {
    tauri_build::build();
}

fn generate_rust_code(paths: &BuildPaths) {
    let src_root = Path::new("src");
    generate_app_nodes(src_root, &paths.app_nodes_rs);
}

fn track_ui_inputs(ui_root: &Path, package_lock: &Path) -> std::io::Result<()> {
    println!("cargo:rerun-if-changed={}", package_lock.display());
    for relative in ["package.json", "svelte.config.js", "vite.config.ts", "src/app.html"] {
        println!("cargo:rerun-if-changed={}", ui_root.join(relative).display());
    }

    emit_rerun_if_changed_for_dir(&ui_root.join("src"))?;
    emit_rerun_if_changed_for_dir(&ui_root.join("static"))?;

    Ok(())
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
            emit_rerun_if_changed_for_dir(&path)?;
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    Ok(())
}

fn run_npm_command(ui_root: &Path, args: &[&str], envs: &[(&str, &OsStr)]) -> std::io::Result<()> {
    let npm = npm_command();
    let mut command = Command::new(npm);
    command.args(args).current_dir(ui_root);
    for (key, value) in envs {
        command.env(key, value);
    }
    let status = command.status().map_err(|err| {
        Error::new(
            err.kind(),
            format!(
                "failed to start {npm} {}: {err}\n{}",
                args.join(" "),
                frontend_toolchain_help()
            ),
        )
    })?;

    if status.success() {
        return Ok(());
    }

    Err(Error::other(format!(
        "{npm} {} exited with status {status}\n{}",
        args.join(" "),
        frontend_toolchain_help()
    )))
}

fn ensure_frontend_toolchain() -> std::io::Result<()> {
    ensure_command_available("node", &["--version"])?;
    ensure_command_available(npm_command(), &["--version"])
}

fn ensure_command_available(command: &str, args: &[&str]) -> std::io::Result<()> {
    let status = Command::new(command)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            Error::new(
                err.kind(),
                format!(
                    "required frontend command `{command}` was not found or could not start: {err}\n{}",
                    frontend_toolchain_help()
                ),
            )
        })?;

    if status.success() {
        return Ok(());
    }

    Err(Error::other(format!(
        "required frontend command `{command}` exited with status {status}\n{}",
        frontend_toolchain_help()
    )))
}

fn npm_command() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

fn frontend_toolchain_help() -> String {
    let bootstrap_command = if cfg!(windows) {
        r".\tools\dev.ps1"
    } else {
        "bash ./tools/dev.sh"
    };

    format!(
        "Chataigne2 builds and embeds the Svelte frontend during `cargo run`.\n\
         Install {REQUIRED_NODE_RANGE} with npm, or run `{bootstrap_command}` from the repository root to install prerequisites and launch the app.\n\
         For engine-only checks, set {GC_SKIP_UI_BUILD}=1."
    )
}

fn generate_ui_assets_module(dist_root: &Path, out_file: &Path) -> std::io::Result<()> {
    if !dist_root.exists() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("bundled UI build output {} does not exist", dist_root.display()),
        ));
    }

    let mut asset_files = Vec::<PathBuf>::new();
    collect_files(dist_root, &mut asset_files)?;
    asset_files.sort();

    let has_index_html = asset_files
        .iter()
        .any(|path| path.file_name() == Some(OsStr::new("index.html")));
    if !has_index_html {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("bundled UI output {} is missing index.html", dist_root.display()),
        ));
    }

    let mut generated = String::from("// @generated by build.rs. Do not edit.\n");
    generated.push_str("pub static APP_UI_ASSETS: &[golden_core::app::UiAsset] = &[\n");

    for asset_path in asset_files {
        let relative_path = asset_path.strip_prefix(dist_root).map_err(|err| {
            Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "failed to compute bundled UI asset path relative to {} for {}: {err}",
                    dist_root.display(),
                    asset_path.display()
                ),
            )
        })?;
        let http_path = format!("/{}", relative_path.to_string_lossy().replace('\\', "/"));
        let content_type = content_type_for_path(relative_path);
        let absolute_path = asset_path.canonicalize()?;

        generated.push_str("    golden_core::app::UiAsset {\n");
        generated.push_str(&format!("        path: {:?},\n", http_path));
        generated.push_str(&format!("        content_type: {:?},\n", content_type));
        generated.push_str(&format!(
            "        bytes: include_bytes!(r#\"{}\"#),\n",
            absolute_path.display()
        ));
        generated.push_str("    },\n");
    }

    generated.push_str("];\n");
    fs::write(out_file, generated)
}

fn generate_empty_ui_assets_module(out_file: &Path) -> std::io::Result<()> {
    fs::write(
        out_file,
        "// @generated by build.rs. Do not edit.\npub static APP_UI_ASSETS: &[golden_core::app::UiAsset] = &[];\n",
    )
}

fn collect_files(dir: &Path, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, output)?;
        } else {
            output.push(path);
        }
    }

    Ok(())
}

fn file_hash(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(format!("{:016x}", hasher.finish()))
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            let value = value.trim();
            value == "1"
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("yes")
                || value.eq_ignore_ascii_case("on")
        })
        .unwrap_or(false)
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
    {
        Some(ext) => match ext.as_str() {
            "css" => "text/css; charset=utf-8",
            "gif" => "image/gif",
            "html" => "text/html; charset=utf-8",
            "ico" => "image/x-icon",
            "jpeg" | "jpg" => "image/jpeg",
            "js" => "text/javascript; charset=utf-8",
            "json" => "application/json; charset=utf-8",
            "map" => "application/json; charset=utf-8",
            "png" => "image/png",
            "svg" => "image/svg+xml",
            "txt" => "text/plain; charset=utf-8",
            "webp" => "image/webp",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            _ => "application/octet-stream",
        },
        None => "application/octet-stream",
    }
}
