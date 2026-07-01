use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::contexts::{UiUserContextsDto, UserContextCandidate, UserContextValueType};
use crate::edit::{Edit, EditOrigin};
use crate::engine::{Engine, EngineTime, ProjectPersistenceError};
use crate::events::{Event, EventKind};
use crate::logger::LogRecord;
use crate::node::{
    CurveBezierFitOptions, CurveFitPoint, CurveNode, DeclId, FOLDER_NODE_TYPE, Node, NodeId, NodeMeta, NodeMetaPatch,
    NodeReference, NodeUserPermissions, NodeUuid, PresentationHint, UserCreatableItem, UserCreatableItemInitialParam,
    UserNodeRole,
};
use crate::parameter::{
    ParamValue, ParamValueProjection, ParameterConstraints, ParameterControlMode, ParameterControlSpec,
    ParameterControlState, ParameterEnumOption, ParameterEventBehaviour, ParameterSnapshot, ParameterUiHints,
    available_control_modes_for_parameter, compatibility_for_binding_values, compatibility_for_values,
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

fn is_default_event_behaviour(value: &ParameterEventBehaviour) -> bool {
    *value == ParameterEventBehaviour::default()
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_empty_create_user_item_initial_params(value: &[UiCreateUserItemInitialParam]) -> bool {
    value.is_empty()
}

fn is_empty_duplicate_node_specs(value: &[UiDuplicateNodeSpec]) -> bool {
    value.is_empty()
}

fn is_empty_duplicate_create_user_item_specs(value: &[UiDuplicateCreateUserItemSpec]) -> bool {
    value.is_empty()
}

fn is_empty_duplicate_dependent_user_items(value: &[UiDuplicateDependentUserItem]) -> bool {
    value.is_empty()
}

fn is_empty_duplicate_dependent_initial_params(value: &[UiDuplicateDependentUserItemInitialParam]) -> bool {
    value.is_empty()
}

fn is_default_parameter_constraints(value: &ParameterConstraints) -> bool {
    *value == ParameterConstraints::default()
}

fn is_default_parameter_ui_hints(value: &ParameterUiHints) -> bool {
    *value == ParameterUiHints::default()
}

/// Scope used by snapshot/event subscriptions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
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

/// HTTP snapshot request payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiSnapshotRequest {
    /// Requested snapshot scope.
    #[serde(default)]
    pub scope: UiSubscriptionScope,
    /// Whether any active grouped edit should be cancelled before snapshotting.
    #[serde(default)]
    pub cancel_active_edit_session: bool,
}

/// HTTP replay request payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiReplayRequest {
    /// Requested replay scope.
    #[serde(default)]
    pub scope: UiSubscriptionScope,
    /// Optional replay cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<EngineTime>,
}

/// HTTP request payload for reference-target queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiReferenceTargetsRequest {
    /// Target reference parameter node id.
    pub param: NodeId,
}

/// HTTP request payload for user-context candidate queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiContextCandidatesRequest {
    /// Target parameter node id.
    pub param: NodeId,
}

/// HTTP request payload for parameter-control info queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiParamControlInfoRequest {
    /// Target parameter node id.
    pub param: NodeId,
}

/// HTTP request payload for script-state queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiScriptStateRequest {
    /// Target script node id.
    pub node: NodeId,
}

/// HTTP request payload for script-config updates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiScriptConfigRequest {
    /// Target script node id.
    pub node: NodeId,
    /// Replacement config payload.
    pub config: ScriptUiConfig,
    /// Whether the runtime should force an immediate reload.
    #[serde(default)]
    pub force_reload: bool,
}

/// HTTP request payload for script reloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiScriptReloadRequest {
    /// Target script node id.
    pub node: NodeId,
}

/// HTTP request payload carrying a project path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectPathRequest {
    /// Project path selected by the host.
    pub path: String,
}

/// HTTP request payload for uploading a browser-selected project file before loading it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectUploadRequest {
    /// Original browser-side file name.
    pub file_name: String,
    /// Uploaded project document contents.
    pub contents: String,
}

/// HTTP response payload carrying a resolved project path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectPathDto {
    /// Resolved project path on the host.
    pub path: String,
}

/// UI-facing app project-file metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiProjectFileSpec {
    /// Human-readable name for one project document, such as `Noisette`.
    pub display_name: String,
    /// Preferred filename extension without a leading dot.
    pub extension: String,
    /// Resolved host path for the currently open project document, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_path: Option<String>,
}

impl Default for UiProjectFileSpec {
    fn default() -> Self {
        Self {
            display_name: "Project".to_string(),
            extension: "json".to_string(),
            current_path: None,
        }
    }
}

impl UiProjectFileSpec {
    /// Builds UI project-file metadata from an app file spec and active host path.
    pub fn from_project_file_spec(spec: crate::app::ProjectFileSpec, current_path: Option<String>) -> Self {
        Self {
            display_name: spec.normalized_display_name(),
            extension: spec.normalized_extension(),
            current_path,
        }
    }
}

impl From<crate::app::ProjectFileSpec> for UiProjectFileSpec {
    fn from(spec: crate::app::ProjectFileSpec) -> Self {
        Self::from_project_file_spec(spec, None)
    }
}

/// UI-facing node metadata payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
    /// Shared declaration-description key resolved via `UiSchemaView.declared_descriptions`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_description_key: Option<String>,
    /// Whether `description` is an instance-level override relative to the declared description.
    #[serde(default, skip_serializing_if = "is_false")]
    pub description_overridden: bool,
    /// Optional tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Presentation hints, including warnings.
    #[serde(default, skip_serializing_if = "is_default_presentation_hint")]
    pub presentation: PresentationHint,
}

/// UI-facing parameter payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiParamDto {
    /// Current value.
    pub value: ParamValue,
    /// Declared default value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<ParamValue>,
    /// Coalescing policy.
    #[serde(default, skip_serializing_if = "is_default_event_behaviour")]
    pub event_behaviour: ParameterEventBehaviour,
    /// Read-only flag.
    #[serde(default, skip_serializing_if = "is_false")]
    pub read_only: bool,
    /// Runtime value constraints.
    #[serde(default, skip_serializing_if = "is_default_parameter_constraints")]
    pub constraints: ParameterConstraints,
    /// Presentation and editing hints.
    #[serde(default, skip_serializing_if = "is_default_parameter_ui_hints")]
    pub ui_hints: ParameterUiHints,
    /// Runtime control-plane state.
    #[serde(default, skip_serializing_if = "is_default_ui_parameter_control_state")]
    pub control: UiParameterControlStateDto,
    /// Optional shared enum-options id resolved via `UiSchemaView.enums`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_options_id: Option<String>,
    /// Engine-computed selectable targets for reference parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_allowed_targets: Vec<NodeId>,
    /// Engine-computed visible tree nodes for reference picker (targets + relevant ancestor paths).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_visible_nodes: Vec<NodeId>,
}

impl From<ParameterSnapshot> for UiParamDto {
    fn from(snapshot: ParameterSnapshot) -> Self {
        let default_value = if snapshot.default_value == snapshot.value {
            None
        } else {
            Some(snapshot.default_value)
        };
        Self {
            value: snapshot.value,
            default_value,
            event_behaviour: snapshot.event_behaviour,
            read_only: snapshot.read_only,
            constraints: snapshot.constraints,
            ui_hints: snapshot.ui_hints,
            control: snapshot.control.into(),
            enum_options_id: None,
            reference_allowed_targets: Vec::new(),
            reference_visible_nodes: Vec::new(),
        }
    }
}

