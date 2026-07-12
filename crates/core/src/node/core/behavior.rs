use std::any::Any;

use crate::{
    edit::{Edit, NodeTree},
    engine::NodeExecutionRule,
    events::{CustomEvent, Event, EventFrame, EventKind},
    node::DashboardWidgetTargetDescriptor,
    parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, ParameterControlState, ParameterSnapshot,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
    script::{ScriptHostPolicy, ScriptNodeConfig, ScriptUiState},
};

use super::{
    DeclId, EventPropagation, Folder, NodeData, NodeId, NodeMetaPatch, NodeReference, NodeScriptDescriptor,
    NodeUserPermissions, USER_CONTEXT_DEFAULT_LABEL, USER_CONTEXT_NODE_TYPE, UserContainerRules, UserContextHostPolicy,
    UserCreatableItem, core_node_script_descriptor, lookup_script_child_by_key_and_type,
    parameter_node_type_from_value,
};

/// Creation context for one node instance after attachment, decode, and child reconciliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeCreationContext {
    /// Fresh node inserted by regular runtime editing or blueprint creation.
    Fresh,
    /// Node recreated while loading a persisted project.
    ProjectLoad,
    /// Node recreated while duplicating/copy-pasting a persisted subtree.
    Duplicate,
}

impl NodeCreationContext {
    /// Returns `true` when this creation path restored persisted data.
    pub const fn has_loaded_data(self) -> bool {
        !matches!(self, Self::Fresh)
    }
}

enum InboxDispatchEvent {
    ParamChanged {
        param: NodeId,
        old_value: ParamValue,
    },
    ParamControlChanged {
        param: NodeId,
        old_state: ParameterControlState,
        new_state: ParameterControlState,
    },
    ParamConstraintsChanged {
        param: NodeId,
        old_constraints: ParameterConstraints,
        new_constraints: ParameterConstraints,
    },
    ChildAdded {
        parent: NodeId,
        child: NodeId,
        decl_id: DeclId,
    },
    ChildRemoved {
        parent: NodeId,
        child: NodeId,
    },
    ChildReplaced {
        parent: NodeId,
        old: NodeId,
        new: NodeId,
        decl_id: DeclId,
    },
    ChildMoved {
        child: NodeId,
        old_parent: NodeId,
        new_parent: NodeId,
    },
    ChildReordered {
        parent: NodeId,
        child: NodeId,
    },
    NodeCreated {
        node: NodeId,
    },
    NodeDeleted {
        node: NodeId,
    },
    MetaChanged {
        node: NodeId,
        patch: NodeMetaPatch,
    },
    GraphTransaction,
    Custom(CustomEvent),
}

#[allow(missing_docs)]
/// Behavior contract implemented by all node types.
pub trait Node: Send + Any {
    /// Returns immutable runtime node data.
    fn node_data(&self) -> &NodeData;
    /// Returns mutable runtime node data.
    fn node_data_mut(&mut self) -> &mut NodeData;

    /// Returns the node type identifier.
    fn get_type(&self) -> &str;

    /// Returns the canonical description for this node type when one exists.
    fn type_description(&self) -> Option<&str> {
        None
    }

    /// Returns the item-kind identifier used by container admission rules.
    fn user_item_kind(&self) -> &str {
        self.get_type()
    }

    /// Returns `true` when this node type is declared with `#[item(...)]`.
    fn is_declared_user_item(&self) -> bool {
        false
    }

