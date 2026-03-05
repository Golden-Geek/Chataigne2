use std::any::Any;
use std::collections::HashMap;

use crate::color::Color;
use crate::edit::Edit;
use crate::engine::NodeExecutionRule;
use crate::events::{CustomEvent, Event, EventKind};
use crate::parameter::{ParamValue, ParamValueProjection, Parameter, ParameterChangeCheck, ParameterControlState, ParameterSnapshot};
use crate::process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot};
use crate::script::{ScriptHostPolicy, ScriptNode, ScriptNodeConfig, ScriptUiState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod animation_curve_nodes;
mod control_animation;
mod handles;
pub use animation_curve_nodes::{AnimationCurveEasingNode, AnimationCurveKeyNode, AnimationCurveNode, curve_from_snapshot};
pub use control_animation::ParameterAnimationControlNode;
pub use handles::{NodeHandle, ParameterHandle, ParameterValueType, PotentialNodeHandle};

/// Stable engine identifier for a node stored in [`crate::engine::node_store::NodeStore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// Persistent UUID assigned to node metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeUuid(pub Uuid);

impl NodeUuid {
    /// Returns the nil UUID value.
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Returns `true` when this UUID is nil.
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for NodeUuid {
    fn default() -> Self {
        Self::nil()
    }
}

/// Persistent reference to another node.
///
/// The UUID is the source of truth and persists on disk.
/// The cached runtime id is optional and never serialized.
/// A cached display name is persisted to keep dangling references user-readable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeReference {
    /// Persistent target identity.
    pub uuid: NodeUuid,
    /// Optional projection applied when resolving source value from this target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<ParamValueProjection>,
    /// Cached user-facing target name used when the reference is currently unresolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_name: Option<String>,
    /// Best-effort runtime cache for faster lookups.
    #[serde(skip, default)]
    pub cached_id: Option<NodeId>,
    /// Optional relative path from the resolved root to this target.
    ///
    /// Each segment is a child `decl_id` under the previous segment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relative_path_from_root: Vec<String>,
}

impl NodeReference {
    /// Creates a reference from a persistent UUID.
    pub fn new(uuid: NodeUuid) -> Self {
        Self {
            uuid,
            projection: None,
            cached_name: None,
            cached_id: None,
            relative_path_from_root: Vec::new(),
        }
    }

    /// Creates an empty reference with a nil UUID.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a reference with an explicit runtime cache.
    pub fn with_cached_id(uuid: NodeUuid, cached_id: Option<NodeId>) -> Self {
        Self {
            uuid,
            projection: None,
            cached_name: None,
            cached_id,
            relative_path_from_root: Vec::new(),
        }
    }

    /// Creates a reference with explicit runtime cache and relative root path.
    pub fn with_hints(uuid: NodeUuid, cached_id: Option<NodeId>, relative_path_from_root: Vec<String>) -> Self {
        Self {
            uuid,
            projection: None,
            cached_name: None,
            cached_id,
            relative_path_from_root,
        }
    }

    /// Returns the persistent target UUID.
    pub fn uuid(&self) -> NodeUuid {
        self.uuid
    }

    /// Returns the optional projection attached to this reference.
    pub fn projection(&self) -> Option<ParamValueProjection> {
        self.projection
    }

    /// Sets or clears the projection attached to this reference.
    pub fn set_projection(&mut self, projection: Option<ParamValueProjection>) {
        self.projection = projection;
    }

    /// Clears the projection attached to this reference.
    pub fn clear_projection(&mut self) {
        self.projection = None;
    }

    /// Returns the cached runtime node id, when available.
    pub fn cached_id(&self) -> Option<NodeId> {
        self.cached_id
    }

    /// Replaces the cached runtime id.
    pub fn set_cached_id(&mut self, cached_id: Option<NodeId>) {
        self.cached_id = cached_id;
    }

    /// Clears the cached runtime id.
    pub fn clear_cached_id(&mut self) {
        self.cached_id = None;
    }

    /// Returns the cached user-facing target name, when available.
    pub fn cached_name(&self) -> Option<&str> {
        self.cached_name.as_deref()
    }

    /// Replaces the cached user-facing target name.
    pub fn set_cached_name(&mut self, cached_name: Option<String>) {
        self.cached_name = cached_name.filter(|name| !name.trim().is_empty());
    }

    /// Clears the cached user-facing target name.
    pub fn clear_cached_name(&mut self) {
        self.cached_name = None;
    }

    /// Returns the relative path from resolved root to target.
    pub fn relative_path_from_root(&self) -> &[String] {
        &self.relative_path_from_root
    }

    /// Replaces the relative path hint used for root-relative recovery.
    pub fn set_relative_path_from_root(&mut self, relative_path_from_root: Vec<String>) {
        self.relative_path_from_root = relative_path_from_root;
    }

    /// Clears the relative path hint used for root-relative recovery.
    pub fn clear_relative_path_from_root(&mut self) {
        self.relative_path_from_root.clear();
    }

    /// Returns `true` when this reference has no persistent target.
    pub fn is_empty(&self) -> bool {
        self.uuid.is_nil()
    }
}

impl Default for NodeReference {
    fn default() -> Self {
        Self::new(NodeUuid::default())
    }
}

impl PartialEq for NodeReference {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid && self.projection == other.projection
    }
}

impl Eq for NodeReference {}

/// Declaration identifier used to refer to node definitions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeclId(pub String);

/// Semantic hints used for tooling, UX, and interpretation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SemanticsHint {
    /// Optional high-level intent of the node.
    pub intent: Option<String>,
    /// Optional unit for value-oriented nodes.
    pub unit: Option<String>,
}

