use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::contexts::{UiUserContextsDto, UserContextCandidate, UserContextValueType};
use crate::edit::{Edit, EditOrigin};
use crate::engine::{Engine, EngineTime};
use crate::events::{Event, EventKind};
use crate::logger::LogRecord;
use crate::node::{DeclId, Node, NodeId, NodeMetaPatch, NodeUserPermissions, NodeUuid, PresentationHint, UserCreatableItem, UserNodeRole};
use crate::parameter::{
    ParamValue, ParameterConstraints, ParameterControlDiagnostic, ParameterControlMode, ParameterControlState, ParameterEventBehaviour, ParameterSnapshot,
    ParameterUiHints,
};
use crate::script::{ScriptNodeConfig, ScriptUiConfig, ScriptUiState};

/// Current UI protocol version.
pub const UI_PROTOCOL_VERSION: &str = "0.1.0";
const UI_USER_CONTEXT_SCOPE_TOPIC: &str = "__user_context.scope_changed";
const UI_USER_CONTEXT_ENTRY_TOPIC: &str = "__user_context.entry_changed";

fn is_default_presentation_hint(value: &PresentationHint) -> bool {
    *value == PresentationHint::default()
}

fn is_default_user_permissions(value: &NodeUserPermissions) -> bool {
    *value == NodeUserPermissions::default()
}

/// Scope used by snapshot/event subscriptions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum UiSubscriptionScope {
    /// Full graph scope.
    #[default]
    WholeGraph,
    /// Subtree-limited scope rooted at `root` with `max_depth`.
    Subtree {
        /// Subtree root node id.
        root: NodeId,
        /// Maximum descendant depth (`0` means only root).
        max_depth: u32,
    },
}

/// UI hello handshake.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiHello {
    /// Requested UI protocol version.
    pub protocol_version: String,
    /// Requested subscription scope.
    #[serde(default)]
    pub scope: UiSubscriptionScope,
    /// Optional replay cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<EngineTime>,
}

/// UI-facing node metadata payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiNodeMetaDto {
    /// Generated short name.
    pub short_name: String,
    /// User-visible label.
    pub label: String,
    /// Enabled flag.
    pub enabled: bool,
    /// Whether disable is allowed.
    pub can_be_disabled: bool,
    /// User-edit permissions.
    #[serde(default, skip_serializing_if = "is_default_user_permissions")]
    pub user_permissions: NodeUserPermissions,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Presentation hints, including warnings.
    #[serde(default, skip_serializing_if = "is_default_presentation_hint")]
    pub presentation: PresentationHint,
}

/// UI-facing parameter payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiParamDto {
    /// Current value.
    pub value: ParamValue,
    /// Declared default value.
    pub default_value: ParamValue,
    /// Coalescing policy.
    pub event_behaviour: ParameterEventBehaviour,
    /// Read-only flag.
    pub read_only: bool,
    /// Runtime value constraints.
    pub constraints: ParameterConstraints,
    /// Presentation and editing hints.
    pub ui_hints: ParameterUiHints,
    /// Runtime control-plane state.
    pub control: ParameterControlState,
    /// Engine-computed selectable targets for reference parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_allowed_targets: Vec<NodeId>,
    /// Engine-computed visible tree nodes for reference picker (targets + relevant ancestor paths).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_visible_nodes: Vec<NodeId>,
}

impl From<ParameterSnapshot> for UiParamDto {
    fn from(snapshot: ParameterSnapshot) -> Self {
        Self {
            value: snapshot.value,
            default_value: snapshot.default_value,
            event_behaviour: snapshot.event_behaviour,
            read_only: snapshot.read_only,
            constraints: snapshot.constraints,
            ui_hints: snapshot.ui_hints,
            control: snapshot.control,
            reference_allowed_targets: Vec::new(),
            reference_visible_nodes: Vec::new(),
        }
    }
}

/// UI payload for on-demand reference picker target resolution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct UiReferenceTargetsDto {
    /// Selectable targets for the requested reference parameter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_targets: Vec<NodeId>,
    /// Visible picker nodes (targets + path ancestors).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_nodes: Vec<NodeId>,
}

/// UI-facing control candidate for proxy/binding pickers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiParamCandidateDto {
    /// Candidate parameter node id.
    pub param: NodeId,
    /// Whether candidate value type is compatible.
    pub compatible: bool,
}

/// UI-facing token suggestion for template/expression editors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTokenSuggestionDto {
    /// Suggested token string.
    pub token: String,
}

/// UI-facing per-parameter control info payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiParamControlInfoDto {
    /// Parameter node id.
    pub param: NodeId,
    /// Active control mode.
    pub active_mode: ParameterControlMode,
    /// Supported control modes for this parameter.
    pub available_modes: Vec<ParameterControlMode>,
    /// Current diagnostics for this control state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ParameterControlDiagnostic>,
    /// Lexical user-context candidates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_candidates: Vec<UserContextCandidate>,
    /// Token suggestions for text/expression editors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub token_suggestions: Vec<UiTokenSuggestionDto>,
    /// Candidate proxy targets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proxy_candidates: Vec<UiParamCandidateDto>,
    /// Candidate binding targets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binding_candidates: Vec<UiParamCandidateDto>,
}

