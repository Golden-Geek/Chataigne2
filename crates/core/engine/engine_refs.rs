use std::collections::HashMap;
use std::collections::HashSet;

use crate::events::EventKind;
use crate::node::{Node, NodeId, NodeUuid};
use crate::parameter::{ParamValue, ReferenceConstraints, ReferenceRoot, ReferenceTargetKind};

use super::Engine;

pub(crate) const MISSING_REFERENCE_WARNING_ID: &str = "missing-reference";

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
        let uuid_map: HashMap<NodeUuid, (NodeId, String)> = self.nodes.iter().map(|(id, node)| (node.node_data().meta.uuid, (id, node.node_data().meta.label.clone()))).collect();
        let node_ids: Vec<NodeId> = self.nodes.keys().collect();
        let mut updated = 0usize;

        for node_id in node_ids {
            if let Some(node) = self.nodes.get_mut(node_id) {
                node.engine_visit_references_mut(&mut |reference| {
                    let resolved = uuid_map.get(&reference.uuid()).map(|(id, _)| *id);
                    if reference.cached_id() != resolved {
                        updated += 1;
                        reference.set_cached_id(resolved);
                    }
                    if let Some((_, cached_name)) = uuid_map.get(&reference.uuid()) {
                        if reference.cached_name() != Some(cached_name.as_str()) {
                            updated += 1;
                            reference.set_cached_name(Some(cached_name.clone()));
                        }
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

    pub(crate) fn sync_missing_reference_warnings(&mut self) -> usize {
        self.sync_missing_reference_warnings_impl(true)
    }

    pub(crate) fn sync_missing_reference_warnings_silent(&mut self) -> usize {
        self.sync_missing_reference_warnings_impl(false)
    }

    fn sync_missing_reference_warnings_impl(&mut self, emit_events: bool) -> usize {
        self.resolve_reference_caches();
        let uuid_map = self.uuid_to_node_id_map();
        let node_ids: Vec<NodeId> = self.nodes.keys().collect();
        let mut pending: Vec<(NodeId, crate::node::PresentationHint)> = Vec::new();

        for node_id in node_ids {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            let Some(snapshot) = node.engine_param_snapshot() else {
                continue;
            };

            let mut next_presentation = node.node_data().meta.presentation.clone();
            match snapshot.value {
                ParamValue::Reference(reference) if !reference.uuid().is_nil() && !uuid_map.contains_key(&reference.uuid()) => {
                    let detail = reference.cached_name().map(|name| format!("Target '{name}' is missing")).unwrap_or_else(|| format!("Target UUID {} is missing", reference.uuid().0));
                    next_presentation.set_warning(crate::node::NodeWarning {
                        id: MISSING_REFERENCE_WARNING_ID.to_string(),
                        message: "Missing reference".to_string(),
                        detail: Some(detail),
                    });
                }
                _ => {
                    next_presentation.clear_warning(Some(MISSING_REFERENCE_WARNING_ID));
                }
            }

            if next_presentation != node.node_data().meta.presentation {
                pending.push((node_id, next_presentation));
            }
        }

        for (node_id, presentation) in pending.iter() {
            if let Some(node) = self.nodes.get_mut(*node_id) {
                node.node_data_mut().meta.presentation = presentation.clone();
            }

            if emit_events {
                self.emit_event(EventKind::MetaChanged {
                    node: *node_id,
                    patch: crate::node::NodeMetaPatch {
                        presentation: Some(presentation.clone()),
                        ..Default::default()
                    },
                });
            }
        }

        pending.len()
    }

    pub(crate) fn normalize_reference_value_for_param(&self, param_node: NodeId, mut reference: crate::node::NodeReference) -> Result<crate::node::NodeReference, String> {
        if reference.uuid().is_nil() && reference.relative_path_from_root().is_empty() {
            reference.clear_cached_id();
            reference.clear_relative_path_from_root();
            reference.clear_cached_name();
            return Ok(reference);
        }

        let constraints = self.reference_constraints_for_param(param_node);
        let root = self.resolve_reference_root(param_node, &constraints).ok_or_else(|| "reference root could not be resolved".to_string())?;

        let mut resolved = None;
        let mut resolved_but_rejected = false;

        if let Some(candidate) = reference.cached_id() {
            if self.nodes.contains(candidate) {
                if self.reference_candidate_allowed(param_node, root, candidate, &constraints)? {
                    resolved = Some(candidate);
                } else {
                    resolved_but_rejected = true;
                }
            }
        }

        if resolved.is_none() && !reference.uuid().is_nil() {
            if let Some(candidate) = self.node_id_by_uuid(reference.uuid()) {
                if self.reference_candidate_allowed(param_node, root, candidate, &constraints)? {
                    resolved = Some(candidate);
                } else {
                    resolved_but_rejected = true;
                }
            }
        }

        if resolved.is_none() && !reference.relative_path_from_root().is_empty() {
            if let Some(candidate) = self.resolve_relative_decl_path(root, reference.relative_path_from_root()) {
                if self.reference_candidate_allowed(param_node, root, candidate, &constraints)? {
                    resolved = Some(candidate);
                } else {
                    resolved_but_rejected = true;
                }
            }
        }

        let Some(target) = resolved else {
            if resolved_but_rejected {
                return Err("reference target violates constraints".to_string());
            }
            reference.clear_cached_id();
            return Ok(reference);
        };

        reference.set_cached_id(Some(target));
        if let Some(target_node) = self.nodes.get(target) {
            reference.uuid = target_node.node_data().meta.uuid;
            reference.set_cached_name(Some(target_node.node_data().meta.label.clone()));
        }
        if let Some(path) = self.relative_decl_path_from_root(root, target) {
            reference.set_relative_path_from_root(path);
        }

        Ok(reference)
    }

    pub(crate) fn reference_constraints_for_param(&self, param_node: NodeId) -> ReferenceConstraints {
        self.nodes.get(param_node).and_then(|node| node.engine_param_snapshot()).map(|snapshot| snapshot.constraints.reference).unwrap_or_default()
    }

    pub(crate) fn resolve_reference_root(&self, param_node: NodeId, constraints: &ReferenceConstraints) -> Option<NodeId> {
        match &constraints.root {
            ReferenceRoot::EngineRoot => Some(self.root),
            ReferenceRoot::Uuid(uuid) => self.node_id_by_uuid(*uuid),
            ReferenceRoot::RelativeToOwner { path } => {
                let owner = self.nodes.get(param_node).and_then(|node| node.node_data().parent)?;
                self.resolve_relative_decl_path(owner, path)
            }
        }
    }

    fn resolve_relative_decl_path(&self, root: NodeId, path: &[String]) -> Option<NodeId> {
        let mut current = root;
        for segment in path {
            let mut child = self.nodes.get(current).and_then(|node| node.node_data().first_child);
            let mut found = None;

            while let Some(child_id) = child {
                let matches = self.nodes.get(child_id).is_some_and(|node| node.node_data().meta.decl_id.0 == *segment);
                if matches {
                    found = Some(child_id);
                    break;
                }
                child = self.nodes.get(child_id).and_then(|node| node.node_data().next_sibling);
            }

            current = found?;
        }

        Some(current)
    }

    fn relative_decl_path_from_root(&self, root: NodeId, target: NodeId) -> Option<Vec<String>> {
        if root == target {
            return Some(Vec::new());
        }

        let mut current = target;
        let mut reversed = Vec::new();

        loop {
            if current == root {
                reversed.reverse();
                return Some(reversed);
            }

            let node = self.nodes.get(current)?;
            let parent = node.node_data().parent?;
            reversed.push(node.node_data().meta.decl_id.0.clone());
            current = parent;
        }
    }

    fn node_within_root(&self, node: NodeId, root: NodeId) -> bool {
        let mut current = Some(node);
        while let Some(id) = current {
            if id == root {
                return true;
            }
            current = self.nodes.get(id).and_then(|entry| entry.node_data().parent);
        }
        false
    }

    pub(crate) fn reference_candidate_allowed(&self, param_node: NodeId, root: NodeId, candidate: NodeId, constraints: &ReferenceConstraints) -> Result<bool, String> {
        if !self.nodes.contains(candidate) {
            return Ok(false);
        }

        if !self.node_within_root(candidate, root) {
            return Ok(false);
        }

        let Some(candidate_node) = self.nodes.get(candidate) else {
            return Ok(false);
        };
        let candidate_type = candidate_node.get_type();
        let is_parameter = candidate_node.engine_param_snapshot().is_some();

        if matches!(constraints.target_kind, ReferenceTargetKind::ParameterOnly) && !is_parameter {
            return Ok(false);
        }

        if !constraints.allowed_node_types.is_empty() && !constraints.allowed_node_types.iter().any(|allowed| allowed == candidate_type) {
            return Ok(false);
        }

        if !constraints.allowed_parameter_types.is_empty() {
            if !is_parameter {
                return Ok(false);
            }
            if !constraints.allowed_parameter_types.iter().any(|allowed| allowed == candidate_type) {
                return Ok(false);
            }
        }

        if let Some(filter_key) = &constraints.custom_filter_key {
            let Some(filter) = self.reference_filters.get(filter_key) else {
                return Err(format!("custom reference filter '{filter_key}' is not registered"));
            };
            if !filter(self, param_node, root, candidate) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub(crate) fn reference_allowed_targets_for_param(&self, param_node: NodeId) -> Vec<NodeId> {
        let constraints = self.reference_constraints_for_param(param_node);
        let Some(root) = self.resolve_reference_root(param_node, &constraints) else {
            return Vec::new();
        };

        let mut targets = Vec::new();
        for candidate in self.nodes.keys() {
            match self.reference_candidate_allowed(param_node, root, candidate, &constraints) {
                Ok(true) => targets.push(candidate),
                Ok(false) => {}
                Err(_) => return Vec::new(),
            }
        }
        targets.sort_by_key(|node_id| node_id.0);
        targets
    }

    pub(crate) fn reference_visible_nodes_for_param(&self, param_node: NodeId) -> Vec<NodeId> {
        let constraints = self.reference_constraints_for_param(param_node);
        if constraints.custom_filter_key.is_none() {
            return Vec::new();
        }

        let Some(root) = self.resolve_reference_root(param_node, &constraints) else {
            return Vec::new();
        };
        let targets = self.reference_allowed_targets_for_param(param_node);
        let mut visible: HashSet<NodeId> = HashSet::new();
        visible.insert(root);

        for target in targets {
            let mut current = Some(target);
            while let Some(node_id) = current {
                visible.insert(node_id);
                if node_id == root {
                    break;
                }
                current = self.nodes.get(node_id).and_then(|entry| entry.node_data().parent);
            }
        }

        let mut result: Vec<NodeId> = visible.into_iter().collect();
        result.sort_by_key(|node_id| node_id.0);
        result
    }
}
