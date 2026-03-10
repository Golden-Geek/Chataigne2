use std::path::{Path, PathBuf};

use golden_codegen_support::{generate_app_nodes, generate_ui_protocol_bindings};

fn main() {
    tauri_build::build();

    let src_root = Path::new("src");
    let out_file = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is not set")).join("app_nodes.rs");
    let ui_protocol_dir = PathBuf::from("src-ui")
        .join("src")
        .join("lib")
        .join("golden_ui")
        .join("generated")
        .join("rust_protocol");

    generate_app_nodes(src_root, &out_file);
    generate_ui_protocol_bindings(&ui_protocol_dir);
}
