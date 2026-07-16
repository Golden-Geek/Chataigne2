//! Build-script support helpers for Golden workspaces.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use golden_graph::GraphRevision;
use golden_protocol::{
    UiAck, UiClientMessage, UiContextCandidatesRequest, UiEditIntent, UiEventBatch, UiParamControlInfoDto,
    UiParamControlInfoRequest, UiProjectLoadProblemDto, UiProjectLoadRecoveryDto, UiProjectPathDto,
    UiProjectPathRequest, UiProjectUploadRequest, UiReferenceTargetsDto, UiReferenceTargetsRequest, UiReplayRequest,
    UiScriptConfigRequest, UiScriptReloadRequest, UiScriptStateRequest, UiServerMessage, UiSnapshot, UiSnapshotRequest,
};
use golden_script::ScriptUiState;
use ts_rs::{Config, TS};

#[derive(Debug, Clone)]
struct NodeEntry {
    module: String,
    type_name: String,
    source_path: String,
    declared_user_item: bool,
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
        let declared_user_item_types = extract_declared_user_item_types(&source_raw);

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
                declared_user_item: declared_user_item_types.contains(&type_name),
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
    let core_src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    println!("cargo:rerun-if-changed={}", core_src_root.display());

    let mut rust_files = Vec::new();
    collect_rs_files(&core_src_root, &mut rust_files);
    rust_files.sort();
    for path in rust_files {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    if out_dir.exists() {
        fs::remove_dir_all(out_dir).unwrap_or_else(|err| panic!("failed to clear {}: {}", out_dir.display(), err));
    }
    fs::create_dir_all(out_dir).unwrap_or_else(|err| panic!("failed to create {}: {}", out_dir.display(), err));

    let config = Config::new().with_out_dir(out_dir).with_large_int("number");

    export_binding::<UiSnapshot>(&config, "UiSnapshot");
    export_binding::<UiEventBatch>(&config, "UiEventBatch");
    export_binding::<UiClientMessage>(&config, "UiClientMessage");
    export_binding::<UiServerMessage>(&config, "UiServerMessage");
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
    export_binding::<UiProjectUploadRequest>(&config, "UiProjectUploadRequest");
    export_binding::<UiProjectLoadProblemDto>(&config, "UiProjectLoadProblemDto");
    export_binding::<UiProjectLoadRecoveryDto>(&config, "UiProjectLoadRecoveryDto");
    export_binding::<UiProjectPathDto>(&config, "UiProjectPathDto");
    normalize_generated_typescript_bindings(out_dir);
}

/// Exports Rust-owned generic graph bindings into the graph UI package.
pub fn generate_graph_ui_bindings(out_dir: &Path) {
    if out_dir.exists() {
        fs::remove_dir_all(out_dir).unwrap_or_else(|err| panic!("failed to clear {}: {}", out_dir.display(), err));
    }
    fs::create_dir_all(out_dir).unwrap_or_else(|err| panic!("failed to create {}: {}", out_dir.display(), err));

    let config = Config::new().with_out_dir(out_dir).with_large_int("number");
    export_binding::<GraphRevision>(&config, "GraphRevision");
    normalize_generated_typescript_bindings(out_dir);
}

/// Small command-line wrapper around the codegen helpers.
pub fn run_cli() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(
            "missing command: expected `app-nodes <src-root> <out-file>`, `ui-protocol <out-dir>`, or `graph-ui <out-dir>`".to_string(),
        );
    };

    match command.as_str() {
        "app-nodes" => {
            let Some(src_root) = args.next() else {
                return Err("missing <src-root> for `app-nodes`".to_string());
            };
            let Some(out_file) = args.next() else {
                return Err("missing <out-file> for `app-nodes`".to_string());
            };
            generate_app_nodes(Path::new(&src_root), Path::new(&out_file));
            Ok(())
        }
        "ui-protocol" => {
            let Some(out_dir) = args.next() else {
                return Err("missing <out-dir> for `ui-protocol`".to_string());
            };
            generate_ui_protocol_bindings(Path::new(&out_dir));
            Ok(())
        }
        "graph-ui" => {
            let Some(out_dir) = args.next() else {
                return Err("missing <out-dir> for `graph-ui`".to_string());
            };
            generate_graph_ui_bindings(Path::new(&out_dir));
            Ok(())
        }
        other => Err(format!(
            "unknown command `{other}`: expected `app-nodes`, `ui-protocol`, or `graph-ui`"
        )),
    }
}