/// Warning message shown in UI for a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeWarning {
    /// Warning identifier. Empty string is the default id.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Main warning message.
    pub message: String,
    /// Optional warning details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl NodeWarning {
    /// Creates a warning with the default empty id.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            id: String::new(),
            message: message.into(),
            detail: None,
        }
    }

    /// Sets or replaces the warning id.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Sets warning detail text.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl Default for NodeWarning {
    fn default() -> Self {
        Self::new("")
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Presentation hints used for editor rendering.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PresentationHint {
    /// Preferred UI color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Warnings attached to this node, keyed by warning id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<NodeWarning>,
    /// If greater than zero, this node surfaces descendant warnings up to this depth.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub show_child_warnings_max_depth: u32,
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn parameter_node_type_from_value(value: &ParamValue) -> &'static str {
    match value {
        ParamValue::Trigger() => "trigger",
        ParamValue::Int(_) => "int",
        ParamValue::Float(_) => "float",
        ParamValue::Str(_) => "str",
        ParamValue::File(_) => "file",
        ParamValue::Enum(_) => "enum",
        ParamValue::Bool(_) => "bool",
        ParamValue::Vec2(_, _) => "vec2",
        ParamValue::Vec3(_, _, _) => "vec3",
        ParamValue::Color(_, _, _, _) => "color",
        ParamValue::Reference(_) => "reference",
    }
}

fn default_parameter_value_for_node_type(node_type: &str) -> Option<ParamValue> {
    match node_type {
        "trigger" => Some(ParamValue::Trigger()),
        "int" => Some(ParamValue::Int(0)),
        "float" => Some(ParamValue::Float(0.0)),
        "str" => Some(ParamValue::Str(String::new())),
        "file" => Some(ParamValue::File(String::new())),
        "enum" => Some(ParamValue::Enum(String::new())),
        "bool" => Some(ParamValue::Bool(false)),
        "vec2" => Some(ParamValue::Vec2(0.0, 0.0)),
        "vec3" => Some(ParamValue::Vec3(0.0, 0.0, 0.0)),
        "color" => Some(ParamValue::Color(1.0, 1.0, 1.0, 1.0)),
        "reference" => Some(ParamValue::Reference(NodeReference::default())),
        _ => None,
    }
}

struct ScriptChildLookup {
    primary: Option<NodeId>,
    primary_matches_type: bool,
    duplicates: Vec<NodeId>,
}

fn script_child_matches_key(node: &ProcessTreeNodeSnapshot, key: &str) -> bool {
    let key = key.trim();
    if key.is_empty() {
        return false;
    }

    node.decl_id.eq_ignore_ascii_case(key) || node.short_name.eq_ignore_ascii_case(key) || node.label.eq_ignore_ascii_case(key)
}

fn lookup_script_child_by_key_and_type(ctx: &ProcessCtx, parent: NodeId, key: &str, expected_node_type: &str) -> ScriptChildLookup {
    let mut matches = Vec::new();
    let mut same_type_matches = Vec::new();

    if let Some(snapshot) = ctx.tree_snapshot() {
        let mut child = snapshot.node(parent).and_then(|node| node.first_child);
        while let Some(child_id) = child {
            let Some(child_snapshot) = snapshot.node(child_id) else {
                break;
            };

            if script_child_matches_key(child_snapshot, key) {
                matches.push(child_id);
                if child_snapshot.node_type.eq_ignore_ascii_case(expected_node_type) {
                    same_type_matches.push(child_id);
                }
            }

            child = child_snapshot.next_sibling;
        }
    }

    let (primary, primary_matches_type) = if let Some(node) = same_type_matches.first().copied() {
        (Some(node), true)
    } else if let Some(node) = matches.first().copied() {
        (Some(node), false)
    } else {
        (None, false)
    };

    let duplicates = matches.into_iter().filter(|candidate| Some(*candidate) != primary).collect::<Vec<_>>();
    ScriptChildLookup { primary, primary_matches_type, duplicates }
}

impl PresentationHint {
    /// Sets or replaces a warning by id.
    pub fn set_warning(&mut self, mut warning: NodeWarning) {
        if warning.id.is_empty() {
            warning.id = String::new();
        }

        if let Some(existing) = self.warnings.iter_mut().find(|existing| existing.id == warning.id) {
            *existing = warning;
            return;
        }

        self.warnings.push(warning);
    }

    /// Convenience wrapper for setting/replacing a warning by `warning_id`.
    ///
    /// `None` uses the default empty warning id.
    pub fn set_warning_message(&mut self, warning_id: Option<&str>, message: impl Into<String>, detail: Option<String>) {
        let warning = NodeWarning {
            id: warning_id.unwrap_or_default().to_string(),
            message: message.into(),
            detail,
        };
        self.set_warning(warning);
    }

    /// Clears one warning by id.
    ///
    /// `None` clears the warning using the default empty id.
    /// Returns `true` when a warning was removed.
    pub fn clear_warning(&mut self, warning_id: Option<&str>) -> bool {
        let warning_id = warning_id.unwrap_or_default();
        let Some(index) = self.warnings.iter().position(|warning| warning.id == warning_id) else {
            return false;
        };

        self.warnings.remove(index);
        true
    }

    /// Clears all warnings attached to this node.
    ///
    /// Returns `true` when at least one warning was removed.
    pub fn clear_warnings(&mut self) -> bool {
        if self.warnings.is_empty() {
            return false;
        }

        self.warnings.clear();
        true
    }

    /// Returns `true` when this node has at least one warning.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Returns the warning associated with `warning_id`.
    ///
    /// `None` addresses the default empty warning id.
    pub fn warning(&self, warning_id: Option<&str>) -> Option<&NodeWarning> {
        let warning_id = warning_id.unwrap_or_default();
        self.warnings.iter().find(|warning| warning.id == warning_id)
    }

    /// Sets the descendant warning visibility depth.
    ///
    /// Returns `true` when the value changed.
    pub fn set_child_warning_depth(&mut self, max_depth: u32) -> bool {
        if self.show_child_warnings_max_depth == max_depth {
            return false;
        }

        self.show_child_warnings_max_depth = max_depth;
        true
    }
}

/// User-edit permissions for UI tooling and editor workflows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeUserPermissions {
    /// Whether the node label can be edited by users.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_edit_name: bool,
    /// Whether the node can be removed or duplicated.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_remove_and_duplicate: bool,
    /// Whether parameter constraints can be edited.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_edit_constraints: bool,
    /// Whether metadata tags can be edited.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_edit_tags: bool,
    /// Whether presentation color can be edited.
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_edit_color: bool,
}

impl NodeUserPermissions {
    /// Returns a permission set with every capability disabled.
    pub const fn none() -> Self {
        Self {
            can_edit_name: false,
            can_remove_and_duplicate: false,
            can_edit_constraints: false,
            can_edit_tags: false,
            can_edit_color: false,
        }
    }

    /// Returns a permission set with every capability enabled.
    pub const fn all() -> Self {
        Self {
            can_edit_name: true,
            can_remove_and_duplicate: true,
            can_edit_constraints: true,
            can_edit_tags: true,
            can_edit_color: true,
        }
    }
}

/// Classification for user-managed structure inside the runtime tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum UserNodeRole {
    /// Regular runtime node (internal/generated or non-curated).
    #[default]
    Regular,
    /// User-curated item root inside a container.
    ItemRoot,
}

/// Declarative container admission rules for user-curated items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserContainerRules {
    /// Supported item kinds accepted by this container.
    pub accepts_item_kinds: &'static [&'static str],
}

impl UserContainerRules {
    /// Creates a new static container rule set.
    pub const fn new(accepts_item_kinds: &'static [&'static str]) -> Self {
        Self { accepts_item_kinds }
    }

    /// Returns `true` when `item_kind` is accepted by this container.
    ///
    /// The wildcard `"*"` accepts any item kind.
    pub fn accepts(&self, item_kind: &str) -> bool {
        self.accepts_item_kinds.iter().any(|kind| *kind == "*" || *kind == item_kind)
    }
}

/// UI-facing descriptor for a user-creatable item node type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCreatableItem {
    /// Runtime node type identifier.
    pub node_type: String,
    /// Logical user-item kind used for container admission.
    pub item_kind: String,
    /// Suggested default label for newly created items.
    pub label: String,
}

impl UserCreatableItem {
    /// Creates a new user-creatable item descriptor.
    pub fn new(node_type: impl Into<String>, item_kind: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            node_type: node_type.into(),
            item_kind: item_kind.into(),
            label: label.into(),
        }
    }
}

