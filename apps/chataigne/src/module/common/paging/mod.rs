//! Shared page identities and naming; each controller owns bounded layout preparation.
use golden_core::{
    node::NodeId,
    process_ctx::{ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
};
pub(crate) const PAGES_CONTAINER: &str = "pages";
pub(crate) const DEFAULT_PAGE_ID: &str = "default";
pub(crate) const ACTIVE_PAGE_PARAM: &str = "active_page";
pub(crate) const PAGEABLE_TAG: &str = "pageable";
pub(crate) const PAGE_HOST_TYPE: &str = "paging_page_host";
/// Normalizes a user-facing page name into a stable, address-safe page id.
pub(crate) fn sanitize_page_id(name: &str) -> String {
    let mut id = String::with_capacity(name.len());
    let mut last_was_sep = false;
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            id.push('_');
            last_was_sep = true;
        }
    }
    let id = id.trim_matches('_').to_string();
    if id.is_empty() || id == DEFAULT_PAGE_ID {
        "page".to_string()
    } else {
        id
    }
}

/// The id used to address a page node: its stable `short_name` (fallback to a slug of label).
pub(crate) fn page_id_of(node: &ProcessTreeNodeSnapshot) -> String {
    if node.short_name.trim().is_empty() {
        sanitize_page_id(&node.label)
    } else {
        node.short_name.clone()
    }
}

/// Returns the derived-pages container under `pages_parent` (control PageHost or values
/// mirror folder), identified by its reserved decl id.
pub(crate) fn container_id(snapshot: &ProcessTreeSnapshot, pages_parent: NodeId) -> Option<NodeId> {
    snapshot.find_child_by_decl_id(pages_parent, PAGES_CONTAINER)
}

pub(crate) fn unique_id(base: &str, claimed: &[String]) -> String {
    let claimed: std::collections::HashSet<&str> = claimed.iter().map(String::as_str).collect();
    if !claimed.contains(base) {
        return base.to_string();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !claimed.contains(candidate.as_str()) {
            return candidate;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests;
