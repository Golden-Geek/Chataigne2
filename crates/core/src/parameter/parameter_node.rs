use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{
    node::{
        DashboardWidgetOptionsNodeKind, DashboardWidgetTargetDescriptor, DashboardWidgetTypeSpec, Node, NodeData,
        NodeReference, PARAMETER_ANIMATION_CONTROL_NODE_TYPE, PARAMETER_CONTROL_ITEM_KIND, UserContainerRules,
    },
    parameter::ParameterAnimationControlNode,
    process_ctx::ProcessCtx,
};

use super::{
    ParamValue, ParameterChangeCheck, ParameterConstraints, ParameterControlState, ParameterEventBehaviour,
    ParameterSnapshot, ParameterUiHints, coerce_param_value_for_target, default_control_modes_enabled,
};

fn is_legacy_vec2_display_decl_id(decl_id: &str) -> bool {
    matches!(
        decl_id,
        "display_mode" | "display_2d_trail_seconds" | "display_2d_unit_step" | "display_2d_view_span"
    )
}

/// Built-in node type that stores a [`ParamValue`].
pub struct Parameter {
    node_data: NodeData,
    /// Current parameter value.
    pub value: ParamValue,
    /// Declared default value.
    pub default_value: ParamValue,
    /// Change-detection policy for `set`.
    pub change_check: ParameterChangeCheck,

    /// Strategy for handling multiple parameter changes within the same process tick.
    pub event_behaviour: ParameterEventBehaviour,
    /// Whether this parameter is read-only for UI editing.
    pub read_only: bool,
    /// Data constraints used for clamping/validation/adaptation.
    pub constraints: ParameterConstraints,
    /// UI-facing editor hints.
    pub ui_hints: ParameterUiHints,
    /// Control mode state for this parameter.
    pub control: ParameterControlState,
    /// Whether control modes other than `manual` are available for this parameter.
    pub control_modes_enabled: bool,
}

impl Parameter {
    /// Creates a new parameter node.
    pub fn new(label: &str, value: ParamValue, change_check: ParameterChangeCheck) -> Self {
        let mut node_data = NodeData::new(label.to_string());
        node_data.meta.can_be_disabled = false;
        let default_value = value.clone();

        Self {
            node_data,
            value,
            default_value,
            change_check,
            event_behaviour: ParameterEventBehaviour::Coalesce,
            read_only: false,
            constraints: ParameterConstraints::default(),
            ui_hints: ParameterUiHints::default(),
            control: ParameterControlState::default(),
            control_modes_enabled: true,
        }
    }

    /// Requests a parameter update through the process context.
    pub fn set(&mut self, ctx: &mut ProcessCtx, new_value: ParamValue) {
        let normalized = match self.constraints.normalize(new_value) {
            Ok(value) => value,
            Err(message) => {
                eprintln!(
                    "Attempted to set invalid value for parameter '{}': {message}",
                    self.node_data().meta.label
                );
                return;
            }
        };

        let is_trigger = matches!(&normalized, ParamValue::Trigger());
        let value_changed = self.value != normalized;
        if is_trigger || self.change_check == ParameterChangeCheck::None || value_changed {
            ctx.set_param_with_behaviour(self.node_data().id, normalized, self.event_behaviour);
        }
    }

    /// Convenience method to fire a trigger parameter.
    pub fn fire(&mut self, ctx: &mut ProcessCtx) {
        if !matches!(self.value, ParamValue::Trigger()) {
            eprintln!(
                "Attempted to fire a non-trigger parameter '{}'",
                self.node_data().meta.label
            );
            return;
        }
        self.set(ctx, ParamValue::Trigger());
    }

    /// Returns the current parameter value.
    pub fn get(&self) -> &ParamValue {
        &self.value
    }

    /// Returns a UI snapshot view of this parameter.
    pub fn snapshot(&self) -> ParameterSnapshot {
        ParameterSnapshot {
            value: self.value.clone(),
            default_value: self.default_value.clone(),
            change_check: self.change_check.clone(),
            event_behaviour: self.event_behaviour,
            read_only: self.read_only,
            constraints: self.constraints.clone(),
            ui_hints: self.ui_hints.clone(),
            control: self.control.clone(),
            control_modes_enabled: self.control_modes_enabled,
        }
    }