/// Built-in node type id used for user-authored lexical context scopes.
pub const USER_CONTEXT_NODE_TYPE: &str = "user_context";
/// Built-in user-item kind used for user-authored lexical context scopes.
pub const USER_CONTEXT_ITEM_KIND: &str = "user_context";
/// Default user-facing label for newly created user-context scope nodes.
pub const USER_CONTEXT_DEFAULT_LABEL: &str = "Context";
/// Built-in folder node type id.
pub const FOLDER_NODE_TYPE: &str = "folder";
/// Built-in item kind used for parameter control helper nodes.
pub const PARAMETER_CONTROL_ITEM_KIND: &str = "parameter_control";
/// Built-in node type id for animation control nodes attached to parameters.
pub const PARAMETER_ANIMATION_CONTROL_NODE_TYPE: &str = "parameter_animation_control";
/// Built-in `decl_id` for parameter animation control nodes.
pub const PARAMETER_ANIMATION_CONTROL_DECL_ID: &str = "animation_control";
/// Built-in node type id for animation-curve container nodes.
pub const PARAMETER_ANIMATION_CURVE_NODE_TYPE: &str = "animation_curve";
/// Built-in item kind used by animation-curve container nodes.
pub const PARAMETER_ANIMATION_CURVE_ITEM_KIND: &str = "animation_curve";
/// Built-in `decl_id` for the animation-curve child under one animation control.
pub const PARAMETER_ANIMATION_CURVE_DECL_ID: &str = "curve";
/// Built-in node type id for animation-curve key nodes.
pub const PARAMETER_ANIMATION_KEY_NODE_TYPE: &str = "animation_curve_key";
/// Built-in item kind used by animation-curve key nodes.
pub const PARAMETER_ANIMATION_KEY_ITEM_KIND: &str = "animation_curve_key";
/// Built-in `decl_id` for key position parameter nodes.
pub const PARAMETER_ANIMATION_KEY_POSITION_DECL_ID: &str = "position";
/// Built-in `decl_id` for key value parameter nodes.
pub const PARAMETER_ANIMATION_KEY_VALUE_DECL_ID: &str = "value";
/// Built-in node type id for animation-curve easing nodes.
pub const PARAMETER_ANIMATION_EASING_NODE_TYPE: &str = "animation_curve_easing";
/// Built-in `decl_id` for easing nodes under key nodes.
pub const PARAMETER_ANIMATION_EASING_DECL_ID: &str = "easing";
/// Built-in `decl_id` for easing kind selector.
pub const PARAMETER_ANIMATION_EASING_KIND_DECL_ID: &str = "kind";
/// Built-in `decl_id` for easing coordinate-space selector.
pub const PARAMETER_ANIMATION_EASING_COORDINATE_SPACE_DECL_ID: &str = "coordinate_space";
/// Built-in `decl_id` for bezier out-handle position.
pub const PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID: &str = "out_position";
/// Built-in `decl_id` for bezier out-handle value.
pub const PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID: &str = "out_value";
/// Built-in `decl_id` for bezier in-handle position.
pub const PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID: &str = "in_position";
/// Built-in `decl_id` for bezier in-handle value.
pub const PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID: &str = "in_value";
/// Built-in `decl_id` for steps mode selector.
pub const PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID: &str = "step_mode";
/// Built-in `decl_id` for steps size parameter.
pub const PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID: &str = "step_size";
/// Built-in `decl_id` for steps count parameter.
pub const PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID: &str = "num_steps";
/// Built-in `decl_id` for shape selector.
pub const PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID: &str = "shape";
/// Built-in `decl_id` for shape/noise amplitude parameter.
pub const PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID: &str = "amplitude";
/// Built-in `decl_id` for shape phase-mode selector.
pub const PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID: &str = "phase_mode";
/// Built-in `decl_id` for shape/noise/random frequency parameter.
pub const PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID: &str = "frequency";
/// Built-in `decl_id` for shape phase-count parameter.
pub const PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID: &str = "num_phases";
/// Built-in `decl_id` for easing fade-in parameter.
pub const PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID: &str = "fade_in";
/// Built-in `decl_id` for easing fade-out parameter.
pub const PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID: &str = "fade_out";
/// Built-in `decl_id` for perlin-noise octave count.
pub const PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID: &str = "octaves";
/// Built-in `decl_id` for perlin-noise phase parameter.
pub const PARAMETER_ANIMATION_EASING_PHASE_DECL_ID: &str = "phase";
/// Built-in `decl_id` for random seed parameter.
pub const PARAMETER_ANIMATION_EASING_SEED_DECL_ID: &str = "seed";
/// Built-in `decl_id` for script easing source.
pub const PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID: &str = "script_source";
/// Built-in child `decl_id` for control reference parameters used by `proxy`/`binding`.
pub const PARAMETER_CONTROL_REFERENCE_DECL_ID: &str = "reference";
/// Built-in child `decl_id` for expression source text on controlled parameters.
pub const PARAMETER_EXPRESSION_SOURCE_DECL_ID: &str = "expression";
/// Built-in child `decl_id` for animation waveform selector.
pub const PARAMETER_ANIMATION_WAVEFORM_DECL_ID: &str = "waveform";
/// Built-in child `decl_id` for animation frequency parameter.
pub const PARAMETER_ANIMATION_FREQUENCY_DECL_ID: &str = "frequency_hz";
/// Built-in child `decl_id` for animation amplitude parameter.
pub const PARAMETER_ANIMATION_AMPLITUDE_DECL_ID: &str = "amplitude";
/// Built-in child `decl_id` for animation offset parameter.
pub const PARAMETER_ANIMATION_OFFSET_DECL_ID: &str = "offset";
/// Built-in child `decl_id` for animation phase parameter.
pub const PARAMETER_ANIMATION_PHASE_DECL_ID: &str = "phase";
/// Built-in child `decl_id` for animation node local update rate in hertz.
pub const PARAMETER_ANIMATION_UPDATE_RATE_DECL_ID: &str = "update_rate_hz";
/// All built-in parameter node type ids.
pub const PARAMETER_NODE_TYPES: [&str; 11] = ["trigger", "int", "float", "str", "file", "enum", "bool", "vec2", "vec3", "color", "reference"];
const USER_CONTEXT_ALLOWED_ITEM_KINDS: [&str; 13] = [FOLDER_NODE_TYPE, "trigger", "int", "float", "str", "file", "enum", "bool", "vec2", "vec3", "color", "reference", "*"];

/// Runtime node links and metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    /// Stable node id assigned by the store.
    pub id: NodeId,
    /// Parent node id (`None` for root or detached nodes).
    pub parent: Option<NodeId>,
    /// First child in sibling chain.
    pub first_child: Option<NodeId>,
    /// Last child in sibling chain.
    pub last_child: Option<NodeId>,
    /// Previous sibling in parent chain.
    pub prev_sibling: Option<NodeId>,
    /// Next sibling in parent chain.
    pub next_sibling: Option<NodeId>,
    /// User-facing role for curation and container logic.
    pub user_role: UserNodeRole,
    /// User-facing metadata.
    pub meta: NodeMeta,
}

impl NodeData {
    /// Creates detached node data initialized with default metadata.
    pub fn new(label: String) -> Self {
        // println!("New node data, label: {}", label);
        let meta = NodeMeta::new(label);

        Self {
            id: NodeId(0),
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            user_role: UserNodeRole::Regular,
            meta,
        }
    }
}

/// Metadata associated with a node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeMeta {
    /// Persistent unique id for this node instance.
    pub uuid: NodeUuid,
    /// Declaration id used by schema and tools.
    pub decl_id: DeclId,
    /// Generated short name.
    pub short_name: String,
    /// Whether the node is currently enabled.
    pub enabled: bool,
    /// Whether the enabled flag may be toggled.
    pub can_be_disabled: bool,
    /// User-visible label.
    pub label: String,
    /// Optional free-form description.
    pub description: Option<String>,
    /// Arbitrary classification tags.
    pub tags: Vec<String>,
    /// User-edit permissions for tooling and editor workflows.
    pub user_permissions: NodeUserPermissions,
    /// Semantic hints.
    pub semantics: SemanticsHint,
    /// Presentation hints.
    pub presentation: PresentationHint,
}

impl NodeMeta {
    /// Creates default metadata from a label.
    pub fn new(label: String) -> Self {
        let short_name = Self::generate_short_name(&label);

        Self {
            uuid: NodeUuid(Uuid::new_v4()),
            decl_id: DeclId(short_name.clone()),
            short_name,
            enabled: true,
            can_be_disabled: true,
            label,
            description: None,
            tags: vec![],
            user_permissions: NodeUserPermissions::default(),
            semantics: SemanticsHint::default(),
            presentation: PresentationHint::default(),
        }
    }

