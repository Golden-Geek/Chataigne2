use std::path::{Path, PathBuf};

use golden_codegen_support::generate_app_nodes;

fn main() {
    tauri_build::build();

    let src_root = Path::new("src");
    let out_file = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is not set")).join("app_nodes.rs");
    generate_app_nodes(src_root, &out_file);
}
