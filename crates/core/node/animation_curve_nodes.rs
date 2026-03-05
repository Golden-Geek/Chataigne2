use std::collections::HashMap;

use crate::animation_curve::{AnimationCurve, AnimationCurveKey, CurveCoordinateSpace, CurveEasing, CurveHandle, CurvePhaseMode, CurveShape, CurveStepMode};
use crate::events::{Event, EventKind};
use crate::node::NodeId;
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, RangeConstraint};
use crate::process_ctx::{ProcessCtx, ProcessTreeSnapshot};

use super::{
    DeclId, EventPropagation, Node, NodeData, PARAMETER_ANIMATION_CURVE_DECL_ID, PARAMETER_ANIMATION_CURVE_ITEM_KIND, PARAMETER_ANIMATION_CURVE_NODE_TYPE, PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, PARAMETER_ANIMATION_EASING_COORDINATE_SPACE_DECL_ID, PARAMETER_ANIMATION_EASING_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID, PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_NODE_TYPE, PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID, PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID, PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_DECL_ID, PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID, PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID, PARAMETER_ANIMATION_EASING_SEED_DECL_ID, PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID,
    PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID, PARAMETER_ANIMATION_KEY_ITEM_KIND, PARAMETER_ANIMATION_KEY_NODE_TYPE, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, UserContainerRules, UserCreatableItem, parameter_child_exists,
};

fn make_float_parameter(label: &str, decl_id: &str, default_value: f64) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Float(default_value), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

fn make_non_negative_float_parameter(label: &str, decl_id: &str, default_value: f64) -> Parameter {
    let mut parameter = make_float_parameter(label, decl_id, default_value);
    parameter.constraints.range = Some(RangeConstraint::Uniform { min: Some(0.0), max: None });
    parameter
}

fn make_int_parameter(label: &str, decl_id: &str, default_value: i32) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Int(default_value), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

fn make_string_parameter(label: &str, decl_id: &str, default_value: &str) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Str(default_value.to_string()), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

fn make_enum_parameter(label: &str, decl_id: &str, default_variant: &str, variants: &[(&str, &str)]) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Enum(default_variant.to_string()), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter.constraints.enum_options = variants
        .iter()
        .enumerate()
        .map(|(index, (variant_id, variant_label))| ParameterEnumOption {
            variant_id: (*variant_id).to_string(),
            value: ParamValue::Enum((*variant_id).to_string()),
            label: (*variant_label).to_string(),
            tags: Vec::new(),
            ordering: Some(index as i32),
        })
        .collect();
    parameter
}

fn make_easing_kind_parameter() -> Parameter {
    make_enum_parameter(
        "Kind",
        PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
        "linear",
        &[("linear", "Linear"), ("bezier", "Bezier"), ("hold", "Hold"), ("steps", "Steps"), ("shape", "Shape"), ("perlinNoise", "Perlin Noise"), ("random", "Random"), ("script", "Script")],
    )
}

fn make_coordinate_space_parameter() -> Parameter {
    make_enum_parameter("Coordinate Space", PARAMETER_ANIMATION_EASING_COORDINATE_SPACE_DECL_ID, "relative", &[("relative", "Relative"), ("absolute", "Absolute")])
}

fn make_step_mode_parameter() -> Parameter {
    make_enum_parameter("Step Mode", PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID, "numSteps", &[("stepSize", "Step Size"), ("numSteps", "Number of Steps")])
}

fn make_shape_parameter() -> Parameter {
    make_enum_parameter("Shape", PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID, "sine", &[("sine", "Sine"), ("triangle", "Triangle"), ("saw", "Saw"), ("reverseSaw", "Reverse Saw"), ("square", "Square")])
}

fn make_phase_mode_parameter() -> Parameter {
    make_enum_parameter("Phase Mode", PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID, "frequency", &[("frequency", "Frequency"), ("numPhases", "Number of Phases")])
}