    /// Sets a description.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Replaces tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Replaces user-edit permissions.
    pub fn with_user_permissions(mut self, user_permissions: NodeUserPermissions) -> Self {
        self.user_permissions = user_permissions;
        self
    }

    /// Replaces semantic hints.
    pub fn with_semantics(mut self, semantics: SemanticsHint) -> Self {
        self.semantics = semantics;
        self
    }

    /// Replaces presentation hints.
    pub fn with_presentation(mut self, presentation: PresentationHint) -> Self {
        self.presentation = presentation;
        self
    }

    /// Sets enablement metadata.
    pub fn with_enabled(mut self, enabled: bool, can_be_disabled: bool) -> Self {
        self.enabled = enabled;
        self.can_be_disabled = can_be_disabled;
        self
    }

    /// Sets or replaces one warning by id.
    ///
    /// `warning_id = None` uses the default empty warning id.
    pub fn set_warning(&mut self, warning_id: Option<&str>, message: impl Into<String>, detail: Option<&str>) {
        self.presentation.set_warning_message(warning_id, message, detail.map(str::to_string));
    }

    /// Clears one warning by id.
    ///
    /// `warning_id = None` clears the default empty warning id.
    pub fn clear_warning(&mut self, warning_id: Option<&str>) -> bool {
        self.presentation.clear_warning(warning_id)
    }

    /// Clears all node warnings.
    pub fn clear_warnings(&mut self) -> bool {
        self.presentation.clear_warnings()
    }

    /// Sets how many child levels should be included when surfacing warnings.
    ///
    /// Returns `true` when the value changed.
    pub fn set_child_warning_depth(&mut self, max_depth: u32) -> bool {
        self.presentation.set_child_warning_depth(max_depth)
    }

    fn generate_short_name(label: &String) -> String {
        //from label to lowerCamelCase, "+" and "-" are replaced by "Plus" and "Minus", other are removed
        let mut short_name = String::new();
        let mut capitalize_next = false;
        for c in label.chars() {
            if c.is_alphanumeric() {
                if capitalize_next {
                    short_name.push(c.to_ascii_uppercase());
                    capitalize_next = false;
                } else {
                    short_name.push(c.to_ascii_lowercase());
                }
            } else if c == '+' {
                short_name.push_str(if capitalize_next { "Plus" } else { "plus" });
                capitalize_next = false;
            } else if c == '-' {
                short_name.push_str(if capitalize_next { "Minus" } else { "minus" });
                capitalize_next = false;
            } else {
                capitalize_next = true;
            }
        }
        short_name
    }
}

/// Patch payload for metadata updates.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeMetaPatch {
    /// Optional short name replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// Optional enabled-state replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Optional disablement capability replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_be_disabled: Option<bool>,
    /// Optional label replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional description replacement (`Some(None)` clears the description).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Option<String>>,
    /// Optional tags replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Optional user-edit permissions replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_permissions: Option<NodeUserPermissions>,
    /// Optional semantic hints replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<SemanticsHint>,
    /// Optional presentation hints replacement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationHint>,
}

/// Script-facing property/method metadata provided by one node instance.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeScriptDescriptor {
    /// Script-readable properties exposed on the node object.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, ParamValue>,
    /// Script-callable method names exposed on the node object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
}

fn push_unique_script_method(methods: &mut Vec<String>, method: &str) {
    if methods.iter().any(|candidate| candidate == method) {
        return;
    }
    methods.push(method.to_string());
}

pub(crate) fn core_node_script_descriptor(node_data: &NodeData, node_type: &str) -> NodeScriptDescriptor {
    let mut descriptor = NodeScriptDescriptor::default();
    descriptor.properties.insert("name".to_string(), ParamValue::Str(node_data.meta.label.clone()));
    descriptor.properties.insert("enabled".to_string(), ParamValue::Bool(node_data.meta.enabled));
    descriptor.properties.insert("type".to_string(), ParamValue::Str(node_type.to_string()));
    descriptor.properties.insert("declId".to_string(), ParamValue::Str(node_data.meta.decl_id.0.clone()));

    for method in [
        "setName",
        "setEnabled",
        "setDescription",
        "setReadOnly",
        "addNode",
        "removeNode",
        "addParameter",
        "removeParameter",
        "addFolder",
        "setParam",
        "listen",
        "unlisten",
        "getProperties",
        "getChildren",
        "getChild",
        "toString",
    ] {
        push_unique_script_method(&mut descriptor.methods, method);
    }

    descriptor
}

impl NodeMetaPatch {
    /// Applies this patch to runtime metadata.
    pub fn apply_to(&self, meta: &mut NodeMeta) {
        if let Some(short_name) = &self.short_name {
            meta.short_name = short_name.clone();
        }
        if let Some(enabled) = self.enabled {
            meta.enabled = enabled;
        }
        if let Some(can_be_disabled) = self.can_be_disabled {
            meta.can_be_disabled = can_be_disabled;
        }
        if let Some(label) = &self.label {
            meta.label = label.clone();
        }
        if let Some(description) = &self.description {
            meta.description = description.clone();
        }
        if let Some(tags) = &self.tags {
            meta.tags = tags.clone();
        }
        if let Some(user_permissions) = &self.user_permissions {
            meta.user_permissions = user_permissions.clone();
        }
        if let Some(semantics) = &self.semantics {
            meta.semantics = semantics.clone();
        }
        if let Some(presentation) = &self.presentation {
            meta.presentation = presentation.clone();
        }
    }
}

/// Controls whether an event reaching a node should notify, pass through, or stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPropagation {
    /// Notify this node when it is interested and continue bubbling.
    Notify,
    /// Do not notify this node, but still allow bubbling to continue.
    PassOn,
    /// Notify this node when it is interested and stop bubbling.
    Stop,
}

/// Runtime listener subscription targeting a node and optional subtree depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventSubscription {
    /// Subscription root node id.
    pub node: NodeId,
    /// Maximum descendant depth from `node` that should match.
    ///
    /// `0` matches only `node`, `1` includes direct children, and so on.
    pub max_depth: u32,
}

impl EventSubscription {
    /// Creates a subscription matching events originating exactly from `node`.
    pub fn node(node: NodeId) -> Self {
        Self { node, max_depth: 0 }
    }

    /// Creates a subscription matching `node` and descendants up to `max_depth`.
    pub fn subtree(node: NodeId, max_depth: u32) -> Self {
        Self { node, max_depth }
    }
}

/// Conversion helper for macro-generated script-host policy methods.
pub trait IntoScriptHostPolicyOption {
    /// Converts a value into an optional script-host policy.
    fn into_script_host_policy_option(self) -> Option<ScriptHostPolicy>;
}

impl IntoScriptHostPolicyOption for ScriptHostPolicy {
    fn into_script_host_policy_option(self) -> Option<ScriptHostPolicy> {
        Some(self)
    }
}

impl IntoScriptHostPolicyOption for Option<ScriptHostPolicy> {
    fn into_script_host_policy_option(self) -> Option<ScriptHostPolicy> {
        self
    }
}

/// User-context host policy for one node type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserContextHostPolicy {
    /// Whether `UserContextNode` creation is enabled for this node.
    pub enabled: bool,
}

