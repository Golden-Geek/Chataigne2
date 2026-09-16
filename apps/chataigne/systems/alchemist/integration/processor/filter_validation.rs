//! Backend-owned validation warnings for editable Mapping filter chains.

use std::{collections::HashSet, sync::Arc};

use chataigne_alchemist::{CompileCtx, ManagedFilterValueMode, ManagedItemId, ManagedRegionKind};
use chataigne_state_machine::{
    InputSetRuntime, ManagedStageRuntime,
    alchemist::{shared_node_registry, shared_value_type_registry},
};
use golden_core::{
    node::NodeId,
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::systems_alchemist_formula::{
    ANODE_NODE_TYPE, formula_from_snapshot, node_has_warning, node_warning_matches,
};

use super::{
    FormulaSourceRef, managed_regions_from_snapshot, managed_source_schema,
    managed_region_roles_from_tags, processor_formula_source_ref,
    processor_managed_region_decl_id,
};

const FILTER_WARNING_ID: &str = "mapping_filter_validation";
const CHAIN_WARNING_ID: &str = "mapping_filter_chain_validation";

struct ValidationIssue {
    node: NodeId,
    message: &'static str,
    detail: String,
}

struct ValidationResult {
    filter_nodes: Vec<NodeId>,
    issue: Option<ValidationIssue>,
}

#[derive(Default)]
pub(super) struct FilterValidationWatch {
    pub(super) dependencies: HashSet<NodeId>,
    pub(super) formula_root: Option<NodeId>,
    pub(super) external_sources: HashSet<NodeId>,
}

pub(super) fn reconcile_filter_warnings(
    ctx: &mut ProcessCtx,
    region_node: NodeId,
) -> FilterValidationWatch {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return FilterValidationWatch::default();
    };
    let watch = filter_validation_watch(snapshot.as_ref(), region_node);
    let result = validate_filter_chain(snapshot.as_ref(), region_node);

    for filter in &result.filter_nodes {
        if result.issue.as_ref().is_some_and(|issue| issue.node == *filter) {
            continue;
        }
        if node_has_warning(snapshot.as_ref(), *filter, FILTER_WARNING_ID) {
            ctx.clear_node_warning(*filter, Some(FILTER_WARNING_ID));
        }
    }

    match result.issue {
        Some(issue) => {
            if !node_warning_matches(
                snapshot.as_ref(),
                issue.node,
                FILTER_WARNING_ID,
                issue.message,
                Some(&issue.detail),
            ) {
                ctx.set_node_warning_with(
                    issue.node,
                    Some(FILTER_WARNING_ID),
                    issue.message,
                    Some(&issue.detail),
                );
            }
            let label = snapshot
                .node(issue.node)
                .map_or("Filter", |node| node.label.as_str());
            let aggregate = format!("{label}: {}", issue.detail);
            if !node_warning_matches(
                snapshot.as_ref(),
                region_node,
                CHAIN_WARNING_ID,
                "Mapping filter chain is not executable",
                Some(&aggregate),
            ) {
                ctx.set_node_warning_with(
                    region_node,
                    Some(CHAIN_WARNING_ID),
                    "Mapping filter chain is not executable",
                    Some(&aggregate),
                );
            }
        }
        None => {
            if node_has_warning(snapshot.as_ref(), region_node, CHAIN_WARNING_ID) {
                ctx.clear_node_warning(region_node, Some(CHAIN_WARNING_ID));
            }
        }
    }
    watch
}

fn filter_validation_watch(
    snapshot: &ProcessTreeSnapshot,
    region_node: NodeId,
) -> FilterValidationWatch {
    let mut watch = FilterValidationWatch::default();
    let Some(processor) = snapshot.node(region_node).and_then(|region| region.parent) else {
        return watch;
    };
    watch.dependencies.insert(processor);
    if let Some(formula_source) = snapshot.find_child_by_decl_id(
        processor,
        super::PROCESSOR_FORMULA_SOURCE_DECL_ID,
    ) {
        watch.dependencies.insert(formula_source);
    }
    for sibling in snapshot.child_ids_slice(processor).iter().copied() {
        let is_relevant_region = sibling == region_node
            || snapshot.node(sibling).is_some_and(|node| {
                managed_region_roles_from_tags(&node.tags)
                    .contains(&chataigne_alchemist::SurfaceItemKind::Input)
            });
        if is_relevant_region {
            extend_subtree(snapshot, sibling, &mut watch.dependencies);
        }
    }
    if let Some(FormulaSourceRef::ProjectNode(reference)) =
        processor_formula_source_ref(snapshot, processor)
    {
        if let Some(formula_root) = snapshot.node_id_by_uuid(reference.uuid()) {
            watch.formula_root = Some(formula_root);
            extend_subtree(snapshot, formula_root, &mut watch.dependencies);
        }
    }
    for node_id in watch.dependencies.iter().copied().collect::<Vec<_>>() {
        let Some(ParamValue::Reference(reference)) =
            snapshot.node(node_id).and_then(|node| node.param_value.as_ref())
        else {
            continue;
        };
        if let Some(target) = reference
            .cached_id()
            .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
        {
            watch.external_sources.insert(target);
            watch.dependencies.insert(target);
        }
    }
    watch
}

