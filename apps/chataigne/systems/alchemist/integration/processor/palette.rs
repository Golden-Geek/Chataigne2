//! Backend Add-menu choices for a managed filter region's current typed layout.

use std::collections::HashSet;

use chataigne_alchemist::{ANodeInstance, ChannelLayout, CompileCtx, ManagedRegionKind, SurfaceItemKind};
use chataigne_state_machine::{
    alchemist::{shared_node_registry, shared_value_type_registry},
    validate_executable_filter_application, validate_trigger_filter_application, ManagedFormulaRuntime,
};
use golden_core::{
    node::{NodeId, UserCreatableItem},
    process_ctx::ProcessTreeSnapshot,
};

use crate::app::systems_alchemist_formula::{
    create_anode_user_item_tree, formula_from_snapshot, ANODE_CREATE_PREFIX, ANODE_ITEM_KIND,
    ANODE_MANAGED_VARIANT_SEPARATOR,
};
use crate::app::AlchemistFormulaDefinition;

use super::{
    managed_regions_from_snapshot, managed_source_schema, processor_formula_source_ref, FormulaSourceRef,
    PROCESSOR_MANAGED_REGIONS_DECL_ID,
};

pub(super) fn filter_palette_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    region_node: NodeId,
) -> Vec<UserCreatableItem> {
    let Some(regions_root) = snapshot.node(region_node).and_then(|region| region.parent) else {
        return Vec::new();
    };
    if !snapshot
        .node(regions_root)
        .is_some_and(|root| root.decl_id == PROCESSOR_MANAGED_REGIONS_DECL_ID)
    {
        return Vec::new();
    }
    let Some(processor_node) = snapshot.node(regions_root).and_then(|root| root.parent) else {
        return Vec::new();
    };
    let Some(FormulaSourceRef::ProjectNode(reference)) = processor_formula_source_ref(snapshot, processor_node) else {
        return Vec::new();
    };
    let Some(formula_node) = snapshot.node_id_by_uuid(reference.uuid()).filter(|node| {
        snapshot
            .node(*node)
            .is_some_and(|node| node.node_type == AlchemistFormulaDefinition::NODE_TYPE)
    }) else {
        return Vec::new();
    };
    let Ok(formula) = formula_from_snapshot(snapshot, formula_node) else {
        return Vec::new();
    };
    let Some(_) = formula.surface.managed_regions.iter().find(|definition| {
        definition.kind == ManagedRegionKind::FilterPipeline
            && snapshot
                .node(region_node)
                .is_some_and(|region| region.decl_id == super::processor_managed_region_decl_id(definition.id.as_str()))
    }) else {
        return Vec::new();
    };
    let mut instance = formula.instantiate();
    let Some(regions) = managed_regions_from_snapshot(snapshot, processor_node, &formula) else {
        return Vec::new();
    };
    instance.managed_regions = regions;
    let ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&formula.properties),
    };
    let Ok(Some(mut managed)) = ManagedFormulaRuntime::compile(&formula, &instance, &ctx) else {
        return Vec::new();
    };
    if managed
        .reconcile_input_source_schema(|source| managed_source_schema(snapshot, source))
        .is_err()
    {
        return Vec::new();
    }
    let Some(layout) = managed.filter_output_layout() else {
        return trigger_filter_palette_items(&ctx);
    };
    filter_palette_items_for_layout(layout, &ctx)
}

pub(super) fn trigger_filter_palette_items(ctx: &CompileCtx<'_>) -> Vec<UserCreatableItem> {
    executable_filter_items(ctx, |instance| {
        validate_trigger_filter_application(instance, ctx).is_ok()
    })
}

pub(super) fn filter_palette_items_for_layout(layout: &ChannelLayout, ctx: &CompileCtx<'_>) -> Vec<UserCreatableItem> {
    executable_filter_items(ctx, |instance| {
        validate_executable_filter_application(instance, layout, ctx).is_ok()
    })
}

fn executable_filter_items(
    ctx: &CompileCtx<'_>,
    mut validate: impl FnMut(&ANodeInstance) -> bool,
) -> Vec<UserCreatableItem> {
    let mut items = Vec::new();
    for declaration in ctx.nodes.iter() {
        if !declaration.supports_role(SurfaceItemKind::Filter) {
            continue;
        }
        for (index, instance) in declaration.managed_application_variants().into_iter().enumerate() {
            if !validate(&instance) {
                continue;
            }
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

pub(super) fn structural_palette_params(snapshot: &ProcessTreeSnapshot, region_node: NodeId) -> HashSet<NodeId> {
    let Some(regions_root) = snapshot.node(region_node).and_then(|region| region.parent) else {
        return HashSet::new();
    };
    let mut params = HashSet::new();
    let mut pending = vec![regions_root];
    while let Some(parent) = pending.pop() {
        for child in snapshot.child_ids_slice(parent) {
            if let Some(node) = snapshot.node(*child) {
                if node.param_value.is_some() && node.decl_id.starts_with("config/") {
                    params.insert(*child);
                }
                pending.push(*child);
            }
        }
    }
    if let Some(processor) = snapshot.node(regions_root).and_then(|node| node.parent) {
        for decl_id in ["formula", super::PROCESSOR_FORMULA_SOURCE_DECL_ID] {
            if let Some(param) = snapshot.find_child_by_decl_id(processor, decl_id) {
                params.insert(param);
            }
        }
    }
    params
}