/// UI-facing parameter control state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiParameterControlStateDto {
    /// Active control mode.
    #[serde(default)]
    pub mode: ParameterControlMode,
    /// Authoring control specification.
    #[serde(default)]
    pub spec: ParameterControlSpec,
}

impl Default for UiParameterControlStateDto {
    fn default() -> Self {
        Self {
            mode: ParameterControlMode::Manual,
            spec: ParameterControlSpec::Manual,
        }
    }
}

fn is_default_ui_parameter_control_state(value: &UiParameterControlStateDto) -> bool {
    *value == UiParameterControlStateDto::default()
}

impl From<ParameterControlState> for UiParameterControlStateDto {
    fn from(state: ParameterControlState) -> Self {
        Self {
            mode: state.mode,
            spec: state.spec,
        }
    }
}

impl From<UiParameterControlStateDto> for ParameterControlState {
    fn from(state: UiParameterControlStateDto) -> Self {
        ParameterControlState::new(state.mode, state.spec)
    }
}

/// UI payload for on-demand reference picker target resolution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, TS)]
pub struct UiReferenceTargetsDto {
    /// Selectable targets for the requested reference parameter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_targets: Vec<NodeId>,
    /// Visible picker nodes (targets + path ancestors).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_nodes: Vec<NodeId>,
    /// Compatibility details for each selectable target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<UiReferenceTargetCandidateDto>,
}

/// UI-facing compatibility details for one selectable reference target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiReferenceTargetCandidateDto {
    /// Candidate node id.
    pub target: NodeId,
    /// Whether this candidate can be consumed without projection.
    pub direct: bool,
    /// Projections that make this candidate compatible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projections: Vec<ParamValueProjection>,
}

/// UI-facing control candidate for proxy/binding target pickers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiParamCandidateDto {
    /// Candidate parameter node id.
    pub param: NodeId,
    /// Whether candidate value type is compatible.
    pub compatible: bool,
    /// Projections that enable compatibility.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projections: Vec<ParamValueProjection>,
}

/// UI-facing token suggestion for template/expression editors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiTokenSuggestionDto {
    /// Suggested token string.
    pub token: String,
}

/// UI-facing per-parameter control info payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiParamControlInfoDto {
    /// Parameter node id.
    pub param: NodeId,
    /// Active control mode.
    pub active_mode: ParameterControlMode,
    /// Supported control modes for this parameter.
    pub available_modes: Vec<ParameterControlMode>,
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeId>,
}

/// UI-facing descriptor of a user-creatable item type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiCreatableUserItemDto {
    /// Runtime node type identifier.
    pub node_type: String,
    /// Logical user-item kind.
    pub item_kind: String,
    /// Suggested default label.
    pub label: String,
    /// Optional Add menu submenu path, excluding the item label.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menu_path: Vec<String>,
    /// Optional direct parameter values applied immediately after creation.
    #[serde(default, skip_serializing_if = "is_empty_create_user_item_initial_params")]
    pub initial_params: Vec<UiCreateUserItemInitialParam>,
    /// Whether UI creation flows should auto-select the created item.
    pub select_when_created: bool,
}

impl From<UserCreatableItem> for UiCreatableUserItemDto {
    fn from(item: UserCreatableItem) -> Self {
        Self {
            node_type: item.node_type,
            item_kind: item.item_kind,
            label: item.label,
            menu_path: item.menu_path,
            initial_params: item.initial_params.into_iter().map(Into::into).collect(),
            select_when_created: item.select_when_created,
        }
    }
}

/// UI-facing node-type descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiNodeTypeDescriptor {
    /// Runtime node type identifier.
    pub node_type: String,
    /// Canonical description shared by all nodes of this type when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// UI-facing shared declaration-description descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiDeclaredDescriptionDescriptor {
    /// Stable key used by nodes that share this declared description.
    pub key: String,
    /// Canonical declared description text.
    pub description: String,
}

/// UI-facing enum descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiEnumDefinition {
    /// Stable enum id.
    pub enum_id: String,
    /// Enum variant definitions.
    pub variants: Vec<UiEnumVariantDefinition>,
}

/// UI-facing enum variant descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiEnumVariantDefinition {
    /// Stable variant id.
    pub variant_id: String,
    /// Value represented by this variant.
    pub value: ParamValue,
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, TS)]
pub struct UiSchemaView {
    /// Known node types within the snapshot scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_types: Vec<UiNodeTypeDescriptor>,
    /// Shared descriptions for repeated declared nodes and parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared_descriptions: Vec<UiDeclaredDescriptionDescriptor>,
    /// Enum definitions used by UI editors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enums: Vec<UiEnumDefinition>,
}

/// UI-facing history status payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
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
    /// Logical content-state id for the current graph relative to undo/redo history.
    pub current_history_state_id: u64,
}

/// UI-facing logger state included in snapshots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, TS)]
pub struct UiLoggerState {
    /// Maximum number of logger records retained server-side.
    pub max_entries: usize,
    /// Retained records in ascending record-id order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<LogRecord>,
}

/// Snapshot payload for initial sync.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
    /// App-provided project file metadata.
    #[serde(default)]
    pub project_file: UiProjectFileSpec,
    /// Current user-context scopes.
    #[serde(default, skip_serializing_if = "is_default_user_contexts")]
    pub user_contexts: UiUserContextsDto,
}

fn is_default_user_contexts(value: &UiUserContextsDto) -> bool {
    *value == UiUserContextsDto::default()
}

/// Direct parameter initializer applied immediately after one user-item is created.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiCreateUserItemInitialParam {
    /// Direct child decl id on the newly-created root node.
    pub decl_id: DeclId,
    /// Initial value to assign.
    pub value: ParamValue,
}

impl From<UserCreatableItemInitialParam> for UiCreateUserItemInitialParam {
    fn from(initial_param: UserCreatableItemInitialParam) -> Self {
        Self {
            decl_id: initial_param.decl_id,
            value: initial_param.value,
        }
    }
}

/// One existing subtree root to clone as part of a copy batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDuplicateNodeSpec {
    /// Source node id to clone. Also acts as the key used by dependent references.
    pub source: NodeId,
    /// Parent receiving the duplicated subtree root.
    pub new_parent: NodeId,
    /// Optional sibling after which insertion occurs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_prev_sibling: Option<NodeId>,
    /// Optional explicit label for the duplicated root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional direct parameter values applied to the duplicated root before the batch completes.
    #[serde(default, skip_serializing_if = "is_empty_create_user_item_initial_params")]
    pub initial_params: Vec<UiCreateUserItemInitialParam>,
}

/// One fresh user item to create as part of a copy batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDuplicateCreateUserItemSpec {
    /// Source key used by dependent references to address this created item.
    pub source: NodeId,
    /// Parent receiving the created item.
    pub parent: NodeId,
    /// Runtime node type identifier to instantiate.
    pub node_type: String,
    /// Optional explicit label for the new item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional direct parameter values applied to the created root before the batch completes.
    #[serde(default, skip_serializing_if = "is_empty_create_user_item_initial_params")]
    pub initial_params: Vec<UiCreateUserItemInitialParam>,
}

/// Initializer for an item that depends on roots materialized earlier in the same copy batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDuplicateDependentUserItemInitialParam {
    /// Direct child decl id on the newly-created dependent item.
    pub decl_id: DeclId,
    /// Literal value or a reference resolved from the copy batch source map.
    pub value: UiDuplicateDependentInitialParamValue,
}

