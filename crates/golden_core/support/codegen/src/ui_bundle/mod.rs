//! UI bundling for application build scripts. Apps select their npm workspace
//! and shared source roots; this module owns dependency preparation, Cargo input
//! tracking, cached bundling, and the generated `APP_UI_ASSETS` module.
//!
//! Paths are resolved from the calling build script's working directory. The
//! workspace must provide `package.json` and `package-lock.json`, and its npm
//! build script must honor `GC_UI_OUT_DIR`. Generated files stay under `out_dir`.
//! `GC_SKIP_UI_BUILD` emits empty assets, `GC_UI_ASSUME_BUILT` embeds `ui_root/build`,
//! and `GC_FORCE_NPM_CI` reinstalls dependencies and forces a bundle rebuild.

use std::collections::hash_map::DefaultHasher;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::build_cache::build_if_stale;

const GC_FORCE_NPM_CI: &str = "GC_FORCE_NPM_CI";
const GC_SKIP_UI_BUILD: &str = "GC_SKIP_UI_BUILD";
const GC_UI_ASSUME_BUILT: &str = "GC_UI_ASSUME_BUILT";
const EXCLUDED_DIRECTORIES: &[&str] = &["node_modules", "build", "dist", ".svelte-kit", "docs", "tests"];

/// Application-owned inputs to the shared frontend build path.
pub struct UiBundleConfig<'a> {
    /// Repository root containing the shared npm lockfile and `node_modules`.
    pub workspace_root: &'a Path,
    /// Application frontend root, including its Vite/Svelte configuration.
    pub ui_root: &'a Path,
    /// npm workspace passed to `npm run build --workspace`.
    pub npm_workspace: &'a str,
    /// This application's Cargo `OUT_DIR`; owns `ui-dist`, its stamp, and `ui_assets.rs`.
    pub out_dir: &'a Path,
    /// Shared package directories whose source files affect this frontend.
    pub shared_input_roots: &'a [PathBuf],
    /// One directory name below `node_modules/.cache` for the dependency stamp.
    pub dependency_cache_namespace: &'a str,
}

/// Prepares assets and writes `out_dir/ui_assets.rs` for inclusion by the app.
/// The generated slice uses the public `golden_core::app::UiAsset` contract.
pub fn prepare(config: &UiBundleConfig<'_>) -> io::Result<()> {
    let paths = BundlePaths::new(config)?;
    let mode = BuildMode::from_flags(env_flag(GC_SKIP_UI_BUILD), env_flag(GC_UI_ASSUME_BUILT));
    for name in [GC_FORCE_NPM_CI, GC_SKIP_UI_BUILD, GC_UI_ASSUME_BUILT] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    match mode {
        BuildMode::Skip => {}
        BuildMode::Prebuilt => emit_rerun_for_dir(&config.ui_root.join("build"))?,
        BuildMode::Build => track_ui_inputs(&paths)?,
    }
    fs::create_dir_all(config.out_dir)?;
    prepare_assets(&paths, mode, env_flag(GC_FORCE_NPM_CI))
}

struct BundlePaths<'a> {
    config: &'a UiBundleConfig<'a>,
    caller_dir: PathBuf,
    bundled_ui_dir: PathBuf,
    ui_bundle_stamp: PathBuf,
    ui_assets_rs: PathBuf,
    node_modules: PathBuf,
    package_lock: PathBuf,
    npm_deps_stamp: PathBuf,
}

impl<'a> BundlePaths<'a> {
    fn new(config: &'a UiBundleConfig<'a>) -> io::Result<Self> {
        Self::new_at(config, std::env::current_dir()?)
    }

    fn new_at(config: &'a UiBundleConfig<'a>, caller_dir: PathBuf) -> io::Result<Self> {
        let namespace = config.dependency_cache_namespace;
        if namespace.is_empty() || [".", ".."].contains(&namespace) || namespace.contains(['/', '\\']) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "UI dependency cache namespace must be a single non-empty directory name",
            ));
        }
        if Path::new(namespace).components().count() != 1
            || !matches!(
                Path::new(namespace).components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "invalid UI dependency cache namespace",
            ));
        }
        if config.npm_workspace.trim().is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "UI npm workspace must not be empty",
            ));
        }
        let node_modules = config.workspace_root.join("node_modules");
        Ok(Self {
            config,
            caller_dir,
            bundled_ui_dir: config.out_dir.join("ui-dist"),
            ui_bundle_stamp: config.out_dir.join("ui-bundle.hash"),
            ui_assets_rs: config.out_dir.join("ui_assets.rs"),
            package_lock: config.workspace_root.join("package-lock.json"),
            npm_deps_stamp: node_modules.join(".cache").join(namespace).join("package-lock.hash"),
            node_modules,
        })
    }
}

#[derive(Clone, Copy)]
enum BuildMode {
    Skip,
    Prebuilt,
    Build,
}

impl BuildMode {
    fn from_flags(skip: bool, assume_built: bool) -> Self {
        if skip {
            Self::Skip
        } else if assume_built {
            Self::Prebuilt
        } else {
            Self::Build
        }
    }
}