    /// Returns container admission rules when this node accepts user-curated items.
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        None
    }

    /// Returns scripting host policy metadata for this node type.
    fn script_host_policy(&self) -> Option<ScriptHostPolicy> {
        None
    }

    /// Returns user-context host policy metadata for this node type.
    fn user_context_host_policy(&self) -> Option<UserContextHostPolicy> {
        None
    }

    #[doc(hidden)]
    fn engine_script_state(&self) -> Option<ScriptUiState> {
        None
    }

    #[doc(hidden)]
    fn engine_set_script_config(&mut self, _config: ScriptNodeConfig, _force_reload: bool) -> Result<(), String> {
        Err(format!(
            "node type '{}' does not support script configuration",
            self.get_type()
        ))
    }

    #[doc(hidden)]
    fn engine_request_script_reload(&mut self) -> Result<(), String> {
        Err(format!(
            "node type '{}' does not support script reload",
            self.get_type()
        ))
    }

    #[doc(hidden)]
    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::Value::Null)
    }

    #[doc(hidden)]
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        if data.is_null() {
            return Ok(());
        }

        Err(format!(
            "node type '{}' does not support persisted project data",
            self.get_type()
        ))
    }

    #[doc(hidden)]
    fn project_create(_node_type: &str) -> Option<Self>
    where
        Self: Sized,
    {
        None
    }

    fn user_container_accepts_item(&self, _item_type: &str, item_kind: &str) -> bool {
        self.user_container_rules()
            .is_some_and(|rules| rules.accepts(item_kind))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        Vec::new()
    }

    fn user_creatable_items_with_context(
        &self,
        _snapshot: &ProcessTreeSnapshot,
        _parent: NodeId,
        _child_catalog: &dyn Fn(NodeId) -> Vec<UserCreatableItem>,
    ) -> Vec<UserCreatableItem> {
        self.user_creatable_items()
    }

    fn create_user_item(&self, _node_type: &str) -> Option<Box<dyn Node>> {
        None
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        self.create_user_item(node_type).map(NodeTree::boxed)
    }

    fn create_user_item_for_host(&self, host_node_type: &str, node_type: &str) -> Option<Box<dyn Node>> {
        let _ = host_node_type;
        self.create_user_item(node_type)
    }

    #[doc(hidden)]
    fn engine_set_param_value(&mut self, _value: ParamValue) -> Option<ParamValue> {
        None
    }

    #[doc(hidden)]
    fn engine_prepare_param_value(&self, value: ParamValue) -> Result<ParamValue, String> {
        Ok(value)
    }

    #[doc(hidden)]
    fn engine_param_snapshot(&self) -> Option<ParameterSnapshot> {
        None
    }

    #[doc(hidden)]
    fn engine_dashboard_widget_target_descriptor(&self) -> DashboardWidgetTargetDescriptor {
        DashboardWidgetTargetDescriptor::inspector_only()
    }

    #[doc(hidden)]
    fn engine_param_control_state(&self) -> Option<ParameterControlState> {
        None
    }

    #[doc(hidden)]
    fn engine_set_param_control_state(&mut self, _state: ParameterControlState) -> Result<(), String> {
        Err("node does not expose parameter control state".to_string())
    }

    #[doc(hidden)]
    fn engine_set_param_constraints(&mut self, _constraints: ParameterConstraints) -> Result<(), String> {
        Err("node does not expose parameter constraints".to_string())
    }

    #[doc(hidden)]
    fn engine_restore_param_state(
        &mut self,
        _value: ParamValue,
        _constraints: ParameterConstraints,
    ) -> Result<(), String> {
        Err("node does not expose parameter state".to_string())
    }

    #[doc(hidden)]
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        core_node_script_descriptor(self.node_data(), self.get_type())
    }

    #[doc(hidden)]
    fn engine_set_script_property(
        &mut self,
        ctx: &mut ProcessCtx,
        property: &str,
        value: ParamValue,
    ) -> Result<bool, String> {
        match property {
            "name" | "label" => {
                let Some(label) = value.as_str() else {
                    return Err(format!("property '{property}' expects a string value"));
                };
                ctx.patch_node_meta(
                    self.id(),
                    NodeMetaPatch {
                        label: Some(label),
                        ..Default::default()
                    },
                );
                Ok(true)
            }
            "enabled" => {
                let Some(enabled) = value.as_bool() else {
                    return Err("property 'enabled' expects a boolean value".to_string());
                };
                ctx.patch_node_meta(
                    self.id(),
                    NodeMetaPatch {
                        enabled: Some(enabled),
                        ..Default::default()
                    },
                );
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    #[doc(hidden)]
    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        match method {
            "setName" => {
                let Some(label) = args.first().and_then(ParamValue::as_str) else {
                    return Err("method 'setName' expects one string argument".to_string());
                };
                ctx.patch_node_meta(
                    self.id(),
                    NodeMetaPatch {
                        label: Some(label),
                        ..Default::default()
                    },
                );
                Ok(true)
            }
            "setEnabled" => {
                let Some(enabled) = args.first().and_then(ParamValue::as_bool) else {
                    return Err("method 'setEnabled' expects one boolean argument".to_string());
                };
                ctx.patch_node_meta(
                    self.id(),
                    NodeMetaPatch {
                        enabled: Some(enabled),
                        ..Default::default()
                    },
                );
                Ok(true)
            }
            "setDescription" => {
                let Some(description) = args.first().and_then(ParamValue::as_str) else {
                    return Err("method 'setDescription' expects one string argument".to_string());
                };
                ctx.patch_node_meta(
                    self.id(),
                    NodeMetaPatch {
                        description: Some(Some(description.to_string())),
                        ..Default::default()
                    },
                );
                Ok(true)
            }
            "setReadOnly" => {
                let Some(read_only) = args.first().and_then(ParamValue::as_bool) else {
                    return Err("method 'setReadOnly' expects one boolean argument".to_string());
                };
                ctx.patch_node_meta(
                    self.id(),
                    NodeMetaPatch {
                        user_permissions: Some(NodeUserPermissions {
                            can_edit_name: !read_only,
                            can_remove_and_duplicate: !read_only,
                            can_edit_constraints: !read_only,
                            can_edit_tags: !read_only,
                            can_edit_color: !read_only,
                        }),
                        ..Default::default()
                    },
                );
                Ok(true)
            }
            "removeNode" => {
                ctx.edits.push(Edit::RemoveNode { node: self.id() });
                Ok(true)
            }
            "addFolder" => {
                let label = args
                    .first()
                    .and_then(ParamValue::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "Folder".to_string());
                let lookup = lookup_script_child_by_key_and_type(ctx, self.id(), label.as_str(), "folder");
                for duplicate in lookup.duplicates {
                    ctx.edits.push(Edit::RemoveNode { node: duplicate });
                }

                if let Some(existing_node) = lookup.primary {
                    if lookup.primary_matches_type {
                        if let Some(existing_label) = ctx
                            .tree_snapshot()
                            .and_then(|snapshot| snapshot.node(existing_node))
                            .map(|snapshot| snapshot.label.clone())
                        {
                            if existing_label != label {
                                ctx.patch_node_meta(
                                    existing_node,
                                    NodeMetaPatch {
                                        label: Some(label),
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                        return Ok(true);
                    }

                    ctx.replace_node_boxed(existing_node, Box::new(Folder::new(label)));
                    return Ok(true);
                }

                ctx.add_child_boxed(self.id(), Box::new(Folder::new(label)), None);
                Ok(true)
            }
            "addParameter" => {
                let parameter_id = args
                    .first()
                    .and_then(ParamValue::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "parameter".to_string());
                let default_value = args.get(1).cloned().unwrap_or(ParamValue::Float(0.0));
                let expected_type = parameter_node_type_from_value(&default_value);

                let lookup = lookup_script_child_by_key_and_type(ctx, self.id(), parameter_id.as_str(), expected_type);
                for duplicate in lookup.duplicates {
                    ctx.edits.push(Edit::RemoveNode { node: duplicate });
                }

                if let Some(existing_node) = lookup.primary {
                    let existing_snapshot = ctx
                        .tree_snapshot()
                        .and_then(|snapshot| snapshot.node(existing_node))
                        .map(|snapshot| {
                            (
                                snapshot.is_parameter(),
                                snapshot.node_type.clone(),
                                snapshot.label.clone(),
                                snapshot.param_value.clone(),
                            )
                        });

                    if let Some((is_parameter, node_type, _, _)) = existing_snapshot {
                        if is_parameter && lookup.primary_matches_type && node_type.eq_ignore_ascii_case(expected_type)
                        {
                            return Ok(true);
                        }
                    }

                    let mut parameter =
                        Parameter::new(parameter_id.as_str(), default_value, ParameterChangeCheck::ValueChange);
                    parameter.node_data_mut().meta.decl_id = DeclId(parameter_id);
                    ctx.replace_node_boxed(existing_node, Box::new(parameter));
                    return Ok(true);
                }

                let mut parameter =
                    Parameter::new(parameter_id.as_str(), default_value, ParameterChangeCheck::ValueChange);
                parameter.node_data_mut().meta.decl_id = DeclId(parameter_id);
                ctx.add_child_boxed(self.id(), Box::new(parameter), None);
                Ok(true)
            }
            "removeParameter" => {
                let Some(key) = args.first().and_then(ParamValue::as_str) else {
                    return Err("method 'removeParameter' expects one string argument".to_string());
                };
                let key = key.trim();
                if key.is_empty() {
                    return Err("method 'removeParameter' expects a non-empty parameter key".to_string());
                }
                let Some(snapshot) = ctx.tree_snapshot() else {
                    return Err("method 'removeParameter' is unavailable without tree snapshot".to_string());
                };
                let Some(param_node) = snapshot.find_child(self.id(), key) else {
                    return Err(format!("parameter '{key}' was not found"));
                };
                let Some(param_snapshot) = snapshot.node(param_node) else {
                    return Err(format!("parameter '{key}' was not found"));
                };
                if !param_snapshot.is_parameter() {
                    return Err(format!("child '{key}' is not a parameter"));
                }
                ctx.edits.push(Edit::RemoveNode { node: param_node });
                Ok(true)
            }
            "setParam" => {
                let Some(key) = args.first().and_then(ParamValue::as_str) else {
                    return Err("method 'setParam' expects (name, value) arguments".to_string());
                };
                let value = args
                    .get(1)
                    .cloned()
                    .ok_or_else(|| "method 'setParam' expects (name, value) arguments".to_string())?;
                let key = key.trim();
                if key.is_empty() {
                    return Err("method 'setParam' expects a non-empty parameter key".to_string());
                }
                let Some(snapshot) = ctx.tree_snapshot() else {
                    return Err("method 'setParam' is unavailable without tree snapshot".to_string());
                };
                let Some(param_node) = snapshot.find_child(self.id(), key) else {
                    return Err(format!("parameter '{key}' was not found"));
                };
                let Some(param_snapshot) = snapshot.node(param_node) else {
                    return Err(format!("parameter '{key}' was not found"));
                };
                if !param_snapshot.is_parameter() {
                    return Err(format!("child '{key}' is not a parameter"));
                }
                ctx.set_param(param_node, value);
                Ok(true)
            }
            "addNode" => {
                let node_type = args
                    .first()
                    .and_then(ParamValue::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "folder".to_string());
                let normalized_node_type = node_type.trim().to_ascii_lowercase();
                let resolved_node_type = if normalized_node_type == "context" {
                    USER_CONTEXT_NODE_TYPE
                } else {
                    node_type.as_str()
                };

                let default_label = match normalized_node_type.as_str() {
                    "parameter" | "param" => "parameter".to_string(),
                    "folder" | "" => "Folder".to_string(),
                    "user_context" | "context" => USER_CONTEXT_DEFAULT_LABEL.to_string(),
                    _ => node_type.clone(),
                };
                let label = args
                    .get(1)
                    .and_then(ParamValue::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(default_label);

                if normalized_node_type.is_empty() || normalized_node_type == "folder" {
                    let lookup = lookup_script_child_by_key_and_type(ctx, self.id(), label.as_str(), "folder");
                    for duplicate in lookup.duplicates {
                        ctx.edits.push(Edit::RemoveNode { node: duplicate });
                    }

                    if let Some(existing_node) = lookup.primary {
                        if lookup.primary_matches_type {
                            if let Some(existing_label) = ctx
                                .tree_snapshot()
                                .and_then(|snapshot| snapshot.node(existing_node))
                                .map(|snapshot| snapshot.label.clone())
                            {
                                if existing_label != label {
                                    ctx.patch_node_meta(
                                        existing_node,
                                        NodeMetaPatch {
                                            label: Some(label),
                                            ..Default::default()
                                        },
                                    );
                                }
                            }
                            return Ok(true);
                        }

                        ctx.replace_node_boxed(existing_node, Box::new(Folder::new(label)));
                        return Ok(true);
                    }

                    ctx.add_child_boxed(self.id(), Box::new(Folder::new(label)), None);
                    return Ok(true);
                }

                if normalized_node_type == "parameter" || normalized_node_type == "param" {
                    let default_value = args.get(2).cloned().unwrap_or(ParamValue::Float(0.0));
                    let expected_type = parameter_node_type_from_value(&default_value);
                    let lookup = lookup_script_child_by_key_and_type(ctx, self.id(), label.as_str(), expected_type);
                    for duplicate in lookup.duplicates {
                        ctx.edits.push(Edit::RemoveNode { node: duplicate });
                    }

                    if let Some(existing_node) = lookup.primary {
                        let existing_snapshot = ctx
                            .tree_snapshot()
                            .and_then(|snapshot| snapshot.node(existing_node))
                            .map(|snapshot| {
                                (
                                    snapshot.is_parameter(),
                                    snapshot.label.clone(),
                                    snapshot.param_value.clone(),
                                )
                            });

                        if let Some((is_parameter, existing_label, param_value)) = existing_snapshot {
                            if is_parameter && lookup.primary_matches_type {
                                if existing_label != label {
                                    ctx.patch_node_meta(
                                        existing_node,
                                        NodeMetaPatch {
                                            label: Some(label.clone()),
                                            ..Default::default()
                                        },
                                    );
                                }
                                if param_value.as_ref() != Some(&default_value) {
                                    ctx.set_param(existing_node, default_value);
                                }
                                return Ok(true);
                            }
                        }

                        let mut parameter =
                            Parameter::new(label.as_str(), default_value, ParameterChangeCheck::ValueChange);
                        parameter.node_data_mut().meta.decl_id = DeclId(label.clone());
                        ctx.replace_node_boxed(existing_node, Box::new(parameter));
                        return Ok(true);
                    }

                    let mut parameter =
                        Parameter::new(label.as_str(), default_value, ParameterChangeCheck::ValueChange);
                    parameter.node_data_mut().meta.decl_id = DeclId(label.clone());
                    ctx.add_child_boxed(self.id(), Box::new(parameter), None);
                    return Ok(true);
                }

                let lookup = lookup_script_child_by_key_and_type(ctx, self.id(), label.as_str(), resolved_node_type);
                for duplicate in lookup.duplicates {
                    ctx.edits.push(Edit::RemoveNode { node: duplicate });
                }

                if let Some(existing_node) = lookup.primary {
                    if lookup.primary_matches_type {
                        if let Some(existing_label) = ctx
                            .tree_snapshot()
                            .and_then(|snapshot| snapshot.node(existing_node))
                            .map(|snapshot| snapshot.label.clone())
                        {
                            if existing_label != label {
                                ctx.patch_node_meta(
                                    existing_node,
                                    NodeMetaPatch {
                                        label: Some(label),
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                        return Ok(true);
                    }

                    if let Some(mut node) = self.create_user_item(resolved_node_type) {
                        node.node_data_mut().meta.label = label;
                        ctx.replace_node_boxed(existing_node, node);
                        return Ok(true);
                    }
                } else if let Some(mut node) = self.create_user_item(resolved_node_type) {
                    node.node_data_mut().meta.label = label;
                    ctx.add_user_item_boxed(self.id(), node, None);
                    return Ok(true);
                }

                Err(format!(
                    "method 'addNode' cannot create node type '{node_type}' under '{}'",
                    self.get_type()
                ))
            }
            _ => Ok(false),
        }
    }

    #[doc(hidden)]
    fn engine_visit_references(&self, _visit: &mut dyn FnMut(&NodeReference)) {}

    #[doc(hidden)]
    fn engine_visit_references_mut(&mut self, _visit: &mut dyn FnMut(&mut NodeReference)) {}

    #[doc(hidden)]
    fn engine_sync_param_handle_cache(&mut self, _param: NodeId, _new_value: &ParamValue) {}

    #[doc(hidden)]
    fn engine_on_attached(&mut self, _ctx: &mut ProcessCtx) {}

    #[doc(hidden)]
    fn engine_sync_bound_param_handles(&mut self, _resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {}

    /// Materializes only generated declared-child structure from structural inbox events.
    ///
    /// This engine hook is intentionally separate from [`Self::engine_preprocess_inbox`]
    /// so persistence baselines can expand nested declarations without invoking parameter
    /// callbacks, application inbox handlers, or runtime lifecycle hooks.
    #[doc(hidden)]
    fn engine_materialize_declared_inbox(&mut self, _ctx: &mut ProcessCtx) {}

    #[doc(hidden)]
    fn engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        for event in &ctx.events {
            if let EventKind::ParamChanged { param, new_value, .. } = &event.kind {
                self.engine_sync_param_handle_cache(*param, new_value);
            }
        }
    }

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn cast_ref<N: Node + 'static>(&self) -> Option<&N>
    where
        Self: Sized,
    {
        self.as_any().downcast_ref::<N>()
    }

    fn cast_mut<N: Node + 'static>(&mut self) -> Option<&mut N>
    where
        Self: Sized,
    {
        self.as_any_mut().downcast_mut::<N>()
    }

    fn from_boxed_node(node: Box<dyn Node>) -> Option<Self>
    where
        Self: Sized,
    {
        let any: Box<dyn Any> = node;
        any.downcast::<Self>().ok().map(|node| *node)
    }

    fn id(&self) -> NodeId {
        self.node_data().id
    }

    fn is(&self, id: NodeId) -> bool {
        self.id() == id
    }

    /// Runs once the node has its default or restored data and bound child handles.
    ///
    /// Keep context-independent setup in [`Self::init`]. Use this hook for work that must know
    /// whether the node is fresh or rebuilt from persisted data, such as binding transports only
    /// after load/duplicate restored the relevant parameters.
    fn on_node_ready(&mut self, _ctx: &mut ProcessCtx, _context: NodeCreationContext) {}

    fn init(&mut self, _ctx: &mut ProcessCtx) {}
    fn update(&mut self, _ctx: &mut ProcessCtx) {}
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {}

    /// Returns `false` when [`Self::update`] is guaranteed to be a no-op this tick,
    /// allowing the engine to skip context setup and `engine_sync_bound_param_handles`.
    /// Default is `true` (always update). Override for modules that can determine
    /// cheaply whether they have work to do.
    fn needs_update(&self) -> bool {
        true
    }

    /// Returns `true` when [`Self::update`] requires a tree snapshot in `ProcessCtx`.
    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }

    /// Returns `true` when this node's inbox callbacks require a tree snapshot
    /// for the current event batch.
    fn inbox_requires_tree_snapshot(&self, _events: &EventFrame) -> bool {
        self.engine_param_snapshot().is_none()
    }

    /// Returns `true` when this node's structural lifecycle callbacks
    /// ([`Self::on_attached`], [`Self::init`], [`Self::on_node_ready`]) require a
    /// tree snapshot in `ProcessCtx`.
    ///
    /// Building a whole-tree snapshot is `O(total nodes)`, so eagerly building one
    /// before every single node insertion makes bulk operations (project load,
    /// subtree paste) `O(n^2)`. Leaf parameters never traverse the tree in their
    /// lifecycle hooks, so they opt out by default; container/module nodes that do
    /// walk the tree keep the snapshot. Mirrors [`Self::inbox_requires_tree_snapshot`].
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        self.engine_param_snapshot().is_none()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::default()
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        1
    }
    fn bubble_event_depth(&self, _event: &Event) -> u32 {
        1
    }
    fn event_propagation(&self, _event: &Event, _depth: u32) -> EventPropagation {
        EventPropagation::Notify
    }

    #[doc(hidden)]
    fn engine_child_event_interest_depth(&self, _event: &Event) -> u32 {
        0
    }
    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.dispatch_inbox(ctx);
    }

    fn add_child_boxed(&mut self, ctx: &mut ProcessCtx, child: Box<dyn Node>, after: Option<NodeId>) {
        ctx.add_child_boxed(self.id(), child, after);
    }

    fn add_child<N>(&mut self, ctx: &mut ProcessCtx, child: N, after: Option<NodeId>)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.add_child_boxed(ctx, Box::new(child), after);
    }

    fn add_user_child_boxed(&mut self, ctx: &mut ProcessCtx, child: Box<dyn Node>, after: Option<NodeId>) {
        ctx.add_user_item_boxed(self.id(), child, after);
    }

    fn add_user_child<N>(&mut self, ctx: &mut ProcessCtx, child: N, after: Option<NodeId>)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.add_user_child_boxed(ctx, Box::new(child), after);
    }

    fn remove_child(&mut self, ctx: &mut ProcessCtx, child: NodeId) {
        ctx.edits.push(Edit::RemoveNode { node: child });
    }
    fn move_child(&mut self, ctx: &mut ProcessCtx, child: NodeId, new_parent: NodeId, after: Option<NodeId>) {
        ctx.edits.push(Edit::MoveNode {
            node: child,
            new_parent,
            new_prev_sibling: after,
        });
    }
    fn add_listener(&mut self, ctx: &mut ProcessCtx, target: NodeId) {
        ctx.add_event_listener(self.id(), target);
    }
    fn add_listener_subtree(&mut self, ctx: &mut ProcessCtx, target: NodeId, max_depth: u32) {
        ctx.add_event_listener_subtree(self.id(), target, max_depth);
    }
    fn remove_listener(&mut self, ctx: &mut ProcessCtx, target: NodeId) {
        ctx.remove_event_listener(self.id(), target);
    }
    fn remove_listener_subtree(&mut self, ctx: &mut ProcessCtx, target: NodeId, max_depth: u32) {
        ctx.remove_event_listener_subtree(self.id(), target, max_depth);
    }
    fn replace_child_boxed(&mut self, ctx: &mut ProcessCtx, old: NodeId, new_node: Box<dyn Node>) {
        ctx.replace_node_boxed(old, new_node);
    }
    fn replace_child<N>(&mut self, ctx: &mut ProcessCtx, old: NodeId, new_node: N)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.replace_child_boxed(ctx, old, Box::new(new_node));
    }

    fn patch_meta(&mut self, ctx: &mut ProcessCtx, patch: NodeMetaPatch) {
        ctx.patch_node_meta(self.id(), patch);
    }

    fn patch_meta_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId, patch: NodeMetaPatch) {
        ctx.patch_node_meta(target, patch);
    }

    fn set_warning(&mut self, ctx: &mut ProcessCtx, message: &str) {
        self.set_warning_with(ctx, None, message, None);
    }

    fn set_warning_with(
        &mut self,
        ctx: &mut ProcessCtx,
        warning_id: Option<&str>,
        message: &str,
        detail: Option<&str>,
    ) {
        ctx.set_node_warning_with(self.id(), warning_id, message, detail);
    }

    fn set_warning_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId, message: &str) {
        self.set_warning_for_node_with(ctx, target, None, message, None);
    }

    fn set_warning_for_node_with(
        &mut self,
        ctx: &mut ProcessCtx,
        target: NodeId,
        warning_id: Option<&str>,
        message: &str,
        detail: Option<&str>,
    ) {
        ctx.set_node_warning_with(target, warning_id, message, detail);
    }

    fn clear_warning(&mut self, ctx: &mut ProcessCtx, warning_id: Option<&str>) {
        ctx.clear_node_warning(self.id(), warning_id);
    }

    fn clear_warnings(&mut self, ctx: &mut ProcessCtx) {
        ctx.clear_all_node_warnings(self.id());
    }

    fn clear_warning_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId, warning_id: Option<&str>) {
        ctx.clear_node_warning(target, warning_id);
    }

    fn clear_warnings_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId) {
        ctx.clear_all_node_warnings(target);
    }

    fn set_child_warning_depth(&mut self, ctx: &mut ProcessCtx, max_depth: u32) {
        ctx.set_node_child_warning_depth(self.id(), max_depth);
    }

    fn set_child_warning_depth_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId, max_depth: u32) {
        ctx.set_node_child_warning_depth(target, max_depth);
    }

    fn dispatch_inbox(&mut self, ctx: &mut ProcessCtx) {
        if ctx.has_structure_changed() {
            self.on_structure_changed(ctx);
        }

        for index in 0..ctx.events.len() {
            let event = match &ctx.events[index].kind {
                EventKind::ParamChanged { param, old_value, .. } => InboxDispatchEvent::ParamChanged {
                    param: *param,
                    old_value: old_value.clone(),
                },
                EventKind::ParamControlChanged {
                    param,
                    old_state,
                    new_state,
                } => InboxDispatchEvent::ParamControlChanged {
                    param: *param,
                    old_state: old_state.clone(),
                    new_state: new_state.clone(),
                },
                EventKind::ParamConstraintsChanged {
                    param,
                    old_constraints,
                    new_constraints,
                } => InboxDispatchEvent::ParamConstraintsChanged {
                    param: *param,
                    old_constraints: old_constraints.clone(),
                    new_constraints: new_constraints.clone(),
                },
                EventKind::ChildAdded { parent, child, decl_id } => InboxDispatchEvent::ChildAdded {
                    parent: *parent,
                    child: *child,
                    decl_id: decl_id.clone(),
                },
                EventKind::ChildRemoved { parent, child } => InboxDispatchEvent::ChildRemoved {
                    parent: *parent,
                    child: *child,
                },
                EventKind::ChildReplaced {
                    parent,
                    old,
                    new,
                    decl_id,
                } => InboxDispatchEvent::ChildReplaced {
                    parent: *parent,
                    old: *old,
                    new: *new,
                    decl_id: decl_id.clone(),
                },
                EventKind::ChildMoved {
                    child,
                    old_parent,
                    new_parent,
                } => InboxDispatchEvent::ChildMoved {
                    child: *child,
                    old_parent: *old_parent,
                    new_parent: *new_parent,
                },
                EventKind::ChildReordered { parent, child } => InboxDispatchEvent::ChildReordered {
                    parent: *parent,
                    child: *child,
                },
                EventKind::NodeCreated { node } => InboxDispatchEvent::NodeCreated { node: *node },
                EventKind::NodeDeleted { node } => InboxDispatchEvent::NodeDeleted { node: *node },
                EventKind::MetaChanged { node, patch } => InboxDispatchEvent::MetaChanged {
                    node: *node,
                    patch: patch.clone(),
                },
                EventKind::GraphTransaction { .. } => {
                    // Handled implicitly by on_structure_changed or individual ops if destructured later.
                    InboxDispatchEvent::GraphTransaction
                }
                EventKind::Custom(event) => InboxDispatchEvent::Custom(event.clone()),
            };
            match event {
                InboxDispatchEvent::ParamChanged { param, old_value } => {
                    self.on_param_change(ctx, param, old_value);
                }
                InboxDispatchEvent::ParamControlChanged {
                    param,
                    old_state,
                    new_state,
                } => {
                    self.on_param_control_changed(ctx, param, old_state, new_state);
                }
                InboxDispatchEvent::ParamConstraintsChanged {
                    param,
                    old_constraints,
                    new_constraints,
                } => {
                    self.on_param_constraints_changed(ctx, param, old_constraints, new_constraints);
                }
                InboxDispatchEvent::ChildAdded { parent, child, decl_id } => {
                    self.on_child_added_decl(ctx, parent, child, &decl_id);
                }
                InboxDispatchEvent::ChildRemoved { parent, child } => {
                    self.on_child_removed(ctx, parent, child);
                }
                InboxDispatchEvent::ChildReplaced {
                    parent,
                    old,
                    new,
                    decl_id,
                } => {
                    self.on_child_replaced_decl(ctx, parent, old, new, &decl_id);
                }
                InboxDispatchEvent::ChildMoved {
                    child,
                    old_parent,
                    new_parent,
                } => {
                    self.on_child_moved(ctx, child, old_parent, new_parent);
                }
                InboxDispatchEvent::ChildReordered { parent, child } => {
                    self.on_child_reordered(ctx, parent, child);
                }
                InboxDispatchEvent::NodeCreated { node } => {
                    self.on_node_created(ctx, node);
                }
                InboxDispatchEvent::NodeDeleted { node } => {
                    self.on_node_deleted(ctx, node);
                }
                InboxDispatchEvent::MetaChanged { node, patch } => {
                    self.on_meta_changed(ctx, node, patch);
                }
                InboxDispatchEvent::GraphTransaction => {}
                InboxDispatchEvent::Custom(event) => {
                    self.on_custom_event(ctx, event);
                }
            }
        }
    }

    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, _param: NodeId, _old_value: ParamValue) {}
    fn on_param_control_changed(
        &mut self,
        _ctx: &mut ProcessCtx,
        _param: NodeId,
        _old_state: ParameterControlState,
        _new_state: ParameterControlState,
    ) {
    }
    fn on_param_constraints_changed(
        &mut self,
        _ctx: &mut ProcessCtx,
        _param: NodeId,
        _old_constraints: ParameterConstraints,
        _new_constraints: ParameterConstraints,
    ) {
    }
    fn on_structure_changed(&mut self, _ctx: &mut ProcessCtx) {}
    fn on_child_added(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    fn on_child_added_decl(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId, _decl_id: &DeclId) {
        self.on_child_added(ctx, parent, child);
    }
    fn on_child_removed(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    fn on_child_replaced(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _old: NodeId, _new: NodeId) {}
    fn on_child_replaced_decl(
        &mut self,
        ctx: &mut ProcessCtx,
        parent: NodeId,
        old: NodeId,
        new: NodeId,
        _decl_id: &DeclId,
    ) {
        self.on_child_replaced(ctx, parent, old, new);
    }
    fn on_child_moved(&mut self, _ctx: &mut ProcessCtx, _child: NodeId, _old_parent: NodeId, _new_parent: NodeId) {}
    fn on_child_reordered(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    fn on_node_created(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}
    fn on_node_deleted(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}
    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, _patch: NodeMetaPatch) {}
    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, _enabled: bool) {}
    fn on_custom_event(&mut self, _ctx: &mut ProcessCtx, _event: CustomEvent) {}
}

