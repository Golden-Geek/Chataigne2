//! Generic paging runtime shared by *pageable* modules.
//!
//! A *pageable* module declares one control layout once (the fixed declaration,
//! authored with the normal `#[children(...)]` DSL). That declared folder is the
//! always-present **`default` page**; its child addresses never move, so links,
//! expressions, dashboard widgets and scripts keep stable targets. Additional
//! pages are *derived* (structural clones of the declared layout) and live under a
//! `pages/` sub-container. A single `active_page` enum parameter — injected into
//! the module `parameters/` folder and constrained to the existing page ids — selects
//! which page the hardware viewport binds to.
//!
//! ```text
//! values/
//!   keys/                      <- pageable folder (declared once = page "default")
//!     key_0/ { text, color, pressed }   <- stable addresses, never relocated
//!     key_1/ { ... }
//!     pages/                   <- derived pages (created lazily)
//!       lighting/
//!         key_0/ { ... }       <- clone of the declared template
//!       audio/ { ... }
//! parameters/
//!   active_page: Enum = "default"        <- orchestrated by the global Preset/State system
//! ```
//!
//! This module is intentionally device-agnostic: it owns page lifecycle and the
//! `active_page` selector. Each concrete module (e.g. Stream Deck) owns the meaning
//! of the control *shapes* inside a page and resolves its own per-slot bindings
//! against the active page root returned here.

use golden_core::{
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeId},
    parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, ParameterEventBehaviour,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

/// Decl id / label of the derived page container nested inside a pageable folder.
pub(crate) const PAGES_CONTAINER: &str = "pages";
/// Decl id / label of the always-present fixed page (the declared layout itself).
pub(crate) const DEFAULT_PAGE_ID: &str = "default";
/// Decl id / label of the injected page-selector parameter (lives in `parameters/`).
pub(crate) const ACTIVE_PAGE_PARAM: &str = "active_page";
/// Reserved metadata tag the `pageable` declaration marker injects on the folder so
/// the generic page-management UI / preset tooling can discover pageable folders.
pub(crate) const PAGEABLE_TAG: &str = "pageable";

/// Normalizes a user-facing page name into a stable, address-safe page id.
///
/// Ids are immutable once created (renaming only changes the display label), which
/// preserves the stable-address guarantee. The `default` id is reserved.
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

/// Returns the ids of every page in declaration order, always starting with `default`.
pub(crate) fn page_ids(snapshot: &ProcessTreeSnapshot, template_folder: NodeId) -> Vec<String> {
    let mut ids = vec![DEFAULT_PAGE_ID.to_string()];
    if let Some(container) = snapshot.find_child(template_folder, PAGES_CONTAINER) {
        for child in snapshot.child_ids(container) {
            if let Some(node) = snapshot.node(child) {
                ids.push(node.label.clone());
            }
        }
    }
    ids
}

/// Resolves the page-root node for `active_page`.
///
/// `default` (or any unknown id) maps to the declared template folder itself, so the
/// fixed layout is always a valid binding target even before any extra page exists.
pub(crate) fn active_page_root(
    snapshot: &ProcessTreeSnapshot,
    template_folder: NodeId,
    active_page: &str,
) -> NodeId {
    if active_page.is_empty() || active_page == DEFAULT_PAGE_ID {
        return template_folder;
    }
    snapshot
        .find_child(template_folder, PAGES_CONTAINER)
        .and_then(|container| snapshot.find_child(container, active_page))
        .unwrap_or(template_folder)
}

/// Reads the current `active_page` value from the module `parameters/` folder.
///
/// Falls back to `default` when the selector has not been materialized yet.
pub(crate) fn active_page_value(snapshot: &ProcessTreeSnapshot, parameters_folder: NodeId) -> String {
    snapshot
        .find_child(parameters_folder, ACTIVE_PAGE_PARAM)
        .and_then(|param| snapshot.node(param))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_enum)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_PAGE_ID.to_string())
}

/// Ensures the `active_page` enum selector exists in `parameters/` and that its options
/// exactly match the current set of pages. Returns `true` when a structural edit was queued.
///
/// Mirrors the gamepad device-selector pattern: a fresh `Parameter` with rebuilt enum
/// options is swapped in via `replace_node` only when something actually changed, so this
/// is cheap to call every tick.
pub(crate) fn sync_selector(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parameters_folder: NodeId,
    template_folder: NodeId,
) -> bool {
    let ids = page_ids(snapshot, template_folder);
    let options = enum_options(&ids);

    match snapshot.find_child(parameters_folder, ACTIVE_PAGE_PARAM) {
        Some(existing_id) => {
            let existing = snapshot.node(existing_id);
            let current_value = existing
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_enum)
                .unwrap_or_else(|| DEFAULT_PAGE_ID.to_string());
            let options_match = existing
                .and_then(|node| node.param_constraints.as_ref())
                .is_some_and(|constraints| constraints.enum_options == options);
            // Snap a dangling selection (page was deleted) back to default.
            let next_value = if ids.iter().any(|id| id == &current_value) {
                current_value
            } else {
                DEFAULT_PAGE_ID.to_string()
            };
            let value_match = existing
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_enum)
                .is_some_and(|value| value == next_value);

            if options_match && value_match {
                return false;
            }
            ctx.replace_node_boxed(existing_id, Box::new(active_page_param(next_value, options)));
            true
        }
        None => {
            ctx.add_child_boxed(
                parameters_folder,
                Box::new(active_page_param(DEFAULT_PAGE_ID.to_string(), options)),
                None,
            );
            true
        }
    }
}

