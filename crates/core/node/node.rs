use std::collections::HashMap;

use crate::color::Color;
use crate::parameter::{ParamValue, ParamValueProjection};
use crate::process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot};
use crate::script::ScriptHostPolicy;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[path = "core/behavior.rs"]
mod behavior;
#[path = "core/builtins.rs"]
mod builtins;

mod animation_curve_nodes;
mod control_animation;
mod dashboard;
mod dashboard_widget_options;
mod handles;
pub use behavior::{Node, ViaTarget};
pub use builtins::{Folder, UserContextNode};
pub use animation_curve_nodes::{
    AnimationCurveEasingNode, AnimationCurveKeyNode, AnimationCurveNode, AnimationCurveRangeConstraint,
    AnimationCurveRangeNode, curve_from_snapshot,
};
pub use control_animation::ParameterAnimationControlNode;
pub use dashboard::{
    DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_ITEM_KIND, DASHBOARD_NODE_TYPE, DASHBOARD_NODE_WIDGET_NODE_TYPE,
    DASHBOARD_PAGE_ITEM_KIND, DASHBOARD_PAGE_NODE_TYPE, DASHBOARD_WIDGET_CONTAINER_NODE_TYPE,
    DASHBOARD_WIDGET_ITEM_KIND, DashboardGenericWidgetNode, DashboardNode, DashboardNodeWidgetNode, DashboardPageNode,
    DashboardWidgetContainerNode, DashboardWidgetOptionsNodeKind, DashboardWidgetTargetDescriptor,
    DashboardWidgetTypeSpec,
};
pub use dashboard_widget_options::{
    DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE,
    DashboardNodeWidgetColorEditorOptionsNode, DashboardNodeWidgetInspectorOptionsNode,
    DashboardNodeWidgetNumberRotaryOptionsNode, DashboardNodeWidgetNumberSliderOptionsNode,
    DashboardNodeWidgetParameterEditorOptionsNode, DashboardNodeWidgetVec2EditorOptionsNode,
    DashboardNodeWidgetVec2PadOptionsNode, DashboardNodeWidgetVec3EditorOptionsNode,
};
pub use handles::{DeclaredNodeHandle, NodeHandle, ParameterHandle, ParameterValueType, PotentialNodeHandle};
pub(crate) use builtins::parameter_child_exists;

/// Stable engine identifier for a node stored in [`crate::engine::node_store::NodeStore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct NodeId(pub u64);

/// Persistent UUID assigned to node metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
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
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct DeclId(pub String);

/// Semantic hints used for tooling, UX, and interpretation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct SemanticsHint {
    /// Optional high-level intent of the node.
    pub intent: Option<String>,
    /// Optional unit for value-oriented nodes.
    pub unit: Option<String>,
}

/// Warning message shown in UI for a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
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

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
/// Node-level presentation hints persisted in metadata.
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

fn parameter_node_type_from_value(value: &ParamValue) -> &'static str {
    match value {
        ParamValue::Trigger() => "trigger",
        ParamValue::Int(_) => "int",
        ParamValue::Float(_) => "float",
        ParamValue::Str(_) => "str",
        ParamValue::File(_) => "file",
        ParamValue::Enum(_) => "enum",
        ParamValue::Bool(_) => "bool",
        ParamValue::CssValue(_) => "css_value",
        ParamValue::Vec2(_, _) => "vec2",
        ParamValue::Vec3(_, _, _) => "vec3",
        ParamValue::Color(_, _, _, _) => "color",
        ParamValue::Reference(_) => "reference",
    }
}