const EASING_REQUIRED_LINEAR_DECL_IDS: [&str; 1] = [PARAMETER_ANIMATION_EASING_KIND_DECL_ID];
const EASING_REQUIRED_BEZIER_DECL_IDS: [&str; 6] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_COORDINATE_SPACE_DECL_ID,
    PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID,
    PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID,
    PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID,
    PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID,
];
const EASING_REQUIRED_HOLD_DECL_IDS: [&str; 1] = [PARAMETER_ANIMATION_EASING_KIND_DECL_ID];
const EASING_REQUIRED_STEPS_DECL_IDS: [&str; 4] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID,
    PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID,
    PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID,
];
const EASING_REQUIRED_SHAPE_DECL_IDS: [&str; 9] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_COORDINATE_SPACE_DECL_ID,
    PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID,
    PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID,
    PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID,
    PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID,
];
const EASING_REQUIRED_PERLIN_NOISE_DECL_IDS: [&str; 7] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID,
    PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID,
    PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_DECL_ID,
];
const EASING_REQUIRED_RANDOM_DECL_IDS: [&str; 5] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID,
    PARAMETER_ANIMATION_EASING_SEED_DECL_ID,
];
const EASING_REQUIRED_SCRIPT_DECL_IDS: [&str; 2] = [PARAMETER_ANIMATION_EASING_KIND_DECL_ID, PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID];

const EASING_MANAGED_DECL_IDS: [&str; 20] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_COORDINATE_SPACE_DECL_ID,
    PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID,
    PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID,
    PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID,
    PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID,
    PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID,
    PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID,
    PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID,
    PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID,
    PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID,
    PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID,
    PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID,
    PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_DECL_ID,
    PARAMETER_ANIMATION_EASING_SEED_DECL_ID,
    PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID,
];

fn read_child_param_value<'a>(snapshot: &'a ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<&'a ParamValue> {
    let child = snapshot.find_child(parent, decl_id)?;
    snapshot.node(child)?.param_value.as_ref()
}

fn read_child_param_f64(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: f64) -> f64 {
    read_child_param_value(snapshot, parent, decl_id).and_then(|value| value.as_float().or_else(|| value.as_int().map(|int_value| int_value as f64))).unwrap_or(default_value)
}

fn read_child_param_u32(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: u32) -> u32 {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(|value| value.as_int().or_else(|| value.as_float().map(|float_value| float_value.round() as i32)))
        .map(|value| value.max(0) as u32)
        .unwrap_or(default_value)
}

fn read_child_param_u64(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: u64) -> u64 {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(|value| value.as_int().or_else(|| value.as_float().map(|float_value| float_value.round() as i32)))
        .map(|value| value as i64 as u64)
        .unwrap_or(default_value)
}

fn read_child_param_enum(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: &str) -> String {
    read_child_param_value(snapshot, parent, decl_id).and_then(ParamValue::as_enum).unwrap_or_else(|| default_value.to_string())
}

fn read_child_param_string(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: &str) -> String {
    read_child_param_value(snapshot, parent, decl_id).and_then(ParamValue::as_str).unwrap_or_else(|| default_value.to_string())
}

fn parse_coordinate_space(value: &str) -> CurveCoordinateSpace {
    match value.trim().to_ascii_lowercase().as_str() {
        "absolute" => CurveCoordinateSpace::Absolute,
        _ => CurveCoordinateSpace::Relative,
    }
}

fn parse_step_mode(value: &str) -> CurveStepMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "stepsize" => CurveStepMode::StepSize,
        _ => CurveStepMode::NumSteps,
    }
}

fn parse_shape(value: &str) -> CurveShape {
    match value.trim().to_ascii_lowercase().as_str() {
        "triangle" => CurveShape::Triangle,
        "saw" => CurveShape::Saw,
        "reversesaw" => CurveShape::ReverseSaw,
        "square" => CurveShape::Square,
        _ => CurveShape::Sine,
    }
}

fn parse_phase_mode(value: &str) -> CurvePhaseMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "numphases" => CurvePhaseMode::NumPhases,
        _ => CurvePhaseMode::Frequency,
    }
}

/// Internal node hosting one animation-curve key list.
pub struct AnimationCurveNode {
    node_data: NodeData,
}

impl AnimationCurveNode {
    /// Creates one animation-curve node.
    pub fn new() -> Self {
        Self::new_with_label("Curve")
    }

    /// Creates one animation-curve node with custom label.
    pub fn new_with_label(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_CURVE_DECL_ID.to_string());
        Self { node_data }
    }
}