    fn coerce_for_current_value_kind(&self, incoming: ParamValue) -> Result<ParamValue, String> {
        coerce_param_value_for_target(&incoming, &self.value, None).ok_or_else(|| match &self.value {
            ParamValue::Trigger() => "trigger parameter only accepts trigger values".to_string(),
            ParamValue::Int(_) => "parameter expects an int-compatible value".to_string(),
            ParamValue::Float(_) => "parameter expects a float-compatible value".to_string(),
            ParamValue::Str(_) => "parameter expects a string-compatible value".to_string(),
            ParamValue::File(_) => "parameter expects a file-compatible value".to_string(),
            ParamValue::Enum(_) => "parameter expects an enum-compatible value".to_string(),
            ParamValue::Bool(_) => "parameter expects a bool-compatible value".to_string(),
            ParamValue::CssValue(_) => "parameter expects a css-value-compatible value".to_string(),
            ParamValue::Vec2(_, _) => "parameter expects a vec2-compatible value".to_string(),
            ParamValue::Vec3(_, _, _) => "parameter expects a vec3-compatible value".to_string(),
            ParamValue::Color(_, _, _, _) => "parameter expects a color-compatible value".to_string(),
            ParamValue::Reference(_) => "parameter expects a reference value".to_string(),
        })
    }

    fn remove_legacy_vec2_display_children(&mut self, ctx: &mut ProcessCtx) {
        if !matches!(self.value, ParamValue::Vec2(_, _)) {
            return;
        }

        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };

        let mut child = snapshot.node(self.id()).and_then(|node| node.first_child);
        let mut legacy_children = Vec::new();
        while let Some(child_id) = child {
            let Some(child_snapshot) = snapshot.node(child_id) else {
                break;
            };
            child = child_snapshot.next_sibling;

            if is_legacy_vec2_display_decl_id(child_snapshot.decl_id.as_str()) {
                legacy_children.push(child_id);
            }
        }