/// UI-facing node data summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiNodeDataDto {
    /// Parameter node payload.
    Parameter {
        /// Parameter details.
        param: UiParamDto,
    },
    /// Non-parameter node summary.
    Node {
        /// Runtime type identifier.
        node_type: String,
    },
}

/// UI-facing node DTO.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiNodeDto {
    /// Runtime node id.
    pub node_id: NodeId,
    /// Stable persistent uuid.
    pub uuid: NodeUuid,
    /// Declared id for this node in its parent scope.
    pub decl_id: DeclId,
    /// Runtime node type identifier.
    pub node_type: String,
    /// User-facing metadata.
    pub meta: UiNodeMetaDto,
    /// Node payload summary.
    pub data: UiNodeDataDto,
    /// Runtime role for user curation semantics.
    pub user_role: UserNodeRole,
    /// Logical item kind used by container admission.
    pub user_item_kind: String,
    /// Accepted item kinds when this node acts as a container.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_user_item_kinds: Vec<String>,
    /// User-creatable item node types for this container instance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub creatable_user_items: Vec<UiCreatableUserItemDto>,
    /// Direct children ids in visual order.
    pub children: Vec<NodeId>,
}

/// UI-facing descriptor of a user-creatable item type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCreatableUserItemDto {
    /// Runtime node type identifier.
    pub node_type: String,
    /// Logical user-item kind.
    pub item_kind: String,
    /// Suggested default label.
    pub label: String,
}

impl From<UserCreatableItem> for UiCreatableUserItemDto {
    fn from(item: UserCreatableItem) -> Self {
        Self {
            node_type: item.node_type,
            item_kind: item.item_kind,
            label: item.label,
        }
    }
}

/// UI-facing node-type descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiNodeTypeDescriptor {
    /// Runtime node type identifier.
    pub node_type: String,
}

/// UI-facing enum descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiEnumDefinition {
    /// Stable enum id.
    pub enum_id: String,
    /// Enum variant definitions.
    pub variants: Vec<UiEnumVariantDefinition>,
}

/// UI-facing enum variant descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiEnumVariantDefinition {
    /// Stable variant id.
    pub variant_id: String,
    /// Display label.
    pub label: String,
    /// Optional tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Optional ordering key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering: Option<i32>,
}

/// UI-facing schema payload needed by editors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UiSchemaView {
    /// Known node types within the snapshot scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_types: Vec<UiNodeTypeDescriptor>,
    /// Enum definitions used by UI editors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enums: Vec<UiEnumDefinition>,
}

/// UI-facing history status payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UiHistoryState {
    /// Whether undo is currently possible.
    pub can_undo: bool,
    /// Whether redo is currently possible.
    pub can_redo: bool,
    /// Number of undo transactions available.
    pub undo_len: usize,
    /// Number of redo transactions available.
    pub redo_len: usize,
    /// Whether an edit session is currently active.
    pub active_edit_session: bool,
}

/// UI-facing logger state included in snapshots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct UiLoggerState {
    /// Maximum number of logger records retained server-side.
    pub max_entries: usize,
    /// Retained records in ascending record-id order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<LogRecord>,
}

/// Snapshot payload for initial sync.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiSnapshot {
    /// Protocol version.
    pub protocol_version: String,
    /// Snapshot scope.
    pub scope: UiSubscriptionScope,
    /// Engine time when snapshot was produced.
    pub at: EngineTime,
    /// Nodes included in this snapshot.
    pub nodes: Vec<UiNodeDto>,
    /// Schema fragments required by editors.
    pub schema: UiSchemaView,
    /// Current undo/redo state.
    pub history: UiHistoryState,
    /// Current logger state.
    pub logger: UiLoggerState,
    /// Current user-context scopes.
    #[serde(default, skip_serializing_if = "is_default_user_contexts")]
    pub user_contexts: UiUserContextsDto,
}

fn is_default_user_contexts(value: &UiUserContextsDto) -> bool {
    *value == UiUserContextsDto::default()
}

