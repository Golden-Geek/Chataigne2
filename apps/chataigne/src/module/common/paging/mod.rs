//! Generic paging runtime shared by *pageable* controller modules.
//!
//! ## Controller convention (input vs control)
//!
//! * **`values/`** — *inputs only*: the read-only physical state of the device as a flat
//!   list of parameters (e.g. one `bool` per key).
//! * **`parameters/`** — *device control / appearance* (color, text, image, …): the paged side.
//!
//! ## Layout
//!
//! The default layout (`keys`) and the derived-pages container (`pages`) are **siblings**;
//! pages are NOT nested inside `keys`. Both `parameters/` and `values/` carry the same page
//! identities — a page exists on each side under `pages/<id>/`, keyed by a stable id.
//!
//! ```text
//! parameters/
//!   active_page: Enum = "default"   <- selector (Preset-orchestrated)
//!   keys/                           <- default page control (tag = "pageable")
//!     key_1/ { color, text, image, unpaged }
//!   pages/                          <- PageHost: "+ New Page" / delete
//!     lighting/ key_1/ { ... }      <- clone of `keys`, stable id "lighting"
//! values/
//!   keys/                           <- default page inputs: key_1: bool, ... (flat)
//!   pages/                          <- mirror (plain folder, no PageHost)
//!     lighting/ key_1: bool, ...
//! ```
//!
//! This module manages the **control** collection (PageHost, selector, page completion).
//! A module mirrors the pages onto its `values/` side by id (see the Stream Deck module).
//!
//! Page ids are stable: a page keeps its id (`short_name`) across renames.

use golden_core::{
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeId, NodeMetaPatch, NodeUserPermissions},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
};

/// Decl id of the derived-pages container (both the control PageHost and the values mirror).
pub(crate) const PAGES_CONTAINER: &str = "pages";
/// Id of the always-present fixed page (the `keys` layout itself).
pub(crate) const DEFAULT_PAGE_ID: &str = "default";
/// Decl id of the injected page-selector parameter.
pub(crate) const ACTIVE_PAGE_PARAM: &str = "active_page";
/// Reserved metadata tag the `pageable` declaration marker injects on the control folder.
pub(crate) const PAGEABLE_TAG: &str = "pageable";
/// Node type of the control-side derived-pages container.
pub(crate) const PAGE_HOST_TYPE: &str = "paging_page_host";

/// One page entry: a stable `id` plus its editable display `label`.
#[derive(Clone)]
pub(crate) struct PageDescriptor {
    pub id: String,
    pub label: String,
}

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
/// mirror folder), identified by node type or the reserved decl id.
pub(crate) fn container_id(snapshot: &ProcessTreeSnapshot, pages_parent: NodeId) -> Option<NodeId> {
    snapshot.child_ids(pages_parent).into_iter().find(|child| {
        snapshot
            .node(*child)
            .is_some_and(|node| node.node_type == PAGE_HOST_TYPE || node.decl_id == PAGES_CONTAINER)
    })
}

/// Ensures the control [`PageHost`](crate::app::PageHost) exists under `pages_parent`.
/// Returns its id, or `None` when it had to be created (available on the next tick).
pub(crate) fn ensure_container(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    pages_parent: NodeId,
) -> Option<NodeId> {
    if let Some(existing) = container_id(snapshot, pages_parent) {
        return Some(existing);
    }
    let mut host = crate::app::PageHost::new();
    host.node_data_mut().meta.label = "Pages".to_string();
    host.node_data_mut().meta.short_name = PAGES_CONTAINER.to_string();
    host.node_data_mut().meta.decl_id = golden_core::node::DeclId(PAGES_CONTAINER.to_string());
    ctx.add_child_boxed(pages_parent, Box::new(host), None);
    None
}

/// Returns every page (the fixed `default` first, then derived pages) found under `pages_parent`.
pub(crate) fn page_descriptors(snapshot: &ProcessTreeSnapshot, pages_parent: NodeId) -> Vec<PageDescriptor> {
    let mut pages = vec![PageDescriptor {
        id: DEFAULT_PAGE_ID.to_string(),
        label: "Default".to_string(),
    }];
    pages.extend(derived_descriptors(snapshot, pages_parent));
    pages
}

