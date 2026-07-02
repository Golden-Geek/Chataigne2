use std::f64::consts::PI;

use crate::engine::NodeExecutionRule;
use crate::events::{Event, EventKind};
use crate::node::{
    CurveNode, DeclId, EventPropagation, Node, NodeData, NodeId, PARAMETER_ANIMATION_AMPLITUDE_DECL_ID,
    PARAMETER_ANIMATION_CONTROL_DECL_ID, PARAMETER_ANIMATION_CONTROL_NODE_TYPE, PARAMETER_ANIMATION_CURVE_DECL_ID,
    PARAMETER_ANIMATION_FREQUENCY_DECL_ID, PARAMETER_ANIMATION_OFFSET_DECL_ID, PARAMETER_ANIMATION_PHASE_DECL_ID,
    PARAMETER_ANIMATION_UPDATE_RATE_DECL_ID, PARAMETER_ANIMATION_WAVEFORM_DECL_ID, PARAMETER_CONTROL_ITEM_KIND,
    curve_from_snapshot, parameter_child_exists,
};
use crate::parameter::{
    AnimationWaveform, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, RangeConstraint,
};
use crate::process_ctx::ProcessCtx;

const DEFAULT_ANIMATION_UPDATE_RATE_HZ: u32 = 60;

fn make_animation_waveform_parameter() -> Parameter {
    let mut waveform = Parameter::new(
        "Waveform",
        ParamValue::Enum("sine".to_string()),
        ParameterChangeCheck::ValueChange,
    );
    waveform.node_data_mut().meta.decl_id = DeclId(PARAMETER_ANIMATION_WAVEFORM_DECL_ID.to_string());
    waveform.node_data_mut().meta.can_be_disabled = false;
    waveform.constraints.enum_options = vec![
        ParameterEnumOption {
            variant_id: "sine".to_string(),
            value: ParamValue::Enum("sine".to_string()),
            label: "Sine".to_string(),
            tags: Vec::new(),
            ordering: Some(0),
        },
        ParameterEnumOption {
            variant_id: "triangle".to_string(),
            value: ParamValue::Enum("triangle".to_string()),
            label: "Triangle".to_string(),
            tags: Vec::new(),
            ordering: Some(1),
        },
        ParameterEnumOption {
            variant_id: "saw".to_string(),
            value: ParamValue::Enum("saw".to_string()),
            label: "Saw".to_string(),
            tags: Vec::new(),
            ordering: Some(2),
        },
        ParameterEnumOption {
            variant_id: "square".to_string(),
            value: ParamValue::Enum("square".to_string()),
            label: "Square".to_string(),
            tags: Vec::new(),
            ordering: Some(3),
        },
        ParameterEnumOption {
            variant_id: "curve".to_string(),
            value: ParamValue::Enum("curve".to_string()),
            label: "Curve".to_string(),
            tags: Vec::new(),
            ordering: Some(4),
        },
    ];
    waveform
}

fn make_animation_frequency_parameter() -> Parameter {
    let mut frequency = Parameter::new(
        "Frequency (Hz)",
        ParamValue::Float(1.0),
        ParameterChangeCheck::ValueChange,
    );
    frequency.node_data_mut().meta.decl_id = DeclId(PARAMETER_ANIMATION_FREQUENCY_DECL_ID.to_string());
    frequency.node_data_mut().meta.can_be_disabled = false;
    frequency.constraints.range = Some(RangeConstraint::Uniform {
        min: Some(0.0),
        max: None,
    });
    frequency
}

fn make_animation_amplitude_parameter() -> Parameter {
    let mut amplitude = Parameter::new("Amplitude", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange);
    amplitude.node_data_mut().meta.decl_id = DeclId(PARAMETER_ANIMATION_AMPLITUDE_DECL_ID.to_string());
    amplitude.node_data_mut().meta.can_be_disabled = false;
    amplitude
}

fn make_animation_offset_parameter() -> Parameter {
    let mut offset = Parameter::new("Offset", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange);
    offset.node_data_mut().meta.decl_id = DeclId(PARAMETER_ANIMATION_OFFSET_DECL_ID.to_string());
    offset.node_data_mut().meta.can_be_disabled = false;
    offset
}

fn make_animation_phase_parameter() -> Parameter {
    let mut phase = Parameter::new("Phase", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange);
    phase.node_data_mut().meta.decl_id = DeclId(PARAMETER_ANIMATION_PHASE_DECL_ID.to_string());
    phase.node_data_mut().meta.can_be_disabled = false;
    phase
}