fn export_binding<T: TS + 'static>(config: &Config, name: &str) {
    T::export_all(config).unwrap_or_else(|err| panic!("failed to export {name}: {err}"));
}

fn normalize_generated_typescript_bindings(out_dir: &Path) {
    let mut paths = fs::read_dir(out_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {}", out_dir.display(), err))
        .map(|entry| {
            entry
                .unwrap_or_else(|err| panic!("failed to read {} entry: {}", out_dir.display(), err))
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "ts"))
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let source =
            fs::read_to_string(&path).unwrap_or_else(|err| panic!("failed to read {}: {}", path.display(), err));
        let mut normalized = source.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
        if source.ends_with('\n') {
            normalized.push('\n');
        }

        if normalized != source {
            fs::write(&path, normalized).unwrap_or_else(|err| panic!("failed to write {}: {}", path.display(), err));
        }
    }
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

fn extract_declared_user_item_types(source: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut offset = 0usize;

    while let Some(found) = source[offset..].find("#[") {
        let attr_start = offset + found;
        let attr_content_start = attr_start + 2;
        let Some(attr_end) = find_attribute_end(source, attr_content_start) else {
            break;
        };

        let attr = &source[attr_content_start..attr_end];
        if attr_is_item(attr)
            && let Some(type_name) = extract_item_target_type(&source[attr_end + 1..])
        {
            out.insert(type_name);
        }

        offset = attr_end + 1;
    }

    out
}

fn find_attribute_end(source: &str, content_start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut nested_brackets = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, byte) in bytes.iter().enumerate().skip(content_start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match *byte {
            b'"' => in_string = true,
            b'[' => nested_brackets += 1,
            b']' if nested_brackets == 0 => return Some(offset),
            b']' => nested_brackets = nested_brackets.saturating_sub(1),
            _ => {}
        }
    }

    None
}

fn attr_is_item(attr: &str) -> bool {
    let attr = attr.trim_start();
    let path_end = attr
        .find(|ch: char| ch == '(' || ch.is_ascii_whitespace())
        .unwrap_or(attr.len());
    let path = &attr[..path_end];
    path.split("::").last() == Some("item")
}

fn extract_item_target_type(tail: &str) -> Option<String> {
    let declaration = skip_leading_outer_attrs(tail);
    let impl_index = declaration.find("impl ");
    let struct_index = declaration.find("struct ");

    if struct_index.is_some_and(|index| impl_index.is_none_or(|impl_index| index < impl_index)) {
        let after_struct = &declaration[struct_index? + "struct ".len()..];
        return extract_rust_ident(after_struct);
    }

    let impl_start = impl_index?;
    let header = declaration[impl_start..].split_once('{')?.0;
    let self_ty = header.rsplit_once(" for ")?.1.trim();
    extract_rust_ident(self_ty.rsplit("::").next().unwrap_or(self_ty))
}

fn skip_leading_outer_attrs(mut tail: &str) -> &str {
    loop {
        let trimmed = tail.trim_start();
        if !trimmed.starts_with("#[") {
            return trimmed;
        }

        let Some(attr_end) = find_attribute_end(trimmed, 2) else {
            return trimmed;
        };
        tail = &trimmed[attr_end + 1..];
    }
}

fn extract_rust_ident(input: &str) -> Option<String> {
    let ident: String = input
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();

    (!ident.is_empty()).then_some(ident)
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

        let stem = s.strip_suffix(".rs").unwrap_or(&s);

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

    render_declared_user_item_catalog(entries, &mut out);

    out
}