impl Node for AnimationCurveNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_CURVE_NODE_TYPE
    }

    fn user_item_kind(&self) -> &str {
        PARAMETER_ANIMATION_CURVE_ITEM_KIND
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[PARAMETER_ANIMATION_KEY_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new(PARAMETER_ANIMATION_KEY_NODE_TYPE, PARAMETER_ANIMATION_KEY_ITEM_KIND, "Key")]
    }

    fn create_user_item(&self, node_type: &str, label: String) -> Option<Box<dyn Node>> {
        let normalized = node_type.trim().to_ascii_lowercase();
        if normalized == PARAMETER_ANIMATION_KEY_NODE_TYPE || normalized == "key" {
            return Some(Box::new(AnimationCurveKeyNode::new_with_label(label)));
        }
        None
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        let key_count = ctx
            .tree_snapshot()
            .map(|snapshot| snapshot.child_ids(self.id()).into_iter().filter(|node_id| snapshot.node(*node_id).is_some_and(|node| node.node_type == PARAMETER_ANIMATION_KEY_NODE_TYPE)).count())
            .unwrap_or(0);

        if key_count == 0 {
            ctx.add_child_boxed(self.id(), Box::new(AnimationCurveKeyNode::new_with_values(0.0, 0.0)), None);
            ctx.add_child_boxed(self.id(), Box::new(AnimationCurveKeyNode::new_with_values(1.0, 1.0)), None);
        }
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

/// Internal node representing one animation curve key.
pub struct AnimationCurveKeyNode {
    node_data: NodeData,
    default_position: f64,
    default_value: f64,
}

impl AnimationCurveKeyNode {
    /// Creates one key node with default position/value set to `0`.
    pub fn new() -> Self {
        Self::new_with_label_and_values("Key", 0.0, 0.0)
    }

    /// Creates one key node with custom label and default position/value.
    pub fn new_with_label(label: impl Into<String>) -> Self {
        Self::new_with_label_and_values(label, 0.0, 0.0)
    }

    /// Creates one key node with explicit initial position/value.
    pub fn new_with_values(position: f64, value: f64) -> Self {
        Self::new_with_label_and_values("Key", position, value)
    }

    fn new_with_label_and_values(label: impl Into<String>, position: f64, value: f64) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        Self {
            node_data,
            default_position: position,
            default_value: value,
        }
    }
}

impl Node for AnimationCurveKeyNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_KEY_NODE_TYPE
    }

    fn user_item_kind(&self) -> &str {
        PARAMETER_ANIMATION_KEY_ITEM_KIND
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_KEY_POSITION_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_float_parameter("Position", PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, self.default_position)), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_KEY_VALUE_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_float_parameter("Value", PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, self.default_value)), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_EASING_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(AnimationCurveEasingNode::new("Easing")), None);
        }
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

/// Internal node storing one key-to-next easing specification.
pub struct AnimationCurveEasingNode {
    node_data: NodeData,
    current_kind: &'static str,
    kind_param: Option<NodeId>,
    managed_children: HashMap<String, Option<NodeId>>,
}

