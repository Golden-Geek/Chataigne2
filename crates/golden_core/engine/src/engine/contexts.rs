use std::collections::HashSet;

use crate::contexts::{
    UiUserContextCandidatesDto, UiUserContextEntryDto, UiUserContextScopeDto, UiUserContextsDto, UserContextEntryKind,
    UserContextLookup, UserContextMultiplexList, UserContextMultiplexListEntry, UserContextValueType,
};
use crate::edit::Edit;
use crate::node::{
    Node, NodeId, USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID, USER_CONTEXT_MULTIPLEX_NODE_TYPE, USER_CONTEXT_NODE_TYPE,
    user_context_multiplex_entry_parameter, user_context_multiplex_list_value_type,
};
use crate::parameter::{ParamValue, compatibility_for_values};

use super::Engine;

impl<T: Node> Engine<T> {
    /// Ensures one `UserContext` scope exists for `owner`.
    pub fn ensure_user_context_scope(&mut self, owner: NodeId) -> Result<bool, String> {
        let scope_owner = self.resolve_user_context_scope_owner(owner)?;
        Ok(self.user_contexts.ensure_scope(scope_owner))
    }

    /// Removes one `UserContext` scope by owner node id.
    pub fn remove_user_context_scope(&mut self, owner: NodeId) -> bool {
        let scope_owner = match self.resolve_user_context_scope_owner(owner) {
            Ok(scope_owner) => scope_owner,
            Err(_) => return false,
        };
        self.user_contexts.remove_scope(scope_owner)
    }

    /// Adds or replaces one entry in a `UserContext` scope.
    ///
    /// `param` must be a parameter node inside a direct `UserContextNode` child scope of `owner`.
    pub fn upsert_user_context_entry(
        &mut self,
        owner: NodeId,
        symbol: impl Into<String>,
        param: NodeId,
    ) -> Result<bool, String> {
        let scope_owner = self.resolve_user_context_scope_owner(owner)?;
        if !self.param_within_user_context_owner_scope(param, scope_owner) {
            return Err(format!(
                "context entry param {:?} must be under a direct '{}' child scope of owner {:?}",
                param, USER_CONTEXT_NODE_TYPE, scope_owner
            ));
        }

        let value_type = self.infer_user_context_value_type_for_param(param)?;
        self.user_contexts.upsert_entry(scope_owner, symbol, param, value_type)
    }

    /// Removes one entry from a `UserContext` scope.
    pub fn remove_user_context_entry(&mut self, owner: NodeId, symbol: &str) -> bool {
        let scope_owner = match self.resolve_user_context_scope_owner(owner) {
            Ok(scope_owner) => scope_owner,
            Err(_) => return false,
        };
        self.user_contexts.remove_entry(scope_owner, symbol)
    }

    /// Resolves one symbol lexically from `consumer`.
    pub fn resolve_user_context_symbol(
        &mut self,
        consumer: NodeId,
        symbol: &str,
        expected: Option<UserContextValueType>,
    ) -> UserContextLookup {
        if !self.nodes.contains(consumer) {
            return UserContextLookup::Missing {
                symbol: symbol.trim().to_string(),
            };
        }

        self.user_contexts.resolve_symbol(consumer, symbol, expected, |node| {
            self.nodes.get(node).and_then(|entry| entry.node_data().parent)
        })
    }

    /// Returns lexical context candidates for one parameter node.
    pub fn ui_context_candidates_for_param(&self, param: NodeId) -> UiUserContextCandidatesDto {
        let expected = self.expected_user_context_type_for_param(param);
        if !self.nodes.contains(param) {
            return UiUserContextCandidatesDto {
                param,
                expected,
                candidates: Vec::new(),
            };
        }

        let target_value = self
            .nodes
            .get(param)
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value);
        let multiplex_index_compatible = target_value
            .as_ref()
            .is_some_and(|target| compatibility_for_values(&ParamValue::Int(0), target).is_compatible());
        let mut candidates = self.user_contexts.collect_candidates(param, expected, |node| {
            self.nodes.get(node).and_then(|entry| entry.node_data().parent)
        });
        candidates.retain(|candidate| candidate.entry_param != param);
        for candidate in &mut candidates {
            candidate.multiplex_index_compatible = candidate.multiplex.is_some() && multiplex_index_compatible;
            let Some(target_value) = &target_value else {
                candidate.compatible = false;
                candidate.directly_compatible = false;
                candidate.projections.clear();
                continue;
            };
            let source_param = match candidate.kind {
                UserContextEntryKind::Scalar => candidate.entry_param,
                UserContextEntryKind::MultiplexList => candidate
                    .multiplex
                    .as_ref()
                    .and_then(|list| list.entries.first())
                    .map(|entry| entry.param)
                    .unwrap_or(candidate.entry_param),
            };
            let Some(source_value) = self
                .nodes
                .get(source_param)
                .and_then(|node| node.engine_param_snapshot())
                .map(|snapshot| snapshot.value)
            else {
                candidate.compatible =
                    candidate.kind == UserContextEntryKind::MultiplexList && Some(candidate.value_type) == expected;
                candidate.directly_compatible = candidate.compatible;
                candidate.projections.clear();
                continue;
            };

            let compatibility = compatibility_for_values(&source_value, target_value);
            candidate.compatible = compatibility.is_compatible();
            candidate.directly_compatible = compatibility.direct;
            candidate.projections = compatibility.projections;
        }