/// UI-facing event kind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiEventKind {
    /// Parameter changed.
    ParamChanged {
        /// Parameter node id.
        param: NodeId,
        /// Previous value.
        old_value: ParamValue,
        /// New value.
        new_value: ParamValue,
    },
    /// Parameter control state changed.
    ParamControlChanged {
        /// Parameter node id.
        param: NodeId,
        /// Previous control state.
        old_state: ParameterControlState,
        /// New control state.
        new_state: ParameterControlState,
    },
    /// Child added.
    ChildAdded {
        /// Parent id.
        parent: NodeId,
        /// Child id.
        child: NodeId,
        /// Declared slot id.
        decl_id: DeclId,
    },
    /// Child removed.
    ChildRemoved {
        /// Parent id.
        parent: NodeId,
        /// Child id.
        child: NodeId,
    },
    /// Child replaced.
    ChildReplaced {
        /// Parent id.
        parent: NodeId,
        /// Old child id.
        old: NodeId,
        /// New child id.
        new: NodeId,
        /// Declared slot id.
        decl_id: DeclId,
    },
    /// Child moved.
    ChildMoved {
        /// Child id.
        child: NodeId,
        /// Previous parent id.
        old_parent: NodeId,
        /// New parent id.
        new_parent: NodeId,
    },
    /// Child reordered.
    ChildReordered {
        /// Parent id.
        parent: NodeId,
        /// Child id.
        child: NodeId,
    },
    /// Node created.
    NodeCreated {
        /// Node id.
        node: NodeId,
    },
    /// Node deleted.
    NodeDeleted {
        /// Node id.
        node: NodeId,
    },
    /// Metadata changed.
    MetaChanged {
        /// Node id.
        node: NodeId,
        /// Applied patch.
        patch: NodeMetaPatch,
    },
    /// Custom event payload.
    Custom {
        /// Topic.
        topic: String,
        /// Origin node when known.
        origin: Option<NodeId>,
        /// Raw JSON payload.
        payload: serde_json::Value,
    },
}

/// UI-facing event payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiEventDto {
    /// Event time.
    pub time: EngineTime,
    /// Event payload.
    #[serde(flatten)]
    pub kind: UiEventKind,
}

impl From<Event> for UiEventDto {
    fn from(event: Event) -> Self {
        let kind = match event.kind {
            EventKind::ParamChanged { param, old_value, new_value } => UiEventKind::ParamChanged { param, old_value, new_value },
            EventKind::ParamControlChanged { param, old_state, new_state } => UiEventKind::ParamControlChanged { param, old_state, new_state },
            EventKind::ChildAdded { parent, child, decl_id } => UiEventKind::ChildAdded { parent, child, decl_id },
            EventKind::ChildRemoved { parent, child } => UiEventKind::ChildRemoved { parent, child },
            EventKind::ChildReplaced { parent, old, new, decl_id } => UiEventKind::ChildReplaced { parent, old, new, decl_id },
            EventKind::ChildMoved { child, old_parent, new_parent } => UiEventKind::ChildMoved { child, old_parent, new_parent },
            EventKind::ChildReordered { parent, child } => UiEventKind::ChildReordered { parent, child },
            EventKind::NodeCreated { node } => UiEventKind::NodeCreated { node },
            EventKind::NodeDeleted { node } => UiEventKind::NodeDeleted { node },
            EventKind::MetaChanged { node, patch } => UiEventKind::MetaChanged { node, patch },
            EventKind::Custom(custom) => UiEventKind::Custom {
                topic: custom.topic,
                origin: custom.origin,
                payload: custom.payload,
            },
        };

        Self { time: event.time, kind }
    }
}

/// Event replay batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiEventBatch {
    /// Replay cursor used by the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<EngineTime>,
    /// Last event timestamp included in this batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<EngineTime>,
    /// Delivered events.
    pub events: Vec<UiEventDto>,
}

/// UI-originated edit intent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiEditIntent {
    /// Begin a grouped edit session.
    BeginEdit {
        /// Client-generated id.
        client_edit_id: String,
        /// Optional label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    /// End a grouped edit session.
    EndEdit {
        /// Client-generated id.
        client_edit_id: String,
    },
    /// Set a parameter value.
    SetParam {
        /// Target node id.
        node: NodeId,
        /// New value.
        value: ParamValue,
        /// Requested coalescing behavior.
        behaviour: ParameterEventBehaviour,
    },
    /// Set a parameter control state.
    SetParamControlState {
        /// Target parameter node id.
        node: NodeId,
        /// New control state payload.
        state: ParameterControlState,
    },
    /// Move a node.
    MoveNode {
        /// Target node id.
        node: NodeId,
        /// New parent id.
        new_parent: NodeId,
        /// Optional previous sibling under the new parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_prev_sibling: Option<NodeId>,
    },
    /// Remove a node.
    RemoveNode {
        /// Target node id.
        node: NodeId,
    },
    /// Creates a user item under `parent` from a node type id.
    CreateUserItem {
        /// Parent node id.
        parent: NodeId,
        /// Runtime node type identifier to instantiate.
        node_type: String,
        /// Optional explicit label for the new item.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    /// Patch node metadata.
    PatchMeta {
        /// Target node id.
        node: NodeId,
        /// Metadata patch.
        patch: NodeMetaPatch,
    },
    /// Ensures one user-context scope exists on `owner`.
    EnsureUserContextScope {
        /// Scope owner node id.
        owner: NodeId,
    },
    /// Removes the user-context scope from `owner`.
    RemoveUserContextScope {
        /// Scope owner node id.
        owner: NodeId,
    },
    /// Adds or replaces one user-context entry.
    UpsertUserContextEntry {
        /// Scope owner node id.
        owner: NodeId,
        /// Symbol name.
        symbol: String,
        /// Parameter node backing this entry.
        param: NodeId,
    },
    /// Removes one user-context entry by symbol.
    RemoveUserContextEntry {
        /// Scope owner node id.
        owner: NodeId,
        /// Symbol to remove.
        symbol: String,
    },
    /// Request graph reevaluation.
    ReevaluateGraph,
    /// Clears retained logger records.
    ClearLogs,
    /// Sets logger retention capacity.
    SetLogMaxEntries {
        /// Requested maximum number of retained records.
        max_entries: usize,
    },
    /// Undo the last history transaction.
    Undo,
    /// Redo the last undone history transaction.
    Redo,
}

/// Ack status for a UI edit intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiAckStatus {
    /// Accepted and applied now.
    Applied,
    /// Accepted but staged for later application.
    Staged,
    /// Rejected.
    Rejected,
}

