//! Registry-owned Add-menu choices for managed filter regions.

use std::sync::OnceLock;

use chataigne_alchemist::{SurfaceItemKind, configured_managed_variant};
use chataigne_state_machine::alchemist::shared_node_registry;
use golden_core::node::UserCreatableItem;

use crate::app::systems_alchemist_formula::{
    ANODE_CREATE_PREFIX, ANODE_ITEM_KIND, ANODE_MANAGED_VARIANT_SEPARATOR,
    create_anode_user_item_tree,
};

static FILTER_CATALOG: OnceLock<Vec<UserCreatableItem>> = OnceLock::new();

/// The app registry is immutable after startup, so its filter catalog is safe to
/// build once. Current sources, tuple shape, and neighboring filters never
/// participate in the authoring contract.
pub(super) fn filter_catalog_items() -> Vec<UserCreatableItem> {
    FILTER_CATALOG.get_or_init(build_filter_catalog).clone()
}

fn build_filter_catalog() -> Vec<UserCreatableItem> {
    let mut items = Vec::new();
    for declaration in shared_node_registry().iter() {
        if !declaration.supports_role(SurfaceItemKind::Filter) {
            continue;
        }
        for (index, _) in declaration.managed_application_variants().iter().enumerate() {
            let Some(instance) = configured_managed_variant(declaration.as_ref(), index, None)
            else {
                continue;
            };
            let create_type = format!(
                "{ANODE_CREATE_PREFIX}{}{ANODE_MANAGED_VARIANT_SEPARATOR}{index}",
                declaration.type_id()
            );
            if create_anode_user_item_tree(&create_type).is_none() {
                continue;
            }
            items.push(
                UserCreatableItem::new(create_type, ANODE_ITEM_KIND, &instance.label)
                    .with_menu_path([declaration.category()]),
            );
        }
    }
    items
}
