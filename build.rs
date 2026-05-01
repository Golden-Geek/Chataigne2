use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;

use golden_codegen_support::generate_app_nodes;

const GC_FORCE_NPM_CI: &str = "GC_FORCE_NPM_CI";
const GC_SKIP_UI_BUILD: &str = "GC_SKIP_UI_BUILD";
const GC_UI_ASSUME_BUILT: &str = "GC_UI_ASSUME_BUILT";

struct BuildPaths {
    bundled_ui_dir: PathBuf,
    ui_assets_rs: PathBuf,
    app_nodes_rs: PathBuf,
    ui_root: PathBuf,
    npm_ci_stamp: PathBuf,
}

impl BuildPaths {
    fn from_env() -> Self {
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is not set"));
        Self {
            bundled_ui_dir: out_dir.join("ui-dist"),
            ui_assets_rs: out_dir.join("ui_assets.rs"),
            app_nodes_rs: out_dir.join("app_nodes.rs"),
            npm_ci_stamp: out_dir.join("src-ui-package-lock.hash"),
            ui_root: PathBuf::from("src-ui"),
        }
    }
}

fn main() -> std::io::Result<()> {
    let paths = BuildPaths::from_env();

    emit_rerun_tracking(&paths)?;
    prepare_ui_assets(&paths)?;
    run_tauri_build();
    generate_rust_code(&paths);

    Ok(())
}

fn emit_rerun_tracking(paths: &BuildPaths) -> std::io::Result<()> {
    println!("cargo:rerun-if-env-changed={GC_FORCE_NPM_CI}");
    println!("cargo:rerun-if-env-changed={GC_SKIP_UI_BUILD}");
    println!("cargo:rerun-if-env-changed={GC_UI_ASSUME_BUILT}");

    track_ui_inputs(&paths.ui_root)?;
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
    let lockfile = paths.ui_root.join("package-lock.json");
    let current_hash = file_hash(&lockfile)?;
    let previous_hash = fs::read_to_string(&paths.npm_ci_stamp)
        .ok()
        .map(|value| value.trim().to_string());

    let force_ci = env_flag(GC_FORCE_NPM_CI);
    let missing_node_modules = !paths.ui_root.join("node_modules").exists();
    let stale_lockfile = previous_hash.as_deref() != Some(current_hash.as_str());

    if force_ci || missing_node_modules || stale_lockfile {
        if force_ci {
            println!("cargo:warning={GC_FORCE_NPM_CI}=1; running npm ci");
        } else if missing_node_modules {
            println!("cargo:warning=src-ui/node_modules is missing; running npm ci");
        } else {
            println!("cargo:warning=src-ui/package-lock.json changed; running npm ci");
        }

        run_npm_command(&paths.ui_root, &["ci"], &[])?;
        fs::write(&paths.npm_ci_stamp, current_hash)?;
    }

    Ok(())
}

fn build_ui_bundle(paths: &BuildPaths) -> std::io::Result<()> {
    if paths.bundled_ui_dir.exists() {
        fs::remove_dir_all(&paths.bundled_ui_dir)?;
    }
    fs::create_dir_all(&paths.bundled_ui_dir)?;

    run_npm_command(
        &paths.ui_root,
        &["run", "build"],
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

fn track_ui_inputs(ui_root: &Path) -> std::io::Result<()> {
    for relative in [
        "package.json",
        "package-lock.json",
        "svelte.config.js",
        "vite.config.ts",
        "src/app.html",
    ] {
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
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let mut command = Command::new(npm);
    command.args(args).current_dir(ui_root);
    for (key, value) in envs {
        command.env(key, value);
    }
    let status = command
        .status()
        .map_err(|err| Error::new(err.kind(), format!("failed to start {npm} {}: {err}", args.join(" "))))?;

    if status.success() {
        return Ok(());
    }

    Err(Error::new(
        ErrorKind::Other,
        format!("{npm} {} exited with status {status}", args.join(" ")),
    ))
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