/// Acknowledgement payload for UI edit intents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiAck {
    /// Success flag.
    pub success: bool,
    /// Ack status.
    pub status: UiAckStatus,
    /// Optional error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Optional error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Optional earliest resulting event timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earliest_event_time: Option<EngineTime>,
    /// Current undo/redo state after applying the intent.
    pub history: UiHistoryState,
}

impl<T: Node> Engine<T> {
    /// Builds a UI snapshot for the requested scope.
    pub fn ui_snapshot(&self, scope: UiSubscriptionScope) -> UiSnapshot {
        let node_ids = self.collect_scope_nodes(scope.clone());
        let visible: HashSet<NodeId> = node_ids.iter().copied().collect();
        let mut nodes = Vec::with_capacity(node_ids.len());
        let mut known_node_types = HashSet::new();

        for node_id in node_ids {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            let node_data = node.node_data();
            known_node_types.insert(node.get_type().to_string());

            let mut children = Vec::new();
            let mut child = node_data.first_child;
            while let Some(child_id) = child {
                if visible.contains(&child_id) {
                    children.push(child_id);
                }
                child = self.nodes.get(child_id).and_then(|next| next.node_data().next_sibling);
            }

            let data = if let Some(param) = node.engine_param_snapshot() {
                UiNodeDataDto::Parameter { param: UiParamDto::from(param) }
            } else {
                UiNodeDataDto::Node { node_type: node.get_type().to_string() }
            };

            let mut creatable_user_items = Vec::new();
            for item in self.catalog_creatable_items(node_id).into_iter() {
                creatable_user_items.push(UiCreatableUserItemDto::from(item));
            }

            let mut accepted_user_item_kinds: Vec<String> = node.user_container_rules().map(|rules| rules.accepts_item_kinds.iter().map(|kind| (*kind).to_string()).collect()).unwrap_or_default();
            if node.script_host_policy().is_some_and(|policy| policy.enabled) && !accepted_user_item_kinds.iter().any(|kind| kind == "script") {
                accepted_user_item_kinds.push("script".to_string());
            }

            nodes.push(UiNodeDto {
                node_id,
                uuid: node_data.meta.uuid,
                decl_id: node_data.meta.decl_id.clone(),
                node_type: node.get_type().to_string(),
                meta: UiNodeMetaDto {
                    short_name: node_data.meta.short_name.clone(),
                    label: node_data.meta.label.clone(),
                    enabled: node_data.meta.enabled,
                    can_be_disabled: node_data.meta.can_be_disabled,
                    user_permissions: node_data.meta.user_permissions.clone(),
                    description: node_data.meta.description.clone(),
                    tags: node_data.meta.tags.clone(),
                    presentation: node_data.meta.presentation.clone(),
                },
                data,
                user_role: node_data.user_role,
                user_item_kind: node.user_item_kind().to_string(),
                accepted_user_item_kinds,
                creatable_user_items,
                children,
            });
        }

        let mut node_types: Vec<UiNodeTypeDescriptor> = known_node_types.into_iter().map(|node_type| UiNodeTypeDescriptor { node_type }).collect();
        node_types.sort_by(|left, right| left.node_type.cmp(&right.node_type));

        UiSnapshot {
            protocol_version: UI_PROTOCOL_VERSION.to_string(),
            scope,
            at: self.time,
            nodes,
            schema: UiSchemaView { node_types, enums: Vec::new() },
            history: self.ui_history_state(),
            logger: UiLoggerState {
                max_entries: crate::logger::max_entries(),
                records: crate::logger::records(),
            },
            user_contexts: self.ui_user_contexts(),
        }
    }

    /// Returns a scoped UI event replay batch starting strictly after `from`.
    pub fn ui_event_batch(&self, from: Option<EngineTime>, scope: UiSubscriptionScope) -> UiEventBatch {
        let start_index = self.ui_event_log_start_index(from);
        let mut events = Vec::<UiEventDto>::new();
        let source_events = &self.ui_event_log()[start_index..];
        events.reserve(source_events.len());

        for event in source_events {
            if self.event_matches_scope(&scope, event) {
                events.push(UiEventDto::from(event.clone()));
            }
        }

        let to = events.last().map(|event| event.time);

        UiEventBatch { from, to, events }
    }