impl AnimationCurveEasingNode {
    /// Creates one easing node.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_EASING_DECL_ID.to_string());
        Self {
            node_data,
            current_kind: "linear",
            kind_param: None,
            managed_children: HashMap::new(),
        }
    }

    fn normalize_kind(kind: &str) -> &'static str {
        match kind.trim().to_ascii_lowercase().as_str() {
            "bezier" => "bezier",
            "hold" => "hold",
            "steps" => "steps",
            "shape" => "shape",
            "perlinnoise" => "perlinnoise",
            "random" => "random",
            "script" => "script",
            _ => "linear",
        }
    }

    fn required_decl_ids_for_kind(kind: &str) -> &'static [&'static str] {
        match Self::normalize_kind(kind) {
            "bezier" => &EASING_REQUIRED_BEZIER_DECL_IDS,
            "hold" => &EASING_REQUIRED_HOLD_DECL_IDS,
            "steps" => &EASING_REQUIRED_STEPS_DECL_IDS,
            "shape" => &EASING_REQUIRED_SHAPE_DECL_IDS,
            "perlinnoise" => &EASING_REQUIRED_PERLIN_NOISE_DECL_IDS,
            "random" => &EASING_REQUIRED_RANDOM_DECL_IDS,
            "script" => &EASING_REQUIRED_SCRIPT_DECL_IDS,
            _ => &EASING_REQUIRED_LINEAR_DECL_IDS,
        }
    }

    fn is_managed_decl_id(decl_id: &str) -> bool {
        EASING_MANAGED_DECL_IDS.contains(&decl_id)
    }

    fn is_required_decl_id_for_kind(kind: &str, decl_id: &str) -> bool {
        Self::required_decl_ids_for_kind(kind).contains(&decl_id)
    }

    fn kind_from_param_value(value: &ParamValue) -> &'static str {
        if let Some(kind) = value.as_enum() {
            return Self::normalize_kind(kind.as_str());
        }
        if let Some(kind) = value.as_str() {
            return Self::normalize_kind(kind.as_str());
        }
        "linear"
    }

    fn kind_from_snapshot(snapshot: &ProcessTreeSnapshot, easing_node: NodeId) -> &'static str {
        let kind = read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID, "linear");
        Self::normalize_kind(kind.as_str())
    }

    fn bind_decl_child(&mut self, decl_id: &str, child: NodeId) {
        if !Self::is_managed_decl_id(decl_id) {
            return;
        }
        self.managed_children.insert(decl_id.to_string(), Some(child));
        if decl_id == PARAMETER_ANIMATION_EASING_KIND_DECL_ID {
            self.kind_param = Some(child);
        }
    }

    fn mark_decl_child_pending_addition(&mut self, decl_id: &str) {
        if !Self::is_managed_decl_id(decl_id) {
            return;
        }
        self.managed_children.entry(decl_id.to_string()).or_insert(None);
    }

    fn unbind_child(&mut self, child: NodeId) {
        let removed_decl = self
            .managed_children
            .iter()
            .find_map(|(decl_id, node_id)| (*node_id == Some(child)).then_some(decl_id.clone()));

        if let Some(decl_id) = removed_decl {
            self.managed_children.remove(decl_id.as_str());
        }
        if self.kind_param == Some(child) {
            self.kind_param = None;
        }
    }

    fn make_parameter_node(decl_id: &str) -> Option<Box<dyn Node>> {
        match decl_id {
            PARAMETER_ANIMATION_EASING_KIND_DECL_ID => Some(Box::new(make_easing_kind_parameter())),
            PARAMETER_ANIMATION_EASING_COORDINATE_SPACE_DECL_ID => Some(Box::new(make_coordinate_space_parameter())),
            PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID => {
                Some(Box::new(make_float_parameter("Out Handle Position", PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID, 1.0 / 3.0)))
            }
            PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID => {
                Some(Box::new(make_float_parameter("Out Handle Value", PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID, 1.0 / 3.0)))
            }
            PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID => {
                Some(Box::new(make_float_parameter("In Handle Position", PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID, -1.0 / 3.0)))
            }
            PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID => {
                Some(Box::new(make_float_parameter("In Handle Value", PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID, -1.0 / 3.0)))
            }
            PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID => Some(Box::new(make_step_mode_parameter())),
            PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID => Some(Box::new(make_non_negative_float_parameter("Step Size", PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID, 0.1))),
            PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID => {
                let mut parameter = make_int_parameter("Number of Steps", PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID, 8);
                parameter.constraints.range = Some(RangeConstraint::Uniform { min: Some(1.0), max: None });
                Some(Box::new(parameter))
            }
            PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID => Some(Box::new(make_shape_parameter())),
            PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID => {
                Some(Box::new(make_float_parameter("Amplitude", PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, 1.0)))
            }
            PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID => Some(Box::new(make_phase_mode_parameter())),
            PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID => {
                Some(Box::new(make_non_negative_float_parameter("Frequency", PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 1.0)))
            }
            PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID => {
                Some(Box::new(make_non_negative_float_parameter("Number of Phases", PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID, 1.0)))
            }
            PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID => {
                Some(Box::new(make_non_negative_float_parameter("Fade In", PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0)))
            }
            PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID => {
                Some(Box::new(make_non_negative_float_parameter("Fade Out", PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0)))
            }
            PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID => {
                let mut parameter = make_int_parameter("Octaves", PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID, 4);
                parameter.constraints.range = Some(RangeConstraint::Uniform { min: Some(1.0), max: None });
                Some(Box::new(parameter))
            }
            PARAMETER_ANIMATION_EASING_PHASE_DECL_ID => {
                Some(Box::new(make_float_parameter("Phase", PARAMETER_ANIMATION_EASING_PHASE_DECL_ID, 0.0)))
            }
            PARAMETER_ANIMATION_EASING_SEED_DECL_ID => {
                Some(Box::new(make_int_parameter("Seed", PARAMETER_ANIMATION_EASING_SEED_DECL_ID, 0)))
            }
            PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID => {
                Some(Box::new(make_string_parameter("Script Source", PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID, "")))
            }
            _ => None,
        }
    }

    fn sync_parameter_children_for_kind(&mut self, ctx: &mut ProcessCtx, kind: &str) {
        let normalized_kind = Self::normalize_kind(kind);
        if let Some(snapshot) = ctx.tree_snapshot() {
            self.managed_children.clear();
            self.kind_param = None;
            let mut duplicate_children = Vec::new();
            for child_id in snapshot.child_ids(self.id()) {
                let Some(child_snapshot) = snapshot.node(child_id) else {
                    continue;
                };
                if child_snapshot.param_value.is_none() {
                    continue;
                }
                let decl_id = child_snapshot.decl_id.as_str();
                if !Self::is_managed_decl_id(decl_id) {
                    continue;
                }
                if self.managed_children.contains_key(decl_id) {
                    duplicate_children.push(child_id);
                    continue;
                }
                self.bind_decl_child(decl_id, child_id);
            }

            for duplicate_child in duplicate_children {
                self.remove_child(ctx, duplicate_child);
            }
        }

        for required_decl_id in Self::required_decl_ids_for_kind(normalized_kind) {
            if self.managed_children.contains_key(*required_decl_id) {
                continue;
            }
            if let Some(parameter_node) = Self::make_parameter_node(required_decl_id) {
                self.mark_decl_child_pending_addition(required_decl_id);
                self.add_child_boxed(ctx, parameter_node, None);
            }
        }

        let existing_children_by_decl = self.managed_children.clone();
        for (decl_id, maybe_child_id) in existing_children_by_decl {
            if Self::is_required_decl_id_for_kind(normalized_kind, decl_id.as_str()) {
                continue;
            }
            self.managed_children.remove(decl_id.as_str());
            if let Some(child_id) = maybe_child_id {
                self.remove_child(ctx, child_id);
                if self.kind_param == Some(child_id) {
                    self.kind_param = None;
                }
            } else if decl_id == PARAMETER_ANIMATION_EASING_KIND_DECL_ID {
                self.kind_param = None;
            }
        }
    }
}

