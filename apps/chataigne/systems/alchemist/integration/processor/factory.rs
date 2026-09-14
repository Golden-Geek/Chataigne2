use std::collections::{HashMap, HashSet};

use chataigne_alchemist::ManagedRegionDefinition;
use golden_core::{
    edit::NodeTree,
    events::{Event, EventKind},
    node::{EventPropagation, Node, NodeId},
    process_ctx::ProcessTreeSnapshot,
};

use crate::app::systems_alchemist_formula::{
    formula_managed_regions_from_snapshot, FORMULA_MANAGED_REGIONS_JSON_DECL_ID,
    PROPERTIES_DECL_ID,
};

use super::{
    find_formula_library, processor_managed_regions_tree, surface::ProcessorSurfaceTemplate,
    FormulaSourceRef, FORMULA_NODE_TYPE,
};

/// Formula structure captured at the palette boundary for detached processor creation.
/// Invalid or still-materializing metadata falls back to normal live reconciliation.
#[derive(Clone, Debug, Default)]
pub(super) struct ProcessorTreeTemplates {
    regions_by_type: HashMap<String, Vec<ManagedRegionDefinition>>,
    surfaces_by_type: HashMap<String, Vec<ProcessorSurfaceTemplate>>,
    watched_params: HashSet<NodeId>,
}

impl ProcessorTreeTemplates {
    pub(super) fn from_snapshot(snapshot: &ProcessTreeSnapshot) -> Self {
        let mut templates = Self::default();
        let Some(library) = find_formula_library(snapshot) else {
            return templates;
        };
        for formula_id in snapshot.child_ids(library) {
            let Some(formula) = snapshot.node(formula_id) else {
                continue;
            };
            if formula.node_type != FORMULA_NODE_TYPE {
                continue;
            }
            if let Some(metadata) = snapshot.find_child_by_decl_id(
                formula_id,
                FORMULA_MANAGED_REGIONS_JSON_DECL_ID,
            ) {
                templates.watched_params.insert(metadata);
            }
            let create_type = FormulaSourceRef::project_uuid(formula.uuid).processor_create_type();
            if let Ok(regions) = formula_managed_regions_from_snapshot(snapshot, formula_id) {
                templates.regions_by_type.insert(create_type.clone(), regions);
            }
            if let Some(properties) = snapshot.find_child_by_decl_id(formula_id, PROPERTIES_DECL_ID) {
                let surfaces = snapshot
                    .child_ids(properties)
                    .into_iter()
                    .filter_map(|child| ProcessorSurfaceTemplate::from_snapshot(snapshot, child))
                    .collect();
                templates.surfaces_by_type.insert(create_type, surfaces);
                let mut stack = vec![properties];
                while let Some(node_id) = stack.pop() {
                    if snapshot.node(node_id).is_some_and(|node| node.param_value.is_some()) {
                        templates.watched_params.insert(node_id);
                    }
                    stack.extend(snapshot.child_ids(node_id));
                }
            }
        }
        templates
    }

    pub(super) fn watches(&self, param: NodeId) -> bool {
        self.watched_params.contains(&param)
    }

    pub(super) fn event_propagation(&self, event: &Event) -> EventPropagation {
        match event.kind {
            EventKind::ParamChanged { param, .. } if !self.watches(param) => EventPropagation::PassOn,
            _ => EventPropagation::Notify,
        }
    }

    pub(super) fn create_tree(
        &self,
        node_type: &str,
        create_item: impl FnOnce() -> Option<Box<dyn Node>>,
    ) -> Option<NodeTree> {
        let mut tree = NodeTree::boxed(create_item()?);
        let create_type = FormulaSourceRef::parse_processor_create_type(node_type)
            .ok()
            .map(|source| source.processor_create_type());
        if let Some(surfaces) = create_type.as_ref().and_then(|key| self.surfaces_by_type.get(key)) {
            for surface in surfaces {
                tree.push_child(surface.clone().into_tree());
            }
        }
        if let Some(regions) = create_type.as_ref().and_then(|key| self.regions_by_type.get(key)) {
            tree.push_child(processor_managed_regions_tree(regions));
        }
        Some(tree)
    }
}