        UiUserContextCandidatesDto {
            param,
            expected,
            candidates,
        }
    }

    /// Returns all current `UserContext` scopes for UI editors.
    pub fn ui_user_contexts(&self) -> UiUserContextsDto {
        let mut scopes = Vec::<UiUserContextScopeDto>::new();

        let mut owners = self.user_contexts.scopes().keys().copied().collect::<Vec<_>>();
        owners.sort_by_key(|owner| owner.0);

        for owner in owners {
            let Some(scope) = self.user_contexts.scope(owner) else {
                continue;
            };

            let mut entries = scope
                .entries
                .values()
                .map(|entry| UiUserContextEntryDto {
                    symbol: entry.symbol.clone(),
                    param: entry.param,
                    value_type: entry.value_type,
                    kind: entry.kind,
                    multiplex: entry.multiplex.clone(),
                })
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| {
                left.symbol
                    .cmp(&right.symbol)
                    .then_with(|| left.param.0.cmp(&right.param.0))
            });

            scopes.push(UiUserContextScopeDto {
                owner,
                generation: scope.generation,
                entries,
            });
        }

        UiUserContextsDto { scopes }
    }

    pub(crate) fn mark_user_context_graph_changed(&mut self) {
        self.user_contexts.mark_graph_changed();
    }

    pub(crate) fn rebuild_user_context_registry_from_nodes(&mut self) {
        let mut rebuilt = crate::contexts::UserContextRegistry::new();

        let mut scope_nodes = self
            .nodes
            .iter()
            .filter_map(|(node_id, node)| (node.get_type() == USER_CONTEXT_NODE_TYPE).then_some(node_id))
            .collect::<Vec<_>>();
        scope_nodes.sort_by_key(|node_id| node_id.0);

        for scope_node in scope_nodes {
            let scope_owner = self.user_context_scope_owner_for_scope_node(scope_node);
            let _ = rebuilt.ensure_scope(scope_owner);
            self.collect_user_context_scope_entries(scope_node, scope_owner, &mut rebuilt);
        }

        self.user_contexts = rebuilt;
    }

    pub(crate) fn queue_user_context_multiplex_resize_for_count(&mut self, count_param: NodeId) -> bool {
        let Some(count_node) = self.nodes.get(count_param) else {
            return false;
        };
        if !count_node
            .node_data()
            .meta
            .decl_id
            .0
            .eq_ignore_ascii_case(USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID)
        {
            return false;
        }
        let Some(multiplex_node) = count_node.node_data().parent else {
            return false;
        };
        if self
            .nodes
            .get(multiplex_node)
            .is_none_or(|node| node.get_type() != USER_CONTEXT_MULTIPLEX_NODE_TYPE)
        {
            return false;
        }
        let target_count = match count_node.engine_param_snapshot().map(|snapshot| snapshot.value) {
            Some(ParamValue::Int(value)) => value.max(0) as usize,
            _ => return false,
        };

        let mut changed = false;
        let mut list_child = self
            .nodes
            .get(multiplex_node)
            .and_then(|node| node.node_data().first_child);
        while let Some(list_id) = list_child {
            let Some(list_node) = self.nodes.get(list_id) else {
                break;
            };
            list_child = list_node.node_data().next_sibling;
            let Some(value_type) = user_context_multiplex_list_value_type(list_node.get_type()).map(str::to_string)
            else {
                continue;
            };

            changed |= self.queue_user_context_multiplex_sync_list_to_count(list_id, value_type.as_str(), target_count);
        }
        changed
    }

    pub(crate) fn queue_user_context_multiplex_resize_for_list(&mut self, list_id: NodeId) -> bool {
        let Some(list_node) = self.nodes.get(list_id) else {
            return false;
        };
        let Some(value_type) = user_context_multiplex_list_value_type(list_node.get_type()).map(str::to_string) else {
            return false;
        };
        let Some(multiplex_node) = list_node.node_data().parent else {
            return false;
        };
        if self
            .nodes
            .get(multiplex_node)
            .is_none_or(|node| node.get_type() != USER_CONTEXT_MULTIPLEX_NODE_TYPE)
        {
            return false;
        }
        let Some(target_count) = self.user_context_multiplex_target_count(multiplex_node) else {
            return false;
        };
        self.queue_user_context_multiplex_sync_list_to_count(list_id, value_type.as_str(), target_count)
    }

    fn user_context_multiplex_target_count(&self, multiplex_node: NodeId) -> Option<usize> {
        let mut child = self
            .nodes
            .get(multiplex_node)
            .and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            let Some(child_node) = self.nodes.get(child_id) else {
                break;
            };
            child = child_node.node_data().next_sibling;
            if !child_node
                .node_data()
                .meta
                .decl_id
                .0
                .eq_ignore_ascii_case(USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID)
            {
                continue;
            }
            return match child_node.engine_param_snapshot().map(|snapshot| snapshot.value) {
                Some(ParamValue::Int(value)) => Some(value.max(0) as usize),
                _ => None,
            };
        }
        None
    }

    fn queue_user_context_multiplex_sync_list_to_count(
        &mut self,
        list_id: NodeId,
        value_type: &str,
        target_count: usize,
    ) -> bool {
        let mut entries = Vec::new();
        let mut entry_child = self.nodes.get(list_id).and_then(|node| node.node_data().first_child);
        while let Some(entry_id) = entry_child {
            let Some(entry_node) = self.nodes.get(entry_id) else {
                break;
            };
            entry_child = entry_node.node_data().next_sibling;
            if entry_node.engine_param_snapshot().is_some() && entry_node.get_type().eq_ignore_ascii_case(value_type) {
                entries.push(entry_id);
            }
        }

        let mut changed = false;
        if entries.len() > target_count {
            for entry_id in entries.iter().skip(target_count).rev() {
                self.edits.push(Edit::RemoveNode { node: *entry_id });
                changed = true;
            }
            return changed;
        }

        for _ in entries.len()..target_count {
            let Some(entry) = user_context_multiplex_entry_parameter(value_type) else {
                continue;
            };
            self.edits.push(Edit::AddNode {
                parent: list_id,
                prev_sibling: None,
                node: Box::new(entry),
            });
            changed = true;
        }
        changed
    }

    pub(crate) fn node_within_user_context_scope(&self, node: NodeId) -> bool {
        let mut cursor = Some(node);
        while let Some(current) = cursor {
            let Some(entry) = self.nodes.get(current) else {
                break;
            };
            if entry.get_type() == USER_CONTEXT_NODE_TYPE {
                return true;
            }
            cursor = entry.node_data().parent;
        }

        false
    }

    fn expected_user_context_type_for_param(&self, param: NodeId) -> Option<UserContextValueType> {
        let snapshot = self.nodes.get(param)?.engine_param_snapshot()?;
        Some(UserContextValueType::from_param_value(&snapshot.value))
    }

    fn infer_user_context_value_type_for_param(&self, param: NodeId) -> Result<UserContextValueType, String> {
        let Some(node) = self.nodes.get(param) else {
            return Err(format!("context entry param {:?} was not found", param));
        };
        let Some(snapshot) = node.engine_param_snapshot() else {
            return Err(format!(
                "context entry param {:?} is not a parameter node (type='{}')",
                param,
                node.get_type()
            ));
        };
        Ok(UserContextValueType::from_param_value(&snapshot.value))
    }

    fn resolve_user_context_scope_owner(&self, owner: NodeId) -> Result<NodeId, String> {
        let Some(owner_node) = self.nodes.get(owner) else {
            return Err(format!("context scope owner {:?} was not found", owner));
        };
        if owner_node.get_type() == USER_CONTEXT_NODE_TYPE {
            let scope_owner = self.user_context_scope_owner_for_scope_node(owner);
            if scope_owner == owner {
                return Err(format!("context scope node {:?} has no direct parent owner", owner));
            }
            return Ok(scope_owner);
        }

        Ok(owner)
    }

    fn user_context_scope_owner_for_scope_node(&self, scope_node: NodeId) -> NodeId {
        self.nodes
            .get(scope_node)
            .and_then(|entry| entry.node_data().parent)
            .unwrap_or(scope_node)
    }

    fn param_within_user_context_owner_scope(&self, param: NodeId, scope_owner: NodeId) -> bool {
        let mut cursor = Some(param);
        while let Some(current) = cursor {
            let Some(node) = self.nodes.get(current) else {
                return false;
            };

            if node.get_type() == USER_CONTEXT_NODE_TYPE {
                return self.user_context_scope_owner_for_scope_node(current) == scope_owner;
            }

            cursor = node.node_data().parent;
        }

        false
    }

    fn collect_user_context_scope_entries(
        &self,
        scope_node: NodeId,
        scope_owner: NodeId,
        registry: &mut crate::contexts::UserContextRegistry,
    ) {
        let mut seen_symbols = HashSet::<String>::new();
        let mut stack = Vec::<NodeId>::new();
        self.push_children_reverse(scope_node, &mut stack);

        while let Some(node_id) = stack.pop() {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };

            if node.get_type() == USER_CONTEXT_NODE_TYPE {
                continue;
            }

            if node.get_type() == USER_CONTEXT_MULTIPLEX_NODE_TYPE {
                self.collect_user_context_multiplex_entries(node_id, scope_owner, &mut seen_symbols, registry);
                continue;
            }

            if user_context_multiplex_list_value_type(node.get_type()).is_some() {
                continue;
            }

            if let Some(snapshot) = node.engine_param_snapshot() {
                let symbol = node.node_data().meta.decl_id.0.trim().to_string();
                if !symbol.is_empty() && seen_symbols.insert(symbol.clone()) {
                    let value_type = UserContextValueType::from_param_value(&snapshot.value);
                    let _ = registry.upsert_entry(scope_owner, symbol, node_id, value_type);
                }
            }

            self.push_children_reverse(node_id, &mut stack);
        }
    }

    fn collect_user_context_multiplex_entries(
        &self,
        multiplex_node: NodeId,
        scope_owner: NodeId,
        seen_symbols: &mut HashSet<String>,
        registry: &mut crate::contexts::UserContextRegistry,
    ) {
        let mut child = self
            .nodes
            .get(multiplex_node)
            .and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            let Some(child_node) = self.nodes.get(child_id) else {
                break;
            };
            child = child_node.node_data().next_sibling;

            if child_node
                .node_data()
                .meta
                .decl_id
                .0
                .eq_ignore_ascii_case(USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID)
            {
                continue;
            }

            let Some(value_type_id) = user_context_multiplex_list_value_type(child_node.get_type()) else {
                continue;
            };
            let Some(value_type) = UserContextValueType::from_parameter_node_type(value_type_id) else {
                continue;
            };
            let symbol = child_node.node_data().meta.decl_id.0.trim().to_string();
            if symbol.is_empty() {
                continue;
            }
            let first_with_symbol = seen_symbols.insert(symbol.clone());

            let mut entries = Vec::<UserContextMultiplexListEntry>::new();
            let mut entry_child = child_node.node_data().first_child;
            let mut index = 0u32;
            while let Some(entry_id) = entry_child {
                let Some(entry_node) = self.nodes.get(entry_id) else {
                    break;
                };
                entry_child = entry_node.node_data().next_sibling;
                let Some(snapshot) = entry_node.engine_param_snapshot() else {
                    continue;
                };
                if UserContextValueType::from_param_value(&snapshot.value) != value_type {
                    continue;
                }
                entries.push(UserContextMultiplexListEntry {
                    param: entry_id,
                    item_id: entry_node.node_data().meta.uuid.0.to_string(),
                    index,
                });
                index = index.saturating_add(1);
            }

            let axis_id = self
                .nodes
                .get(multiplex_node)
                .map(|node| node.node_data().meta.uuid.0.to_string())
                .unwrap_or_else(|| multiplex_node.0.to_string());
            let list = UserContextMultiplexList {
                multiplex: multiplex_node,
                list: child_id,
                index_link_symbol: crate::contexts::multiplex_index_context_link_symbol(axis_id.as_str(), false),
                index0_link_symbol: crate::contexts::multiplex_index_context_link_symbol(axis_id.as_str(), true),
                list_link_symbol: crate::contexts::multiplex_list_context_link_symbol(
                    axis_id.as_str(),
                    symbol.as_str(),
                ),
                axis_id,
                value_type,
                entries,
            };
            let _ = if first_with_symbol {
                registry.upsert_multiplex_list_entry(scope_owner, symbol, list)
            } else {
                registry.upsert_additional_multiplex_list_entry(scope_owner, symbol, list)
            };
        }
    }

    fn push_children_reverse(&self, parent: NodeId, stack: &mut Vec<NodeId>) {
        let mut children = Vec::<NodeId>::new();
        let mut child = self.nodes.get(parent).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            children.push(child_id);
            child = self.nodes.get(child_id).and_then(|node| node.node_data().next_sibling);
        }

        for child_id in children.into_iter().rev() {
            stack.push(child_id);
        }
    }
}
