use std::collections::BTreeSet;

use chataigne_alchemist::{SurfaceItemKind, configured_managed_variant};
use chataigne_state_machine::alchemist::shared_node_registry;

use super::super::palette::filter_catalog_items;
use crate::app::systems_alchemist_formula::{
    ANODE_CREATE_PREFIX, ANODE_MANAGED_VARIANT_SEPARATOR, create_anode_user_item_tree,
};

#[test]
fn filter_catalog_contains_every_registered_managed_application() {
    let actual = filter_catalog_items()
        .into_iter()
        .map(|item| item.node_type)
        .collect::<BTreeSet<_>>();
    let expected = shared_node_registry()
        .iter()
        .filter(|declaration| declaration.supports_role(SurfaceItemKind::Filter))
        .flat_map(|declaration| {
            (0..declaration.managed_application_variants().len()).filter_map(move |index| {
                configured_managed_variant(declaration.as_ref(), index, None)?;
                let node_type = format!(
                    "{ANODE_CREATE_PREFIX}{}{ANODE_MANAGED_VARIANT_SEPARATOR}{index}",
                    declaration.type_id()
                );
                create_anode_user_item_tree(&node_type).map(|_| node_type)
            })
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
    assert!(actual.iter().all(|node_type| !node_type.contains("/inputs/")));
}

#[test]
fn filter_catalog_keeps_incompatible_and_fixed_arity_filters_creatable() {
    let items = filter_catalog_items();
    let contains = |type_id: &str| {
        items.iter().any(|item| {
            item.node_type
                .starts_with(&format!("{ANODE_CREATE_PREFIX}{type_id}@managed/"))
        })
    };

    for type_id in [
        "remap",
        "smooth_filter",
        "math",
        "sum",
        "pack_vec2",
        "pack_vec3",
        "extract_vec2",
        "extract_vec3",
        "extract_color",
        "convert_tuple",
    ] {
        assert!(contains(type_id), "static catalog is missing {type_id}");
    }
    assert_eq!(
        items.iter().filter(|item| item.label.starts_with("Math (")).count(),
        2
    );
    assert!(items
        .iter()
        .all(|item| create_anode_user_item_tree(&item.node_type).is_some()));
}

#[test]
fn explicit_legacy_input_count_specs_remain_bounded() {
    assert!(create_anode_user_item_tree("alchemist_anode:sum@managed/0/inputs/3").is_some());
    assert!(create_anode_user_item_tree("alchemist_anode:sum@managed/0/inputs/65").is_none());
    assert!(create_anode_user_item_tree("alchemist_anode:remap@managed/0/inputs/3").is_none());
}