impl Node for AnimationCurveEasingNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_EASING_NODE_TYPE
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        let current_kind = ctx
            .tree_snapshot()
            .map(|snapshot| Self::kind_from_snapshot(snapshot, self.id()))
            .unwrap_or(self.current_kind);
        self.current_kind = current_kind;
        self.sync_parameter_children_for_kind(ctx, self.current_kind);
    }

    fn engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        let mut next_kind_from_event: Option<&'static str> = None;
        let mut should_sync = !self.managed_children.contains_key(PARAMETER_ANIMATION_EASING_KIND_DECL_ID);
        let mut duplicate_children_to_remove = Vec::new();

        for event in &ctx.events {
            match &event.kind {
                EventKind::ParamChanged { param, new_value, .. } => {
                    if Some(*param) == self.kind_param {
                        next_kind_from_event = Some(Self::kind_from_param_value(new_value));
                        should_sync = true;
                    }
                }
                EventKind::ChildAdded { parent, child, decl_id } => {
                    if *parent == self.id() {
                        let child_decl_id = decl_id.0.as_str();
                        if let Some(Some(existing_child)) = self.managed_children.get(child_decl_id) {
                            if *existing_child != *child {
                                duplicate_children_to_remove.push(*child);
                                should_sync = true;
                                continue;
                            }
                        }
                        self.bind_decl_child(child_decl_id, *child);
                        should_sync = true;
                    }
                }
                EventKind::ChildRemoved { parent, child } => {
                    if *parent == self.id() {
                        self.unbind_child(*child);
                        should_sync = true;
                    }
                }
                EventKind::ChildReplaced { parent, old, new, decl_id } => {
                    if *parent == self.id() {
                        let child_decl_id = decl_id.0.as_str();
                        self.unbind_child(*old);
                        if let Some(Some(existing_child)) = self.managed_children.get(child_decl_id) {
                            if *existing_child != *new {
                                duplicate_children_to_remove.push(*new);
                                should_sync = true;
                                continue;
                            }
                        }
                        self.bind_decl_child(child_decl_id, *new);
                        should_sync = true;
                    }
                }
                EventKind::ChildReordered { parent, .. } => {
                    if *parent == self.id() {
                        should_sync = true;
                    }
                }
                EventKind::ChildMoved {
                    old_parent,
                    new_parent,
                    ..
                } => {
                    if *old_parent == self.id() || *new_parent == self.id() {
                        should_sync = true;
                    }
                }
                _ => {}
            }
        }

        for duplicate_child in duplicate_children_to_remove {
            self.remove_child(ctx, duplicate_child);
        }

        if !should_sync {
            return;
        }

        if let Some(next_kind) = next_kind_from_event {
            self.current_kind = next_kind;
        }
        self.sync_parameter_children_for_kind(ctx, self.current_kind);
    }
}