/// Value source for a dependent item initializer inside a copy batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiDuplicateDependentInitialParamValue {
    /// Use this parameter value as-is.
    Literal {
        /// Parameter value assigned directly to the dependent item.
        value: ParamValue,
    },
    /// Reference the copied root produced from `source`.
    DuplicatedNodeReference {
        /// Source key whose copied root becomes the reference target.
        source: NodeId,
    },
}

/// One dependent user item to create after copy-batch roots have been materialized.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiDuplicateDependentUserItem {
    /// Parent receiving the dependent item.
    pub parent: NodeId,
    /// Runtime node type identifier to instantiate.
    pub node_type: String,
    /// Optional explicit label for the dependent item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Initial values applied after references to copied roots have been resolved.
    #[serde(default, skip_serializing_if = "is_empty_duplicate_dependent_initial_params")]
    pub initial_params: Vec<UiDuplicateDependentUserItemInitialParam>,
}

/// Post-edit direct child order for one parent node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiChildrenOrderPatch {
    /// Parent whose child list changed.
    pub parent: NodeId,
    /// Complete direct child order after the operation.
    pub children: Vec<NodeId>,
}

/// Incremental metadata patch for one UI node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiNodeMetaPatch {
    /// Replacement display label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Replacement short script/reference name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// Replacement enabled state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Replacement disablement capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_be_disabled: Option<bool>,
    /// Replacement optional description, where `Some(None)` clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    /// Replacement user-edit permissions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_permissions: Option<NodeUserPermissions>,
    /// Replacement tag list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Replacement presentation hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationHint>,
}

impl UiNodeMetaPatch {
    pub(crate) fn is_empty(&self) -> bool {
        self.label.is_none()
            && self.short_name.is_none()
            && self.enabled.is_none()
            && self.can_be_disabled.is_none()
            && self.description.is_none()
            && self.user_permissions.is_none()
            && self.tags.is_none()
            && self.presentation.is_none()
    }
}

impl From<&NodeMetaPatch> for UiNodeMetaPatch {
    fn from(patch: &NodeMetaPatch) -> Self {
        Self {
            label: patch.label.clone(),
            short_name: patch.short_name.clone(),
            enabled: patch.enabled,
            can_be_disabled: patch.can_be_disabled,
            description: patch.description.clone(),
            user_permissions: patch.user_permissions.clone(),
            tags: patch.tags.clone(),
            presentation: patch.presentation.clone(),
        }
    }
}

/// Incremental parameter patch for one UI parameter node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiParamPatch {
    /// Replacement parameter value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<ParamValue>,
    /// Replacement control state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<UiParameterControlStateDto>,
    /// Replacement runtime constraints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<ParameterConstraints>,
}

/// One deterministic graph patch operation inside an atomic UI transaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiGraphOp {
    /// Inserts a node that did not exist in the client's graph.
    NodeCreated {
        /// Full UI snapshot for the created node.
        snapshot: UiNodeDto,
        /// Parent receiving the node, if attached.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<NodeId>,
        /// Direct child index under `parent`, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    /// Inserts a complete subtree that did not exist in the client's graph.
    ///
    /// Used for bulk insertions (N > 8 nodes) to avoid an O(N²) `ui_child_index` scan
    /// and to reduce the op list to a single entry.
    SubtreeInserted {
        /// Root of the inserted subtree.
        root: NodeId,
        /// Parent node where `root` was attached.
        parent: NodeId,
        /// Full snapshots for all inserted nodes (root and descendants, depth-first).
        nodes: Vec<UiNodeDto>,
        /// Final direct child order for `parent` after insertion.
        parent_children_after: Vec<NodeId>,
    },
    /// Removes a subtree from the client's graph.
    SubtreeRemoved {
        /// Root of the removed subtree.
        root: NodeId,
        /// Root and descendant ids removed by this operation.
        removed_ids: Vec<NodeId>,
        /// Post-removal child order for the former parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_after: Option<UiChildrenOrderPatch>,
    },
    /// Moves one existing node between parents or positions.
    NodeMoved {
        /// Node that moved.
        node: NodeId,
        /// Previous parent before the move.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_parent: Option<NodeId>,
        /// New parent after the move.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_parent: Option<NodeId>,
        /// Post-move child order for the previous parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_parent_after: Option<UiChildrenOrderPatch>,
        /// Post-move child order for the new parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_parent_after: Option<UiChildrenOrderPatch>,
    },
    /// Replaces the direct child order for one parent.
    ChildrenReordered {
        /// Parent whose children were reordered.
        parent: NodeId,
        /// Complete direct child order after the reorder.
        children: Vec<NodeId>,
    },
    /// Applies an incremental metadata patch to one node.
    NodeMetaPatched {
        /// Node whose metadata changed.
        node: NodeId,
        /// Metadata fields that changed.
        patch: UiNodeMetaPatch,
    },
    /// Applies an incremental parameter patch.
    ParamPatched {
        /// Node owning the parameter in the UI graph.
        node: NodeId,
        /// Parameter node that changed.
        param: NodeId,
        /// Parameter fields that changed.
        patch: UiParamPatch,
    },
    /// Replaces the UI undo/redo history state.
    HistoryPatched {
        /// Current history state after the transaction.
        history: UiHistoryState,
    },
    /// Adds or drops UI logger records.
    LoggerPatched {
        /// New logger records appended by the transaction.
        records_added: Vec<LogRecord>,
        /// Earliest retained record id after dropping old records.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dropped_before: Option<u64>,
    },
}

/// Atomic UI graph transaction applied in order against a known graph version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct UiGraphTransaction {
    /// Monotonic transaction id within the current project epoch.
    pub tx_id: u64,
    /// Project epoch this transaction belongs to.
    pub epoch: u64,
    /// Graph version expected before applying `ops`.
    pub base_graph_version: u64,
    /// Graph version after applying `ops`.
    pub next_graph_version: u64,
    /// Ordered patch operations applied atomically by the UI.
    pub ops: Vec<UiGraphOp>,
}