fn extend_subtree(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    nodes: &mut HashSet<NodeId>,
) {
    if !nodes.insert(root) {
        return;
    }
    for child in snapshot.child_ids_slice(root) {
        extend_subtree(snapshot, *child, nodes);
    }
}

fn validate_filter_chain(
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    region_node: NodeId,
) -> ValidationResult {
    let filter_nodes = snapshot
        .child_ids(region_node)
        .into_iter()
        .filter(|child| {
            snapshot
                .node(*child)
                .is_some_and(|node| node.node_type == ANODE_NODE_TYPE)
        })
        .collect::<Vec<_>>();
    let Some(first_enabled) = filter_nodes.iter().copied().find(|node| {
        snapshot.node(*node).is_some_and(|node| node.enabled)
    }) else {
        return ValidationResult {
            filter_nodes,
            issue: None,
        };
    };
    let Some(processor) = snapshot.node(region_node).and_then(|region| region.parent) else {
        return ValidationResult {
            filter_nodes,
            issue: None,
        };
    };
    let Some(FormulaSourceRef::ProjectNode(reference)) =
        processor_formula_source_ref(snapshot, processor)
    else {
        return ValidationResult {
            filter_nodes,
            issue: None,
        };
    };
    let Some(formula_node) = snapshot.node_id_by_uuid(reference.uuid()) else {
        return ValidationResult {
            filter_nodes,
            issue: None,
        };
    };
    let Ok(formula) = formula_from_snapshot(snapshot, formula_node) else {
        return ValidationResult {
            filter_nodes,
            issue: None,
        };
    };
    let Some(filter_definition) = formula.surface.managed_regions.iter().find(|definition| {
        definition.kind == ManagedRegionKind::FilterPipeline
            && snapshot.node(region_node).is_some_and(|region| {
                region.decl_id == processor_managed_region_decl_id(definition.id.as_str())
            })
    }) else {
        return ValidationResult {
            filter_nodes,
            issue: None,
        };
    };
    let Some(input_definition) = formula
        .surface
        .managed_regions
        .iter()
        .find(|definition| definition.kind == ManagedRegionKind::InputSet)
    else {
        return ValidationResult {
            filter_nodes,
            issue: None,
        };
    };
    let Some(regions) = managed_regions_from_snapshot(snapshot, processor, &formula) else {
        return invalid_result(
            filter_nodes,
            first_enabled,
            "Filter configuration is invalid",
            "The authored Mapping regions cannot be decoded. Repair this filter's controls.",
        );
    };
    let Some(input_region) = regions.regions.get(&input_definition.id) else {
        return invalid_result(
            filter_nodes,
            first_enabled,
            "Awaiting source schema",
            "The Mapping input region is unavailable.",
        );
    };
    let Some(filter_region) = regions.regions.get(&filter_definition.id) else {
        return ValidationResult {
            filter_nodes,
            issue: None,
        };
    };
    let mut inputs = match InputSetRuntime::from_managed_region(input_definition, input_region) {
        Ok(inputs) => inputs,
        Err(error) => {
            return invalid_result(
                filter_nodes,
                first_enabled,
                "Awaiting source schema",
                &error.to_string(),
            );
        }
    };
    if let Err(error) = inputs.reconcile_source_schema(|source| managed_source_schema(snapshot, source)) {
        return invalid_result(
            filter_nodes,
            first_enabled,
            "Awaiting source schema",
            &error.to_string(),
        );
    }
    if inputs.layout().channels().is_empty()
        || inputs
            .layout()
            .channels()
            .iter()
            .any(|channel| channel.value_type.is_none())
    {
        return invalid_result(
            filter_nodes,
            first_enabled,
            "Awaiting source schema",
            "Add or resolve the Mapping inputs required by this filter.",
        );
    }

    let by_item = filter_nodes
        .iter()
        .filter_map(|node| {
            snapshot
                .node(*node)
                .map(|entry| (ManagedItemId::from_uuid(entry.uuid.0), *node))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: Some(&formula.properties),
    };
    let mut layout = Arc::clone(inputs.layout());
    for item in filter_region
        .items
        .iter()
        .filter(|item| item.enabled && item.anode.enabled)
    {
        let Some(node) = by_item.get(&item.id).copied() else {
            continue;
        };
        match ManagedStageRuntime::compile(
            item.clone(),
            layout.as_ref(),
            &ctx,
            filter_definition.filter_value_mode,
        ) {
            Ok(Some(stage)) => layout = Arc::clone(stage.output_layout()),
            Ok(None) if filter_definition.filter_value_mode == ManagedFilterValueMode::Routed => {}
            Ok(None) => {
                return invalid_result(
                    filter_nodes,
                    node,
                    "Filter is incompatible",
                    "The filter does not accept the complete Mapping value.",
                );
            }
            Err(error) => {
                return invalid_result(
                    filter_nodes,
                    node,
                    "Filter is incompatible",
                    &error.to_string(),
                );
            }
        }
    }

    ValidationResult {
        filter_nodes,
        issue: None,
    }
}

fn invalid_result(
    filter_nodes: Vec<NodeId>,
    node: NodeId,
    message: &'static str,
    detail: &str,
) -> ValidationResult {
    ValidationResult {
        filter_nodes,
        issue: Some(ValidationIssue {
            node,
            message,
            detail: detail.to_owned(),
        }),
    }
}