fn prepare_assets(paths: &BundlePaths<'_>, mode: BuildMode, force_ci: bool) -> io::Result<()> {
    let asset_root = match mode {
        BuildMode::Skip => {
            println!("cargo:warning={GC_SKIP_UI_BUILD}=1; compiling without bundled UI assets");
            return fs::write(
                &paths.ui_assets_rs,
                "// @generated by build.rs. Do not edit.\npub static APP_UI_ASSETS: &[golden_core::app::UiAsset] = &[];\n",
            );
        }
        BuildMode::Prebuilt => {
            let assumed_dist = paths.config.ui_root.join("build");
            println!(
                "cargo:warning={GC_UI_ASSUME_BUILT}=1; using prebuilt UI at {}",
                assumed_dist.display()
            );
            assumed_dist
        }
        BuildMode::Build => {
            let toolchain = ensure_ui_dependencies(paths, force_ci)?;
            let inputs = ui_input_files(paths)?;
            let mut environment = std::env::vars_os()
                .filter(|(name, _)| {
                    let name = name.to_string_lossy();
                    name == "NODE_ENV" || name.starts_with("VITE_") || name.starts_with("PUBLIC_")
                })
                .collect::<Vec<_>>();
            environment.sort();
            println!("cargo:rerun-if-env-changed=NODE_ENV");
            for (name, _) in &environment {
                println!("cargo:rerun-if-env-changed={}", name.to_string_lossy());
            }
            let built = build_if_stale(
                &paths.ui_bundle_stamp,
                &paths.bundled_ui_dir,
                &inputs,
                &bundle_context(paths, &toolchain, &environment),
                force_ci,
                || build_ui_bundle(paths),
            )?;
            println!(
                "cargo:warning=UI bundle {}",
                if built {
                    "rebuilt (frontend inputs or output changed)"
                } else {
                    "reused (frontend inputs unchanged)"
                }
            );
            emit_rerun_for_dir(&paths.bundled_ui_dir)?;
            paths.bundled_ui_dir.clone()
        }
    };
    generate_ui_assets_module(&asset_root, &paths.ui_assets_rs)
}

fn bundle_context(paths: &BundlePaths<'_>, toolchain: &str, environment: &[(OsString, OsString)]) -> String {
    let config = paths.config;
    format!(
        "ui-bundle-v1\ncaller_dir={:?}\nworkspace_root={:?}\nui_root={:?}\nnpm_workspace={:?}\nshared_input_roots={:?}\ndependency_cache_namespace={:?}\n{toolchain}\n{environment:?}",
        paths.caller_dir,
        config.workspace_root,
        config.ui_root,
        config.npm_workspace,
        config.shared_input_roots,
        config.dependency_cache_namespace,
    )
}

fn ensure_ui_dependencies(paths: &BundlePaths<'_>, force_ci: bool) -> io::Result<String> {
    let toolchain = ensure_frontend_toolchain()?;
    let current_hash = file_hash(&paths.package_lock)?;
    let previous_hash = fs::read_to_string(&paths.npm_deps_stamp)
        .ok()
        .map(|value| value.trim().to_string());
    let has_node_modules = paths.node_modules.exists();
    let node_modules_ready = paths.node_modules.join(".bin").join(vite_bin_name()).exists();

    let install = if force_ci {
        println!("cargo:warning={GC_FORCE_NPM_CI}=1; running npm ci");
        Some("ci")
    } else if !has_node_modules {
        println!("cargo:warning=node_modules is missing; running npm ci");
        Some("ci")
    } else if !node_modules_ready {
        println!("cargo:warning=node_modules is incomplete; running npm install");
        Some("install")
    } else if previous_hash
        .as_deref()
        .is_some_and(|previous| previous != current_hash)
    {
        println!("cargo:warning=package-lock.json changed; running npm install");
        Some("install")
    } else {
        None
    };
    if let Some(install) = install {
        run_npm_command(paths.config.workspace_root, &[install, "--ignore-scripts"], &[])?;
    } else if previous_hash.is_none() {
        println!("cargo:warning=UI dependency stamp is missing; trusting existing node_modules");
    }
    if install.is_some() || previous_hash.is_none() {
        if let Some(parent) = paths.npm_deps_stamp.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&paths.npm_deps_stamp, current_hash)?;
    }
    Ok(toolchain)
}

fn vite_bin_name() -> &'static str {
    if cfg!(windows) { "vite.cmd" } else { "vite" }
}

fn build_ui_bundle(paths: &BundlePaths<'_>) -> io::Result<()> {
    if paths.bundled_ui_dir.exists() {
        fs::remove_dir_all(&paths.bundled_ui_dir)?;
    }
    fs::create_dir_all(&paths.bundled_ui_dir)?;
    run_npm_command(
        paths.config.workspace_root,
        &["run", "build", "--workspace", paths.config.npm_workspace],
        &[("GC_UI_OUT_DIR", paths.bundled_ui_dir.as_os_str())],
    )
}

