use super::*;

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
        param: Box<UiParamDto>,
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
    /// Whether the Add menu should render a divider immediately above this item.
    #[serde(default, skip_serializing_if = "is_false")]
    pub separator_before: bool,
    /// Optional icon shown for this item in Add menus, as a data URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
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