fn make_animation_update_rate_parameter() -> Parameter {
    let mut update_rate = Parameter::new(
        "Update Rate (Hz)",
        ParamValue::Int(DEFAULT_ANIMATION_UPDATE_RATE_HZ as i32),
        ParameterChangeCheck::ValueChange,
    );
    update_rate.node_data_mut().meta.decl_id = DeclId(PARAMETER_ANIMATION_UPDATE_RATE_DECL_ID.to_string());
    update_rate.node_data_mut().meta.can_be_disabled = false;
    update_rate.constraints.range = Some(RangeConstraint::Uniform {
        min: Some(1.0),
        max: None,
    });
    update_rate
}

fn parse_waveform(value: &ParamValue) -> AnimationWaveform {
    match value
        .as_enum()
        .unwrap_or_else(|| "sine".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "sine" => AnimationWaveform::Sine,
        "triangle" => AnimationWaveform::Triangle,
        "saw" => AnimationWaveform::Saw,
        "square" => AnimationWaveform::Square,
        "curve" => AnimationWaveform::Curve,
        _ => AnimationWaveform::Sine,
    }
}

fn parse_update_rate_hz(value: &ParamValue) -> u32 {
    let parsed = value
        .as_int()
        .map(|rate| rate as f64)
        .or_else(|| value.as_float())
        .unwrap_or(DEFAULT_ANIMATION_UPDATE_RATE_HZ as f64);

    let rounded = parsed.round().clamp(1.0, u32::MAX as f64);
    rounded as u32
}

/// Internal control node attached to one parameter for animation behavior.
pub struct ParameterAnimationControlNode {
    node_data: NodeData,
    curve_node: Option<NodeId>,
    waveform_param: Option<NodeId>,
    frequency_param: Option<NodeId>,
    amplitude_param: Option<NodeId>,
    offset_param: Option<NodeId>,
    phase_param: Option<NodeId>,
    update_rate_param: Option<NodeId>,
    waveform: AnimationWaveform,
    frequency_hz: f64,
    amplitude: f64,
    offset: f64,
    phase: f64,
    update_rate_hz: u32,
    elapsed_seconds: f64,
}