    /// Resolves custom-filter reference picker targets for one parameter node.
    ///
    /// Returns empty vectors when the parameter has no custom filter or cannot be resolved.
    pub fn ui_reference_targets_for_param(&self, param_node: NodeId) -> UiReferenceTargetsDto {
        let constraints = self.reference_constraints_for_param(param_node);
        if constraints.custom_filter_key.is_none() {
            return UiReferenceTargetsDto::default();
        }

        UiReferenceTargetsDto {
            allowed_targets: self.reference_allowed_targets_for_param(param_node),
            visible_nodes: self.reference_visible_nodes_for_param(param_node),
        }
    }

    /// Returns control-related UI information for one parameter node.
    pub fn ui_param_control_info(&self, param_node: NodeId) -> Result<UiParamControlInfoDto, String> {
        let Some(node) = self.nodes.get(param_node) else {
            return Err(format!("node {} not found", param_node.0));
        };
        let Some(snapshot) = node.engine_param_snapshot() else {
            return Err(format!("node {} is not a parameter node", param_node.0));
        };

        let context_candidates = self.ui_context_candidates_for_param(param_node).candidates;

        let mut token_set = HashSet::<String>::new();
        token_set.insert("$name".to_string());
        token_set.insert("$type".to_string());
        token_set.insert("$id".to_string());
        token_set.insert("$uuid".to_string());
        for candidate in &context_candidates {
            token_set.insert(candidate.symbol.clone());
            if candidate.value_type == UserContextValueType::Reference {
                token_set.insert(format!("{}.$name", candidate.symbol));
                token_set.insert(format!("{}.$type", candidate.symbol));
                token_set.insert(format!("{}.$id", candidate.symbol));
                token_set.insert(format!("{}.$uuid", candidate.symbol));
            }
        }
        let mut token_suggestions = token_set
            .into_iter()
            .map(|token| UiTokenSuggestionDto { token })
            .collect::<Vec<_>>();
        token_suggestions.sort_by(|left, right| left.token.cmp(&right.token));

        let mut proxy_candidates = Vec::<UiParamCandidateDto>::new();
        let mut binding_candidates = Vec::<UiParamCandidateDto>::new();
        for (candidate_id, candidate_node) in self.nodes.iter() {
            if candidate_id == param_node {
                continue;
            }
            let Some(candidate_snapshot) = candidate_node.engine_param_snapshot() else {
                continue;
            };
            let compatible = param_types_compatible(&candidate_snapshot.value, &snapshot.value);
            let candidate = UiParamCandidateDto {
                param: candidate_id,
                compatible,
            };
            proxy_candidates.push(candidate.clone());
            binding_candidates.push(candidate);
        }
        proxy_candidates.sort_by_key(|candidate| candidate.param.0);
        binding_candidates.sort_by_key(|candidate| candidate.param.0);

        Ok(UiParamControlInfoDto {
            param: param_node,
            active_mode: snapshot.control.mode,
            available_modes: available_control_modes_for_value(&snapshot.value),
            diagnostics: snapshot.control.diagnostics,
            context_candidates,
            token_suggestions,
            proxy_candidates,
            binding_candidates,
        })
    }

    /// Returns script runtime state for `node` when it is a script node.
    pub fn ui_script_state(&self, node: NodeId) -> Result<ScriptUiState, String> {
        let Some(target) = self.nodes.get(node) else {
            return Err(format!("node {} not found", node.0));
        };

        target.engine_script_state().ok_or_else(|| format!("node {} does not expose script runtime state", node.0))
    }

    /// Replaces script configuration for `node`.
    pub fn ui_set_script_config(&mut self, node: NodeId, config: ScriptUiConfig, force_reload: bool) -> Result<(), String> {
        self.edits.push(Edit::SetScriptConfig {
            node,
            config: ScriptNodeConfig::from(config),
            force_reload,
        });
        self.apply_edits().map_err(|err| err.to_string())
    }

    /// Requests runtime reload for script node `node`.
    pub fn ui_reload_script(&mut self, node: NodeId) -> Result<(), String> {
        {
            let Some(target) = self.nodes.get_mut(node) else {
                return Err(format!("node {} not found", node.0));
            };

            target.engine_request_script_reload()?;
        }

        self.push_ui_custom_event(
            "__transport.resync_required",
            Some(node),
            serde_json::json!({
                "reason": "script_reload_requested",
                "node": node.0
            }),
        );

        Ok(())
    }

