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
    tauri_build::build();
    Ok(())
}