impl ParameterAnimationControlNode {
    /// Creates a new animation-control node.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_CONTROL_DECL_ID.to_string());
        Self {
            node_data,
            curve_node: None,
            waveform_param: None,
            frequency_param: None,
            amplitude_param: None,
            offset_param: None,
            phase_param: None,
            update_rate_param: None,
            waveform: AnimationWaveform::Sine,
            frequency_hz: 1.0,
            amplitude: 1.0,
            offset: 0.0,
            phase: 0.0,
            update_rate_hz: DEFAULT_ANIMATION_UPDATE_RATE_HZ,
            elapsed_seconds: 0.0,
        }
    }

    fn bind_decl_child(&mut self, decl_id: &str, child: NodeId) {
        match decl_id {
            PARAMETER_ANIMATION_CURVE_DECL_ID => self.curve_node = Some(child),
            PARAMETER_ANIMATION_WAVEFORM_DECL_ID => self.waveform_param = Some(child),
            PARAMETER_ANIMATION_FREQUENCY_DECL_ID => self.frequency_param = Some(child),
            PARAMETER_ANIMATION_AMPLITUDE_DECL_ID => self.amplitude_param = Some(child),
            PARAMETER_ANIMATION_OFFSET_DECL_ID => self.offset_param = Some(child),
            PARAMETER_ANIMATION_PHASE_DECL_ID => self.phase_param = Some(child),
            PARAMETER_ANIMATION_UPDATE_RATE_DECL_ID => self.update_rate_param = Some(child),
            _ => {}
        }
    }

    fn unbind_child(&mut self, child: NodeId) {
        if self.curve_node == Some(child) {
            self.curve_node = None;
        }
        if self.waveform_param == Some(child) {
            self.waveform_param = None;
        }
        if self.frequency_param == Some(child) {
            self.frequency_param = None;
        }
        if self.amplitude_param == Some(child) {
            self.amplitude_param = None;
        }
        if self.offset_param == Some(child) {
            self.offset_param = None;
        }
        if self.phase_param == Some(child) {
            self.phase_param = None;
        }
        if self.update_rate_param == Some(child) {
            self.update_rate_param = None;
        }
    }

    fn sync_child_value(&mut self, param: NodeId, value: &ParamValue) {
        if self.waveform_param == Some(param) {
            self.waveform = parse_waveform(value);
        }
        if self.frequency_param == Some(param) {
            if let Some(parsed) = value.as_float() {
                self.frequency_hz = parsed;
            }
        }
        if self.amplitude_param == Some(param) {
            if let Some(parsed) = value.as_float() {
                self.amplitude = parsed;
            }
        }
        if self.offset_param == Some(param) {
            if let Some(parsed) = value.as_float() {
                self.offset = parsed;
            }
        }
        if self.phase_param == Some(param) {
            if let Some(parsed) = value.as_float() {
                self.phase = parsed;
            }
        }
        if self.update_rate_param == Some(param) {
            self.update_rate_hz = parse_update_rate_hz(value);
        }
    }

    fn sync_bound_value(&mut self, param: Option<NodeId>, resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {
        if let Some(param) = param {
            if let Some(value) = resolve(param) {
                self.sync_child_value(param, &value);
            }
        }
    }

    fn current_value(&self) -> Option<f64> {
        if self.frequency_hz.is_sign_negative() {
            return None;
        }

        let phase = self.phase + self.elapsed_seconds * self.frequency_hz;
        let waveform = match self.waveform {
            AnimationWaveform::Sine => (phase * (PI * 2.0)).sin(),
            AnimationWaveform::Triangle => 2.0 * (2.0 * (phase - (phase + 0.5).floor())).abs() - 1.0,
            AnimationWaveform::Saw => 2.0 * (phase - (phase + 0.5).floor()),
            AnimationWaveform::Square => {
                if (phase * (PI * 2.0)).sin().is_sign_negative() {
                    -1.0
                } else {
                    1.0
                }
            }
            AnimationWaveform::Curve => return None,
        };

        Some(self.offset + self.amplitude * waveform)
    }

    fn curve_value(&self, ctx: &ProcessCtx) -> Option<f64> {
        if self.frequency_hz.is_sign_negative() {
            return None;
        }
        let curve_node_id = self.curve_node?;
        let snapshot = ctx.tree_snapshot()?;
        let phase = self.phase + self.elapsed_seconds * self.frequency_hz;
        let normalized = phase - phase.floor();
        curve_from_snapshot(snapshot, curve_node_id).and_then(|curve| curve.sample(normalized))
    }

    fn sync_waveform_dependent_nodes(&mut self, ctx: &mut ProcessCtx) {
        let is_curve = self.waveform == AnimationWaveform::Curve;

        let (curve_child, amplitude_child, offset_child) = ctx
            .tree_snapshot()
            .map(|snapshot| {
                (
                    snapshot.find_child(self.id(), PARAMETER_ANIMATION_CURVE_DECL_ID),
                    snapshot.find_child(self.id(), PARAMETER_ANIMATION_AMPLITUDE_DECL_ID),
                    snapshot.find_child(self.id(), PARAMETER_ANIMATION_OFFSET_DECL_ID),
                )
            })
            .unwrap_or((None, None, None));

        if is_curve {
            if curve_child.is_none() {
                ctx.add_child_boxed(self.id(), Box::new(CurveNode::new()), None);
            }
            if let Some(amp_id) = amplitude_child {
                self.remove_child(ctx, amp_id);
            }
            if let Some(off_id) = offset_child {
                self.remove_child(ctx, off_id);
            }
        } else {
            if let Some(curve_id) = curve_child {
                self.remove_child(ctx, curve_id);
            }
            if amplitude_child.is_none() {
                ctx.add_child_boxed(self.id(), Box::new(make_animation_amplitude_parameter()), None);
            }
            if offset_child.is_none() {
                ctx.add_child_boxed(self.id(), Box::new(make_animation_offset_parameter()), None);
            }
        }
    }
}

