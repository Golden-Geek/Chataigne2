use std::path::{Path, PathBuf};

#[path = "submodules/golden_core/crates/core/node/node_codegen.rs"]
mod node_codegen;

fn main() {
    let src_root = Path::new("src");
    let out_file = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is not set"))
        .join("app_nodes.rs");

    node_codegen::generate_app_nodes(src_root, &out_file);
}