/// UI-facing event kind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiEventKind {
    /// Graph transaction containing multiple atomic updates.
    GraphTransaction {
        /// Atomic graph transaction payload.
        #[serde(flatten)]
        transaction: UiGraphTransaction,
    },
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
        old_state: UiParameterControlStateDto,
        /// New control state.
        new_state: UiParameterControlStateDto,
    },
    /// Parameter constraints changed.
    ParamConstraintsChanged {
        /// Parameter node id.
        param: NodeId,
        /// Previous constraints.
        old_constraints: ParameterConstraints,
        /// New constraints.
        new_constraints: ParameterConstraints,
    },
    /// Child added.
    ChildAdded {
        /// Parent id.
        parent: NodeId,
        /// Child id.
        child: NodeId,
        /// Declared slot id.
        decl_id: DeclId,
        /// Current direct child order for the parent when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_children: Option<Vec<NodeId>>,
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
        /// Current direct child order for the previous parent when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_parent_children: Option<Vec<NodeId>>,
        /// Current direct child order for the new parent when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_parent_children: Option<Vec<NodeId>>,
    },
    /// Child reordered.
    ChildReordered {
        /// Parent id.
        parent: NodeId,
        /// Child id.
        child: NodeId,
        /// Current direct child order for the parent when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_children: Option<Vec<NodeId>>,
    },
    /// Node created.
    NodeCreated {
        /// Node id.
        node: NodeId,
        /// Node snapshot for incremental UI insertion when the node is still live.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        snapshot: Option<UiNodeDto>,
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
            EventKind::ParamChanged {
                param,
                old_value,
                new_value,
            } => UiEventKind::ParamChanged {
                param,
                old_value,
                new_value,
            },
            EventKind::ParamControlChanged {
                param,
                old_state,
                new_state,
            } => UiEventKind::ParamControlChanged {
                param,
                old_state: old_state.into(),
                new_state: new_state.into(),
            },
            EventKind::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            } => UiEventKind::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            },
            EventKind::ChildAdded { parent, child, decl_id } => UiEventKind::ChildAdded {
                parent,
                child,
                decl_id,
                parent_children: None,
            },
            EventKind::ChildRemoved { parent, child } => UiEventKind::ChildRemoved { parent, child },
            EventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => UiEventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            },
            EventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
            } => UiEventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
                old_parent_children: None,
                new_parent_children: None,
            },
            EventKind::ChildReordered { parent, child } => UiEventKind::ChildReordered {
                parent,
                child,
                parent_children: None,
            },
            EventKind::NodeCreated { node } => UiEventKind::NodeCreated { node, snapshot: None },
            EventKind::NodeDeleted { node } => UiEventKind::NodeDeleted { node },
            EventKind::MetaChanged { node, patch } => UiEventKind::MetaChanged { node, patch },
            EventKind::GraphTransaction { transaction } => UiEventKind::GraphTransaction { transaction },
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
        state: UiParameterControlStateDto,
    },
    /// Replace a parameter's live runtime constraints.
    SetParamConstraints {
        /// Target parameter node id.
        node: NodeId,
        /// New constraints payload.
        constraints: ParameterConstraints,
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
    /// Remove multiple nodes in one intent transaction.
    RemoveNodes {
        /// Target node ids.
        nodes: Vec<NodeId>,
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
        /// Optional direct parameter values applied before the intent completes.
        #[serde(default, skip_serializing_if = "is_empty_create_user_item_initial_params")]
        initial_params: Vec<UiCreateUserItemInitialParam>,
    },
    /// Duplicates an existing node subtree under `new_parent`.
    DuplicateNode {
        /// Source node id to clone.
        source: NodeId,
        /// Parent receiving the duplicated subtree root.
        new_parent: NodeId,
        /// Optional sibling after which insertion occurs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_prev_sibling: Option<NodeId>,
        /// Optional explicit label for the duplicated root.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// Optional direct parameter values applied to the duplicated root before the intent completes.
        #[serde(default, skip_serializing_if = "is_empty_create_user_item_initial_params")]
        initial_params: Vec<UiCreateUserItemInitialParam>,
    },
    /// Materializes copied roots and dependent user items as one edit.
    DuplicateNodes {
        /// Existing subtree roots to clone.
        #[serde(default, skip_serializing_if = "is_empty_duplicate_node_specs")]
        nodes: Vec<UiDuplicateNodeSpec>,
        /// Fresh user items to create and expose to dependent references.
        #[serde(default, skip_serializing_if = "is_empty_duplicate_create_user_item_specs")]
        created_items: Vec<UiDuplicateCreateUserItemSpec>,
        /// Items whose initial parameters can reference roots created earlier in the batch.
        #[serde(default, skip_serializing_if = "is_empty_duplicate_dependent_user_items")]
        dependent_items: Vec<UiDuplicateDependentUserItem>,
    },
    /// Replaces one curve range with a sparse bezier fit of recorded samples.
    FitAnimationCurvePath {
        /// Target animation-curve node id.
        curve: NodeId,
        /// Recorded path samples.
        points: Vec<CurveFitPoint>,
        /// Fit controls.
        #[serde(default)]
        options: CurveBezierFitOptions,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
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
    fn ui_enum_options_fingerprint(options: &[ParameterEnumOption]) -> String {
        serde_json::to_string(options).unwrap_or_default()
    }

    fn ui_enum_definition_from_options(enum_id: String, options: Vec<ParameterEnumOption>) -> UiEnumDefinition {
        UiEnumDefinition {
            enum_id,
            variants: options
                .into_iter()
                .map(|option| UiEnumVariantDefinition {
                    variant_id: option.variant_id,
                    value: option.value,
                    label: option.label,
                    tags: option.tags,
                    ordering: option.ordering,
                })
                .collect(),
        }
    }

    /// Builds a UI snapshot for the requested scope.
    pub fn ui_snapshot(&self, scope: UiSubscriptionScope) -> UiSnapshot {
        let node_ids = self.collect_scope_nodes(scope.clone());
        let visible: HashSet<NodeId> = node_ids.iter().copied().collect();
        let mut nodes = Vec::with_capacity(node_ids.len());
        let mut known_node_types = HashMap::<String, Option<String>>::new();
        let mut known_declared_descriptions = HashMap::<String, String>::new();
        let mut enum_id_by_fingerprint = HashMap::<String, String>::new();
        let mut enums = Vec::<UiEnumDefinition>::new();

        for node_id in node_ids {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            let node_data = node.node_data();
            let node_type = node.get_type().to_string();
            let type_description = node.type_description().map(str::to_string);
            known_node_types
                .entry(node_type.clone())
                .and_modify(|existing| {
                    if existing.is_none() && type_description.is_some() {
                        *existing = type_description.clone();
                    }
                })
                .or_insert(type_description);

            let mut children = Vec::new();
            let mut child = node_data.first_child;
            while let Some(child_id) = child {
                if visible.contains(&child_id) {
                    children.push(child_id);
                }
                child = self.nodes.get(child_id).and_then(|next| next.node_data().next_sibling);
            }

            let data = if let Some(mut param) = node.engine_param_snapshot() {
                let enum_options_id = if param.constraints.enum_options.is_empty() {
                    None
                } else {
                    let enum_options = std::mem::take(&mut param.constraints.enum_options);
                    let fingerprint = Self::ui_enum_options_fingerprint(enum_options.as_slice());
                    if let Some(existing_id) = enum_id_by_fingerprint.get(&fingerprint) {
                        Some(existing_id.clone())
                    } else {
                        let enum_id = format!("enumOptions{}", enums.len() + 1);
                        enum_id_by_fingerprint.insert(fingerprint, enum_id.clone());
                        enums.push(Self::ui_enum_definition_from_options(enum_id.clone(), enum_options));
                        Some(enum_id)
                    }
                };
                let mut dto = UiParamDto::from(param);
                dto.enum_options_id = enum_options_id;
                UiNodeDataDto::Parameter { param: dto }
            } else {
                UiNodeDataDto::Node {
                    node_type: node.get_type().to_string(),
                }
            };

            let listed_user_items = node.user_creatable_items();
            let can_query_creatable_items = node.get_type() == FOLDER_NODE_TYPE
                || node.user_container_rules().is_some()
                || !listed_user_items.is_empty();

            let mut creatable_user_items = Vec::new();
            if can_query_creatable_items {
                for item in self.catalog_creatable_items(node_id).into_iter() {
                    creatable_user_items.push(UiCreatableUserItemDto::from(item));
                }
            }

            let mut accepted_user_item_kinds: Vec<String> = node
                .user_container_rules()
                .map(|rules| {
                    rules
                        .accepts_item_kinds
                        .iter()
                        .map(|kind| (*kind).to_string())
                        .collect()
                })
                .unwrap_or_default();
            for item in &listed_user_items {
                if !accepted_user_item_kinds.iter().any(|kind| kind == &item.item_kind) {
                    accepted_user_item_kinds.push(item.item_kind.clone());
                }
            }

            let declared_description_key = node_data
                .meta
                .declared_description_key
                .as_deref()
                .filter(|key| !key.trim().is_empty());
            let declared_description = node_data
                .meta
                .declared_description
                .as_deref()
                .filter(|description| !description.trim().is_empty());
            let effective_description = node_data.meta.description.as_deref();

            let (description, declared_description_key, description_overridden) =
                if let (Some(key), Some(description)) = (declared_description_key, declared_description) {
                    known_declared_descriptions
                        .entry(key.to_string())
                        .or_insert_with(|| description.to_string());
                    let overridden = effective_description != Some(description);
                    (
                        if overridden {
                            effective_description.map(str::to_string)
                        } else {
                            None
                        },
                        Some(key.to_string()),
                        overridden,
                    )
                } else {
                    (
                        match effective_description {
                            Some(description) if Some(description) != node.type_description() => {
                                Some(description.to_string())
                            }
                            _ => None,
                        },
                        None,
                        false,
                    )
                };

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
                    description,
                    declared_description_key,
                    description_overridden,
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

        let mut node_types: Vec<UiNodeTypeDescriptor> = known_node_types
            .into_iter()
            .map(|(node_type, description)| UiNodeTypeDescriptor { node_type, description })
            .collect();
        node_types.sort_by(|left, right| left.node_type.cmp(&right.node_type));
        let mut declared_descriptions: Vec<UiDeclaredDescriptionDescriptor> = known_declared_descriptions
            .into_iter()
            .map(|(key, description)| UiDeclaredDescriptionDescriptor { key, description })
            .collect();
        declared_descriptions.sort_by(|left, right| left.key.cmp(&right.key));

        UiSnapshot {
            protocol_version: UI_PROTOCOL_VERSION.to_string(),
            scope,
            at: self.time,
            nodes,
            schema: UiSchemaView {
                node_types,
                declared_descriptions,
                enums,
            },
            history: self.ui_history_state(),
            logger: UiLoggerState {
                max_entries: crate::logger::max_entries(),
                records: crate::logger::records(),
            },
            project_file: UiProjectFileSpec::default(),
            user_contexts: self.ui_user_contexts(),
        }
    }

    /// Returns a scoped UI event replay batch starting strictly after `from`.
    pub fn ui_event_batch(&self, from: Option<EngineTime>, scope: UiSubscriptionScope) -> UiEventBatch {
        let start_index = self.ui_event_log_start_index(from);
        let mut events = Vec::<UiEventDto>::new();
        let source_events = &self.ui_event_log()[start_index..];
        events.reserve(source_events.len());
        let mut child_order_cache = HashMap::<NodeId, Option<Vec<NodeId>>>::new();

        for event in source_events {
            if self.event_matches_scope(&scope, event) {
                events.push(self.ui_event_dto_with_child_order_cache(event, &mut child_order_cache));
            }
        }

        let to = events.last().map(|event| event.time);

        UiEventBatch { from, to, events }
    }

    /// Resolves custom-filter reference picker targets for one parameter node.
    ///
    /// Returns empty vectors when the parameter cannot be resolved or is not a reference parameter.
    pub fn ui_reference_targets_for_param(&self, param_node: NodeId) -> UiReferenceTargetsDto {
        let Some(node) = self.nodes.get(param_node) else {
            return UiReferenceTargetsDto::default();
        };
        let Some(snapshot) = node.engine_param_snapshot() else {
            return UiReferenceTargetsDto::default();
        };
        if !matches!(snapshot.value, ParamValue::Reference(_)) {
            return UiReferenceTargetsDto::default();
        }

        let constraints = self.reference_constraints_for_param(param_node);
        let mut allowed_targets = self.reference_allowed_targets_for_param(param_node);
        allowed_targets.sort_by_key(|node_id| node_id.0);
        let expected_parameter_values = self.expected_reference_parameter_values(param_node, &constraints);

        let mut candidates = allowed_targets
            .iter()
            .map(|target| {
                if let Some(compatibility) = self.reference_candidate_compatibility_for_expected_values(
                    param_node,
                    *target,
                    expected_parameter_values.as_slice(),
                    &constraints,
                ) {
                    UiReferenceTargetCandidateDto {
                        target: *target,
                        direct: compatibility.direct,
                        projections: compatibility.projections,
                    }
                } else {
                    UiReferenceTargetCandidateDto {
                        target: *target,
                        direct: true,
                        projections: Vec::new(),
                    }
                }
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| candidate.target.0);

        UiReferenceTargetsDto {
            allowed_targets,
            visible_nodes: self.reference_visible_nodes_for_param(param_node),
            candidates,
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
        let mut available_modes =
            available_control_modes_for_parameter(&snapshot.value, snapshot.control_modes_enabled);
        if context_candidates.is_empty() {
            available_modes.retain(|mode| *mode != ParameterControlMode::TemplateText);
        }

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
            let compatibility = compatibility_for_values(&candidate_snapshot.value, &snapshot.value);
            proxy_candidates.push(UiParamCandidateDto {
                param: candidate_id,
                compatible: compatibility.is_compatible(),
                projections: compatibility.projections,
            });
            let binding_compatibility = compatibility_for_binding_values(&candidate_snapshot.value, &snapshot.value);
            binding_candidates.push(UiParamCandidateDto {
                param: candidate_id,
                compatible: binding_compatibility.is_compatible(),
                projections: binding_compatibility.projections,
            });
        }
        proxy_candidates.sort_by_key(|candidate| candidate.param.0);
        binding_candidates.sort_by_key(|candidate| candidate.param.0);

        Ok(UiParamControlInfoDto {
            param: param_node,
            active_mode: snapshot.control.mode,
            available_modes,
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

        target
            .engine_script_state()
            .ok_or_else(|| format!("node {} does not expose script runtime state", node.0))
    }

    /// Replaces script configuration for `node`.
    pub fn ui_set_script_config(
        &mut self,
        node: NodeId,
        config: ScriptUiConfig,
        force_reload: bool,
    ) -> Result<(), String> {
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

    fn ui_resolve_created_child(
        &self,
        parent: NodeId,
        known_children: &HashSet<NodeId>,
    ) -> Result<NodeId, crate::engine::EngineEditError> {
        const OPERATION: &str = "CreateUserItem";

        let snapshot = self.build_process_tree_snapshot();
        let mut created_children = snapshot
            .child_ids(parent)
            .into_iter()
            .filter(|child_id| !known_children.contains(child_id));

        let Some(created_child) = created_children.next() else {
            return Err(crate::engine::EngineEditError::NodeMutationRejected {
                edit_index: 0,
                operation: OPERATION,
                node: parent,
                node_type: self
                    .nodes
                    .get(parent)
                    .map(|node| node.get_type().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                message: "createUserItem did not materialize a new direct child".to_string(),
            });
        };

        if created_children.next().is_some() {
            return Err(crate::engine::EngineEditError::NodeMutationRejected {
                edit_index: 0,
                operation: OPERATION,
                node: parent,
                node_type: self
                    .nodes
                    .get(parent)
                    .map(|node| node.get_type().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                message: "createUserItem materialized multiple new direct children".to_string(),
            });
        }

        Ok(created_child)
    }

    fn ui_find_created_item_param_node(&self, created_child: NodeId, decl_id: &str) -> Option<NodeId> {
        let snapshot = self.build_process_tree_snapshot();
        if decl_id.contains('/') {
            if let Some(node_id) = snapshot.resolve_path_from(created_child, decl_id) {
                return snapshot
                    .node(node_id)
                    .and_then(|node| node.is_parameter().then_some(node_id));
            }
        }

        let mut stack = vec![created_child];
        while let Some(node_id) = stack.pop() {
            let mut child = snapshot.node(node_id).and_then(|node| node.first_child);
            while let Some(child_id) = child {
                let child_snapshot = snapshot.node(child_id)?;
                if child_snapshot.is_parameter()
                    && (child_snapshot.decl_id == decl_id || child_snapshot.decl_id.rsplit('/').next() == Some(decl_id))
                {
                    return Some(child_id);
                }
                stack.push(child_id);
                child = child_snapshot.next_sibling;
            }
        }

        None
    }

    fn ui_apply_create_user_item(
        &mut self,
        parent: NodeId,
        node_type: String,
        label: Option<String>,
        initial_params: Vec<UiCreateUserItemInitialParam>,
    ) -> Result<(), crate::engine::EngineEditError> {
        self.ui_apply_create_user_item_returning_node(parent, node_type, label, initial_params)
            .map(|_| ())
    }

    fn ui_apply_create_user_item_returning_node(
        &mut self,
        parent: NodeId,
        node_type: String,
        label: Option<String>,
        initial_params: Vec<UiCreateUserItemInitialParam>,
    ) -> Result<NodeId, crate::engine::EngineEditError> {
        const OPERATION: &str = "CreateUserItem";

        let known_children: HashSet<NodeId> = self
            .build_process_tree_snapshot()
            .child_ids(parent)
            .into_iter()
            .collect();
        self.queue_catalog_create(parent, node_type, label, None)?;
        self.apply_ui_stabilization_to_fixed_point(16)?;

        let created_child = self.ui_resolve_created_child(parent, &known_children)?;
        if !initial_params.is_empty() {
            self.ui_apply_initial_params_to_node(created_child, initial_params, OPERATION)?;
        }
        Ok(created_child)
    }

    /// Duplicates/copied roots and then creates dependent user items within the same edit session.
    pub fn ui_apply_duplicate_nodes_with_dependent_user_items<Encode, Decode>(
        &mut self,
        nodes: Vec<UiDuplicateNodeSpec>,
        created_items: Vec<UiDuplicateCreateUserItemSpec>,
        dependent_items: Vec<UiDuplicateDependentUserItem>,
        mut encode_data: Encode,
        mut decode_node: Decode,
    ) -> Result<Vec<NodeId>, ProjectPersistenceError>
    where
        Encode: FnMut(&T) -> Result<serde_json::Value, String>,
        Decode: FnMut(&str, &serde_json::Value, &NodeMeta) -> Result<T, String>,
    {
        let mut copied_by_source = HashMap::<NodeId, NodeId>::new();
        let mut copied_roots = Vec::with_capacity(nodes.len() + created_items.len());

        for spec in nodes {
            let source = spec.source;
            let duplicated_root = self.duplicate_subtree_with(
                source,
                spec.new_parent,
                spec.new_prev_sibling,
                spec.label,
                &mut encode_data,
                &mut decode_node,
            )?;
            if !spec.initial_params.is_empty() {
                self.ui_apply_initial_params_to_node(duplicated_root, spec.initial_params, "DuplicateNodes")
                    .map_err(ProjectPersistenceError::Engine)?;
            }
            copied_by_source.insert(source, duplicated_root);
            copied_roots.push(duplicated_root);
        }

        for spec in created_items {
            let source = spec.source;
            let created_root = self
                .ui_apply_create_user_item_returning_node(spec.parent, spec.node_type, spec.label, spec.initial_params)
                .map_err(ProjectPersistenceError::Engine)?;
            copied_by_source.insert(source, created_root);
            copied_roots.push(created_root);
        }

        for item in dependent_items {
            let initial_params =
                self.ui_resolve_duplicate_dependent_initial_params(item.initial_params, &copied_by_source)?;
            self.ui_apply_create_user_item_returning_node(item.parent, item.node_type, item.label, initial_params)
                .map_err(ProjectPersistenceError::Engine)?;
        }

        Ok(copied_roots)
    }

    fn ui_resolve_duplicate_dependent_initial_params(
        &self,
        initial_params: Vec<UiDuplicateDependentUserItemInitialParam>,
        copied_by_source: &HashMap<NodeId, NodeId>,
    ) -> Result<Vec<UiCreateUserItemInitialParam>, ProjectPersistenceError> {
        initial_params
            .into_iter()
            .map(|initial_param| {
                let value =
                    match initial_param.value {
                        UiDuplicateDependentInitialParamValue::Literal { value } => value,
                        UiDuplicateDependentInitialParamValue::DuplicatedNodeReference { source } => {
                            let copied = copied_by_source.get(&source).copied().ok_or_else(|| {
                                ProjectPersistenceError::Codec {
                                    node_type: "duplicateNodes".to_string(),
                                    message: format!(
                                        "dependent item references source node {:?} that was not copied",
                                        source
                                    ),
                                }
                            })?;
                            let node = self
                                .nodes
                                .get(copied)
                                .ok_or(ProjectPersistenceError::MissingNode(copied))?;
                            let node_data = node.node_data();
                            let mut reference = NodeReference::with_cached_id(node_data.meta.uuid, Some(copied));
                            reference.cached_name = Some(node_data.meta.label.clone());
                            ParamValue::Reference(reference)
                        }
                    };
                Ok(UiCreateUserItemInitialParam {
                    decl_id: initial_param.decl_id,
                    value,
                })
            })
            .collect()
    }

    /// Applies direct parameter initializers to an already-created user item root.
    pub fn ui_apply_initial_params_to_node(
        &mut self,
        created_child: NodeId,
        initial_params: Vec<UiCreateUserItemInitialParam>,
        operation: &'static str,
    ) -> Result<(), crate::engine::EngineEditError> {
        for initial_param in initial_params {
            let Some(param_node) =
                self.ui_find_created_item_param_node(created_child, initial_param.decl_id.0.as_str())
            else {
                return Err(crate::engine::EngineEditError::NodeMutationRejected {
                    edit_index: 0,
                    operation,
                    node: created_child,
                    node_type: self
                        .nodes
                        .get(created_child)
                        .map(|node| node.get_type().to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    message: format!(
                        "parameter '{}' is unavailable on the created item",
                        initial_param.decl_id.0
                    ),
                });
            };

            self.edits.push(Edit::SetParam {
                node: param_node,
                value: initial_param.value,
                behaviour: ParameterEventBehaviour::Coalesce,
            });
            self.apply_ui_initialization_to_fixed_point(16)?;
        }

        Ok(())
    }

    /// Applies one UI edit intent and returns an acknowledgement payload.
    pub fn apply_ui_intent(&mut self, intent: UiEditIntent) -> UiAck {
        self.apply_ui_intent_from_client(intent, None)
    }

    /// Applies one UI edit intent while attributing any opened edit session to a stable client.
    pub fn apply_ui_intent_from_client(&mut self, intent: UiEditIntent, ui_client_instance_id: Option<&str>) -> UiAck {
        let before_len = self.ui_event_log().len();
        // eprintln!("[gc-ui] intent recv: {intent:?} | undo_len={} redo_len={} active_session={}", self.undo_len(), self.redo_len(), self.has_active_edit_session());

        let ack = match intent {
            UiEditIntent::BeginEdit { client_edit_id, label } => {
                self.edits.push(Edit::BeginEditSession {
                    origin: EditOrigin::Ui,
                    label,
                    client_edit_id,
                    ui_client_instance_id: ui_client_instance_id.map(str::to_owned),
                });
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
                let result = self.apply_ui_stabilization_to_fixed_point(16);
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::SetParamControlState { node, state } => {
                match self.apply_set_param_control_state(0, node, state.into()) {
                    Ok(Some(effect)) => {
                        self.record_set_param_control_state_history(effect);
                        self.finish_ui_apply_now(before_len, Ok(()))
                    }
                    Ok(None) => self.finish_ui_apply_now(before_len, Ok(())),
                    Err(err) => self.finish_ui_apply_now(before_len, Err(err)),
                }
            }
            UiEditIntent::SetParamConstraints { node, constraints } => {
                match self.apply_set_param_constraints(0, node, constraints) {
                    Ok(Some(effect)) => {
                        self.record_set_param_constraints_history(effect);
                        self.finish_ui_apply_now(before_len, Ok(()))
                    }
                    Ok(None) => self.finish_ui_apply_now(before_len, Ok(())),
                    Err(err) => self.finish_ui_apply_now(before_len, Err(err)),
                }
            }
            UiEditIntent::MoveNode {
                node,
                new_parent,
                new_prev_sibling,
            } => {
                self.edits.push(Edit::MoveNode {
                    node,
                    new_parent,
                    new_prev_sibling,
                });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::RemoveNode { node } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Remove node",
                    "__ui-remove-node",
                    ui_client_instance_id,
                    |engine| {
                        engine.edits.push(Edit::RemoveNode { node });
                        engine.apply_ui_stabilization_to_fixed_point(16)
                    },
                );
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::RemoveNodes { nodes } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Remove nodes",
                    "__ui-remove-nodes",
                    ui_client_instance_id,
                    |engine| {
                        let mut seen = HashSet::<NodeId>::new();
                        for node in nodes {
                            if seen.insert(node) {
                                engine.edits.push(Edit::RemoveNode { node });
                            }
                        }
                        engine.apply_ui_stabilization_to_fixed_point(16)
                    },
                );
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::CreateUserItem {
                parent,
                node_type,
                label,
                initial_params,
            } => {
                let edit_label = label.clone().unwrap_or_else(|| "Create user item".to_string());
                let result = self.apply_implicit_ui_edit_session(
                    &edit_label,
                    "__ui-create-user-item",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_create_user_item(parent, node_type, label, initial_params),
                );
                self.finish_ui_apply_now(before_len, result)
            }
            UiEditIntent::DuplicateNode {
                source,
                new_parent,
                new_prev_sibling,
                label,
                initial_params: _,
            } => UiAck {
                success: false,
                status: UiAckStatus::Rejected,
                error_code: Some("duplicate_node_transport_required".to_string()),
                error_message: Some(format!(
                    "duplicateNode requires transport-level project codec support (source={}, new_parent={}, new_prev_sibling={:?}, label={:?})",
                    source.0, new_parent.0, new_prev_sibling, label
                )),
                earliest_event_time: None,
                history: self.ui_history_state(),
            },
            UiEditIntent::DuplicateNodes {
                nodes,
                created_items,
                dependent_items,
            } => UiAck {
                success: false,
                status: UiAckStatus::Rejected,
                error_code: Some("duplicate_nodes_transport_required".to_string()),
                error_message: Some(format!(
                    "duplicateNodes requires transport-level project codec support (nodes={}, created_items={}, dependent_items={})",
                    nodes.len(),
                    created_items.len(),
                    dependent_items.len()
                )),
                earliest_event_time: None,
                history: self.ui_history_state(),
            },
            UiEditIntent::FitAnimationCurvePath { curve, points, options } => {
                self.edits.push(Edit::CallNodeMutation {
                    node: curve,
                    callback: Box::new(move |node, ctx| {
                        let Some(curve_node) = node.as_any_mut().downcast_mut::<CurveNode>() else {
                            return Err("target node should be CurveNode".to_string());
                        };
                        curve_node.replace_range_with_fitted_samples(ctx, points.as_slice(), options)?;
                        Ok(())
                    }),
                });
                let result = self.apply_edits_to_fixed_point(16);
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
            UiEditIntent::UpsertUserContextEntry { owner, symbol, param } => {
                match self.upsert_user_context_entry(owner, symbol.as_str(), param) {
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
                }
            }
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
                self.push_ui_custom_event(
                    crate::logger::UI_LOG_MAX_ENTRIES_TOPIC,
                    None,
                    serde_json::json!({ "max_entries": applied_max_entries }),
                );
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

    fn apply_implicit_ui_edit_session<F>(
        &mut self,
        label: &str,
        client_edit_id: &str,
        ui_client_instance_id: Option<&str>,
        operation: F,
    ) -> Result<(), crate::engine::EngineEditError>
    where
        F: FnOnce(&mut Self) -> Result<(), crate::engine::EngineEditError>,
    {
        if self.has_active_edit_session() {
            return operation(self);
        }

        let client_edit_id = client_edit_id.to_string();
        self.edits.push(Edit::BeginEditSession {
            origin: EditOrigin::Ui,
            label: Some(label.to_string()),
            client_edit_id: client_edit_id.clone(),
            ui_client_instance_id: ui_client_instance_id.map(str::to_owned),
        });
        self.apply_edits()?;

        let operation_result = operation(self);
        self.edits.push(Edit::EndEditSession { client_edit_id });
        let end_result = self.apply_edits();

        operation_result?;
        end_result
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

    fn apply_edits_to_fixed_point(&mut self, max_passes: usize) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            self.apply_edits()?;
            if self.edits.pending.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "UiApplyFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!("ui intent left pending edits after {pass_limit} stabilization passes"),
        })
    }

    fn apply_ui_stabilization_to_fixed_point(
        &mut self,
        max_passes: usize,
    ) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            if !self.edits.pending.is_empty() {
                self.apply_edits()?;
            }

            if !self.inbox.events.is_empty() {
                self.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)?;
            }

            if self.edits.pending.is_empty() && self.inbox.events.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "UiApplyStabilizationFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!("ui intent left pending edits or inbox events after {pass_limit} stabilization passes"),
        })
    }

    fn apply_ui_initialization_to_fixed_point(
        &mut self,
        max_passes: usize,
    ) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            if !self.edits.pending.is_empty() {
                self.apply_edits_without_history()?;
            }

            if !self.inbox.events.is_empty() {
                self.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)?;
            }

            if self.edits.pending.is_empty() && self.inbox.events.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "UiApplyInitializationFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!(
                "ui initializer left pending edits or inbox events after {pass_limit} stabilization passes"
            ),
        })
    }

    /// Returns the current UI history summary for transport acknowledgements.
    pub fn ui_history_state(&self) -> UiHistoryState {
        UiHistoryState {
            can_undo: self.undo_len() > 0,
            can_redo: self.redo_len() > 0,
            undo_len: self.undo_len(),
            redo_len: self.redo_len(),
            active_edit_session: self.has_active_edit_session(),
            current_history_state_id: self.current_history_state_id(),
        }
    }

    pub(crate) fn ui_direct_children(&self, parent: NodeId) -> Option<Vec<NodeId>> {
        let mut children = Vec::new();
        let mut child = self.nodes.get(parent)?.node_data().first_child;
        while let Some(child_id) = child {
            children.push(child_id);
            child = self.nodes.get(child_id).and_then(|node| node.node_data().next_sibling);
        }
        Some(children)
    }

    fn ui_direct_children_cached(
        &self,
        parent: NodeId,
        cache: &mut HashMap<NodeId, Option<Vec<NodeId>>>,
    ) -> Option<Vec<NodeId>> {
        if let Some(children) = cache.get(&parent) {
            return children.clone();
        }
        let children = self.ui_direct_children(parent);
        cache.insert(parent, children.clone());
        children
    }

    pub(crate) fn ui_node_dto_for_event(&self, node_id: NodeId) -> Option<UiNodeDto> {
        let node = self.nodes.get(node_id)?;
        let node_data = node.node_data();
        let node_type = node.get_type().to_string();
        let children = self.ui_direct_children(node_id).unwrap_or_default();

        let data = if let Some(param) = node.engine_param_snapshot() {
            UiNodeDataDto::Parameter {
                param: UiParamDto::from(param),
            }
        } else {
            UiNodeDataDto::Node {
                node_type: node_type.clone(),
            }
        };

        let listed_user_items = node.user_creatable_items();
        let can_query_creatable_items = node.get_type() == FOLDER_NODE_TYPE
            || node.user_container_rules().is_some()
            || !listed_user_items.is_empty();

        let mut creatable_user_items = Vec::new();
        if can_query_creatable_items {
            for item in self.catalog_creatable_items(node_id).into_iter() {
                creatable_user_items.push(UiCreatableUserItemDto::from(item));
            }
        }

        let mut accepted_user_item_kinds: Vec<String> = node
            .user_container_rules()
            .map(|rules| {
                rules
                    .accepts_item_kinds
                    .iter()
                    .map(|kind| (*kind).to_string())
                    .collect()
            })
            .unwrap_or_default();
        for item in &listed_user_items {
            if !accepted_user_item_kinds.iter().any(|kind| kind == &item.item_kind) {
                accepted_user_item_kinds.push(item.item_kind.clone());
            }
        }

        let description = node_data
            .meta
            .description
            .as_deref()
            .or(node_data.meta.declared_description.as_deref())
            .or_else(|| node.type_description())
            .filter(|description| !description.trim().is_empty())
            .map(str::to_string);

        Some(UiNodeDto {
            node_id,
            uuid: node_data.meta.uuid,
            decl_id: node_data.meta.decl_id.clone(),
            node_type,
            meta: UiNodeMetaDto {
                short_name: node_data.meta.short_name.clone(),
                label: node_data.meta.label.clone(),
                enabled: node_data.meta.enabled,
                can_be_disabled: node_data.meta.can_be_disabled,
                user_permissions: node_data.meta.user_permissions.clone(),
                description,
                declared_description_key: None,
                description_overridden: false,
                tags: node_data.meta.tags.clone(),
                presentation: node_data.meta.presentation.clone(),
            },
            data,
            user_role: node_data.user_role,
            user_item_kind: node.user_item_kind().to_string(),
            accepted_user_item_kinds,
            creatable_user_items,
            children,
        })
    }

    fn ui_event_dto_with_child_order_cache(
        &self,
        event: &Event,
        child_order_cache: &mut HashMap<NodeId, Option<Vec<NodeId>>>,
    ) -> UiEventDto {
        let kind = match &event.kind {
            EventKind::ParamChanged {
                param,
                old_value,
                new_value,
            } => UiEventKind::ParamChanged {
                param: *param,
                old_value: old_value.clone(),
                new_value: new_value.clone(),
            },
            EventKind::ParamControlChanged {
                param,
                old_state,
                new_state,
            } => UiEventKind::ParamControlChanged {
                param: *param,
                old_state: old_state.clone().into(),
                new_state: new_state.clone().into(),
            },
            EventKind::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            } => UiEventKind::ParamConstraintsChanged {
                param: *param,
                old_constraints: old_constraints.clone(),
                new_constraints: new_constraints.clone(),
            },
            EventKind::ChildAdded { parent, child, decl_id } => UiEventKind::ChildAdded {
                parent: *parent,
                child: *child,
                decl_id: decl_id.clone(),
                parent_children: self.ui_direct_children_cached(*parent, child_order_cache),
            },
            EventKind::ChildRemoved { parent, child } => UiEventKind::ChildRemoved {
                parent: *parent,
                child: *child,
            },
            EventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => UiEventKind::ChildReplaced {
                parent: *parent,
                old: *old,
                new: *new,
                decl_id: decl_id.clone(),
            },
            EventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
            } => UiEventKind::ChildMoved {
                child: *child,
                old_parent: *old_parent,
                new_parent: *new_parent,
                old_parent_children: self.ui_direct_children_cached(*old_parent, child_order_cache),
                new_parent_children: self.ui_direct_children_cached(*new_parent, child_order_cache),
            },
            EventKind::ChildReordered { parent, child } => UiEventKind::ChildReordered {
                parent: *parent,
                child: *child,
                parent_children: self.ui_direct_children_cached(*parent, child_order_cache),
            },
            EventKind::NodeCreated { node } => UiEventKind::NodeCreated {
                node: *node,
                snapshot: self.ui_node_dto_for_event(*node),
            },
            EventKind::NodeDeleted { node } => UiEventKind::NodeDeleted { node: *node },
            EventKind::MetaChanged { node, patch } => UiEventKind::MetaChanged {
                node: *node,
                patch: patch.clone(),
            },
            EventKind::GraphTransaction { transaction } => UiEventKind::GraphTransaction {
                transaction: transaction.clone(),
            },
            EventKind::Custom(custom) => UiEventKind::Custom {
                topic: custom.topic.clone(),
                origin: custom.origin,
                payload: custom.payload.clone(),
            },
        };

        UiEventDto { time: event.time, kind }
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
                    EventKind::ChildAdded { .. }
                        | EventKind::ChildRemoved { .. }
                        | EventKind::ChildReplaced { .. }
                        | EventKind::ChildMoved { .. }
                        | EventKind::ChildReordered { .. }
                        | EventKind::NodeCreated { .. }
                        | EventKind::NodeDeleted { .. }
                        | EventKind::GraphTransaction { .. }
                ) {
                    return true;
                }

                let candidate_nodes: Vec<NodeId> = match &event.kind {
                    EventKind::ParamChanged { param, .. } => vec![*param],
                    EventKind::ParamControlChanged { param, .. } => vec![*param],
                    EventKind::ParamConstraintsChanged { param, .. } => vec![*param],
                    EventKind::ChildAdded { parent, child, .. } => vec![*parent, *child],
                    EventKind::ChildRemoved { parent, child } => vec![*parent, *child],
                    EventKind::ChildReplaced { parent, old, new, .. } => vec![*parent, *old, *new],
                    EventKind::ChildMoved {
                        child,
                        old_parent,
                        new_parent,
                    } => vec![*child, *old_parent, *new_parent],
                    EventKind::ChildReordered { parent, child } => vec![*parent, *child],
                    EventKind::NodeCreated { node } => vec![*node],
                    EventKind::NodeDeleted { node } => vec![*node],
                    EventKind::MetaChanged { node, .. } => vec![*node],
                    EventKind::GraphTransaction { .. } => return true,
                    EventKind::Custom(custom) => custom.origin.into_iter().collect(),
                };

                candidate_nodes
                    .into_iter()
                    .any(|node| self.node_within_subtree(node, *root, *max_depth))
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

fn ui_error_code(error: &crate::engine::EngineEditError) -> &'static str {
    match error {
        crate::engine::EngineEditError::NodeTypeMismatch { .. } => "node_type_mismatch",
        crate::engine::EngineEditError::ParamEditTargetMismatch { .. } => "param_edit_target_mismatch",
        crate::engine::EngineEditError::ParamConstraintViolation { .. } => "param_constraint_violation",
        crate::engine::EngineEditError::ParamConstraintsRejected { .. } => "param_constraints_rejected",
        crate::engine::EngineEditError::ParamControlStateRejected { .. } => "param_control_error",
        crate::engine::EngineEditError::ScriptConfigRejected { .. } => "script_config_rejected",
        crate::engine::EngineEditError::ScriptPropertyRejected { .. } => "script_property_rejected",
        crate::engine::EngineEditError::ScriptMethodRejected { .. } => "script_method_rejected",
        crate::engine::EngineEditError::NodeMutationRejected { .. } => "node_mutation_rejected",
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