impl Node for ParameterAnimationControlNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_CONTROL_NODE_TYPE
    }

    fn type_description(&self) -> Option<&str> {
        Some("Internal control node attached to one parameter for animation behavior.")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn user_item_kind(&self) -> &str {
        PARAMETER_CONTROL_ITEM_KIND
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(self.update_rate_hz.max(1))
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        // `engine_on_attached` consults the tree snapshot via `parameter_child_exists`
        // to avoid recreating declared child parameters.
        true
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_WAVEFORM_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_animation_waveform_parameter()), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_FREQUENCY_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_animation_frequency_parameter()), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_AMPLITUDE_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_animation_amplitude_parameter()), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_OFFSET_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_animation_offset_parameter()), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_PHASE_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_animation_phase_parameter()), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_UPDATE_RATE_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_animation_update_rate_parameter()), None);
        }
        // Waveform-dependent node presence (CurveNode, amplitude, offset) is reconciled
        // in init() rather than here. Removing nodes during engine_on_attached would
        // place those NodeIds in loaded_node_ids before reconcile_loaded_declared_children
        // runs, causing a MissingNode error when the reconcile traverses them.
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        // Read the persisted waveform value directly from the snapshot so that loaded
        // projects immediately reflect the correct waveform-dependent node presence.
        // self.waveform is still the default (Sine) at this point.
        let effective_waveform = ctx
            .tree_snapshot()
            .and_then(|snapshot| {
                snapshot
                    .find_child(self.id(), PARAMETER_ANIMATION_WAVEFORM_DECL_ID)
                    .and_then(|id| snapshot.node(id))
                    .and_then(|n| n.param_value.as_ref().map(parse_waveform))
            })
            .unwrap_or(self.waveform);
        self.waveform = effective_waveform;
        self.sync_waveform_dependent_nodes(ctx);
    }

    fn engine_sync_param_handle_cache(&mut self, param: NodeId, new_value: &ParamValue) {
        self.sync_child_value(param, new_value);
    }

    fn engine_sync_bound_param_handles(&mut self, resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {
        self.sync_bound_value(self.waveform_param, resolve);
        self.sync_bound_value(self.frequency_param, resolve);
        self.sync_bound_value(self.amplitude_param, resolve);
        self.sync_bound_value(self.offset_param, resolve);
        self.sync_bound_value(self.phase_param, resolve);
        self.sync_bound_value(self.update_rate_param, resolve);
    }

    fn engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        let mut reevaluate_graph = false;
        let mut waveform_changed = false;
        for event in &ctx.events {
            match &event.kind {
                EventKind::ParamChanged { param, new_value, .. } => {
                    let old_rate = self.update_rate_hz;
                    let old_waveform = self.waveform;
                    self.sync_child_value(*param, new_value);
                    if self.update_rate_param == Some(*param) && self.update_rate_hz != old_rate {
                        reevaluate_graph = true;
                    }
                    if self.waveform_param == Some(*param) && self.waveform != old_waveform {
                        waveform_changed = true;
                    }
                }
                EventKind::ChildAdded { parent, child, decl_id } if *parent == self.id() => {
                    self.bind_decl_child(decl_id.0.as_str(), *child);
                }
                EventKind::ChildReplaced {
                    parent,
                    old,
                    new,
                    decl_id,
                } if *parent == self.id() => {
                    self.unbind_child(*old);
                    self.bind_decl_child(decl_id.0.as_str(), *new);
                }
                EventKind::ChildRemoved { parent, child } if *parent == self.id() => {
                    self.unbind_child(*child);
                }
                _ => {}
            }
        }
        if reevaluate_graph {
            ctx.reevaluate_graph();
        }
        if waveform_changed {
            self.sync_waveform_dependent_nodes(ctx);
        }
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.waveform == AnimationWaveform::Curve
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.elapsed_seconds += ctx.delta_time.as_secs_f64();
        let Some(parent_param) = self.node_data.parent else {
            return;
        };
        let value = if self.waveform == AnimationWaveform::Curve {
            self.curve_value(ctx)
        } else {
            self.current_value()
        };
        let Some(value) = value else {
            return;
        };

        ctx.set_param(parent_param, ParamValue::Float(value));
    }

    fn on_child_added_decl(&mut self, _ctx: &mut ProcessCtx, parent: NodeId, child: NodeId, decl_id: &DeclId) {
        if parent != self.id() {
            return;
        }

        self.bind_decl_child(decl_id.0.as_str(), child);
    }

    fn on_child_replaced_decl(
        &mut self,
        _ctx: &mut ProcessCtx,
        parent: NodeId,
        old: NodeId,
        new: NodeId,
        decl_id: &DeclId,
    ) {
        if parent != self.id() {
            return;
        }

        self.unbind_child(old);
        self.bind_decl_child(decl_id.0.as_str(), new);
    }

    fn on_child_removed(&mut self, _ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        if parent != self.id() {
            return;
        }

        self.unbind_child(child);
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}