        for child_id in legacy_children {
            self.remove_child(ctx, child_id);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ParameterProjectData {
    value: ParamValue,
    default_value: ParamValue,
    change_check: ParameterChangeCheck,
    event_behaviour: ParameterEventBehaviour,
    read_only: bool,
    constraints: ParameterConstraints,
    ui_hints: ParameterUiHints,
    control: ParameterControlState,
    #[serde(default = "default_true")]
    control_modes_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawParameterProjectData {
    value: JsonValue,
    default_value: JsonValue,
    change_check: ParameterChangeCheck,
    event_behaviour: ParameterEventBehaviour,
    read_only: bool,
    constraints: ParameterConstraints,
    ui_hints: ParameterUiHints,
    control: ParameterControlState,
    #[serde(default = "default_true")]
    control_modes_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawLegacyParameterProjectData {
    value: JsonValue,
    change_check: ParameterChangeCheck,
    event_behaviour: ParameterEventBehaviour,
}

fn default_true() -> bool {
    true
}

fn decode_project_param_value(value: &JsonValue) -> Result<ParamValue, String> {
    if value.is_null() {
        return Ok(ParamValue::Trigger());
    }

    if let Ok(decoded) = serde_json::from_value::<ParamValue>(value.clone()) {
        return Ok(decoded);
    }

    if value
        .as_object()
        .is_some_and(|object| object.len() == 1 && object.contains_key("Trigger"))
    {
        return Ok(ParamValue::Trigger());
    }

    ParamValue::from_script_json(value)
}

fn decode_parameter_project_data(node_type: &str, data: &JsonValue) -> Result<ParameterProjectData, String> {
    if data.is_null() {
        let fallback_value = crate::node::default_parameter_value_for_node_type(node_type)
            .ok_or_else(|| format!("unsupported parameter node type '{node_type}'"))?;
        return Ok(ParameterProjectData {
            value: fallback_value.clone(),
            default_value: fallback_value,
            change_check: ParameterChangeCheck::ValueChange,
            event_behaviour: ParameterEventBehaviour::Coalesce,
            read_only: false,
            constraints: ParameterConstraints::default(),
            ui_hints: ParameterUiHints::default(),
            control: ParameterControlState::default(),
            control_modes_enabled: default_control_modes_enabled(),
        });
    }

    if let Ok(full) = serde_json::from_value::<RawParameterProjectData>(data.clone()) {
        return Ok(ParameterProjectData {
            value: decode_project_param_value(&full.value)
                .map_err(|err| format!("invalid parameter payload: {err}"))?,
            default_value: decode_project_param_value(&full.default_value)
                .map_err(|err| format!("invalid parameter payload: {err}"))?,
            change_check: full.change_check,
            event_behaviour: full.event_behaviour,
            read_only: full.read_only,
            constraints: full.constraints,
            ui_hints: full.ui_hints,
            control: full.control,
            control_modes_enabled: full.control_modes_enabled,
        });
    }

    let legacy = serde_json::from_value::<RawLegacyParameterProjectData>(data.clone())
        .map_err(|err| format!("invalid parameter payload: {err}"))?;
    Ok(ParameterProjectData {
        value: decode_project_param_value(&legacy.value).map_err(|err| format!("invalid parameter payload: {err}"))?,
        default_value: decode_project_param_value(&legacy.value)
            .map_err(|err| format!("invalid parameter payload: {err}"))?,
        change_check: legacy.change_check,
        event_behaviour: legacy.event_behaviour,
        read_only: false,
        constraints: ParameterConstraints::default(),
        ui_hints: ParameterUiHints::default(),
        control: ParameterControlState::default(),
        control_modes_enabled: default_control_modes_enabled(),
    })
}

impl Node for Parameter {
    fn node_data(&self) -> &crate::node::NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut crate::node::NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        match self.value {
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

    fn type_description(&self) -> Option<&str> {
        Some(match self.value {
            ParamValue::Trigger() => "Parameter node storing an instantaneous trigger event.",
            ParamValue::Int(_) => "Parameter node storing an integer value.",
            ParamValue::Float(_) => "Parameter node storing a floating-point value.",
            ParamValue::Str(_) => "Parameter node storing text.",
            ParamValue::File(_) => "Parameter node storing a file path.",
            ParamValue::Enum(_) => "Parameter node storing an enumerated option.",
            ParamValue::Bool(_) => "Parameter node storing a boolean value.",
            ParamValue::CssValue(_) => "Parameter node storing a CSS scalar value with an explicit unit.",
            ParamValue::Vec2(_, _) => "Parameter node storing a 2D vector.",
            ParamValue::Vec3(_, _, _) => "Parameter node storing a 3D vector.",
            ParamValue::Color(_, _, _, _) => "Parameter node storing an RGBA color.",
            ParamValue::Reference(_) => "Parameter node storing a reference to another node.",
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(ParameterProjectData {
            value: self.value.clone(),
            default_value: self.default_value.clone(),
            change_check: self.change_check.clone(),
            event_behaviour: self.event_behaviour,
            read_only: self.read_only,
            constraints: self.constraints.clone(),
            ui_hints: self.ui_hints.clone(),
            control: self.control.clone(),
            control_modes_enabled: self.control_modes_enabled,
        })
        .map_err(|err| format!("failed to encode parameter node data: {err}"))
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        let parsed = decode_parameter_project_data(self.get_type(), data)?;
        self.value = parsed.value;
        self.default_value = parsed.default_value;
        self.change_check = parsed.change_check;
        self.event_behaviour = parsed.event_behaviour;
        self.read_only = parsed.read_only;
        self.constraints = parsed.constraints;
        self.ui_hints = parsed.ui_hints;
        self.control = parsed.control;
        self.control_modes_enabled = parsed.control_modes_enabled;
        Ok(())
    }

    fn project_create(node_type: &str) -> Option<Self> {
        let default_value = crate::node::default_parameter_value_for_node_type(node_type)?;
        Some(Self::new(node_type, default_value, ParameterChangeCheck::ValueChange))
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        self.remove_legacy_vec2_display_children(ctx);
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[PARAMETER_CONTROL_ITEM_KIND]))
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            PARAMETER_ANIMATION_CONTROL_NODE_TYPE => Some(Box::new(ParameterAnimationControlNode::new("Animation"))),
            _ => None,
        }
    }

    fn engine_set_param_value(&mut self, value: ParamValue) -> Option<ParamValue> {
        let old = std::mem::replace(&mut self.value, value);
        Some(old)
    }

    fn engine_prepare_param_value(&self, value: ParamValue) -> Result<ParamValue, String> {
        let coerced = self.coerce_for_current_value_kind(value)?;
        self.constraints.normalize(coerced)
    }

    fn engine_param_snapshot(&self) -> Option<crate::parameter::ParameterSnapshot> {
        Some(self.snapshot())
    }

    fn engine_dashboard_widget_target_descriptor(&self) -> DashboardWidgetTargetDescriptor {
        let mut widget_types = vec![
            DashboardWidgetTypeSpec::new("inspector", "Inspector")
                .with_options_node_kind(DashboardWidgetOptionsNodeKind::Inspector),
        ];

        match &self.value {
            ParamValue::Int(_) | ParamValue::Float(_) => {
                widget_types.insert(
                    0,
                    DashboardWidgetTypeSpec::new("default", "Default")
                        .with_options_node_kind(DashboardWidgetOptionsNodeKind::NumberSlider),
                );
                widget_types.push(
                    DashboardWidgetTypeSpec::new("slider", "Slider")
                        .with_options_node_kind(DashboardWidgetOptionsNodeKind::NumberSlider),
                );
                widget_types.push(
                    DashboardWidgetTypeSpec::new("rotary", "Rotary")
                        .with_options_node_kind(DashboardWidgetOptionsNodeKind::NumberRotary),
                );
            }
            ParamValue::Vec2(_, _) | ParamValue::Vec3(_, _, _) => {
                let default_kind = if matches!(&self.value, ParamValue::Vec2(_, _)) {
                    DashboardWidgetOptionsNodeKind::Vec2Editor
                } else {
                    DashboardWidgetOptionsNodeKind::Vec3Editor
                };
                widget_types.insert(
                    0,
                    DashboardWidgetTypeSpec::new("default", "Default").with_options_node_kind(default_kind),
                );

                if matches!(&self.value, ParamValue::Vec2(_, _)) {
                    widget_types.push(
                        DashboardWidgetTypeSpec::new("vec2Pad", "2D Pad")
                            .with_options_node_kind(DashboardWidgetOptionsNodeKind::Vec2Pad),
                    );
                }
            }
            ParamValue::Color(_, _, _, _) => {
                widget_types.insert(
                    0,
                    DashboardWidgetTypeSpec::new("default", "Default")
                        .with_options_node_kind(DashboardWidgetOptionsNodeKind::ColorEditor),
                );
            }
            _ => {
                widget_types.insert(
                    0,
                    DashboardWidgetTypeSpec::new("default", "Default")
                        .with_options_node_kind(DashboardWidgetOptionsNodeKind::ParameterEditor),
                );
            }
        }

        DashboardWidgetTargetDescriptor {
            widget_types,
            default_widget_type_id: "default".to_string(),
        }
    }

    fn engine_param_control_state(&self) -> Option<crate::parameter::ParameterControlState> {
        Some(self.control.clone())
    }

    fn engine_set_param_control_state(&mut self, state: crate::parameter::ParameterControlState) -> Result<(), String> {
        self.control = state;
        Ok(())
    }

    fn engine_script_descriptor(&self) -> crate::node::NodeScriptDescriptor {
        let mut descriptor = crate::node::core_node_script_descriptor(&self.node_data, self.get_type());
        descriptor.properties.insert("value".to_string(), self.value.clone());
        descriptor
    }

    fn engine_set_script_property(
        &mut self,
        ctx: &mut ProcessCtx,
        property: &str,
        value: ParamValue,
    ) -> Result<bool, String> {
        match property {
            "value" => {
                let normalized = self.constraints.normalize(value)?;
                ctx.set_param_with_behaviour(self.id(), normalized, ParameterEventBehaviour::Coalesce);
                Ok(true)
            }
            "name" | "label" => {
                let Some(label) = value.as_str() else {
                    return Err(format!("property '{property}' expects a string value"));
                };
                ctx.patch_node_meta(
                    self.id(),
                    crate::node::NodeMetaPatch {
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
                    crate::node::NodeMetaPatch {
                        enabled: Some(enabled),
                        ..Default::default()
                    },
                );
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn engine_visit_references_mut(&mut self, visit: &mut dyn FnMut(&mut NodeReference)) {
        if let ParamValue::Reference(reference) = &mut self.value {
            visit(reference);
        }
    }
}