fn track_ui_inputs(paths: &BundlePaths<'_>) -> io::Result<()> {
    for input in ui_input_files(paths)? {
        println!("cargo:rerun-if-changed={}", input.display());
    }
    // Directories catch additions and removals; generated outputs are excluded.
    for root in source_roots(paths) {
        emit_rerun_for_dir(&root)?;
    }
    Ok(())
}

fn source_roots<'a>(paths: &'a BundlePaths<'_>) -> impl Iterator<Item = PathBuf> + 'a {
    [paths.config.ui_root.join("src"), paths.config.ui_root.join("static")]
        .into_iter()
        .chain(paths.config.shared_input_roots.iter().cloned())
}

fn ui_input_files(paths: &BundlePaths<'_>) -> io::Result<Vec<PathBuf>> {
    let mut files = vec![
        paths.package_lock.clone(),
        paths.config.workspace_root.join("package.json"),
        PathBuf::from("build.rs"),
    ];
    let toolchain_manifest = paths.config.workspace_root.join("tools/bootstrap/toolchain.json");
    if toolchain_manifest.is_file() {
        files.push(toolchain_manifest);
    }
    // Include frontend configuration and .env files alongside source assets.
    for entry in fs::read_dir(paths.config.ui_root)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    for root in source_roots(paths) {
        collect_ui_input_files(&root, &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_ui_input_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if !EXCLUDED_DIRECTORIES.contains(&entry.file_name().to_string_lossy().as_ref()) {
                collect_ui_input_files(&path, files)?;
            }
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn emit_rerun_for_dir(dir: &Path) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if !EXCLUDED_DIRECTORIES.contains(&entry.file_name().to_string_lossy().as_ref()) {
                emit_rerun_for_dir(&path)?;
            }
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    Ok(())
}

fn run_npm_command(workspace_root: &Path, args: &[&str], envs: &[(&str, &OsStr)]) -> io::Result<()> {
    let npm = npm_command();
    let mut command = Command::new(npm);
    command.args(args).current_dir(workspace_root);
    for (key, value) in envs {
        command.env(key, value);
    }
    let status = command.status().map_err(|error| {
        Error::new(
            error.kind(),
            format!(
                "failed to start {npm} {}: {error}\n{}",
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

fn ensure_frontend_toolchain() -> io::Result<String> {
    let node = ensure_command_available("node")?;
    let npm = ensure_command_available(npm_command())?;
    Ok(format!("node={node}\nnpm={npm}"))
}

fn ensure_command_available(command: &str) -> io::Result<String> {
    let output = Command::new(command)
        .arg("--version")
        .stderr(Stdio::null())
        .output()
        .map_err(|error| {
            Error::new(
                error.kind(),
                format!(
                    "required frontend command `{command}` was not found or could not start: {error}\n{}",
                    frontend_toolchain_help()
                ),
            )
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }
    Err(Error::other(format!(
        "required frontend command `{command}` exited with status {}\n{}",
        output.status,
        frontend_toolchain_help()
    )))
}

fn npm_command() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

fn frontend_toolchain_help() -> &'static str {
    "The application build embeds its frontend. Install the Node.js and npm versions required by the workspace package.json and tools/bootstrap/toolchain.json.\nFor engine-only checks, set GC_SKIP_UI_BUILD=1."
}

fn generate_ui_assets_module(dist_root: &Path, out_file: &Path) -> io::Result<()> {
    if !dist_root.exists() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("bundled UI build output {} does not exist", dist_root.display()),
        ));
    }
    if !dist_root.join("index.html").is_file() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("bundled UI output {} is missing index.html", dist_root.display()),
        ));
    }
    let mut asset_files = Vec::new();
    collect_files(dist_root, &mut asset_files)?;
    asset_files.sort();
    let mut generated = String::from(
        "// @generated by build.rs. Do not edit.\npub static APP_UI_ASSETS: &[golden_core::app::UiAsset] = &[\n",
    );
    for asset_path in asset_files {
        let relative_path = asset_path.strip_prefix(dist_root).map_err(|error| {
            Error::new(
                ErrorKind::InvalidInput,
                format!("invalid bundled UI asset path: {error}"),
            )
        })?;
        let http_path = format!("/{}", utf8_path(relative_path)?.replace('\\', "/"));
        let content_type = content_type_for_path(relative_path);
        let absolute_path = asset_path.canonicalize()?;
        generated.push_str(&format!(
            "    golden_core::app::UiAsset {{\n        path: {http_path:?},\n        content_type: {content_type:?},\n        bytes: include_bytes!({:?}),\n    }},\n",
            utf8_path(&absolute_path)?
        ));
    }
    generated.push_str("];\n");
    fs::write(out_file, generated)
}

fn utf8_path(path: &Path) -> io::Result<&str> {
    path.to_str()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "non-UTF-8 bundled UI asset path"))
}

fn collect_files(dir: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
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

fn file_hash(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(format!("{:016x}", hasher.finish()))
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("css") => "text/css; charset=utf-8",
        Some("gif") => "image/gif",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("webp") => "image/webp",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests;
