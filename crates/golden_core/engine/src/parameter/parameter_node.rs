use serde_json::Value as JsonValue;

use crate::{
    events::EventFrame,
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
    /// Whether sparse persistence should keep value deltas even when `read_only`.
    pub persist_read_only_value: bool,
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
            persist_read_only_value: false,
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

#[derive(Clone, Debug)]
struct ParameterProjectData {
    value: ParamValue,
    default_value: ParamValue,
    change_check: ParameterChangeCheck,
    event_behaviour: ParameterEventBehaviour,
    read_only: bool,
    persist_read_only_value: bool,
    constraints: ParameterConstraints,
    ui_hints: ParameterUiHints,
    control: ParameterControlState,
    control_modes_enabled: bool,
}

fn default_parameter_project_data(node_type: &str) -> Result<ParameterProjectData, String> {
    let fallback_value = crate::node::default_parameter_value_for_node_type(node_type)
        .ok_or_else(|| format!("unsupported parameter node type '{node_type}'"))?;
    Ok(ParameterProjectData {
        value: fallback_value.clone(),
        default_value: fallback_value,
        change_check: ParameterChangeCheck::ValueChange,
        event_behaviour: ParameterEventBehaviour::Coalesce,
        read_only: false,
        persist_read_only_value: false,
        constraints: ParameterConstraints::default(),
        ui_hints: ParameterUiHints::default(),
        control: ParameterControlState::default(),
        control_modes_enabled: default_control_modes_enabled(),
    })
}

fn current_parameter_project_data(parameter: &Parameter) -> ParameterProjectData {
    ParameterProjectData {
        value: parameter.value.clone(),
        default_value: parameter.default_value.clone(),
        change_check: parameter.change_check.clone(),
        event_behaviour: parameter.event_behaviour,
        read_only: parameter.read_only,
        persist_read_only_value: parameter.persist_read_only_value,
        constraints: parameter.constraints.clone(),
        ui_hints: parameter.ui_hints.clone(),
        control: parameter.control.clone(),
        control_modes_enabled: parameter.control_modes_enabled,
    }
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
    let baseline = default_parameter_project_data(node_type)?;
    decode_parameter_project_data_with_baseline(baseline, data)
}

fn decode_parameter_project_data_with_baseline(
    mut parsed: ParameterProjectData,
    data: &JsonValue,
) -> Result<ParameterProjectData, String> {
    if data.is_null() {
        return Ok(parsed);
    }

    let Some(object) = data.as_object() else {
        return Err(format!("invalid parameter payload: expected an object, got {data:?}"));
    };

    let value_followed_default = parsed.value == parsed.default_value;
    if let Some(value) = object.get("value") {
        parsed.value = decode_project_param_value(value).map_err(|err| format!("invalid parameter payload: {err}"))?;
    }
    if let Some(default_value) = object.get("default_value") {
        parsed.default_value =
            decode_project_param_value(default_value).map_err(|err| format!("invalid parameter payload: {err}"))?;
        if object.get("value").is_none() && value_followed_default {
            // Older sparse records for read-only, dynamically constructed
            // parameters persisted only their changed default. Such parameters
            // start at that default, so preserve the constructor invariant.
            parsed.value = parsed.default_value.clone();
        }
    }
    if let Some(change_check) = object.get("change_check") {
        parsed.change_check = serde_json::from_value(change_check.clone())
            .map_err(|err| format!("invalid change_check payload: {err}"))?;
    }
    if let Some(event_behaviour) = object.get("event_behaviour") {
        parsed.event_behaviour = serde_json::from_value(event_behaviour.clone())
            .map_err(|err| format!("invalid event_behaviour payload: {err}"))?;
    }
    if let Some(read_only) = object.get("read_only") {
        parsed.read_only =
            serde_json::from_value(read_only.clone()).map_err(|err| format!("invalid read_only payload: {err}"))?;
    }
    if let Some(persist_read_only_value) = object.get("persist_read_only_value") {
        parsed.persist_read_only_value = serde_json::from_value(persist_read_only_value.clone())
            .map_err(|err| format!("invalid persist_read_only_value payload: {err}"))?;
    }
    if let Some(constraints) = object.get("constraints") {
        parsed.constraints =
            serde_json::from_value(constraints.clone()).map_err(|err| format!("invalid constraints payload: {err}"))?;
    }
    if let Some(ui_hints) = object.get("ui_hints") {
        parsed.ui_hints =
            serde_json::from_value(ui_hints.clone()).map_err(|err| format!("invalid ui_hints payload: {err}"))?;
    }
    if let Some(control) = object.get("control") {
        parsed.control =
            serde_json::from_value(control.clone()).map_err(|err| format!("invalid control payload: {err}"))?;
    }
    if let Some(control_modes_enabled) = object.get("control_modes_enabled") {
        parsed.control_modes_enabled = serde_json::from_value(control_modes_enabled.clone())
            .map_err(|err| format!("invalid control_modes_enabled payload: {err}"))?;
    }

    Ok(parsed)
}

impl Parameter {
    pub(crate) fn project_encode_data_against_baseline(
        &self,
        baseline_data: Option<&JsonValue>,
        persist_runtime_value: bool,
        persist_constraints: bool,
    ) -> Result<JsonValue, String> {
        let baseline = match baseline_data {
            Some(data) => decode_parameter_project_data(self.get_type(), data)?,
            None => default_parameter_project_data(self.get_type())?,
        };
        let mut data = serde_json::Map::new();

        let value_is_dynamic_default = self.default_value != baseline.default_value && self.value == self.default_value;
        if (persist_runtime_value || value_is_dynamic_default) && self.value != baseline.value {
            data.insert(
                "value".to_string(),
                serde_json::to_value(&self.value).map_err(|err| format!("failed to encode 'value' field: {err}"))?,
            );
        }
        if self.default_value != baseline.default_value {
            data.insert(
                "default_value".to_string(),
                serde_json::to_value(&self.default_value)
                    .map_err(|err| format!("failed to encode 'default_value' field: {err}"))?,
            );
        }
        if self.change_check != baseline.change_check {
            data.insert(
                "change_check".to_string(),
                serde_json::to_value(&self.change_check)
                    .map_err(|err| format!("failed to encode 'change_check' field: {err}"))?,
            );
        }
        if self.event_behaviour != baseline.event_behaviour {
            data.insert(
                "event_behaviour".to_string(),
                serde_json::to_value(self.event_behaviour)
                    .map_err(|err| format!("failed to encode 'event_behaviour' field: {err}"))?,
            );
        }
        if self.read_only != baseline.read_only {
            data.insert(
                "read_only".to_string(),
                serde_json::to_value(self.read_only)
                    .map_err(|err| format!("failed to encode 'read_only' field: {err}"))?,
            );
        }
        if self.persist_read_only_value != baseline.persist_read_only_value {
            data.insert(
                "persist_read_only_value".to_string(),
                serde_json::to_value(self.persist_read_only_value)
                    .map_err(|err| format!("failed to encode 'persist_read_only_value' field: {err}"))?,
            );
        }
        if persist_constraints && self.constraints != baseline.constraints {
            data.insert(
                "constraints".to_string(),
                serde_json::to_value(&self.constraints)
                    .map_err(|err| format!("failed to encode 'constraints' field: {err}"))?,
            );
        }
        if self.ui_hints != baseline.ui_hints {
            data.insert(
                "ui_hints".to_string(),
                serde_json::to_value(&self.ui_hints)
                    .map_err(|err| format!("failed to encode 'ui_hints' field: {err}"))?,
            );
        }
        if self.control != baseline.control {
            data.insert(
                "control".to_string(),
                serde_json::to_value(&self.control)
                    .map_err(|err| format!("failed to encode 'control' field: {err}"))?,
            );
        }
        if self.control_modes_enabled != baseline.control_modes_enabled {
            data.insert(
                "control_modes_enabled".to_string(),
                serde_json::to_value(self.control_modes_enabled)
                    .map_err(|err| format!("failed to encode 'control_modes_enabled' field: {err}"))?,
            );
        }

        if data.is_empty() {
            Ok(serde_json::Value::Null)
        } else {
            Ok(serde_json::Value::Object(data))
        }
    }
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

    // The default implementation answers this by building a full parameter snapshot
    // (value + constraints clones) just to test `is_none()`. Parameters are leaves that
    // never walk the tree in lifecycle hooks, so answer with a constant instead — this
    // gate runs for every node on bulk paths like project load.
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        let baseline = default_parameter_project_data(self.get_type())?;
        let mut data = serde_json::Map::new();

        if self.value != baseline.value {
            data.insert(
                "value".to_string(),
                serde_json::to_value(&self.value).map_err(|err| format!("failed to encode 'value' field: {err}"))?,
            );
        }
        if self.default_value != baseline.default_value {
            data.insert(
                "default_value".to_string(),
                serde_json::to_value(&self.default_value)
                    .map_err(|err| format!("failed to encode 'default_value' field: {err}"))?,
            );
        }
        if self.change_check != baseline.change_check {
            data.insert(
                "change_check".to_string(),
                serde_json::to_value(&self.change_check)
                    .map_err(|err| format!("failed to encode 'change_check' field: {err}"))?,
            );
        }
        if self.event_behaviour != baseline.event_behaviour {
            data.insert(
                "event_behaviour".to_string(),
                serde_json::to_value(self.event_behaviour)
                    .map_err(|err| format!("failed to encode 'event_behaviour' field: {err}"))?,
            );
        }
        if self.read_only != baseline.read_only {
            data.insert(
                "read_only".to_string(),
                serde_json::to_value(self.read_only)
                    .map_err(|err| format!("failed to encode 'read_only' field: {err}"))?,
            );
        }
        if self.persist_read_only_value != baseline.persist_read_only_value {
            data.insert(
                "persist_read_only_value".to_string(),
                serde_json::to_value(self.persist_read_only_value)
                    .map_err(|err| format!("failed to encode 'persist_read_only_value' field: {err}"))?,
            );
        }
        if self.constraints != baseline.constraints {
            data.insert(
                "constraints".to_string(),
                serde_json::to_value(&self.constraints)
                    .map_err(|err| format!("failed to encode 'constraints' field: {err}"))?,
            );
        }
        if self.ui_hints != baseline.ui_hints {
            data.insert(
                "ui_hints".to_string(),
                serde_json::to_value(&self.ui_hints)
                    .map_err(|err| format!("failed to encode 'ui_hints' field: {err}"))?,
            );
        }
        if self.control != baseline.control {
            data.insert(
                "control".to_string(),
                serde_json::to_value(&self.control)
                    .map_err(|err| format!("failed to encode 'control' field: {err}"))?,
            );
        }
        if self.control_modes_enabled != baseline.control_modes_enabled {
            data.insert(
                "control_modes_enabled".to_string(),
                serde_json::to_value(self.control_modes_enabled)
                    .map_err(|err| format!("failed to encode 'control_modes_enabled' field: {err}"))?,
            );
        }

        if data.is_empty() {
            Ok(serde_json::Value::Null)
        } else {
            Ok(serde_json::Value::Object(data))
        }
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        let parsed = decode_parameter_project_data_with_baseline(current_parameter_project_data(self), data)?;
        self.value = parsed.value;
        self.default_value = parsed.default_value;
        self.change_check = parsed.change_check;
        self.event_behaviour = parsed.event_behaviour;
        self.read_only = parsed.read_only;
        self.persist_read_only_value = parsed.persist_read_only_value;
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

    fn inbox_requires_tree_snapshot(&self, _events: &EventFrame) -> bool {
        false
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

    fn engine_set_param_constraints(&mut self, constraints: ParameterConstraints) -> Result<(), String> {
        let value = self.coerce_for_current_value_kind(self.value.clone())?;
        self.value = constraints.normalize(value)?;
        self.constraints = constraints;
        Ok(())
    }

    fn engine_restore_param_state(
        &mut self,
        value: ParamValue,
        constraints: ParameterConstraints,
    ) -> Result<(), String> {
        let value = self.coerce_for_current_value_kind(value)?;
        self.value = constraints.normalize(value)?;
        self.constraints = constraints;
        Ok(())
    }

    fn engine_script_descriptor(&self) -> crate::node::NodeScriptDescriptor {
        crate::node::core_node_script_descriptor(&self.node_data, self.get_type())
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

    fn engine_visit_references(&self, visit: &mut dyn FnMut(&NodeReference)) {
        if let ParamValue::Reference(reference) = &self.value {
            visit(reference);
        }
    }
}
