use std::collections::HashMap;

use crate::node::{Node, NodeId, NodeUuid};

use super::Engine;

impl<T: Node> Engine<T> {
    /// Builds a runtime lookup map from persistent UUID to current node id.
    pub fn uuid_to_node_id_map(&self) -> HashMap<NodeUuid, NodeId> {
        self.nodes.iter().map(|(id, node)| (node.node_data().meta.uuid, id)).collect()
    }

    /// Returns the current runtime node id for a persistent UUID, when present.
    pub fn node_id_by_uuid(&self, uuid: NodeUuid) -> Option<NodeId> {
        self.nodes.iter().find_map(|(id, node)| (node.node_data().meta.uuid == uuid).then_some(id))
    }

    /// Rebuilds cached runtime ids inside all reference parameter values.
    ///
    /// Returns how many cached entries were updated.
    pub fn resolve_reference_caches(&mut self) -> usize {
        let uuid_map = self.uuid_to_node_id_map();
        let node_ids: Vec<NodeId> = self.nodes.keys().collect();
        let mut updated = 0usize;

        for node_id in node_ids {
            if let Some(node) = self.nodes.get_mut(node_id) {
                node.engine_visit_references_mut(&mut |reference| {
                    let resolved = uuid_map.get(&reference.uuid()).copied();
                    if reference.cached_id() != resolved {
                        updated += 1;
                        reference.set_cached_id(resolved);
                    }
                });
            }
        }

        updated
    }

    /// Clears cached runtime ids inside all reference parameter values.
    ///
    /// Returns how many cached entries were cleared.
    pub fn clear_reference_caches(&mut self) -> usize {
        let node_ids: Vec<NodeId> = self.nodes.keys().collect();
        let mut cleared = 0usize;

        for node_id in node_ids {
            if let Some(node) = self.nodes.get_mut(node_id) {
                node.engine_visit_references_mut(&mut |reference| {
                    if reference.cached_id().is_some() {
                        reference.clear_cached_id();
                        cleared += 1;
                    }
                });
            }
        }

        cleared
    }
}
