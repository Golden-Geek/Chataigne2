use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use chataigne_alchemist::{ANodeId, CompiledAlchemistGraph, FormulaPropertySchema, ManagedItemInstance, ValueTypeId};

const MAX_SPECIALIZATIONS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct StageSpecializationKey {
    node_definition: String,
    primary_types: Vec<ValueTypeId>,
    formula_properties: String,
}

impl StageSpecializationKey {
    pub(super) fn new(
        item: &ManagedItemInstance,
        primary_types: Vec<ValueTypeId>,
        properties: Option<&FormulaPropertySchema>,
    ) -> Self {
        let node = &item.anode;
        Self {
            // Instance identity and editor layout do not affect the executable graph.
            node_definition: format!(
                "{:?}",
                (
                    &node.type_id,
                    &node.config,
                    &node.input_defaults,
                    &node.type_bindings,
                    &node.forced_type_bindings,
                )
            ),
            primary_types,
            formula_properties: format!("{properties:?}"),
        }
    }
}

#[derive(Clone)]
pub(super) struct StageSpecialization {
    pub(super) compiled: Arc<CompiledAlchemistGraph>,
    pub(super) authored_node: ANodeId,
}

/// Scoped to one app runtime and its fixed ANode/value-type registries, and bounded
/// independently of the number of processors. Never persist this cache.
#[derive(Default)]
pub struct ManagedStageSpecializationCache {
    entries: HashMap<StageSpecializationKey, StageSpecialization>,
    insertion_order: VecDeque<StageSpecializationKey>,
}

impl ManagedStageSpecializationCache {
    pub(super) fn get(&self, key: &StageSpecializationKey) -> Option<StageSpecialization> {
        self.entries.get(key).cloned()
    }

    pub(super) fn insert(&mut self, key: StageSpecializationKey, plan: StageSpecialization) {
        if self.entries.contains_key(&key) {
            return;
        }
        if self.entries.len() == MAX_SPECIALIZATIONS
            && let Some(oldest) = self.insertion_order.pop_front()
        {
            self.entries.remove(&oldest);
        }
        self.insertion_order.push_back(key.clone());
        self.entries.insert(key, plan);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