pub(crate) fn default_parameter_value_for_node_type(node_type: &str) -> Option<ParamValue> {
    match node_type {
        "trigger" => Some(ParamValue::Trigger()),
        "int" => Some(ParamValue::Int(0)),
        "float" => Some(ParamValue::Float(0.0)),
        "str" => Some(ParamValue::Str(String::new())),
        "file" => Some(ParamValue::File(String::new())),
        "enum" => Some(ParamValue::Enum(String::new())),
        "bool" => Some(ParamValue::Bool(false)),
        "css_value" => Some(ParamValue::CssValue(crate::parameter::CssValue::default())),
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

    node.decl_id.eq_ignore_ascii_case(key)
        || node.short_name.eq_ignore_ascii_case(key)
        || node.label.eq_ignore_ascii_case(key)
}

fn lookup_script_child_by_key_and_type(
    ctx: &ProcessCtx,
    parent: NodeId,
    key: &str,
    expected_node_type: &str,
) -> ScriptChildLookup {
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

    let duplicates = matches
        .into_iter()
        .filter(|candidate| Some(*candidate) != primary)
        .collect::<Vec<_>>();
    ScriptChildLookup {
        primary,
        primary_matches_type,
        duplicates,
    }
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
    pub fn set_warning_message(
        &mut self,
        warning_id: Option<&str>,
        message: impl Into<String>,
        detail: Option<String>,
    ) {
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
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
        self.accepts_item_kinds
            .iter()
            .any(|kind| *kind == "*" || *kind == item_kind)
    }
}

/// UI-facing descriptor for a user-creatable item node type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
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

/// Compile-time metadata and construction hook for declared user-item node types.
pub trait DeclaredUserItemNode: Node + Sized {
    /// Runtime node type identifier exposed by the item declaration.
    const ITEM_NODE_TYPE: &'static str;
    /// Logical user-item kind used for container admission.
    const ITEM_KIND: &'static str;

    /// Default label shown for newly created instances of this item.
    fn item_default_label() -> String;

    /// Creates a new item instance using the item's own constructor policy.
    fn create_item() -> Self;
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
/// Built-in node type id for animation-curve range constraint nodes.
pub const PARAMETER_ANIMATION_RANGE_NODE_TYPE: &str = "animation_curve_range";
/// Built-in `decl_id` for range constraint nodes under animation-curve nodes.
pub const PARAMETER_ANIMATION_RANGE_DECL_ID: &str = "range";
/// Built-in `decl_id` for range x-axis bounds parameter.
pub const PARAMETER_ANIMATION_RANGE_X_DECL_ID: &str = "x";
/// Built-in `decl_id` for range y-axis bounds parameter.
pub const PARAMETER_ANIMATION_RANGE_Y_DECL_ID: &str = "y";
/// Built-in node type id for animation-curve easing nodes.
pub const PARAMETER_ANIMATION_EASING_NODE_TYPE: &str = "animation_curve_easing";
/// Built-in `decl_id` for easing nodes under key nodes.
pub const PARAMETER_ANIMATION_EASING_DECL_ID: &str = "easing";
/// Built-in `decl_id` for easing kind selector.
pub const PARAMETER_ANIMATION_EASING_KIND_DECL_ID: &str = "kind";
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
pub const PARAMETER_NODE_TYPES: [&str; 11] = [
    "trigger",
    "int",
    "float",
    "str",
    "file",
    "enum",
    "bool",
    "vec2",
    "vec3",
    "color",
    "reference",
];
const USER_CONTEXT_ALLOWED_ITEM_KINDS: [&str; 13] = [
    FOLDER_NODE_TYPE,
    "trigger",
    "int",
    "float",
    "str",
    "file",
    "enum",
    "bool",
    "vec2",
    "vec3",
    "color",
    "reference",
    "*",
];

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
    /// Canonical declaration-description key shared by repeated declared nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_description_key: Option<String>,
    /// Canonical declaration description before any instance-level override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_description: Option<String>,
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
            declared_description_key: None,
            declared_description: None,
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

    /// Sets the canonical declaration-backed description for this node.
    pub fn with_declared_description(mut self, key: impl Into<String>, description: impl Into<String>) -> Self {
        self.set_declared_description(key, description);
        self
    }

    /// Replaces the canonical declaration-backed description for this node.
    pub fn set_declared_description(&mut self, key: impl Into<String>, description: impl Into<String>) {
        let description = description.into();
        self.declared_description_key = Some(key.into());
        self.declared_description = Some(description.clone());
        self.description = Some(description);
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
        self.presentation
            .set_warning_message(warning_id, message, detail.map(str::to_string));
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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
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
    descriptor
        .properties
        .insert("name".to_string(), ParamValue::Str(node_data.meta.label.clone()));
    descriptor
        .properties
        .insert("enabled".to_string(), ParamValue::Bool(node_data.meta.enabled));
    descriptor
        .properties
        .insert("type".to_string(), ParamValue::Str(node_type.to_string()));
    descriptor
        .properties
        .insert("declId".to_string(), ParamValue::Str(node_data.meta.decl_id.0.clone()));

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
