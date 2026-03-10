//! Build-script support helpers for Golden workspaces.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use golden_core::script::ScriptUiState;
use golden_core::ui_sync::{
    UiAck, UiContextCandidatesRequest, UiEditIntent, UiEventBatch, UiParamControlInfoDto,
    UiParamControlInfoRequest, UiProjectPathRequest, UiReferenceTargetsDto, UiReferenceTargetsRequest,
    UiReplayRequest, UiScriptConfigRequest, UiScriptReloadRequest, UiScriptStateRequest, UiSnapshot,
    UiSnapshotRequest,
};
use ts_rs::{Config, TS};

#[derive(Debug, Clone)]
struct NodeEntry {
    module: String,
    type_name: String,
    source_path: String,
}

/// Scans `src_root` for node declarations and writes a generated node registry file.
pub fn generate_app_nodes(src_root: &Path, out_file: &Path) {
    println!("cargo:rerun-if-changed={}", src_root.display());

    let mut rust_files = Vec::new();
    collect_rs_files(src_root, &mut rust_files);
    rust_files.sort();

    let mut entries = Vec::new();
    for path in rust_files {
        println!("cargo:rerun-if-changed={}", path.display());

        let source_raw =
            fs::read_to_string(&path).unwrap_or_else(|err| panic!("failed to read {}: {}", path.display(), err));
        let source = strip_for_scanning(&source_raw);

        if !declares_node_type(&source) {
            continue;
        }

        let module = module_name_from_relative(src_root, &path);
        let source_path = normalize_absolute_path(
            path.canonicalize()
                .unwrap_or_else(|err| panic!("failed to canonicalize {}: {}", path.display(), err)),
        );

        let type_names = extract_struct_names(&source);
        if type_names.is_empty() {
            panic!(
                "could not find any `struct <Type>` in {} (expected node type declaration)",
                path.display()
            );
        }

        for type_name in type_names {
            entries.push(NodeEntry {
                module: module.clone(),
                type_name,
                source_path: source_path.clone(),
            });
        }
    }

    entries.sort_by(|a, b| a.type_name.cmp(&b.type_name));
    ensure_unique_type_names(&entries);

    let generated = render_registry(&entries);
    fs::write(out_file, generated).unwrap_or_else(|err| panic!("failed to write {}: {}", out_file.display(), err));
}

/// Exports Rust-owned UI transport bindings into the TypeScript workspace.
pub fn generate_ui_protocol_bindings(out_dir: &Path) {
    let core_src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("core").join("src");
    println!("cargo:rerun-if-changed={}", core_src_root.display());

    let mut rust_files = Vec::new();
    collect_rs_files(&core_src_root, &mut rust_files);
    rust_files.sort();
    for path in rust_files {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    if out_dir.exists() {
        fs::remove_dir_all(out_dir)
            .unwrap_or_else(|err| panic!("failed to clear {}: {}", out_dir.display(), err));
    }
    fs::create_dir_all(out_dir).unwrap_or_else(|err| panic!("failed to create {}: {}", out_dir.display(), err));

    let config = Config::new().with_out_dir(out_dir).with_large_int("number");

    export_binding::<UiSnapshot>(&config, "UiSnapshot");
    export_binding::<UiEventBatch>(&config, "UiEventBatch");
    export_binding::<UiAck>(&config, "UiAck");
    export_binding::<UiEditIntent>(&config, "UiEditIntent");
    export_binding::<UiReferenceTargetsDto>(&config, "UiReferenceTargetsDto");
    export_binding::<UiParamControlInfoDto>(&config, "UiParamControlInfoDto");
    export_binding::<ScriptUiState>(&config, "ScriptUiState");
    export_binding::<UiSnapshotRequest>(&config, "UiSnapshotRequest");
    export_binding::<UiReplayRequest>(&config, "UiReplayRequest");
    export_binding::<UiReferenceTargetsRequest>(&config, "UiReferenceTargetsRequest");
    export_binding::<UiContextCandidatesRequest>(&config, "UiContextCandidatesRequest");
    export_binding::<UiParamControlInfoRequest>(&config, "UiParamControlInfoRequest");
    export_binding::<UiScriptStateRequest>(&config, "UiScriptStateRequest");
    export_binding::<UiScriptConfigRequest>(&config, "UiScriptConfigRequest");
    export_binding::<UiScriptReloadRequest>(&config, "UiScriptReloadRequest");
    export_binding::<UiProjectPathRequest>(&config, "UiProjectPathRequest");
}

fn export_binding<T: TS + 'static>(config: &Config, name: &str) {
    T::export_all(config).unwrap_or_else(|err| panic!("failed to export {name}: {err}"));
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if !dir.exists() {
        return;
    }

    let read_dir = fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {}: {}", dir.display(), err));

    for entry in read_dir {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {}", err));
        let path = entry.path();

        if path.is_dir() {
            collect_rs_files(&path, out);
            continue;
        }

        if is_rust_file(&path) {
            out.push(path);
        }
    }
}