    /// Applies one UI edit intent and returns an acknowledgement payload.
    pub fn apply_ui_intent(&mut self, intent: UiEditIntent) -> UiAck {
        let before_len = self.ui_event_log().len();
        // eprintln!("[gc-ui] intent recv: {intent:?} | undo_len={} redo_len={} active_session={}", self.undo_len(), self.redo_len(), self.has_active_edit_session());

        let ack = match intent {
            UiEditIntent::BeginEdit { client_edit_id, label } => {
                self.edits.push(Edit::BeginEditSession { origin: EditOrigin::Ui, label, client_edit_id });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::EndEdit { client_edit_id } => {
                self.edits.push(Edit::EndEditSession { client_edit_id });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::SetParam { node, value, behaviour } => {
                self.edits.push(Edit::SetParam { node, value, behaviour });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::SetParamControlState { node, state } => match self.set_param_control_state(node, state) {
                Ok(changed) => {
                    if changed {
                        self.evaluate_parameter_controls();
                    }
                    let result = if self.edits.pending.is_empty() {
                        Ok(())
                    } else {
                        self.apply_edits_without_history()
                    };
                    self.finish_ui_apply_now(before_len, result)
                }
                Err(message) => UiAck {
                    success: false,
                    status: UiAckStatus::Rejected,
                    error_code: Some("param_control_error".to_string()),
                    error_message: Some(message),
                    earliest_event_time: None,
                    history: self.ui_history_state(),
                },
            },
            UiEditIntent::MoveNode { node, new_parent, new_prev_sibling } => {
                self.edits.push(Edit::MoveNode { node, new_parent, new_prev_sibling });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::RemoveNode { node } => {
                self.edits.push(Edit::RemoveNode { node });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::CreateUserItem { parent, node_type, label } => {
                if let Err(err) = self.queue_catalog_create(parent, node_type, label, None) {
                    return self.finish_ui_apply_now(before_len, Err(err));
                }
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::PatchMeta { node, patch } => {
                self.edits.push(Edit::PatchMeta { node, patch });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::EnsureUserContextScope { owner } => match self.ensure_user_context_scope(owner) {
                Ok(changed) => {
                    if changed {
                        self.push_ui_custom_event(
                            UI_USER_CONTEXT_SCOPE_TOPIC,
                            Some(owner),
                            serde_json::json!({
                                "action": "ensure_scope",
                                "owner": owner.0,
                            }),
                        );
                    }
                    UiAck {
                        success: true,
                        status: UiAckStatus::Applied,
                        error_code: None,
                        error_message: None,
                        earliest_event_time: self.ui_event_log().get(before_len).map(|event| event.time),
                        history: self.ui_history_state(),
                    }
                }
                Err(message) => UiAck {
                    success: false,
                    status: UiAckStatus::Rejected,
                    error_code: Some("user_context_scope_error".to_string()),
                    error_message: Some(message),
                    earliest_event_time: None,
                    history: self.ui_history_state(),
                },
            },
            UiEditIntent::RemoveUserContextScope { owner } => {
                let removed = self.remove_user_context_scope(owner);
                if removed {
                    self.push_ui_custom_event(
                        UI_USER_CONTEXT_SCOPE_TOPIC,
                        Some(owner),
                        serde_json::json!({
                            "action": "remove_scope",
                            "owner": owner.0,
                        }),
                    );
                }
                UiAck {
                    success: true,
                    status: UiAckStatus::Applied,
                    error_code: None,
                    error_message: None,
                    earliest_event_time: self.ui_event_log().get(before_len).map(|event| event.time),
                    history: self.ui_history_state(),
                }
            }
            UiEditIntent::UpsertUserContextEntry { owner, symbol, param } => match self.upsert_user_context_entry(owner, symbol.as_str(), param) {
                Ok(changed) => {
                    if changed {
                        self.push_ui_custom_event(
                            UI_USER_CONTEXT_ENTRY_TOPIC,
                            Some(owner),
                            serde_json::json!({
                                "action": "upsert_entry",
                                "owner": owner.0,
                                "symbol": symbol,
                                "param": param.0,
                            }),
                        );
                    }
                    UiAck {
                        success: true,
                        status: UiAckStatus::Applied,
                        error_code: None,
                        error_message: None,
                        earliest_event_time: self.ui_event_log().get(before_len).map(|event| event.time),
                        history: self.ui_history_state(),
                    }
                }
                Err(message) => UiAck {
                    success: false,
                    status: UiAckStatus::Rejected,
                    error_code: Some("user_context_entry_error".to_string()),
                    error_message: Some(message),
                    earliest_event_time: None,
                    history: self.ui_history_state(),
                },
            },
            UiEditIntent::RemoveUserContextEntry { owner, symbol } => {
                let removed = self.remove_user_context_entry(owner, symbol.as_str());
                if removed {
                    self.push_ui_custom_event(
                        UI_USER_CONTEXT_ENTRY_TOPIC,
                        Some(owner),
                        serde_json::json!({
                            "action": "remove_entry",
                            "owner": owner.0,
                            "symbol": symbol,
                        }),
                    );
                }
                UiAck {
                    success: true,
                    status: UiAckStatus::Applied,
                    error_code: None,
                    error_message: None,
                    earliest_event_time: self.ui_event_log().get(before_len).map(|event| event.time),
                    history: self.ui_history_state(),
                }
            }
            UiEditIntent::ReevaluateGraph => {
                self.edits.push(Edit::ReevaluateGraph);
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::ClearLogs => {
                crate::logger::clear();
                self.push_ui_custom_event(crate::logger::UI_LOG_CLEARED_TOPIC, None, serde_json::json!({}));
                UiAck {
                    success: true,
                    status: UiAckStatus::Applied,
                    error_code: None,
                    error_message: None,
                    earliest_event_time: self.ui_event_log().get(before_len).map(|event| event.time),
                    history: self.ui_history_state(),
                }
            }
            UiEditIntent::SetLogMaxEntries { max_entries } => {
                let applied_max_entries = crate::logger::set_max_entries(max_entries);
                self.push_ui_custom_event(crate::logger::UI_LOG_MAX_ENTRIES_TOPIC, None, serde_json::json!({ "max_entries": applied_max_entries }));
                UiAck {
                    success: true,
                    status: UiAckStatus::Applied,
                    error_code: None,
                    error_message: None,
                    earliest_event_time: self.ui_event_log().get(before_len).map(|event| event.time),
                    history: self.ui_history_state(),
                }
            }
            UiEditIntent::Undo => match self.undo() {
                Ok(_) => UiAck {
                    success: true,
                    status: UiAckStatus::Applied,
                    error_code: None,
                    error_message: None,
                    earliest_event_time: self.ui_event_log().get(before_len).map(|event| event.time),
                    history: self.ui_history_state(),
                },
                Err(err) => UiAck {
                    success: false,
                    status: UiAckStatus::Rejected,
                    error_code: Some(ui_error_code(&err).to_string()),
                    error_message: Some(err.to_string()),
                    earliest_event_time: None,
                    history: self.ui_history_state(),
                },
            },
            UiEditIntent::Redo => match self.redo() {
                Ok(_) => UiAck {
                    success: true,
                    status: UiAckStatus::Applied,
                    error_code: None,
                    error_message: None,
                    earliest_event_time: self.ui_event_log().get(before_len).map(|event| event.time),
                    history: self.ui_history_state(),
                },
                Err(err) => UiAck {
                    success: false,
                    status: UiAckStatus::Rejected,
                    error_code: Some(ui_error_code(&err).to_string()),
                    error_message: Some(err.to_string()),
                    earliest_event_time: None,
                    history: self.ui_history_state(),
                },
            },
        };

        // eprintln!(
        //     "[gc-ui] intent ack: success={} status={:?} code={:?} earliest={:?} history={{undo_len:{}, redo_len:{}, can_undo:{}, can_redo:{}, active_session:{}}}",
        //     ack.success, ack.status, ack.error_code, ack.earliest_event_time, ack.history.undo_len, ack.history.redo_len, ack.history.can_undo, ack.history.can_redo, ack.history.active_edit_session
        // );

        ack
    }

    fn finish_ui_apply_now(&self, before_len: usize, result: Result<(), crate::engine::EngineEditError>) -> UiAck {
        match result {
            Ok(()) => UiAck {
                success: true,
                status: UiAckStatus::Applied,
                error_code: None,
                error_message: None,
                earliest_event_time: self.ui_event_log().get(before_len).map(|event| event.time),
                history: self.ui_history_state(),
            },
            Err(err) => UiAck {
                success: false,
                status: UiAckStatus::Rejected,
                error_code: Some(ui_error_code(&err).to_string()),
                error_message: Some(err.to_string()),
                earliest_event_time: None,
                history: self.ui_history_state(),
            },
        }
    }

    fn ui_history_state(&self) -> UiHistoryState {
        UiHistoryState {
            can_undo: self.undo_len() > 0,
            can_redo: self.redo_len() > 0,
            undo_len: self.undo_len(),
            redo_len: self.redo_len(),
            active_edit_session: self.has_active_edit_session(),
        }
    }

    fn collect_scope_nodes(&self, scope: UiSubscriptionScope) -> Vec<NodeId> {
        match scope {
            UiSubscriptionScope::WholeGraph => self.collect_subtree_nodes(self.root, u32::MAX),
            UiSubscriptionScope::Subtree { root, max_depth } => self.collect_subtree_nodes(root, max_depth),
        }
    }

    fn collect_subtree_nodes(&self, root: NodeId, max_depth: u32) -> Vec<NodeId> {
        let mut nodes = Vec::new();
        if !self.nodes.contains(root) {
            return nodes;
        }

        let mut stack = vec![(root, 0u32)];
        while let Some((node_id, depth)) = stack.pop() {
            nodes.push(node_id);

            if depth >= max_depth {
                continue;
            }

            let mut child = self.nodes.get(node_id).and_then(|node| node.node_data().first_child);
            let mut children = Vec::new();
            while let Some(child_id) = child {
                children.push(child_id);
                child = self.nodes.get(child_id).and_then(|node| node.node_data().next_sibling);
            }

            for child_id in children.into_iter().rev() {
                stack.push((child_id, depth.saturating_add(1)));
            }
        }

        nodes
    }

    fn event_matches_scope(&self, scope: &UiSubscriptionScope, event: &Event) -> bool {
        match scope {
            UiSubscriptionScope::WholeGraph => true,
            UiSubscriptionScope::Subtree { root, max_depth } => {
                if matches!(
                    event.kind,
                    EventKind::ChildAdded { .. } | EventKind::ChildRemoved { .. } | EventKind::ChildReplaced { .. } | EventKind::ChildMoved { .. } | EventKind::ChildReordered { .. } | EventKind::NodeCreated { .. } | EventKind::NodeDeleted { .. }
                ) {
                    return true;
                }

                let candidate_nodes: Vec<NodeId> = match &event.kind {
                    EventKind::ParamChanged { param, .. } => vec![*param],
                    EventKind::ParamControlChanged { param, .. } => vec![*param],
                    EventKind::ChildAdded { parent, child, .. } => vec![*parent, *child],
                    EventKind::ChildRemoved { parent, child } => vec![*parent, *child],
                    EventKind::ChildReplaced { parent, old, new, .. } => vec![*parent, *old, *new],
                    EventKind::ChildMoved { child, old_parent, new_parent } => vec![*child, *old_parent, *new_parent],
                    EventKind::ChildReordered { parent, child } => vec![*parent, *child],
                    EventKind::NodeCreated { node } => vec![*node],
                    EventKind::NodeDeleted { node } => vec![*node],
                    EventKind::MetaChanged { node, .. } => vec![*node],
                    EventKind::Custom(custom) => custom.origin.into_iter().collect(),
                };

                candidate_nodes.into_iter().any(|node| self.node_within_subtree(node, *root, *max_depth))
            }
        }
    }

    fn node_within_subtree(&self, node: NodeId, root: NodeId, max_depth: u32) -> bool {
        let mut current = Some(node);
        let mut depth = 0u32;

        while let Some(node_id) = current {
            if node_id == root {
                return depth <= max_depth;
            }
            if depth >= max_depth {
                return false;
            }
            current = self.nodes.get(node_id).and_then(|entry| entry.node_data().parent);
            depth = depth.saturating_add(1);
        }

        false
    }
}

fn available_control_modes_for_value(_value: &ParamValue) -> Vec<ParameterControlMode> {
    vec![
        ParameterControlMode::Manual,
        ParameterControlMode::ContextLink,
        ParameterControlMode::TemplateText,
        ParameterControlMode::Expression,
        ParameterControlMode::Proxy,
        ParameterControlMode::Binding,
        ParameterControlMode::Animation,
    ]
}

fn param_types_compatible(source: &ParamValue, target: &ParamValue) -> bool {
    match target {
        ParamValue::Trigger() => matches!(source, ParamValue::Trigger()),
        ParamValue::Int(_) => source.as_int().is_some(),
        ParamValue::Float(_) => source.as_float().is_some(),
        ParamValue::Str(_) => source.as_str().is_some(),
        ParamValue::File(_) => source.as_str().is_some(),
        ParamValue::Enum(_) => source.as_enum().is_some(),
        ParamValue::Bool(_) => source.as_bool().is_some(),
        ParamValue::Vec2(_, _) => source.as_vec2().is_some(),
        ParamValue::Vec3(_, _, _) => source.as_vec3().is_some(),
        ParamValue::Color(_, _, _, _) => source.as_color().is_some(),
        ParamValue::Reference(_) => matches!(source, ParamValue::Reference(_)),
    }
}

fn ui_error_code(error: &crate::engine::EngineEditError) -> &'static str {
    match error {
        crate::engine::EngineEditError::NodeTypeMismatch { .. } => "node_type_mismatch",
        crate::engine::EngineEditError::ParamEditTargetMismatch { .. } => "param_edit_target_mismatch",
        crate::engine::EngineEditError::ParamConstraintViolation { .. } => "param_constraint_violation",
        crate::engine::EngineEditError::ScriptConfigRejected { .. } => "script_config_rejected",
        crate::engine::EngineEditError::ScriptPropertyRejected { .. } => "script_property_rejected",
        crate::engine::EngineEditError::ScriptMethodRejected { .. } => "script_method_rejected",
        crate::engine::EngineEditError::NodeNotFound { .. } => "node_not_found",
        crate::engine::EngineEditError::ParentNotFound { .. } => "parent_not_found",
        crate::engine::EngineEditError::SiblingNotFound { .. } => "sibling_not_found",
        crate::engine::EngineEditError::InvalidSiblingParent { .. } => "invalid_sibling_parent",
        crate::engine::EngineEditError::InvalidSiblingReference { .. } => "invalid_sibling_reference",
        crate::engine::EngineEditError::CannotMutateRoot { .. } => "cannot_mutate_root",
        crate::engine::EngineEditError::CycleDetected { .. } => "cycle_detected",
        crate::engine::EngineEditError::EditSessionAlreadyActive { .. } => "edit_session_already_active",
        crate::engine::EngineEditError::EditSessionNotActive { .. } => "edit_session_not_active",
        crate::engine::EngineEditError::EditSessionIdMismatch { .. } => "edit_session_id_mismatch",
        crate::engine::EngineEditError::UserItemContainerRequired { .. } => "user_item_container_required",
        crate::engine::EngineEditError::UserItemKindRejected { .. } => "user_item_kind_rejected",
        crate::engine::EngineEditError::UserItemTypeUnavailable { .. } => "user_item_type_unavailable",
    }
}
