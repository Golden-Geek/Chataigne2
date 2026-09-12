use std::path::{Component, Path, PathBuf};

use crate::logger;

const SCRIPT_TEMPLATE_DIR: &str = "src/script/templates";
const SCRIPT_TEMPLATE_INCLUDE_PREFIX: &str = "{{include:";
const SCRIPT_TEMPLATE_INCLUDE_SUFFIX: &str = "}}";
const SCRIPT_TEMPLATE_EXTENSIONS: [&str; 3] = ["js", "mjs", "cjs"];
const SCRIPT_TEMPLATE_DEFAULT_SOURCE: &str = include_str!("templates/default.js");

pub(super) struct ScriptTemplateResolved {
    pub(super) source: String,
}

fn script_template_root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(SCRIPT_TEMPLATE_DIR)
}

fn normalized_template_key(value: &str) -> Option<String> {
    let mut output = String::new();
    let mut previous_was_separator = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
            continue;
        }

        if !previous_was_separator {
            output.push('_');
            previous_was_separator = true;
        }
    }

    let output = output.trim_matches('_').to_string();
    if output.is_empty() { None } else { Some(output) }
}

fn template_candidate_basenames(host_node_type: &str) -> Vec<String> {
    let mut basenames = Vec::new();
    let raw = host_node_type.trim().to_ascii_lowercase();
    if !raw.is_empty() {
        basenames.push(raw);
    }
    if let Some(normalized) = normalized_template_key(host_node_type)
        && !basenames.iter().any(|candidate| candidate == &normalized)
    {
        basenames.push(normalized);
    }
    basenames.push("default".to_string());
    basenames
}

fn normalize_include_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("template include path is empty".to_string());
    }

    let source = Path::new(trimmed);
    if source.is_absolute() {
        return Err(format!("template include path '{trimmed}' must be relative"));
    }

    let mut normalized = PathBuf::new();
    for component in source.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("template include path '{trimmed}' escapes template directory"));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(format!("template include path '{trimmed}' resolved to empty path"));
    }

    Ok(normalized)
}

fn include_stack_contains(stack: &[String], key: &str) -> bool {
    stack.iter().any(|item| item == key)
}

fn core_include_path(include_relative_path: &Path) -> Option<PathBuf> {
    let mut components = include_relative_path.components();
    match components.next() {
        Some(Component::Normal(segment)) if segment.to_string_lossy().eq_ignore_ascii_case("core") => {
            let remainder = components.as_path().to_path_buf();
            if remainder.as_os_str().is_empty() {
                None
            } else {
                Some(remainder)
            }
        }
        _ => None,
    }
}

fn resolve_include_path(include_relative_path: &Path, root_dir: &Path) -> Result<(PathBuf, PathBuf, String), String> {
    let (target_root, resolved_relative_path, include_key) =
        if let Some(core_path) = core_include_path(include_relative_path) {
            let include_key = format!(
                "core/{}",
                core_path.to_string_lossy().replace('\\', "/").to_ascii_lowercase()
            );
            (script_template_root_dir(), core_path, include_key)
        } else {
            (
                root_dir.to_path_buf(),
                include_relative_path.to_path_buf(),
                include_relative_path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_ascii_lowercase(),
            )
        };

    let include_path = target_root.join(&resolved_relative_path);
    if include_path.is_file() {
        return Ok((include_path, target_root, include_key));
    }

    let rel_forward = include_relative_path.to_string_lossy().replace('\\', "/");
    Err(format!(
        "failed to read template include '{}': looked in {}",
        rel_forward,
        target_root.join(&rel_forward).display()
    ))
}

fn expand_template_source(source: &str, root_dir: &Path, include_stack: &mut Vec<String>) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let mut cursor = source;

    while let Some(start_index) = cursor.find(SCRIPT_TEMPLATE_INCLUDE_PREFIX) {
        output.push_str(&cursor[..start_index]);
        let after_prefix = &cursor[start_index + SCRIPT_TEMPLATE_INCLUDE_PREFIX.len()..];
        let Some(end_index) = after_prefix.find(SCRIPT_TEMPLATE_INCLUDE_SUFFIX) else {
            return Err("template include directive is missing closing '}}'".to_string());
        };

        let include_path = &after_prefix[..end_index];
        let include_relative_path = normalize_include_path(include_path)?;
        let (include_path, include_root_dir, include_key) = resolve_include_path(&include_relative_path, root_dir)?;
        if include_stack_contains(include_stack, &include_key) {
            let cycle = include_stack
                .iter()
                .cloned()
                .chain(std::iter::once(include_key.clone()))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(format!("template include cycle detected: {cycle}"));
        }

        let include_source = std::fs::read_to_string(&include_path)
            .map_err(|err| format!("failed to read template include '{}': {err}", include_path.display()))?;
        include_stack.push(include_key);
        let expanded = expand_template_source(&include_source, &include_root_dir, include_stack);
        include_stack.pop();
        output.push_str(&expanded?);
        cursor = &after_prefix[end_index + SCRIPT_TEMPLATE_INCLUDE_SUFFIX.len()..];
    }

    output.push_str(cursor);
    Ok(output)
}

pub(super) fn read_template_from_path(path: &Path, root_dir: &Path) -> Result<String, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read script template '{}': {err}", path.display()))?;
    let mut stack = Vec::new();
    let relative = path
        .strip_prefix(root_dir)
        .map(|path| path.to_string_lossy().replace('\\', "/").to_ascii_lowercase())
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/").to_ascii_lowercase());
    stack.push(relative);
    expand_template_source(&source, root_dir, &mut stack)
}

fn default_embedded_template() -> String {
    let root_dir = script_template_root_dir();
    let mut stack = vec!["default.js".to_string()];
    expand_template_source(SCRIPT_TEMPLATE_DEFAULT_SOURCE, &root_dir, &mut stack)
        .unwrap_or_else(|_| SCRIPT_TEMPLATE_DEFAULT_SOURCE.to_string())
}

pub(super) fn resolve_template_for_host_in_dir(
    host_node_type: &str,
    root_dir: &Path,
) -> Option<ScriptTemplateResolved> {
    for basename in template_candidate_basenames(host_node_type) {
        for extension in SCRIPT_TEMPLATE_EXTENSIONS {
            let path = root_dir.join(format!("{basename}.{extension}"));
            if !path.is_file() {
                continue;
            }

            match read_template_from_path(&path, root_dir) {
                Ok(source) => {
                    return Some(ScriptTemplateResolved { source });
                }
                Err(error) => {
                    let _ = logger::log_message(
                        logger::LogLevel::Warning,
                        "script".to_string(),
                        None,
                        format!("failed to load script template '{}': {error}", path.display()),
                    );
                }
            }
        }
    }

    None
}

pub(super) fn resolve_template_for_host(host_node_type: &str) -> ScriptTemplateResolved {
    let root_dir = script_template_root_dir();
    if let Some(template) = resolve_template_for_host_in_dir(host_node_type, &root_dir) {
        return template;
    }

    ScriptTemplateResolved {
        source: default_embedded_template(),
    }
}