fn is_rust_file(path: &Path) -> bool {
    path.is_file() && matches!(path.extension().and_then(|ext| ext.to_str()), Some("rs"))
}

fn extract_struct_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();

    names.extend(extract_all_after_marker(source, "pub struct "));

    names.sort();
    names.dedup();
    names
}

fn extract_all_after_marker(source: &str, marker: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut offset = 0usize;

    while let Some(found) = source[offset..].find(marker) {
        let start = offset + found + marker.len();
        let tail = &source[start..];
        let ident: String = tail
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();

        if !ident.is_empty() {
            out.push(ident);
        }

        offset = start;
    }

    out
}

fn module_name_from_relative(src_root: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(src_root).unwrap_or_else(|_| {
        panic!(
            "failed to compute relative path for {} from {}",
            file.display(),
            src_root.display()
        )
    });

    let mut module = String::new();
    for component in relative.components() {
        let s = component.as_os_str().to_string_lossy();
        if s.eq_ignore_ascii_case("mod.rs") {
            continue;
        }

        let stem = if s.ends_with(".rs") { &s[..s.len() - 3] } else { &s };

        if module.is_empty() {
            module.push_str(&sanitize_ident_part(stem));
        } else {
            module.push('_');
            module.push_str(&sanitize_ident_part(stem));
        }
    }

    module
}

fn sanitize_ident_part(input: &str) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        out.push_str("node");
    }

    out
}

fn ensure_unique_type_names(entries: &[NodeEntry]) {
    let mut seen = HashSet::new();
    for entry in entries {
        if !seen.insert(entry.type_name.clone()) {
            panic!(
                "duplicate node type `{}` detected; node type names must be unique",
                entry.type_name
            );
        }
    }
}

fn declares_node_type(source: &str) -> bool {
    source.contains("#[node(")
        || source.contains("#[node]")
        || source.contains("#[golden_core::node(")
        || source.contains("#[golden_core::node]")
        || source.contains("#[::golden_core::node(")
        || source.contains("#[::golden_core::node]")
}

fn render_registry(entries: &[NodeEntry]) -> String {
    let mut out = String::new();
    out.push_str("// @generated by build.rs via golden_codegen_support. Do not edit.\n");
    out.push_str("use golden_core::define_node_enum;\n\n");

    let mut emitted_modules = HashSet::new();
    for entry in entries {
        if emitted_modules.insert(entry.module.clone()) {
            out.push_str(&format!("#[path = \"{}\"]\n", entry.source_path));
            out.push_str(&format!("pub mod {};\n", entry.module));
        }
    }

    if !entries.is_empty() {
        out.push('\n');
    }

    for entry in entries {
        out.push_str(&format!("pub use {}::{};\n", entry.module, entry.type_name));
    }

    if !entries.is_empty() {
        out.push('\n');
    }

    out.push_str("define_node_enum!(\n");
    out.push_str("    pub enum AppNode {\n");
    for entry in entries {
        out.push_str(&format!("        {},\n", entry.type_name));
    }
    out.push_str("    }\n");
    out.push_str(");\n");

    out
}

fn normalize_absolute_path(path: PathBuf) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized.strip_prefix("//?/").unwrap_or(&normalized).to_string()
}