/// Returns only the derived pages (excludes `default`) under `pages_parent`.
pub(crate) fn derived_descriptors(snapshot: &ProcessTreeSnapshot, pages_parent: NodeId) -> Vec<PageDescriptor> {
    let mut pages = Vec::new();
    if let Some(container) = container_id(snapshot, pages_parent) {
        for child in snapshot.child_ids(container) {
            if let Some(node) = snapshot.node(child) {
                pages.push(PageDescriptor {
                    id: page_id_of(node),
                    label: node.label.clone(),
                });
            }
        }
    }
    pages
}

/// Resolves the active page-root node. `default` (or an unknown id) maps to `default_folder`.
pub(crate) fn active_page_root(
    snapshot: &ProcessTreeSnapshot,
    default_folder: NodeId,
    pages_parent: NodeId,
    active_page: &str,
) -> NodeId {
    if active_page.is_empty() || active_page == DEFAULT_PAGE_ID {
        return default_folder;
    }
    if let Some(container) = container_id(snapshot, pages_parent) {
        for child in snapshot.child_ids(container) {
            if snapshot.node(child).is_some_and(|node| page_id_of(node) == active_page) {
                return child;
            }
        }
    }
    default_folder
}

/// Reads the current `active_page` value from the module `parameters/` folder.
pub(crate) fn active_page_value(snapshot: &ProcessTreeSnapshot, parameters_folder: NodeId) -> String {
    snapshot
        .find_child(parameters_folder, ACTIVE_PAGE_PARAM)
        .and_then(|param| snapshot.node(param))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_enum)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_PAGE_ID.to_string())
}

