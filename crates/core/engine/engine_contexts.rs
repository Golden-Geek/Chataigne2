use std::collections::HashSet;

use crate::contexts::{UiUserContextCandidatesDto, UiUserContextEntryDto, UiUserContextScopeDto, UiUserContextsDto, UserContextLookup, UserContextValueType};
use crate::node::{Node, NodeId, USER_CONTEXT_NODE_TYPE};

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
    pub fn upsert_user_context_entry(&mut self, owner: NodeId, symbol: impl Into<String>, param: NodeId) -> Result<bool, String> {
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
    pub fn resolve_user_context_symbol(&mut self, consumer: NodeId, symbol: &str, expected: Option<UserContextValueType>) -> UserContextLookup {
        if !self.nodes.contains(consumer) {
            return UserContextLookup::Missing { symbol: symbol.trim().to_string() };
        }

        self.user_contexts.resolve_symbol(consumer, symbol, expected, |node| self.nodes.get(node).and_then(|entry| entry.node_data().parent))
    }

    /// Returns lexical context candidates for one parameter node.
    pub fn ui_context_candidates_for_param(&self, param: NodeId) -> UiUserContextCandidatesDto {
        let expected = self.expected_user_context_type_for_param(param);
        if !self.nodes.contains(param) {
            return UiUserContextCandidatesDto { param, expected, candidates: Vec::new() };
        }

        let mut candidates = self.user_contexts.collect_candidates(param, expected, |node| self.nodes.get(node).and_then(|entry| entry.node_data().parent));
        candidates.retain(|candidate| candidate.entry_param != param);

        UiUserContextCandidatesDto { param, expected, candidates }
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
                })
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| left.symbol.cmp(&right.symbol).then_with(|| left.param.0.cmp(&right.param.0)));

            scopes.push(UiUserContextScopeDto { owner, generation: scope.generation, entries });
        }

        UiUserContextsDto { scopes }
    }

    pub(crate) fn mark_user_context_graph_changed(&mut self) {
        self.user_contexts.mark_graph_changed();
    }

    pub(crate) fn rebuild_user_context_registry_from_nodes(&mut self) {
        let mut rebuilt = crate::contexts::UserContextRegistry::new();

        let mut scope_nodes = self.nodes.iter().filter_map(|(node_id, node)| (node.get_type() == USER_CONTEXT_NODE_TYPE).then_some(node_id)).collect::<Vec<_>>();
        scope_nodes.sort_by_key(|node_id| node_id.0);

        for scope_node in scope_nodes {
            let scope_owner = self.user_context_scope_owner_for_scope_node(scope_node);
            let _ = rebuilt.ensure_scope(scope_owner);
            self.collect_user_context_scope_entries(scope_node, scope_owner, &mut rebuilt);
        }

        self.user_contexts = rebuilt;
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
            return Err(format!("context entry param {:?} is not a parameter node (type='{}')", param, node.get_type()));
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

    fn collect_user_context_scope_entries(&self, scope_node: NodeId, scope_owner: NodeId, registry: &mut crate::contexts::UserContextRegistry) {
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
