//! Derived-pages container node for the paging framework.
//!
//! This file is **codegen-only**: it is intentionally NOT declared in `common/mod.rs`,
//! so (like the concrete module files) it is compiled exactly once via the generated app
//! node registry. That keeps `PageHost` a single `AppNode` type. The paging runtime
//! constructs it through the registered re-export `crate::app::PageHost` (see
//! `paging::ensure_container`).
//!
//! A `PageHost` is a user-container that offers a "+ New Page" item and accepts plain
//! folders — each folder is one page. Page contents are cloned from the owning module's
//! default layout by `paging::complete_pages`; deletion uses standard node removal.

use golden_core::{
    node,
    node::{Folder, Node, UserContainerRules, UserCreatableItem, FOLDER_NODE_TYPE},
    process_ctx::ProcessCtx,
};

#[node("paging_page_host", label = "Pages")]
pub struct PageHost {}

#[node("paging_page_host", from_struct)]
impl Node for PageHost {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        // The container is structural: editable into, but not itself removable/duplicable.
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[FOLDER_NODE_TYPE]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_type == FOLDER_NODE_TYPE && item_kind == FOLDER_NODE_TYPE
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        // Do not auto-select the new page: keep the inspector on the current node (the page
        // appears under it anyway). This matches the convention for inspector-created items.
        vec![UserCreatableItem::new(FOLDER_NODE_TYPE, FOLDER_NODE_TYPE, "New Page").with_select_when_created(false)]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == FOLDER_NODE_TYPE).then(|| {
            let mut folder = Folder::new("Page");
            crate::app::module::enable_module_authoring(folder.node_data_mut());
            Box::new(folder) as Box<dyn Node>
        })
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}