impl Default for UserContextHostPolicy {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl UserContextHostPolicy {
    /// Default policy used by `#[node(contextualizable)]` and `#[item(..., contextualizable)]`.
    pub fn default_contextualizable() -> Self {
        Self { enabled: true }
    }
}

/// Conversion helper for macro-generated user-context host policy methods.
pub trait IntoUserContextHostPolicyOption {
    /// Converts a value into an optional user-context host policy.
    fn into_user_context_host_policy_option(self) -> Option<UserContextHostPolicy>;
}

impl IntoUserContextHostPolicyOption for UserContextHostPolicy {
    fn into_user_context_host_policy_option(self) -> Option<UserContextHostPolicy> {
        Some(self)
    }
}

impl IntoUserContextHostPolicyOption for Option<UserContextHostPolicy> {
    fn into_user_context_host_policy_option(self) -> Option<UserContextHostPolicy> {
        self
    }
}

/// Behavior contract implemented by all node types.
pub trait Node: Send + Any {
    /// Returns immutable runtime node data.
    fn node_data(&self) -> &NodeData;
    /// Returns mutable runtime node data.
    fn node_data_mut(&mut self) -> &mut NodeData;

    /// Returns the node type identifier.
    fn get_type(&self) -> &str;

    /// Returns the item-kind identifier used by container admission rules.
    ///
    /// By default this matches [`Self::get_type`].
    fn user_item_kind(&self) -> &str {
        self.get_type()
    }

    /// Returns `true` when this node type is declared with `#[item(...)]`.
    ///
    /// This marker is used by creation-time metadata inference (for example, default
    /// user permissions) and should not depend on runtime state.
    fn is_declared_user_item(&self) -> bool {
        false
    }

    /// Returns container admission rules when this node accepts user-curated items.
    ///
    /// Nodes that are not containers return `None`.
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        None
    }

    /// Returns scripting host policy when this node supports hosting script nodes.
    fn script_host_policy(&self) -> Option<ScriptHostPolicy> {
        None
    }

    /// Returns user-context host policy when this node supports hosting `UserContextNode`.
    fn user_context_host_policy(&self) -> Option<UserContextHostPolicy> {
        None
    }

    /// Engine-internal hook used by UI tooling to expose script runtime state.
    #[doc(hidden)]
    fn engine_script_state(&self) -> Option<ScriptUiState> {
        None
    }

    /// Engine-internal hook used by UI tooling to replace script configuration.
    #[doc(hidden)]
    fn engine_set_script_config(&mut self, _config: ScriptNodeConfig, _force_reload: bool) -> Result<(), String> {
        Err(format!("node type '{}' does not support script configuration", self.get_type()))
    }

    /// Engine-internal hook used by UI tooling to request script runtime reload.
    #[doc(hidden)]
    fn engine_request_script_reload(&mut self) -> Result<(), String> {
        Err(format!("node type '{}' does not support script reload", self.get_type()))
    }

