use std::path::{Path, PathBuf};

use golden_codegen_support::ui_bundle::{self, UiBundleConfig};

fn main() -> std::io::Result<()> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is not set"));
    let workspace_root = Path::new("../..");
    let shared_input_roots =
        ["golden-ui", "golden-graph-ui"].map(|package| workspace_root.join("packages").join(package));
    ui_bundle::prepare(&UiBundleConfig {
        workspace_root,
        ui_root: Path::new("ui"),
        npm_workspace: "mapgyver-ui",
        out_dir: &out_dir,
        shared_input_roots: &shared_input_roots,
        dependency_cache_namespace: "mapgyver",
    })?;
    if std::env::var_os("CARGO_FEATURE_NATIVE_PREVIEW_SMOKE").is_some() {
        let preview_root = workspace_root.join("packages/golden-video-ui");
        let preview_inputs = [
            preview_root.clone(),
            preview_root.join("tests/native"),
            workspace_root.join("packages/golden-ui"),
        ];
        ui_bundle::prepare(&UiBundleConfig {
            workspace_root,
            ui_root: &preview_root,
            npm_workspace: "golden_video_ui",
            out_dir: &out_dir.join("native-preview"),
            shared_input_roots: &preview_inputs,
            dependency_cache_namespace: "mapgyver-native-preview",
        })?;
    }
    tauri_build::build();
    Ok(())
}