#[allow(missing_docs)]
/// Adapter used by `#[node(..., via = ...)]` to access either a `NodeData` field directly or a composed node that owns the runtime identity.
pub trait ViaTarget {
    /// Returns the node data backing the via target.
    fn via_node_data(&self) -> &NodeData;
    /// Returns mutable node data backing the via target.
    fn via_node_data_mut(&mut self) -> &mut NodeData;

    fn via_engine_child_event_interest_depth(&self, _event: &Event) -> u32 {
        0
    }

    fn via_engine_on_attached(&mut self, _ctx: &mut ProcessCtx) {}

    fn via_engine_sync_param_handle_cache(&mut self, _param: NodeId, _new_value: &ParamValue) {}

    fn via_engine_sync_bound_param_handles(&mut self, _resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {}

    fn via_engine_materialize_declared_inbox(&mut self, _ctx: &mut ProcessCtx) {}

    fn via_engine_preprocess_inbox(&mut self, _ctx: &mut ProcessCtx) {}

    fn via_script_host_policy(&self) -> Option<ScriptHostPolicy> {
        None
    }

    fn via_user_context_host_policy(&self) -> Option<UserContextHostPolicy> {
        None
    }

    fn via_user_container_rules(&self) -> Option<UserContainerRules> {
        None
    }

    fn via_user_container_accepts_item(&self, _item_type: &str, _item_kind: &str) -> bool {
        false
    }

    fn via_user_creatable_items(&self) -> Vec<UserCreatableItem> {
        Vec::new()
    }

    fn via_create_user_item(&self, _node_type: &str) -> Option<Box<dyn Node>> {
        None
    }

    fn via_create_user_item_for_host(&self, host_node_type: &str, node_type: &str) -> Option<Box<dyn Node>> {
        let _ = host_node_type;
        self.via_create_user_item(node_type)
    }

    fn via_project_encode_data(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::Value::Null)
    }