fn parse_easing_from_snapshot(snapshot: &ProcessTreeSnapshot, easing_node: NodeId) -> CurveEasing {
    let kind = read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID, "linear");
    match kind.trim().to_ascii_lowercase().as_str() {
        "bezier" => CurveEasing::Bezier {
            coordinate_space: parse_coordinate_space(read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_COORDINATE_SPACE_DECL_ID, "relative").as_str()),
            out_handle: CurveHandle::new(
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID, 1.0 / 3.0),
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID, 1.0 / 3.0),
            ),
            in_handle: CurveHandle::new(
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID, -1.0 / 3.0),
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID, -1.0 / 3.0),
            ),
        },
        "hold" => CurveEasing::Hold,
        "steps" => CurveEasing::Steps {
            step_mode: parse_step_mode(read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID, "numSteps").as_str()),
            step_size: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID, 0.1),
            num_steps: read_child_param_u32(snapshot, easing_node, PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID, 8).max(1),
        },
        "shape" => CurveEasing::Shape {
            coordinate_space: parse_coordinate_space(read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_COORDINATE_SPACE_DECL_ID, "relative").as_str()),
            shape: parse_shape(read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID, "sine").as_str()),
            amplitude: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, 1.0),
            phase_mode: parse_phase_mode(read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID, "frequency").as_str()),
            frequency: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 1.0),
            num_phases: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID, 1.0),
            fade_in: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0),
            fade_out: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0),
        },
        "perlinnoise" => CurveEasing::PerlinNoise {
            frequency: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 1.0),
            amplitude: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, 1.0),
            octaves: read_child_param_u32(snapshot, easing_node, PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID, 4).max(1),
            fade_in: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0),
            fade_out: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0),
            phase: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_PHASE_DECL_ID, 0.0),
        },
        "random" => CurveEasing::Random {
            frequency: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 6.0),
            fade_in: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0),
            fade_out: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0),
            seed: read_child_param_u64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_SEED_DECL_ID, 0),
        },
        "script" => CurveEasing::Script {
            source: read_child_param_string(snapshot, easing_node, PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID, ""),
        },
        _ => CurveEasing::Linear,
    }
}

fn parse_key_from_snapshot(snapshot: &ProcessTreeSnapshot, key_node: NodeId) -> Option<AnimationCurveKey> {
    let key = snapshot.node(key_node)?;
    if key.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
        return None;
    }

    let position = read_child_param_f64(snapshot, key_node, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, 0.0);
    let value = read_child_param_f64(snapshot, key_node, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, 0.0);
    let easing = snapshot.find_child(key_node, PARAMETER_ANIMATION_EASING_DECL_ID).map(|easing_node| parse_easing_from_snapshot(snapshot, easing_node)).unwrap_or(CurveEasing::Linear);

    Some(AnimationCurveKey::new(position, value, easing))
}

/// Builds an [`AnimationCurve`] from one curve-node subtree in a processing snapshot.
pub fn curve_from_snapshot(snapshot: &ProcessTreeSnapshot, curve_node: NodeId) -> Option<AnimationCurve> {
    let curve_snapshot = snapshot.node(curve_node)?;
    if curve_snapshot.node_type != PARAMETER_ANIMATION_CURVE_NODE_TYPE {
        return None;
    }

    let keys = snapshot.child_ids(curve_node).into_iter().filter_map(|child_id| parse_key_from_snapshot(snapshot, child_id)).collect::<Vec<_>>();

    Some(AnimationCurve::new(keys))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_helpers_map_variants() {
        assert_eq!(parse_coordinate_space("absolute"), CurveCoordinateSpace::Absolute);
        assert_eq!(parse_step_mode("stepSize"), CurveStepMode::StepSize);
        assert_eq!(parse_shape("reverseSaw"), CurveShape::ReverseSaw);
        assert_eq!(parse_phase_mode("numPhases"), CurvePhaseMode::NumPhases);
    }
}