    /// Returns `true` when this container currently accepts `item_type` / `item_kind`.
    ///
    /// The default implementation checks [`Self::user_container_rules`].
    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        if item_type == "script" && item_kind == "script" {
            return self.script_host_policy().is_some_and(|policy| policy.enabled);
        }
        if (item_type == USER_CONTEXT_NODE_TYPE || item_type == "context") && item_kind == USER_CONTEXT_ITEM_KIND {
            return self.user_context_host_policy().is_some_and(|policy| policy.enabled);
        }
        let _ = item_type;
        self.user_container_rules().is_some_and(|rules| rules.accepts(item_kind))
    }

    /// Returns user-creatable item descriptors for this node instance.
    ///
    /// Nodes that do not create items return an empty list.
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = Vec::new();
        if self.script_host_policy().is_some_and(|policy| policy.enabled) {
            items.push(UserCreatableItem::new("script", "script", "Script"));
        }
        if self.user_context_host_policy().is_some_and(|policy| policy.enabled) {
            items.push(UserCreatableItem::new(USER_CONTEXT_NODE_TYPE, USER_CONTEXT_ITEM_KIND, USER_CONTEXT_DEFAULT_LABEL));
        }
        items
    }

    /// Creates one user item for `node_type` with `label` when supported.
    ///
    /// Containers that do not support creation return `None`.
    fn create_user_item(&self, node_type: &str, label: String) -> Option<Box<dyn Node>> {
        if node_type == "script" && self.script_host_policy().is_some_and(|policy| policy.enabled) {
            return Some(Box::new(ScriptNode::new(label, ScriptNodeConfig::for_host_node_type(self.get_type()))));
        }
        if (node_type == USER_CONTEXT_NODE_TYPE || node_type == "context") && self.user_context_host_policy().is_some_and(|policy| policy.enabled) {
            return Some(Box::new(UserContextNode::new(label)));
        }
        None
    }

    /// Engine-internal hook used while applying `SetParam` edits.
    ///
    /// App node code should request parameter changes through [`crate::parameter::Parameter::set`]
    /// (or `ProcessCtx::set_param`) instead of calling this directly.
    #[doc(hidden)]
    fn engine_set_param_value(&mut self, _value: ParamValue) -> Option<ParamValue> {
        None
    }

    /// Engine-internal hook used to validate or normalize parameter values before apply.
    #[doc(hidden)]
    fn engine_prepare_param_value(&self, value: ParamValue) -> Result<ParamValue, String> {
        Ok(value)
    }

    /// Engine-internal hook used by UI snapshot building to expose parameter details.
    #[doc(hidden)]
    fn engine_param_snapshot(&self) -> Option<ParameterSnapshot> {
        None
    }

    /// Engine-internal hook used by UI/runtime layers to expose parameter control state.
    #[doc(hidden)]
    fn engine_param_control_state(&self) -> Option<ParameterControlState> {
        None
    }

    /// Engine-internal hook used to replace parameter control state.
    #[doc(hidden)]
    fn engine_set_param_control_state(&mut self, _state: ParameterControlState) -> Result<(), String> {
        Err("node does not expose parameter control state".to_string())
    }

    /// Engine-internal hook used to expose script-facing properties and methods.
    #[doc(hidden)]
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        core_node_script_descriptor(self.node_data(), self.get_type())
    }

    /// Engine-internal hook used to handle script property writes.
    ///
    /// Returns `Ok(true)` when the property was handled by this node.
    #[doc(hidden)]
    fn engine_set_script_property(&mut self, ctx: &mut ProcessCtx, property: &str, value: ParamValue) -> Result<bool, String> {
        match property {
            "name" | "label" => {
                let Some(label) = value.as_str() else {
                    return Err(format!("property '{property}' expects a string value"));
                };
                ctx.patch_node_meta(self.id(), NodeMetaPatch { label: Some(label), ..Default::default() });
                Ok(true)
            }
            "enabled" => {
                let Some(enabled) = value.as_bool() else {
                    return Err("property 'enabled' expects a boolean value".to_string());
                };
                ctx.patch_node_meta(self.id(), NodeMetaPatch { enabled: Some(enabled), ..Default::default() });
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Engine-internal hook used to invoke script-exposed node methods.
    ///
    /// Returns `Ok(true)` when the method was handled by this node.
    #[doc(hidden)]
    fn engine_call_script_method(&mut self, ctx: &mut ProcessCtx, method: &str, args: &[ParamValue]) -> Result<bool, String> {
        match method {
            "setName" => {
                let Some(label) = args.first().and_then(ParamValue::as_str) else {
                    return Err("method 'setName' expects one string argument".to_string());
                };
                ctx.patch_node_meta(self.id(), NodeMetaPatch { label: Some(label), ..Default::default() });
                Ok(true)
            }
            "setEnabled" => {
                let Some(enabled) = args.first().and_then(ParamValue::as_bool) else {
                    return Err("method 'setEnabled' expects one boolean argument".to_string());
                };
                ctx.patch_node_meta(self.id(), NodeMetaPatch { enabled: Some(enabled), ..Default::default() });
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
                let label = args.first().and_then(ParamValue::as_str).filter(|value| !value.trim().is_empty()).unwrap_or_else(|| "Folder".to_string());
                let lookup = lookup_script_child_by_key_and_type(ctx, self.id(), label.as_str(), "folder");
                for duplicate in lookup.duplicates {
                    ctx.edits.push(Edit::RemoveNode { node: duplicate });
                }

                if let Some(existing_node) = lookup.primary {
                    if lookup.primary_matches_type {
                        if let Some(existing_label) = ctx.tree_snapshot().and_then(|snapshot| snapshot.node(existing_node)).map(|snapshot| snapshot.label.clone()) {
                            if existing_label != label {
                                ctx.patch_node_meta(existing_node, NodeMetaPatch { label: Some(label), ..Default::default() });
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
                let parameter_id = args.first().and_then(ParamValue::as_str).filter(|value| !value.trim().is_empty()).unwrap_or_else(|| "parameter".to_string());
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
                        .map(|snapshot| (snapshot.is_parameter(), snapshot.node_type.clone(), snapshot.label.clone(), snapshot.param_value.clone()));

                    if let Some((is_parameter, node_type, label, param_value)) = existing_snapshot {
                        if is_parameter && lookup.primary_matches_type && node_type.eq_ignore_ascii_case(expected_type) {
                            if label != parameter_id {
                                ctx.patch_node_meta(
                                    existing_node,
                                    NodeMetaPatch {
                                        label: Some(parameter_id.clone()),
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

                    let mut parameter = Parameter::new(parameter_id.as_str(), default_value, ParameterChangeCheck::ValueChange);
                    parameter.node_data_mut().meta.decl_id = DeclId(parameter_id);
                    ctx.replace_node_boxed(existing_node, Box::new(parameter));
                    return Ok(true);
                }

                let mut parameter = Parameter::new(parameter_id.as_str(), default_value, ParameterChangeCheck::ValueChange);
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
                let value = args.get(1).cloned().ok_or_else(|| "method 'setParam' expects (name, value) arguments".to_string())?;
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
                let node_type = args.first().and_then(ParamValue::as_str).filter(|value| !value.trim().is_empty()).unwrap_or_else(|| "folder".to_string());
                let normalized_node_type = node_type.trim().to_ascii_lowercase();
                let resolved_node_type = if normalized_node_type == "context" { USER_CONTEXT_NODE_TYPE } else { node_type.as_str() };

                let default_label = match normalized_node_type.as_str() {
                    "parameter" | "param" => "parameter".to_string(),
                    "folder" | "" => "Folder".to_string(),
                    "user_context" | "context" => USER_CONTEXT_DEFAULT_LABEL.to_string(),
                    _ => node_type.clone(),
                };
                let label = args.get(1).and_then(ParamValue::as_str).filter(|value| !value.trim().is_empty()).unwrap_or(default_label);

                if normalized_node_type.is_empty() || normalized_node_type == "folder" {
                    let lookup = lookup_script_child_by_key_and_type(ctx, self.id(), label.as_str(), "folder");
                    for duplicate in lookup.duplicates {
                        ctx.edits.push(Edit::RemoveNode { node: duplicate });
                    }

                    if let Some(existing_node) = lookup.primary {
                        if lookup.primary_matches_type {
                            if let Some(existing_label) = ctx.tree_snapshot().and_then(|snapshot| snapshot.node(existing_node)).map(|snapshot| snapshot.label.clone()) {
                                if existing_label != label {
                                    ctx.patch_node_meta(existing_node, NodeMetaPatch { label: Some(label), ..Default::default() });
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
                        let existing_snapshot = ctx.tree_snapshot().and_then(|snapshot| snapshot.node(existing_node)).map(|snapshot| (snapshot.is_parameter(), snapshot.label.clone(), snapshot.param_value.clone()));

                        if let Some((is_parameter, existing_label, param_value)) = existing_snapshot {
                            if is_parameter && lookup.primary_matches_type {
                                if existing_label != label {
                                    ctx.patch_node_meta(existing_node, NodeMetaPatch { label: Some(label.clone()), ..Default::default() });
                                }
                                if param_value.as_ref() != Some(&default_value) {
                                    ctx.set_param(existing_node, default_value);
                                }
                                return Ok(true);
                            }
                        }

                        let mut parameter = Parameter::new(label.as_str(), default_value, ParameterChangeCheck::ValueChange);
                        parameter.node_data_mut().meta.decl_id = DeclId(label.clone());
                        ctx.replace_node_boxed(existing_node, Box::new(parameter));
                        return Ok(true);
                    }

                    let mut parameter = Parameter::new(label.as_str(), default_value, ParameterChangeCheck::ValueChange);
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
                        if let Some(existing_label) = ctx.tree_snapshot().and_then(|snapshot| snapshot.node(existing_node)).map(|snapshot| snapshot.label.clone()) {
                            if existing_label != label {
                                ctx.patch_node_meta(existing_node, NodeMetaPatch { label: Some(label), ..Default::default() });
                            }
                        }
                        return Ok(true);
                    }

                    if let Some(node) = self.create_user_item(resolved_node_type, label) {
                        ctx.replace_node_boxed(existing_node, node);
                        return Ok(true);
                    }
                } else if let Some(node) = self.create_user_item(resolved_node_type, label) {
                    ctx.add_user_item_boxed(self.id(), node, None);
                    return Ok(true);
                }

                Err(format!("method 'addNode' cannot create node type '{node_type}' under '{}'", self.get_type()))
            }
            _ => Ok(false),
        }
    }

    /// Engine-internal hook used to traverse mutable node references.
    ///
    /// App node code should not call this directly.
    #[doc(hidden)]
    fn engine_visit_references_mut(&mut self, _visit: &mut dyn FnMut(&mut NodeReference)) {}

    /// Engine-internal hook used to synchronize local parameter-handle caches.
    ///
    /// App node code should not call this directly.
    #[doc(hidden)]
    fn engine_sync_param_handle_cache(&mut self, _param: NodeId, _new_value: &ParamValue) {}

    /// Engine-internal hook run immediately after attachment, before `init`.
    ///
    /// App node code should not call this directly.
    #[doc(hidden)]
    fn engine_on_attached(&mut self, _ctx: &mut ProcessCtx) {}

    /// Engine-internal hook to refresh all currently bound parameter handles.
    ///
    /// App node code should not call this directly.
    #[doc(hidden)]
    fn engine_sync_bound_param_handles(&mut self, _resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {}

    /// Engine-internal inbox preprocessing hook run before app-level inbox handling.
    #[doc(hidden)]
    fn engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        for event in ctx.events.clone() {
            if let EventKind::ParamChanged { param, new_value, .. } = event.kind {
                self.engine_sync_param_handle_cache(param, &new_value);
            }
        }
    }

    /// Attempts to downcast a boxed trait object into `Self`.
    fn from_boxed_node(node: Box<dyn Node>) -> Option<Self>
    where
        Self: Sized,
    {
        let any: Box<dyn Any> = node;
        any.downcast::<Self>().ok().map(|node| *node)
    }

    /// Returns this node id.
    fn id(&self) -> NodeId {
        self.node_data().id
    }

    /// Returns `true` when this node id matches `id`.
    fn is(&self, id: NodeId) -> bool {
        self.id() == id
    }

    /// Called when the node is initialized.
    ///
    /// Core internal attachment/setup hooks run before this callback.
    fn init(&mut self, _ctx: &mut ProcessCtx) {}
    /// Called at this node's update rate.
    fn update(&mut self, _ctx: &mut ProcessCtx) {} // called at this node's desired update rate
    /// Called before node destruction.
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {}

    /// Returns the runtime execution rule used by the engine scheduler.
    ///
    /// The default rule is passive: no dependencies and no update rate.
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::default()
    }

    /// Returns how many descendant levels of events this node subscribes to.
    ///
    /// `0` means this node does not subscribe to descendant events, `1` means it subscribes to direct child events, etc.
    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        1
    }
    /// Returns how many additional ancestor hops this node grants to a received event.
    ///
    /// `0` means no additional bubbling, `1` allows bubbling to the direct parent by default.
    fn bubble_event_depth(&self, _event: &Event) -> u32 {
        1
    }
    /// Returns propagation behavior for an event that has reached this node.
    ///
    /// `depth` is the ancestor distance from event origin (`0` for the origin node).
    fn event_propagation(&self, _event: &Event, _depth: u32) -> EventPropagation {
        EventPropagation::Notify
    }

    /// Engine-internal additional descendant interest depth.
    ///
    /// This is merged by the engine with `child_event_interest_depth` so core
    /// features can subscribe without taking over app-facing callbacks.
    #[doc(hidden)]
    fn engine_child_event_interest_depth(&self, _event: &Event) -> u32 {
        0
    }
    /// Dispatches inbox events to per-event handlers.
    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.dispatch_inbox(ctx);
    }

    /// Queues insertion of a boxed child node.
    fn add_child_boxed(&mut self, ctx: &mut ProcessCtx, child: Box<dyn Node>, after: Option<NodeId>) {
        ctx.add_child_boxed(self.id(), child, after);
    }

    /// Queues insertion of a typed child node.
    fn add_child<N>(&mut self, ctx: &mut ProcessCtx, child: N, after: Option<NodeId>)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.add_child_boxed(ctx, Box::new(child), after);
    }

    /// Queues insertion of a boxed user-curated item.
    fn add_user_child_boxed(&mut self, ctx: &mut ProcessCtx, child: Box<dyn Node>, after: Option<NodeId>) {
        ctx.add_user_item_boxed(self.id(), child, after);
    }

    /// Queues insertion of a typed user-curated item.
    fn add_user_child<N>(&mut self, ctx: &mut ProcessCtx, child: N, after: Option<NodeId>)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.add_user_child_boxed(ctx, Box::new(child), after);
    }

    /// Queues removal of an existing child.
    fn remove_child(&mut self, ctx: &mut ProcessCtx, child: NodeId) {
        ctx.edits.push(Edit::RemoveNode { node: child });
    }
    /// Queues move of an existing child.
    fn move_child(&mut self, ctx: &mut ProcessCtx, child: NodeId, new_parent: NodeId, after: Option<NodeId>) {
        ctx.edits.push(Edit::MoveNode { node: child, new_parent, new_prev_sibling: after });
    }
    /// Subscribes this node to direct events originating from `target`.
    fn add_listener(&mut self, ctx: &mut ProcessCtx, target: NodeId) {
        ctx.add_event_listener(self.id(), target);
    }
    /// Subscribes this node to events from `target` and descendants up to `max_depth`.
    fn add_listener_subtree(&mut self, ctx: &mut ProcessCtx, target: NodeId, max_depth: u32) {
        ctx.add_event_listener_subtree(self.id(), target, max_depth);
    }
    /// Removes this node's direct listener subscription to `target`.
    fn remove_listener(&mut self, ctx: &mut ProcessCtx, target: NodeId) {
        ctx.remove_event_listener(self.id(), target);
    }
    /// Removes this node's subtree listener subscription to `target`.
    fn remove_listener_subtree(&mut self, ctx: &mut ProcessCtx, target: NodeId, max_depth: u32) {
        ctx.remove_event_listener_subtree(self.id(), target, max_depth);
    }
    /// Queues replacement of a child by a boxed node.
    fn replace_child_boxed(&mut self, ctx: &mut ProcessCtx, old: NodeId, new_node: Box<dyn Node>) {
        ctx.replace_node_boxed(old, new_node);
    }
    /// Queues replacement of a child by a typed node.
    fn replace_child<N>(&mut self, ctx: &mut ProcessCtx, old: NodeId, new_node: N)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.replace_child_boxed(ctx, old, Box::new(new_node));
    }

    /// Queues a metadata patch on this node.
    fn patch_meta(&mut self, ctx: &mut ProcessCtx, patch: NodeMetaPatch) {
        ctx.patch_node_meta(self.id(), patch);
    }

    /// Queues a metadata patch on any `target` node.
    fn patch_meta_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId, patch: NodeMetaPatch) {
        ctx.patch_node_meta(target, patch);
    }

    /// Sets or replaces the default warning on this node.
    fn set_warning(&mut self, ctx: &mut ProcessCtx, message: &str) {
        self.set_warning_with(ctx, None, message, None);
    }

    /// Sets or replaces a warning on this node.
    ///
    /// `warning_id = None` uses the default empty warning id.
    fn set_warning_with(&mut self, ctx: &mut ProcessCtx, warning_id: Option<&str>, message: &str, detail: Option<&str>) {
        ctx.set_node_warning_with(self.id(), warning_id, message, detail);
    }

    /// Sets or replaces the default warning on any `target` node.
    ///
    /// This is useful when a child node wants to surface problems on a parent.
    fn set_warning_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId, message: &str) {
        self.set_warning_for_node_with(ctx, target, None, message, None);
    }

    /// Sets or replaces a warning on any `target` node.
    ///
    /// This is useful when a child node wants to surface problems on a parent.
    fn set_warning_for_node_with(&mut self, ctx: &mut ProcessCtx, target: NodeId, warning_id: Option<&str>, message: &str, detail: Option<&str>) {
        ctx.set_node_warning_with(target, warning_id, message, detail);
    }

    /// Clears one warning from this node.
    ///
    /// `warning_id = None` clears the default empty warning id.
    fn clear_warning(&mut self, ctx: &mut ProcessCtx, warning_id: Option<&str>) {
        ctx.clear_node_warning(self.id(), warning_id);
    }

    /// Clears all warnings from this node.
    fn clear_warnings(&mut self, ctx: &mut ProcessCtx) {
        ctx.clear_all_node_warnings(self.id());
    }

    /// Clears one warning from any `target` node.
    ///
    /// `warning_id = None` clears the default empty warning id.
    fn clear_warning_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId, warning_id: Option<&str>) {
        ctx.clear_node_warning(target, warning_id);
    }

    /// Clears all warnings from any `target` node.
    fn clear_warnings_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId) {
        ctx.clear_all_node_warnings(target);
    }

    /// Sets child warning surfacing depth for this node.
    fn set_child_warning_depth(&mut self, ctx: &mut ProcessCtx, max_depth: u32) {
        ctx.set_node_child_warning_depth(self.id(), max_depth);
    }

    /// Sets child warning surfacing depth for any `target` node.
    fn set_child_warning_depth_for_node(&mut self, ctx: &mut ProcessCtx, target: NodeId, max_depth: u32) {
        ctx.set_node_child_warning_depth(target, max_depth);
    }

    /// Dispatches all events in the process context to typed handlers.
    fn dispatch_inbox(&mut self, ctx: &mut ProcessCtx) {
        if ctx.has_structure_changed() {
            self.on_structure_changed(ctx);
        }

        for event in ctx.events.clone() {
            match event.kind {
                EventKind::ParamChanged { param, old_value, .. } => {
                    self.on_param_change(ctx, param, old_value);
                }
                EventKind::ParamControlChanged { param, old_state, new_state } => {
                    self.on_param_control_changed(ctx, param, old_state, new_state);
                }
                EventKind::ChildAdded { parent, child, decl_id } => {
                    self.on_child_added_decl(ctx, parent, child, &decl_id);
                }
                EventKind::ChildRemoved { parent, child } => {
                    self.on_child_removed(ctx, parent, child);
                }
                EventKind::ChildReplaced { parent, old, new, decl_id } => {
                    self.on_child_replaced_decl(ctx, parent, old, new, &decl_id);
                }
                EventKind::ChildMoved { child, old_parent, new_parent } => {
                    self.on_child_moved(ctx, child, old_parent, new_parent);
                }
                EventKind::ChildReordered { parent, child } => {
                    self.on_child_reordered(ctx, parent, child);
                }
                EventKind::NodeCreated { node } => {
                    self.on_node_created(ctx, node);
                }
                EventKind::NodeDeleted { node } => {
                    self.on_node_deleted(ctx, node);
                }
                EventKind::MetaChanged { node, patch } => {
                    self.on_meta_changed(ctx, node, patch);
                }
                EventKind::Custom(event) => {
                    self.on_custom_event(ctx, event);
                }
            }
        }
    }

    /// Called when a parameter has changed.
    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, _param: NodeId, _old_value: ParamValue) {}
    /// Called when a parameter control state has changed.
    fn on_param_control_changed(&mut self, _ctx: &mut ProcessCtx, _param: NodeId, _old_state: ParameterControlState, _new_state: ParameterControlState) {}
    /// Called once when any structural event is present in the current callback frame.
    fn on_structure_changed(&mut self, _ctx: &mut ProcessCtx) {}
    /// Called when a child is added.
    fn on_child_added(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    /// Called when a child is added, including its declared slot id.
    fn on_child_added_decl(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId, _decl_id: &DeclId) {
        self.on_child_added(ctx, parent, child);
    }
    /// Called when a child is removed.
    fn on_child_removed(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    /// Called when a child is replaced.
    fn on_child_replaced(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _old: NodeId, _new: NodeId) {}
    /// Called when a child is replaced, including the replacement slot id.
    fn on_child_replaced_decl(&mut self, ctx: &mut ProcessCtx, parent: NodeId, old: NodeId, new: NodeId, _decl_id: &DeclId) {
        self.on_child_replaced(ctx, parent, old, new);
    }
    /// Called when a child is moved to another parent.
    fn on_child_moved(&mut self, _ctx: &mut ProcessCtx, _child: NodeId, _old_parent: NodeId, _new_parent: NodeId) {}
    /// Called when a child is reordered under the same parent.
    fn on_child_reordered(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    /// Called when a node is created.
    fn on_node_created(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}
    /// Called when a node is deleted.
    fn on_node_deleted(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}
    /// Called when node metadata changes.
    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, _patch: NodeMetaPatch) {}
    /// Called when a custom event is emitted.
    fn on_custom_event(&mut self, _ctx: &mut ProcessCtx, _event: CustomEvent) {}
}

/// Adapter used by `#[node(..., via = ...)]` to access either a `NodeData` field
/// directly or a composed node that owns the runtime identity.
pub trait ViaTarget {
    /// Returns the node data backing the via target.
    fn via_node_data(&self) -> &NodeData;
    /// Returns mutable node data backing the via target.
    fn via_node_data_mut(&mut self) -> &mut NodeData;

    /// Forwards generated engine descendant-interest depth to the via target when relevant.
    fn via_engine_child_event_interest_depth(&self, _event: &Event) -> u32 {
        0
    }

    /// Forwards generated attachment wiring to the via target when relevant.
    fn via_engine_on_attached(&mut self, _ctx: &mut ProcessCtx) {}

    /// Forwards generated runtime cache sync to the via target when relevant.
    fn via_engine_sync_param_handle_cache(&mut self, _param: NodeId, _new_value: &ParamValue) {}

    /// Forwards generated bound-handle refresh to the via target when relevant.
    fn via_engine_sync_bound_param_handles(&mut self, _resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {}

    /// Forwards generated inbox preprocessing to the via target when relevant.
    fn via_engine_preprocess_inbox(&mut self, _ctx: &mut ProcessCtx) {}

    /// Forwards generated script-host policy lookup to the via target when relevant.
    fn via_script_host_policy(&self) -> Option<ScriptHostPolicy> {
        None
    }

    /// Forwards generated user-context host policy lookup to the via target when relevant.
    fn via_user_context_host_policy(&self) -> Option<UserContextHostPolicy> {
        None
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

    fn via_engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.engine_preprocess_inbox(ctx);
    }

    fn via_script_host_policy(&self) -> Option<ScriptHostPolicy> {
        self.script_host_policy()
    }

    fn via_user_context_host_policy(&self) -> Option<UserContextHostPolicy> {
        self.user_context_host_policy()
    }
}

/// Internal scope node used to host user-authored lexical context entries.
///
/// Context entries are regular descendant parameter nodes. Symbols are derived from
/// parameter `decl_id` values during resolver indexing.
pub struct UserContextNode {
    node_data: NodeData,
}

impl UserContextNode {
    /// Creates a new user-context scope node.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        Self { node_data }
    }
}

impl Node for UserContextNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        USER_CONTEXT_NODE_TYPE
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&USER_CONTEXT_ALLOWED_ITEM_KINDS))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = vec![UserCreatableItem::new(FOLDER_NODE_TYPE, FOLDER_NODE_TYPE, "Folder")];
        items.extend([
            UserCreatableItem::new("trigger", "trigger", "Trigger"),
            UserCreatableItem::new("int", "int", "Int"),
            UserCreatableItem::new("float", "float", "Float"),
            UserCreatableItem::new("str", "str", "String"),
            UserCreatableItem::new("file", "file", "File"),
            UserCreatableItem::new("enum", "enum", "Enum"),
            UserCreatableItem::new("bool", "bool", "Bool"),
            UserCreatableItem::new("vec2", "vec2", "Vec2"),
            UserCreatableItem::new("vec3", "vec3", "Vec3"),
            UserCreatableItem::new("color", "color", "Color"),
            UserCreatableItem::new("reference", "reference", "Reference"),
        ]);
        items
    }

    fn create_user_item(&self, node_type: &str, label: String) -> Option<Box<dyn Node>> {
        let node_type = node_type.trim().to_ascii_lowercase();
        if node_type == FOLDER_NODE_TYPE {
            return Some(Box::new(Folder::new(label)));
        }

        if node_type == "param" || node_type == "parameter" {
            return Some(Box::new(Parameter::new(label.as_str(), ParamValue::Float(0.0), ParameterChangeCheck::ValueChange)));
        }

        let default_value = default_parameter_value_for_node_type(node_type.as_str())?;
        Some(Box::new(Parameter::new(label.as_str(), default_value, ParameterChangeCheck::ValueChange)))
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

pub(super) fn parameter_child_exists(ctx: &ProcessCtx, parent: NodeId, decl_id: &str) -> bool {
    ctx.tree_snapshot().and_then(|snapshot| snapshot.find_child(parent, decl_id)).is_some()
}

/// Internal Folder-like node used as empty organizational structure and root for user content without process or bubbling.
pub struct Folder {
    node_data: NodeData,
}

impl Folder {
    /// Creates a new folder node.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        Self { node_data }
    }
}

impl Node for Folder {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        FOLDER_NODE_TYPE
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        println!("Folder init {}", self.node_data.meta.label);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        println!("Folder destroy {}", self.node_data.meta.label);
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}