fn strip_for_scanning(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum Mode {
        Normal,
        LineComment,
        BlockComment(usize),
        StringLiteral,
        CharLiteral,
        RawString(usize),
    }

    fn starts_with(bytes: &[u8], at: usize, pat: &[u8]) -> bool {
        at + pat.len() <= bytes.len() && &bytes[at..at + pat.len()] == pat
    }

    fn raw_start(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
        let mut i = at;
        if starts_with(bytes, at, b"br") {
            i += 2;
        } else if starts_with(bytes, at, b"r") {
            i += 1;
        } else {
            return None;
        }

        let mut hashes = 0usize;
        while i < bytes.len() && bytes[i] == b'#' {
            hashes += 1;
            i += 1;
        }

        if i < bytes.len() && bytes[i] == b'"' {
            Some((hashes, i + 1 - at))
        } else {
            None
        }
    }

    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0usize;
    let mut mode = Mode::Normal;

    while i < bytes.len() {
        match mode {
            Mode::Normal => {
                if starts_with(bytes, i, b"//") {
                    out.push_str("  ");
                    i += 2;
                    mode = Mode::LineComment;
                } else if starts_with(bytes, i, b"/*") {
                    out.push_str("  ");
                    i += 2;
                    mode = Mode::BlockComment(1);
                } else if let Some((hashes, consumed)) = raw_start(bytes, i) {
                    out.push_str(&" ".repeat(consumed));
                    i += consumed;
                    mode = Mode::RawString(hashes);
                } else if starts_with(bytes, i, b"b\"") {
                    out.push_str("  ");
                    i += 2;
                    mode = Mode::StringLiteral;
                } else if bytes[i] == b'"' {
                    out.push(' ');
                    i += 1;
                    mode = Mode::StringLiteral;
                } else if bytes[i] == b'\'' {
                    out.push(' ');
                    i += 1;
                    mode = Mode::CharLiteral;
                } else {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
            Mode::LineComment => {
                if bytes[i] == b'\n' {
                    out.push('\n');
                    i += 1;
                    mode = Mode::Normal;
                } else {
                    out.push(' ');
                    i += 1;
                }
            }
            Mode::BlockComment(depth) => {
                if starts_with(bytes, i, b"/*") {
                    out.push_str("  ");
                    i += 2;
                    mode = Mode::BlockComment(depth + 1);
                } else if starts_with(bytes, i, b"*/") {
                    out.push_str("  ");
                    i += 2;
                    if depth == 1 {
                        mode = Mode::Normal;
                    } else {
                        mode = Mode::BlockComment(depth - 1);
                    }
                } else {
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
            }
            Mode::StringLiteral => {
                if starts_with(bytes, i, b"\\") {
                    out.push(' ');
                    i += 1;
                    if i < bytes.len() {
                        if bytes[i] == b'\n' {
                            out.push('\n');
                        } else {
                            out.push(' ');
                        }
                        i += 1;
                    }
                } else if bytes[i] == b'"' {
                    out.push(' ');
                    i += 1;
                    mode = Mode::Normal;
                } else {
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
            }
            Mode::CharLiteral => {
                if starts_with(bytes, i, b"\\") {
                    out.push(' ');
                    i += 1;
                    if i < bytes.len() {
                        if bytes[i] == b'\n' {
                            out.push('\n');
                        } else {
                            out.push(' ');
                        }
                        i += 1;
                    }
                } else if bytes[i] == b'\'' {
                    out.push(' ');
                    i += 1;
                    mode = Mode::Normal;
                } else {
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
            }
            Mode::RawString(hashes) => {
                if bytes[i] == b'"' {
                    let mut closes = true;
                    for j in 0..hashes {
                        if i + 1 + j >= bytes.len() || bytes[i + 1 + j] != b'#' {
                            closes = false;
                            break;
                        }
                    }

                    if closes {
                        out.push(' ');
                        i += 1;
                        for _ in 0..hashes {
                            out.push(' ');
                        }
                        i += hashes;
                        mode = Mode::Normal;
                        continue;
                    }
                }

                if bytes[i] == b'\n' {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                i += 1;
            }
        }
    }

    out
}