    fn via_project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        if data.is_null() {
            return Ok(());
        }

        Err("via target does not support persisted project data".to_string())
    }
}

impl ViaTarget for NodeData {
    fn via_node_data(&self) -> &NodeData {
        self
    }

    fn via_node_data_mut(&mut self) -> &mut NodeData {
        self
    }
}

impl<T: Node + ?Sized> ViaTarget for T {
    fn via_node_data(&self) -> &NodeData {
        self.node_data()
    }

    fn via_node_data_mut(&mut self) -> &mut NodeData {
        self.node_data_mut()
    }

    fn via_engine_child_event_interest_depth(&self, event: &Event) -> u32 {
        self.engine_child_event_interest_depth(event)
    }

    fn via_engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        self.engine_on_attached(ctx);
    }

    fn via_engine_sync_param_handle_cache(&mut self, param: NodeId, new_value: &ParamValue) {
        self.engine_sync_param_handle_cache(param, new_value);
    }

    fn via_engine_sync_bound_param_handles(&mut self, resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {
        self.engine_sync_bound_param_handles(resolve);
    }

    fn via_engine_materialize_declared_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.engine_materialize_declared_inbox(ctx);
    }

    fn via_engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.engine_preprocess_inbox(ctx);
    }

    fn via_script_host_policy(&self) -> Option<ScriptHostPolicy> {
        self.script_host_policy()
    }

    fn via_user_context_host_policy(&self) -> Option<UserContextHostPolicy> {
        self.user_context_host_policy()
    }

    fn via_user_container_rules(&self) -> Option<UserContainerRules> {
        self.user_container_rules()
    }

    fn via_user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        self.user_container_accepts_item(item_type, item_kind)
    }

    fn via_user_creatable_items(&self) -> Vec<UserCreatableItem> {
        self.user_creatable_items()
    }

    fn via_create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        self.create_user_item(node_type)
    }

    fn via_create_user_item_for_host(&self, host_node_type: &str, node_type: &str) -> Option<Box<dyn Node>> {
        self.create_user_item_for_host(host_node_type, node_type)
    }

    fn via_project_encode_data(&self) -> Result<serde_json::Value, String> {
        self.project_encode_data()
    }

    fn via_project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        self.project_decode_data(data)
    }
}