fn render_declared_user_item_catalog(entries: &[NodeEntry], out: &mut String) {
    let declared_entries = entries
        .iter()
        .filter(|entry| entry.declared_user_item)
        .collect::<Vec<_>>();

    if declared_entries.is_empty() {
        return;
    }

    out.push_str("\npub(crate) fn declared_user_creatable_items(item_kind: &str) -> Vec<golden_core::node::UserCreatableItem> {\n");
    out.push_str("    let mut items = Vec::new();\n");
    for entry in &declared_entries {
        let ty = &entry.type_name;
        out.push_str(&format!(
            "    if item_kind == <{ty} as golden_core::node::DeclaredUserItemNode>::ITEM_KIND {{\n"
        ));
        out.push_str("        let mut item = golden_core::node::UserCreatableItem::new(\n");
        out.push_str(&format!(
            "            <{ty} as golden_core::node::DeclaredUserItemNode>::ITEM_NODE_TYPE,\n"
        ));
        out.push_str(&format!(
            "            <{ty} as golden_core::node::DeclaredUserItemNode>::ITEM_KIND,\n"
        ));
        out.push_str(&format!(
            "            <{ty} as golden_core::node::DeclaredUserItemNode>::item_default_label(),\n"
        ));
        out.push_str("        );\n");
        out.push_str(&format!(
            "        let __golden_menu_path = <{ty} as golden_core::node::DeclaredUserItemNode>::item_menu_path();\n"
        ));
        out.push_str("        if !__golden_menu_path.is_empty() {\n");
        out.push_str("            item = item.with_menu_path(__golden_menu_path);\n");
        out.push_str("        }\n");
        out.push_str("        items.push(item);\n");
        out.push_str("    }\n");
    }
    out.push_str("    items\n");
    out.push_str("}\n\n");

    out.push_str("pub(crate) fn declared_user_item_type_matches(node_type: &str, item_kind: &str) -> bool {\n");
    for entry in &declared_entries {
        let ty = &entry.type_name;
        out.push_str(&format!(
            "    if node_type == <{ty} as golden_core::node::DeclaredUserItemNode>::ITEM_NODE_TYPE {{\n"
        ));
        out.push_str(&format!(
            "        return item_kind == <{ty} as golden_core::node::DeclaredUserItemNode>::ITEM_KIND;\n"
        ));
        out.push_str("    }\n");
    }
    out.push_str("    false\n");
    out.push_str("}\n\n");

    out.push_str(
        "pub(crate) fn create_declared_user_item(node_type: &str, item_kind: &str) -> Option<Box<dyn golden_core::node::Node>> {\n",
    );
    for entry in &declared_entries {
        let ty = &entry.type_name;
        out.push_str(&format!(
            "    if node_type == <{ty} as golden_core::node::DeclaredUserItemNode>::ITEM_NODE_TYPE\n"
        ));
        out.push_str(&format!(
            "        && item_kind == <{ty} as golden_core::node::DeclaredUserItemNode>::ITEM_KIND\n"
        ));
        out.push_str("    {\n");
        out.push_str(&format!(
            "        let __golden_label = <{ty} as golden_core::node::DeclaredUserItemNode>::item_default_label();\n"
        ));
        out.push_str(&format!(
            "        let mut node = <{ty} as golden_core::node::DeclaredUserItemNode>::create_item();\n"
        ));
        out.push_str("        if golden_core::node::Node::node_data(&node).meta.label != __golden_label {\n");
        out.push_str("            golden_core::node::Node::node_data_mut(&mut node).meta.label = __golden_label;\n");
        out.push_str("        }\n");
        out.push_str("        return Some(Box::new(node));\n");
        out.push_str("    }\n");
    }
    out.push_str("    None\n");
    out.push_str("}\n");
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

    fn char_literal_len(source: &str, at: usize) -> Option<usize> {
        let bytes = source.as_bytes();
        if bytes.get(at) != Some(&b'\'') {
            return None;
        }

        let rest = source.get(at + 1..)?;
        let mut chars = rest.chars();
        let first = chars.next()?;

        if first == '\\' {
            let escaped = chars.next()?;
            return match escaped {
                'x' => {
                    let mut tail = chars.as_str().chars();
                    let high = tail.next()?;
                    let low = tail.next()?;
                    if !high.is_ascii_hexdigit() || !low.is_ascii_hexdigit() {
                        return None;
                    }

                    let end = at + 5;
                    (bytes.get(end) == Some(&b'\'')).then_some(end + 1 - at)
                }
                'u' => {
                    let tail = chars.as_str();
                    let stripped = tail.strip_prefix('{')?;
                    let close = stripped.find('}')?;
                    let digits = &stripped[..close];
                    if digits.is_empty() || digits.len() > 6 || !digits.chars().all(|ch| ch.is_ascii_hexdigit()) {
                        return None;
                    }

                    let end = at + 4 + close;
                    (bytes.get(end) == Some(&b'\'')).then_some(end + 1 - at)
                }
                _ => {
                    let end = at + 3;
                    (bytes.get(end) == Some(&b'\'')).then_some(end + 1 - at)
                }
            };
        }

        let end = at + 1 + first.len_utf8();
        (bytes.get(end) == Some(&b'\'')).then_some(end + 1 - at)
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
                } else if char_literal_len(source, i).is_some() {
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

#[cfg(test)]
mod tests;