/// Ensures the `active_page` enum selector exists in `parameters/` with options matching the
/// current pages. Returns `true` when a structural edit was queued.
pub(crate) fn sync_selector(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parameters_folder: NodeId,
    pages_parent: NodeId,
) -> bool {
    let pages = page_descriptors(snapshot, pages_parent);
    let options = enum_options(&pages);

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
            let next_value = if pages.iter().any(|page| page.id == current_value) {
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

/// Completes any freshly-created (empty) control pages: assigns a unique stable id and clones
/// `template_folder` (the default `keys` layout) into them. Idempotent.
/// `skip` lists control labels/decl-ids that are *default-only* and must not be cloned into
/// derived pages (e.g. a per-key `Unpaged` flag that only has meaning on the default layout).
pub(crate) fn complete_pages(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    pages_parent: NodeId,
    template_folder: NodeId,
    skip: &[&str],
) {
    let Some(container) = container_id(snapshot, pages_parent) else {
        return;
    };
    let template_children = snapshot.child_ids(template_folder);

    let mut claimed: Vec<String> = vec![DEFAULT_PAGE_ID.to_string()];
    for child in snapshot.child_ids(container) {
        if let Some(node) = snapshot.node(child) {
            if node.child_count > 0 {
                claimed.push(page_id_of(node));
            }
        }
    }

    for child in snapshot.child_ids(container) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if node.child_count > 0 {
            continue;
        }
        let id = unique_id(&sanitize_page_id(&node.label), &claimed);
        claimed.push(id.clone());
        if node.short_name != id {
            ctx.patch_node_meta(
                child,
                NodeMetaPatch {
                    short_name: Some(id),
                    ..Default::default()
                },
            );
        }
        for template_child in &template_children {
            if let Some(tree) = clone_subtree(snapshot, *template_child, skip) {
                ctx.add_child_tree(child, tree, None);
            }
        }
    }
}

/// Creates a new (empty) control page under the [`PageHost`](crate::app::PageHost).
pub(crate) fn add_page(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    pages_parent: NodeId,
    name: &str,
) -> Option<String> {
    let container = ensure_container(ctx, snapshot, pages_parent)?;
    let label = if name.trim().is_empty() { "Page" } else { name.trim() };
    ctx.add_child_tree(container, NodeTree::new(authored_folder(label)), None);
    Some(label.to_string())
}

/// Removes a derived control page by id. The `default` page cannot be removed.
pub(crate) fn remove_page(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    pages_parent: NodeId,
    page_id: &str,
) -> bool {
    if page_id == DEFAULT_PAGE_ID {
        return false;
    }
    let Some(container) = container_id(snapshot, pages_parent) else {
        return false;
    };
    for child in snapshot.child_ids(container) {
        if snapshot.node(child).is_some_and(|node| page_id_of(node) == page_id) {
            ctx.edits.push(Edit::RemoveNode { node: child });
            return true;
        }
    }
    false
}

fn unique_id(base: &str, claimed: &[String]) -> String {
    if !claimed.iter().any(|id| id == base) {
        return base.to_string();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !claimed.iter().any(|id| id == &candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Deep-clones a control subtree (folders + parameters only) from the snapshot into a
/// detached [`NodeTree`], preserving values and constraints.
fn clone_subtree(snapshot: &ProcessTreeSnapshot, node_id: NodeId, skip: &[&str]) -> Option<NodeTree> {
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
            if snapshot
                .node(child)
                .is_some_and(|c| skip.iter().any(|s| c.label == *s || c.decl_id == *s))
            {
                continue; // default-only field: not cloned into derived pages
            }
            if let Some(child_tree) = clone_subtree(snapshot, child, skip) {
                tree.push_child(child_tree);
            }
        }
        Some(tree)
    }
}

/// Mirrors the derived control pages onto a parallel `pages` collection under
/// `mirror_pages_parent` (e.g. the `values/` side). This is the standard counterpart for
/// pageable controllers that carry both control and inputs.
///
/// Mirror pages are **locked**: their name and existence are synced to the canonical control
/// pages (not user-editable). The mirror container is created when the first page appears and
/// removed when the last page is deleted. `build_page_keys` supplies the per-key body for a
/// newly created mirror page (sized to the current model by the caller).
pub(crate) fn mirror_pages(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    source_pages_parent: NodeId,
    mirror_pages_parent: NodeId,
    build_page_keys: impl Fn() -> Vec<NodeTree>,
) {
    let pages = derived_descriptors(snapshot, source_pages_parent);
    let existing = container_id(snapshot, mirror_pages_parent);

    if pages.is_empty() {
        if let Some(container) = existing {
            ctx.edits.push(Edit::RemoveNode { node: container });
        }
        return;
    }

    let Some(container) = existing else {
        ctx.add_child_tree(
            mirror_pages_parent,
            NodeTree::new(locked_named_folder("Pages", PAGES_CONTAINER)),
            None,
        );
        return;
    };

    let mirror_children = snapshot.child_ids(container);
    for page in &pages {
        match mirror_children
            .iter()
            .copied()
            .find(|id| snapshot.node(*id).is_some_and(|node| page_id_of(node) == page.id))
        {
            Some(mirror) => {
                if snapshot.node(mirror).is_some_and(|node| node.label != page.label) {
                    ctx.patch_node_meta(
                        mirror,
                        NodeMetaPatch {
                            label: Some(page.label.clone()),
                            ..Default::default()
                        },
                    );
                }
            }
            None => {
                let mut tree = NodeTree::new(locked_named_folder(&page.label, &page.id));
                for key in build_page_keys() {
                    tree.push_child(key);
                }
                ctx.add_child_tree(container, tree, None);
            }
        }
    }

    for mirror in mirror_children {
        let keep = snapshot
            .node(mirror)
            .is_some_and(|node| pages.iter().any(|page| page.id == page_id_of(node)));
        if !keep {
            ctx.edits.push(Edit::RemoveNode { node: mirror });
        }
    }
}

/// A folder with a pinned stable id whose name/existence is not user-editable (locked).
fn locked_named_folder(label: &str, id: &str) -> Folder {
    let mut folder = Folder::new(label);
    let data = folder.node_data_mut();
    data.meta.short_name = id.to_string();
    data.meta.decl_id = golden_core::node::DeclId(id.to_string());
    data.meta.user_permissions = NodeUserPermissions::none();
    folder
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
    param.node_data_mut().meta.decl_id = golden_core::node::DeclId(ACTIVE_PAGE_PARAM.to_string());
    crate::app::module::enable_module_authoring(param.node_data_mut());
    param
}

fn enum_options(pages: &[PageDescriptor]) -> Vec<ParameterEnumOption> {
    pages
        .iter()
        .enumerate()
        .map(|(index, page)| ParameterEnumOption {
            variant_id: page.id.clone(),
            value: ParamValue::Enum(page.id.clone()),
            label: page.label.clone(),
            tags: Vec::new(),
            ordering: Some(index as i32),
        })
        .collect()
}

#[cfg(test)]
mod tests;