/// Derives a new page by cloning the declared template (the `default` layout) into
/// `pages/<id>`. Returns the stable page id, or `None` when the id already exists.
pub(crate) fn add_page(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    template_folder: NodeId,
    name: &str,
) -> Option<String> {
    let page_id = unique_page_id(snapshot, template_folder, &sanitize_page_id(name));

    // Clone every declared child of the template (skipping the reserved sub-container).
    let mut page_tree = NodeTree::new(authored_folder(&page_id));
    for child in snapshot.child_ids(template_folder) {
        if is_reserved_child(snapshot, child) {
            continue;
        }
        if let Some(child_tree) = clone_subtree(snapshot, child) {
            page_tree.push_child(child_tree);
        }
    }

    match snapshot.find_child(template_folder, PAGES_CONTAINER) {
        Some(container) => ctx.add_child_tree(container, page_tree, None),
        None => {
            let mut container = NodeTree::new(authored_folder(PAGES_CONTAINER));
            container.push_child(page_tree);
            ctx.add_child_tree(template_folder, container, None);
        }
    }
    Some(page_id)
}

/// Removes a derived page. The `default` page cannot be removed. Returns `true` when an
/// edit was queued.
pub(crate) fn remove_page(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    template_folder: NodeId,
    page_id: &str,
) -> bool {
    if page_id == DEFAULT_PAGE_ID {
        return false;
    }
    let Some(container) = snapshot.find_child(template_folder, PAGES_CONTAINER) else {
        return false;
    };
    let Some(page) = snapshot.find_child(container, page_id) else {
        return false;
    };
    ctx.edits.push(Edit::RemoveNode { node: page });
    true
}

fn is_reserved_child(snapshot: &ProcessTreeSnapshot, child: NodeId) -> bool {
    snapshot
        .node(child)
        .is_some_and(|node| node.label == PAGES_CONTAINER || node.decl_id.ends_with(PAGES_CONTAINER))
}

fn unique_page_id(snapshot: &ProcessTreeSnapshot, template_folder: NodeId, base: &str) -> String {
    let existing = page_ids(snapshot, template_folder);
    if !existing.iter().any(|id| id == base) {
        return base.to_string();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !existing.iter().any(|id| id == &candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Deep-clones a declared control subtree (folders + parameters only) from the snapshot
/// into a detached [`NodeTree`], preserving values and constraints.
fn clone_subtree(snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> Option<NodeTree> {
    let node = snapshot.node(node_id)?;
    if let Some(value) = node.param_value.as_ref() {
        let mut param = Parameter::new(&node.label, value.clone(), ParameterChangeCheck::ValueChange);
        if let Some(constraints) = node.param_constraints.as_ref() {
            param.constraints = constraints.clone();
        }
        crate::app::module::enable_module_authoring(param.node_data_mut());
        Some(NodeTree::new(param))
    } else {
        let mut tree = NodeTree::new(authored_folder(&node.label));
        for child in snapshot.child_ids(node_id) {
            if let Some(child_tree) = clone_subtree(snapshot, child) {
                tree.push_child(child_tree);
            }
        }
        Some(tree)
    }
}

fn authored_folder(label: &str) -> Folder {
    let mut folder = Folder::new(label);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    folder
}

fn active_page_param(value: String, options: Vec<ParameterEnumOption>) -> Parameter {
    let mut param = Parameter::new("Active Page", ParamValue::Enum(value), ParameterChangeCheck::ValueChange);
    param.event_behaviour = ParameterEventBehaviour::Coalesce;
    param.constraints.enum_options = options;
    // Pin a stable decl id so the selector resolves by `active_page` regardless of label,
    // and so repeated `sync_selector` calls update the same node instead of duplicating it.
    param.node_data_mut().meta.decl_id = golden_core::node::DeclId(ACTIVE_PAGE_PARAM.to_string());
    crate::app::module::enable_module_authoring(param.node_data_mut());
    param
}

fn enum_options(ids: &[String]) -> Vec<ParameterEnumOption> {
    ids.iter()
        .enumerate()
        .map(|(index, id)| ParameterEnumOption {
            variant_id: id.clone(),
            value: ParamValue::Enum(id.clone()),
            label: human_label(id),
            tags: Vec::new(),
            ordering: Some(index as i32),
        })
        .collect()
}

fn human_label(id: &str) -> String {
    let mut label = String::with_capacity(id.len());
    let mut capitalize = true;
    for ch in id.chars() {
        if ch == '_' {
            label.push(' ');
            capitalize = true;
        } else if capitalize {
            label.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            label.push(ch);
        }
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_page_id_is_address_safe_and_reserves_default() {
        assert_eq!(sanitize_page_id("Main Mix"), "main_mix");
        assert_eq!(sanitize_page_id("  A//B  "), "a_b");
        assert_eq!(sanitize_page_id("EQ-Focus!"), "eq_focus");
        // "A B" and "A_B" must NOT collide into the same id silently losing information:
        // both normalize deterministically and uniqueness is enforced by `unique_page_id`.
        assert_eq!(sanitize_page_id("default"), "page");
        assert_eq!(sanitize_page_id(""), "page");
    }

    #[test]
    fn human_label_titlecases_segments() {
        assert_eq!(human_label("main_mix"), "Main Mix");
        assert_eq!(human_label("default"), "Default");
    }
}
