use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    sync::Arc,
    time::Duration,
};

use indexmap::{IndexMap, IndexSet};
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::{
    ANodeId, ColorValue, CompiledAlchemistGraph, CompiledExecNode, CompiledFormulaPropertySchema,
    CompiledNodeOperation, ExecNodeId, FormulaId, FormulaPropertyId, FormulaPropertySlotId, InputValueSource,
    RuntimeValue, SocketId, StableRef, TriggerValue, ValueSlotId, ValueTypeId, ValueTypeRegistry,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContextAxisId(SmolStr);

impl ContextAxisId {
    #[must_use]
    pub fn new(id: impl Into<SmolStr>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for ContextAxisId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ContextAxisId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SmolStr> for ContextAxisId {
    fn from(value: SmolStr) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContextItemId(SmolStr);

impl ContextItemId {
    #[must_use]
    pub fn new(id: impl Into<SmolStr>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for ContextItemId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ContextItemId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SmolStr> for ContextItemId {
    fn from(value: SmolStr) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContextKeyPart {
    pub axis: ContextAxisId,
    pub item: ContextItemId,
}

impl ContextKeyPart {
    #[must_use]
    pub fn new(axis: impl Into<ContextAxisId>, item: impl Into<ContextItemId>) -> Self {
        Self {
            axis: axis.into(),
            item: item.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContextKey {
    pub parts: SmallVec<[ContextKeyPart; 4]>,
}

impl ContextKey {
    #[must_use]
    pub fn default_lane() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn new(parts: impl IntoIterator<Item = ContextKeyPart>) -> Self {
        Self {
            parts: parts.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn single(axis: impl Into<ContextAxisId>, item: impl Into<ContextItemId>) -> Self {
        Self::new([ContextKeyPart::new(axis, item)])
    }

    #[must_use]
    pub fn is_default_lane(&self) -> bool {
        self.parts.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ContextKeyPart> {
        self.parts.iter()
    }

    #[must_use]
    pub fn project(&self, axes: &AxisSet) -> Self {
        if axes.is_empty() {
            return Self::default_lane();
        }
        Self::new(self.parts.iter().filter(|part| axes.contains(&part.axis)).cloned())
    }
}

pub type AxisSet = IndexSet<ContextAxisId>;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContextValuePath {
    pub segments: SmallVec<[SmolStr; 4]>,
}

impl ContextValuePath {
    #[must_use]
    pub fn new<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SmolStr>,
    {
        Self {
            segments: segments.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEvent {
    pub topic: Arc<str>,
    pub value: RuntimeValue,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeInputSnapshot {
    values: HashMap<StableRef, RuntimeValue>,
}

impl RuntimeInputSnapshot {
    pub fn insert(&mut self, reference: StableRef, value: RuntimeValue) -> Option<RuntimeValue> {
        self.values.insert(reference, value)
    }

    #[must_use]
    pub fn get(&self, reference: &StableRef) -> Option<&RuntimeValue> {
        self.values.get(reference)
    }
}

pub struct RuntimeRegistries<'a> {
    pub value_types: &'a ValueTypeRegistry,
}

pub struct EvaluationCtx<'a> {
    pub logical_tick: u64,
    pub delta_time: Duration,
    pub events: &'a [RuntimeEvent],
    pub inputs: &'a RuntimeInputSnapshot,
    pub registries: &'a RuntimeRegistries<'a>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeIntent {
    pub kind: Arc<str>,
    pub source_node: Option<ANodeId>,
    pub source_socket: Option<SocketId>,
    pub target: Option<StableRef>,
    pub payload: RuntimeValue,
    pub logical_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeDiagnostic {
    pub exec_node: ExecNodeId,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DebugValueSample {
    pub formula_id: Option<FormulaId>,
    pub context_key: Option<ContextKey>,
    pub author_node_id: ANodeId,
    pub exec_node: ExecNodeId,
    pub output_socket: SocketId,
    pub output_slot: ValueSlotId,
    pub value_type: ValueTypeId,
    pub value: RuntimeValue,
    pub logical_tick: u64,
    pub status: OutputPreviewStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputPreviewStatus {
    Live,
    DefaultPreview,
    Stale,
    Error,
    Suppressed,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugCaptureMode {
    Off,
    All {
        history_len: usize,
    },
    FormulaDefaults {
        formula_id: FormulaId,
        history_len: usize,
    },
    ProcessorLane {
        formula_id: FormulaId,
        context_key: Option<ContextKey>,
        history_len: usize,
    },
    SelectedNodes {
        formula_id: Option<FormulaId>,
        context_key: Option<ContextKey>,
        nodes: IndexSet<ANodeId>,
        history_len: usize,
    },
}

impl Default for DebugCaptureMode {
    fn default() -> Self {
        Self::Off
    }
}

impl DebugCaptureMode {
    #[must_use]
    pub fn history_len(&self) -> usize {
        match self {
            Self::Off => 0,
            Self::All { history_len }
            | Self::FormulaDefaults { history_len, .. }
            | Self::ProcessorLane { history_len, .. }
            | Self::SelectedNodes { history_len, .. } => *history_len,
        }
    }

    #[must_use]
    pub fn is_off(&self) -> bool {
        matches!(self, Self::Off) || self.history_len() == 0
    }

    fn sample_status(&self) -> Option<OutputPreviewStatus> {
        match self {
            Self::Off => None,
            Self::All { .. } | Self::ProcessorLane { .. } | Self::SelectedNodes { .. } => {
                Some(OutputPreviewStatus::Live)
            }
            Self::FormulaDefaults { .. } => Some(OutputPreviewStatus::DefaultPreview),
        }
    }

    fn formula_id(&self) -> Option<&FormulaId> {
        match self {
            Self::FormulaDefaults { formula_id, .. } | Self::ProcessorLane { formula_id, .. } => Some(formula_id),
            Self::SelectedNodes { formula_id, .. } => formula_id.as_ref(),
            Self::Off | Self::All { .. } => None,
        }
    }

    fn accepts(&self, sample: &DebugValueSample) -> bool {
        match self {
            Self::Off => false,
            Self::All { .. } | Self::FormulaDefaults { .. } => true,
            Self::ProcessorLane { context_key, .. } => context_key == &sample.context_key,
            Self::SelectedNodes { context_key, nodes, .. } => {
                context_key == &sample.context_key && nodes.contains(&sample.author_node_id)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutputPreviewHistory {
    pub samples: VecDeque<DebugValueSample>,
    pub max_len: usize,
}

impl OutputPreviewHistory {
    #[must_use]
    pub fn new(max_len: usize) -> Self {
        Self {
            samples: VecDeque::new(),
            max_len,
        }
    }

    pub fn push(&mut self, sample: DebugValueSample) {
        if self.max_len == 0 {
            return;
        }
        self.samples.push_back(sample);
        while self.samples.len() > self.max_len {
            self.samples.pop_front();
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeOutput {
    pub intents: Vec<RuntimeIntent>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub debug_samples: Vec<DebugValueSample>,
}

#[derive(Clone, Debug)]
pub struct AlchemistMemory {
    values: Vec<RuntimeValue>,
    value_initialized: Vec<bool>,
    value_revisions: Vec<u64>,
    states: Vec<RuntimeValue>,
    node_inputs: Vec<Option<Vec<RuntimeValue>>>,
    node_initialized: Vec<bool>,
    dirty_nodes: Vec<bool>,
    last_executed_nodes: Vec<ExecNodeId>,
}

impl AlchemistMemory {
    #[must_use]
    pub fn for_graph(compiled: &CompiledAlchemistGraph) -> Self {
        Self {
            values: vec![RuntimeValue::Unit; compiled.state_layout.value_slot_count],
            value_initialized: vec![false; compiled.state_layout.value_slot_count],
            value_revisions: vec![0; compiled.state_layout.value_slot_count],
            states: vec![RuntimeValue::Unit; compiled.state_layout.state_slot_count],
            node_inputs: vec![None; compiled.exec_nodes.len()],
            node_initialized: vec![false; compiled.exec_nodes.len()],
            dirty_nodes: vec![false; compiled.exec_nodes.len()],
            last_executed_nodes: Vec::new(),
        }
    }

    #[must_use]
    pub fn value(&self, slot: ValueSlotId) -> Option<&RuntimeValue> {
        self.values.get(slot.index())
    }

    #[must_use]
    pub fn value_len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn state_len(&self) -> usize {
        self.states.len()
    }

    #[must_use]
    fn is_compatible_with_graph(&self, compiled: &CompiledAlchemistGraph) -> bool {
        self.values.len() == compiled.state_layout.value_slot_count
            && self.value_initialized.len() == compiled.state_layout.value_slot_count
            && self.value_revisions.len() == compiled.state_layout.value_slot_count
            && self.states.len() == compiled.state_layout.state_slot_count
            && self.node_inputs.len() == compiled.exec_nodes.len()
            && self.node_initialized.len() == compiled.exec_nodes.len()
            && self.dirty_nodes.len() == compiled.exec_nodes.len()
    }

    #[must_use]
    pub fn is_stateless(&self) -> bool {
        self.states.is_empty()
    }

    #[must_use]
    pub fn last_executed_nodes(&self) -> &[ExecNodeId] {
        &self.last_executed_nodes
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePropertyFrame {
    values: Box<[RuntimeValue]>,
}

impl RuntimePropertyFrame {
    #[must_use]
    pub fn from_defaults(schema: &CompiledFormulaPropertySchema) -> Self {
        let mut values = vec![RuntimeValue::Unit; schema.len()];
        for property in schema.properties.values() {
            values[property.slot.index()] = property.default_value.clone();
        }
        Self {
            values: values.into_boxed_slice(),
        }
    }

    pub fn with_overrides(
        schema: &CompiledFormulaPropertySchema,
        overrides: &indexmap::IndexMap<FormulaPropertyId, RuntimeValue>,
    ) -> Result<Self, RuntimePropertyFrameError> {
        let mut frame = Self::from_defaults(schema);
        for (id, value) in overrides {
            let property = schema
                .get(id)
                .ok_or_else(|| RuntimePropertyFrameError::UnknownProperty(id.clone()))?;
            let actual = value.value_type();
            if actual != property.value_type {
                return Err(RuntimePropertyFrameError::InvalidOverrideType {
                    property: id.clone(),
                    expected: property.value_type.clone(),
                    actual,
                });
            }
            frame.values[property.slot.index()] = value.clone();
        }
        Ok(frame)
    }

    #[must_use]
    pub fn get(&self, slot: FormulaPropertySlotId) -> Option<&RuntimeValue> {
        self.values.get(slot.index())
    }

    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.values.len()
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuntimePropertyFrameError {
    #[error("processor override references unknown property `{0}`")]
    UnknownProperty(FormulaPropertyId),
    #[error("processor override for property `{property}` has type `{actual}`, expected `{expected}`")]
    InvalidOverrideType {
        property: FormulaPropertyId,
        expected: ValueTypeId,
        actual: ValueTypeId,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeContextFrame {
    context_key: ContextKey,
}

impl RuntimeContextFrame {
    #[must_use]
    pub fn default_lane() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn new(context_key: ContextKey) -> Self {
        Self { context_key }
    }

    #[must_use]
    pub fn context_key(&self) -> &ContextKey {
        &self.context_key
    }
}

#[derive(Clone, Debug, Default)]
pub enum LaneRuntimePool {
    #[default]
    Stateless,
    Stateful(IndexMap<ContextKey, AlchemistMemory>),
}

impl LaneRuntimePool {
    #[must_use]
    pub fn for_graph(compiled: &CompiledAlchemistGraph) -> Self {
        if compiled.state_layout.state_slot_count == 0 && !compiled.analysis.has_input_gated_nodes {
            Self::Stateless
        } else {
            Self::Stateful(IndexMap::new())
        }
    }

    #[must_use]
    pub fn is_stateless(&self) -> bool {
        matches!(self, Self::Stateless)
    }

    #[must_use]
    pub fn is_compatible_with_graph(&self, compiled: &CompiledAlchemistGraph) -> bool {
        let needs_stateful_pool = compiled.state_layout.state_slot_count > 0 || compiled.analysis.has_input_gated_nodes;
        match self {
            Self::Stateless => !needs_stateful_pool,
            Self::Stateful(lanes) => {
                needs_stateful_pool && lanes.values().all(|memory| memory.is_compatible_with_graph(compiled))
            }
        }
    }

    #[must_use]
    pub fn memory_count(&self) -> usize {
        match self {
            Self::Stateless => 0,
            Self::Stateful(lanes) => lanes.len(),
        }
    }

    pub fn clear(&mut self) {
        if let Self::Stateful(lanes) = self {
            lanes.clear();
        }
    }

    pub fn retain_keys(&mut self, keys: &IndexSet<ContextKey>) {
        if let Self::Stateful(lanes) = self {
            lanes.retain(|key, _| keys.contains(key));
        }
    }

    pub fn memory_for_key(
        &mut self,
        key: ContextKey,
        compiled: &CompiledAlchemistGraph,
    ) -> Option<&mut AlchemistMemory> {
        match self {
            Self::Stateless => None,
            Self::Stateful(lanes) => Some(lanes.entry(key).or_insert_with(|| AlchemistMemory::for_graph(compiled))),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DebugCaptureSink {
    mode: DebugCaptureMode,
    samples: Vec<DebugValueSample>,
}

impl Default for DebugCaptureSink {
    fn default() -> Self {
        Self::new(DebugCaptureMode::default())
    }
}

impl DebugCaptureSink {
    #[must_use]
    pub fn new(mode: DebugCaptureMode) -> Self {
        Self {
            mode,
            samples: Vec::new(),
        }
    }

    #[must_use]
    pub fn off() -> Self {
        Self::new(DebugCaptureMode::Off)
    }

    #[must_use]
    pub fn mode(&self) -> &DebugCaptureMode {
        &self.mode
    }

    pub fn capture(&mut self, mut sample: DebugValueSample) -> Option<DebugValueSample> {
        if self.mode.is_off() || !self.mode.accepts(&sample) {
            return None;
        }
        sample.status = self.mode.sample_status()?;
        sample.formula_id = self.mode.formula_id().cloned();
        self.samples.push(sample.clone());
        let history_len = self.mode.history_len();
        if history_len != usize::MAX && self.samples.len() > history_len {
            let overflow = self.samples.len() - history_len;
            self.samples.drain(0..overflow);
        }
        Some(sample)
    }

    #[must_use]
    pub fn samples(&self) -> &[DebugValueSample] {
        &self.samples
    }

    #[must_use]
    pub fn into_samples(self) -> Vec<DebugValueSample> {
        self.samples
    }
}

pub struct EvaluationFrame<'a, 'ctx> {
    pub ctx: &'a EvaluationCtx<'ctx>,
    pub properties: &'a RuntimePropertyFrame,
    pub context: &'a RuntimeContextFrame,
    pub debug: &'a mut DebugCaptureSink,
    pub force_process_unchanged_inputs: bool,
    pub capture_unchanged_outputs: bool,
}

pub struct NodeEvaluation<'a, 'ctx> {
    pub exec_node: ExecNodeId,
    pub author_node_id: ANodeId,
    pub ctx: &'a EvaluationCtx<'ctx>,
    pub inputs: &'a [RuntimeValue],
    pub properties: &'a RuntimePropertyFrame,
    pub context: &'a RuntimeContextFrame,
    pub debug: &'a mut DebugCaptureSink,
    pub state: &'a mut [RuntimeValue],
    pub intents: &'a mut Vec<RuntimeIntent>,
}

impl<'a, 'ctx> NodeEvaluation<'a, 'ctx> {
    pub fn capture_debug_value(&mut self, output_socket: impl Into<SocketId>, value: RuntimeValue) {
        let value_type = value.value_type();
        self.debug.capture(DebugValueSample {
            formula_id: None,
            context_key: (!self.context.context_key().is_default_lane()).then(|| self.context.context_key().clone()),
            author_node_id: self.author_node_id,
            exec_node: self.exec_node,
            output_socket: output_socket.into(),
            output_slot: ValueSlotId::new(u32::MAX),
            value_type,
            value,
            logical_tick: self.ctx.logical_tick,
            status: OutputPreviewStatus::Unavailable,
        });
    }
}

pub trait CompiledNodeEvaluator: Send + Sync + Debug {
    fn change_detection_inputs(&self, _ctx: &EvaluationCtx<'_>) -> Result<Vec<RuntimeValue>, String> {
        Ok(Vec::new())
    }

    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String>;
}

pub struct AlchemistRuntime {
    pub compiled: Arc<CompiledAlchemistGraph>,
    pub memory: AlchemistMemory,
    pub properties: RuntimePropertyFrame,
    execution_counts: Vec<u64>,
    evaluating: bool,
}

impl AlchemistRuntime {
    #[must_use]
    pub fn new(compiled: Arc<CompiledAlchemistGraph>) -> Self {
        let memory = AlchemistMemory::for_graph(&compiled);
        let properties = RuntimePropertyFrame::from_defaults(&compiled.properties);
        let execution_counts = vec![0; compiled.exec_nodes.len()];
        Self {
            compiled,
            memory,
            properties,
            execution_counts,
            evaluating: false,
        }
    }

    #[must_use]
    pub fn with_property_frame(compiled: Arc<CompiledAlchemistGraph>, properties: RuntimePropertyFrame) -> Self {
        let memory = AlchemistMemory::for_graph(&compiled);
        let execution_counts = vec![0; compiled.exec_nodes.len()];
        Self {
            compiled,
            memory,
            properties,
            execution_counts,
            evaluating: false,
        }
    }

    pub fn set_property_frame(&mut self, properties: RuntimePropertyFrame) {
        self.properties = properties;
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        self.evaluate_with_capture_mode(ctx, DebugCaptureMode::default())
    }

    pub fn evaluate_with_capture_mode(
        &mut self,
        ctx: &EvaluationCtx<'_>,
        capture_mode: DebugCaptureMode,
    ) -> RuntimeOutput {
        if self.evaluating {
            return RuntimeOutput {
                diagnostics: vec![RuntimeDiagnostic {
                    exec_node: ExecNodeId::new(0),
                    message: "Alchemist runtime evaluation is non-reentrant".into(),
                }],
                ..RuntimeOutput::default()
            };
        }
        self.evaluating = true;
        let mut debug = DebugCaptureSink::new(capture_mode);
        let context = RuntimeContextFrame::default_lane();
        let output = evaluate_compiled_graph(
            &self.compiled,
            &mut self.memory,
            EvaluationFrame {
                ctx,
                properties: &self.properties,
                context: &context,
                debug: &mut debug,
                force_process_unchanged_inputs: false,
                capture_unchanged_outputs: false,
            },
        );
        for exec_id in self.memory.last_executed_nodes() {
            self.execution_counts[exec_id.index()] += 1;
        }
        self.evaluating = false;
        output
    }

    #[must_use]
    pub fn execution_count(&self, exec_node: ExecNodeId) -> u64 {
        self.execution_counts[exec_node.index()]
    }
}

pub fn evaluate_compiled_graph(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    frame: EvaluationFrame<'_, '_>,
) -> RuntimeOutput {
    let mut output = RuntimeOutput::default();
    seed_dirty_nodes(compiled, memory, &frame, &mut output);
    for exec_id in &compiled.topo_order {
        if !memory.dirty_nodes[exec_id.index()] {
            continue;
        }
        memory.dirty_nodes[exec_id.index()] = false;
        let node = &compiled.exec_nodes[exec_id.index()];
        let inputs = runtime_node_inputs(node, memory, frame.ctx);
        let inputs = match inputs {
            Ok(inputs) => inputs,
            Err(message) => {
                output.diagnostics.push(RuntimeDiagnostic {
                    exec_node: *exec_id,
                    message,
                });
                continue;
            }
        };
        let change_inputs = match change_detection_inputs(&node.operation, &inputs, frame.properties, frame.ctx) {
            Ok(change_inputs) => change_inputs,
            Err(message) => {
                output.diagnostics.push(RuntimeDiagnostic {
                    exec_node: *exec_id,
                    message,
                });
                continue;
            }
        };
        if node.process_on_input_change_only && !frame.force_process_unchanged_inputs {
            let previous_inputs = memory.node_inputs.get(exec_id.index()).and_then(Option::as_ref);
            if memory.node_initialized[exec_id.index()]
                && previous_inputs.is_some_and(|previous| runtime_values_equivalent(previous, &change_inputs))
            {
                continue;
            }
        }
        if let Some(previous_inputs) = memory.node_inputs.get_mut(exec_id.index()) {
            *previous_inputs = Some(change_inputs);
        }
        memory.node_initialized[exec_id.index()] = true;
        memory.last_executed_nodes.push(*exec_id);
        let state = &mut memory.states[node.state_range.clone()];
        let result = evaluate_operation(
            &node.operation,
            NodeEvaluation {
                exec_node: *exec_id,
                author_node_id: node.authored_id,
                ctx: frame.ctx,
                inputs: &inputs,
                properties: frame.properties,
                context: frame.context,
                debug: &mut *frame.debug,
                state,
                intents: &mut output.intents,
            },
        );
        match result {
            Ok(values) if values.len() == node.outputs.len() => {
                let output_values = values.clone();
                for (output_index, (slot, value)) in node.outputs.iter().zip(values).enumerate() {
                    let previous_value = memory.values.get(slot.index());
                    let output_changed = !memory.value_initialized[slot.index()]
                        || previous_value.is_none_or(|previous| !runtime_value_equivalent(previous, &value));
                    memory.values[slot.index()] = value.clone();
                    memory.value_initialized[slot.index()] = true;
                    if output_changed {
                        memory.value_revisions[slot.index()] = memory.value_revisions[slot.index()].saturating_add(1);
                        mark_slot_dependents_dirty(compiled, memory, *slot);
                    }
                    if node.send_on_output_change_only && !output_changed && !frame.capture_unchanged_outputs {
                        continue;
                    }
                    let output_socket = node
                        .output_sockets
                        .get(output_index)
                        .cloned()
                        .unwrap_or_else(|| SocketId::new(format!("slot_{}", slot.index())));
                    let value_type = node
                        .output_types
                        .get(output_index)
                        .and_then(Clone::clone)
                        .unwrap_or_else(|| value.value_type());
                    let sample = DebugValueSample {
                        formula_id: None,
                        context_key: (!frame.context.context_key().is_default_lane())
                            .then(|| frame.context.context_key().clone()),
                        author_node_id: node.authored_id,
                        exec_node: *exec_id,
                        output_socket,
                        output_slot: *slot,
                        value_type,
                        value,
                        logical_tick: frame.ctx.logical_tick,
                        status: OutputPreviewStatus::Unavailable,
                    };
                    frame.debug.capture(sample);
                }
                if node.log_enabled {
                    for (output_index, value) in output_values.into_iter().enumerate() {
                        output.intents.push(RuntimeIntent {
                            kind: Arc::from("debug.log"),
                            source_node: Some(node.authored_id),
                            source_socket: node.output_sockets.get(output_index).cloned(),
                            target: None,
                            payload: value,
                            logical_tick: frame.ctx.logical_tick,
                        });
                    }
                }
            }
            Ok(values) => output.diagnostics.push(RuntimeDiagnostic {
                exec_node: *exec_id,
                message: format!(
                    "node produced {} output(s), expected {}",
                    values.len(),
                    node.outputs.len()
                ),
            }),
            Err(message) => output.diagnostics.push(RuntimeDiagnostic {
                exec_node: *exec_id,
                message,
            }),
        }
    }
    output.debug_samples = frame.debug.samples().to_vec();
    output
}

fn seed_dirty_nodes(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    frame: &EvaluationFrame<'_, '_>,
    output: &mut RuntimeOutput,
) {
    memory.dirty_nodes.fill(false);
    memory.last_executed_nodes.clear();

    if frame.force_process_unchanged_inputs {
        for exec_id in &compiled.topo_order {
            memory.dirty_nodes[exec_id.index()] = true;
        }
        return;
    }

    for exec_id in &compiled.topo_order {
        if !memory.node_initialized[exec_id.index()] {
            memory.dirty_nodes[exec_id.index()] = true;
        }
    }

    for exec_id in &compiled.dependencies.always_process_nodes {
        memory.dirty_nodes[exec_id.index()] = true;
    }

    for exec_id in &compiled.dependencies.external_input_nodes {
        match node_change_inputs_changed(compiled, memory, *exec_id, frame) {
            Ok(true) => {
                memory.dirty_nodes[exec_id.index()] = true;
            }
            Ok(false) => {}
            Err(message) => {
                output.diagnostics.push(RuntimeDiagnostic {
                    exec_node: *exec_id,
                    message,
                });
            }
        }
    }
}

fn node_change_inputs_changed(
    compiled: &CompiledAlchemistGraph,
    memory: &AlchemistMemory,
    exec_id: ExecNodeId,
    frame: &EvaluationFrame<'_, '_>,
) -> Result<bool, String> {
    if !memory.node_initialized[exec_id.index()] {
        return Ok(true);
    }
    let node = &compiled.exec_nodes[exec_id.index()];
    let inputs = runtime_node_inputs(node, memory, frame.ctx)?;
    let change_inputs = change_detection_inputs(&node.operation, &inputs, frame.properties, frame.ctx)?;
    let previous_inputs = memory.node_inputs.get(exec_id.index()).and_then(Option::as_ref);
    Ok(!previous_inputs.is_some_and(|previous| runtime_values_equivalent(previous, &change_inputs)))
}

fn mark_slot_dependents_dirty(compiled: &CompiledAlchemistGraph, memory: &mut AlchemistMemory, slot: ValueSlotId) {
    if let Some(dependents) = compiled.dependencies.slot_dependents.get(slot.index()) {
        for dependent in dependents {
            memory.dirty_nodes[dependent.index()] = true;
        }
    }
}

fn runtime_node_inputs(
    node: &CompiledExecNode,
    memory: &AlchemistMemory,
    ctx: &EvaluationCtx<'_>,
) -> Result<SmallVec<[RuntimeValue; 4]>, String> {
    let mut inputs = SmallVec::<[RuntimeValue; 4]>::new();
    for source in &node.inputs {
        inputs.push(runtime_input_value(
            source,
            memory,
            ctx.inputs,
            ctx.registries.value_types,
        )?);
    }
    Ok(inputs)
}

fn change_detection_inputs(
    operation: &CompiledNodeOperation,
    inputs: &[RuntimeValue],
    properties: &RuntimePropertyFrame,
    ctx: &EvaluationCtx<'_>,
) -> Result<Vec<RuntimeValue>, String> {
    let mut change_inputs = inputs.to_vec();
    if let CompiledNodeOperation::ReadProperty(slot) = operation {
        change_inputs.push(
            properties
                .get(*slot)
                .cloned()
                .ok_or_else(|| format!("property slot {} is unavailable", slot.index()))?,
        );
    }
    if let CompiledNodeOperation::Custom(evaluator) = operation {
        change_inputs.extend(evaluator.change_detection_inputs(ctx)?);
    }
    Ok(change_inputs)
}

fn runtime_values_equivalent(left: &[RuntimeValue], right: &[RuntimeValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| runtime_value_equivalent(left, right))
}

fn runtime_value_equivalent(left: &RuntimeValue, right: &RuntimeValue) -> bool {
    match (left, right) {
        (RuntimeValue::Trigger(left), RuntimeValue::Trigger(right)) if !left.fired && !right.fired => true,
        _ => left == right,
    }
}

pub fn evaluate_compiled_graph_stateless(
    compiled: &CompiledAlchemistGraph,
    frame: EvaluationFrame<'_, '_>,
) -> RuntimeOutput {
    let mut memory = AlchemistMemory::for_graph(compiled);
    evaluate_compiled_graph(compiled, &mut memory, frame)
}

fn runtime_input_value(
    source: &InputValueSource,
    memory: &AlchemistMemory,
    inputs: &RuntimeInputSnapshot,
    value_types: &ValueTypeRegistry,
) -> Result<RuntimeValue, String> {
    match source {
        InputValueSource::Slot(slot) => Ok(memory.values[slot.index()].clone()),
        InputValueSource::Converted { source, target_type } => {
            let value = runtime_input_value(source, memory, inputs, value_types)?;
            value_types.convert_automatically(&value, target_type)
        }
        InputValueSource::Component { source, component } => {
            let value = runtime_input_value(source, memory, inputs, value_types)?;
            value
                .component(*component)
                .ok_or_else(|| format!("value type `{}` has no component `{component:?}`", value.value_type()))
        }
        InputValueSource::Composite {
            target_type,
            base,
            components,
        } => {
            let base = runtime_input_value(base, memory, inputs, value_types)?;
            let mut value = value_types.convert_automatically(&base, target_type)?;
            for (component, source) in components {
                let component_value = runtime_input_value(source, memory, inputs, value_types)?;
                value = value.with_component(*component, &component_value)?;
            }
            Ok(value)
        }
        InputValueSource::RuntimeInput { reference, fallback } => inputs
            .get(reference)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| runtime_input_value(fallback, memory, inputs, value_types)),
        InputValueSource::Constant(value) => Ok(value.clone()),
        InputValueSource::Unset => Ok(RuntimeValue::Unit),
    }
}

fn evaluate_operation(
    operation: &CompiledNodeOperation,
    mut evaluation: NodeEvaluation<'_, '_>,
) -> Result<Vec<RuntimeValue>, String> {
    match operation {
        CompiledNodeOperation::Disabled { outputs } => Ok(outputs
            .iter()
            .map(|output| {
                output
                    .input_index
                    .and_then(|input_index| evaluation.inputs.get(input_index))
                    .cloned()
                    .unwrap_or_else(|| output.default_value.clone())
            })
            .collect()),
        CompiledNodeOperation::Constant(value) => Ok(vec![value.clone()]),
        CompiledNodeOperation::ReadProperty(slot) => evaluation
            .properties
            .get(*slot)
            .cloned()
            .map(|value| vec![value])
            .ok_or_else(|| format!("property slot {} is unavailable", slot.index())),
        CompiledNodeOperation::Add => {
            let [left, right] = require_inputs::<2>(evaluation.inputs)?;
            Ok(vec![add_values(left, right)?])
        }
        CompiledNodeOperation::Compare => {
            let [left, right] = require_inputs::<2>(evaluation.inputs)?;
            Ok(vec![RuntimeValue::Bool(left == right)])
        }
        CompiledNodeOperation::BoolAnd => {
            let [left, right] = bool_inputs::<2>(evaluation.inputs)?;
            Ok(vec![RuntimeValue::Bool(left && right)])
        }
        CompiledNodeOperation::BoolOr => {
            let [left, right] = bool_inputs::<2>(evaluation.inputs)?;
            Ok(vec![RuntimeValue::Bool(left || right)])
        }
        CompiledNodeOperation::BoolNot => {
            let [value] = bool_inputs::<1>(evaluation.inputs)?;
            Ok(vec![RuntimeValue::Bool(!value)])
        }
        CompiledNodeOperation::Edge => {
            let [value] = bool_inputs::<1>(evaluation.inputs)?;
            let previous = matches!(evaluation.state.first(), Some(RuntimeValue::Bool(true)));
            if let Some(state) = evaluation.state.first_mut() {
                *state = RuntimeValue::Bool(value);
            }
            Ok(vec![RuntimeValue::Trigger(TriggerValue {
                fired: value && !previous,
                edge_id: u64::from(evaluation.exec_node.index() as u32),
                logical_tick: evaluation.ctx.logical_tick,
            })])
        }
        CompiledNodeOperation::Gate => {
            let [trigger, open] = require_inputs::<2>(evaluation.inputs)?;
            let RuntimeValue::Trigger(trigger) = trigger else {
                return Err("Gate expects a trigger input".into());
            };
            let RuntimeValue::Bool(open) = open else {
                return Err("Gate expects a boolean open input".into());
            };
            Ok(vec![RuntimeValue::Trigger(TriggerValue {
                fired: trigger.fired && *open,
                ..*trigger
            })])
        }
        CompiledNodeOperation::MapRange => Ok(vec![map_range_values(evaluation.inputs)?]),
        CompiledNodeOperation::Clamp => Ok(vec![clamp_values(evaluation.inputs)?]),
        CompiledNodeOperation::DelayOneTick => {
            let [value] = require_inputs::<1>(evaluation.inputs)?;
            if let Some(state) = evaluation.state.first_mut() {
                *state = value.clone();
            }
            Ok(vec![value.clone()])
        }
        CompiledNodeOperation::DebugLog => {
            let [value] = require_inputs::<1>(evaluation.inputs)?;
            evaluation.intents.push(RuntimeIntent {
                kind: Arc::from("debug.log"),
                source_node: Some(evaluation.author_node_id),
                source_socket: None,
                target: None,
                payload: value.clone(),
                logical_tick: evaluation.ctx.logical_tick,
            });
            Ok(Vec::new())
        }
        CompiledNodeOperation::Custom(evaluator) => evaluator.evaluate(&mut evaluation),
    }
}

fn require_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[&RuntimeValue; N], String> {
    inputs
        .iter()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| format!("node expects {N} input(s)"))
}

fn bool_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[bool; N], String> {
    require_inputs::<N>(inputs)?
        .map(|value| match value {
            RuntimeValue::Bool(value) => Ok(*value),
            _ => Err("node expects boolean inputs".into()),
        })
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "invalid boolean input count".into())
}

fn add_values(left: &RuntimeValue, right: &RuntimeValue) -> Result<RuntimeValue, String> {
    match (left, right) {
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => Ok(RuntimeValue::Int(left + right)),
        (RuntimeValue::Float(left), RuntimeValue::Float(right)) => Ok(RuntimeValue::Float(left + right)),
        (RuntimeValue::Int(left), RuntimeValue::Float(right)) => Ok(RuntimeValue::Float(*left as f64 + right)),
        (RuntimeValue::Float(left), RuntimeValue::Int(right)) => Ok(RuntimeValue::Float(left + *right as f64)),
        (RuntimeValue::Vec2(left), RuntimeValue::Vec2(right)) => {
            Ok(RuntimeValue::Vec2([left[0] + right[0], left[1] + right[1]]))
        }
        (RuntimeValue::Vec3(left), RuntimeValue::Vec3(right)) => Ok(RuntimeValue::Vec3([
            left[0] + right[0],
            left[1] + right[1],
            left[2] + right[2],
        ])),
        (RuntimeValue::Color(left), RuntimeValue::Color(right)) => Ok(RuntimeValue::Color(ColorValue {
            red: left.red + right.red,
            green: left.green + right.green,
            blue: left.blue + right.blue,
            alpha: left.alpha + right.alpha,
        })),
        _ => Err("Add received incompatible runtime values".into()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericShape {
    Int,
    Scalar,
    Vec2,
    Vec3,
    Color,
}

fn numeric_components(value: &RuntimeValue) -> Result<(NumericShape, Vec<f64>), String> {
    match value {
        RuntimeValue::Int(value) => Ok((NumericShape::Int, vec![*value as f64])),
        RuntimeValue::Float(value) => Ok((NumericShape::Scalar, vec![*value])),
        RuntimeValue::Vec2(value) => Ok((NumericShape::Vec2, value.to_vec())),
        RuntimeValue::Vec3(value) => Ok((NumericShape::Vec3, value.to_vec())),
        RuntimeValue::Color(value) => Ok((
            NumericShape::Color,
            vec![
                f64::from(value.red),
                f64::from(value.green),
                f64::from(value.blue),
                f64::from(value.alpha),
            ],
        )),
        _ => Err("node expects numeric inputs".into()),
    }
}

fn numeric_from_components(shape: NumericShape, components: &[f64]) -> RuntimeValue {
    match shape {
        NumericShape::Int => RuntimeValue::Int(components[0] as i64),
        NumericShape::Scalar => RuntimeValue::Float(components[0]),
        NumericShape::Vec2 => RuntimeValue::Vec2([components[0], components[1]]),
        NumericShape::Vec3 => RuntimeValue::Vec3([components[0], components[1], components[2]]),
        NumericShape::Color => RuntimeValue::Color(ColorValue {
            red: components[0] as f32,
            green: components[1] as f32,
            blue: components[2] as f32,
            alpha: components[3] as f32,
        }),
    }
}

fn aligned_numeric_components(values: &[RuntimeValue]) -> Result<(NumericShape, Vec<Vec<f64>>), String> {
    let mut components = values.iter().map(numeric_components).collect::<Result<Vec<_>, _>>()?;
    let Some((shape, first)) = components.first() else {
        return Err("node expects numeric inputs".into());
    };
    let shape = *shape;
    let count = first.len();
    if components
        .iter()
        .any(|(candidate_shape, values)| *candidate_shape != shape || values.len() != count)
    {
        return Err("node received incompatible numeric shapes".into());
    }
    Ok((shape, components.drain(..).map(|(_, components)| components).collect()))
}

fn map_range_values(inputs: &[RuntimeValue]) -> Result<RuntimeValue, String> {
    let values = require_inputs::<5>(inputs)?.map(Clone::clone);
    let (shape, values) = aligned_numeric_components(&values)?;
    let [value, in_min, in_max, out_min, out_max] = values
        .try_into()
        .map_err(|_| "invalid Map Range input count".to_string())?;
    let mut result = Vec::with_capacity(value.len());
    for index in 0..value.len() {
        if (in_max[index] - in_min[index]).abs() <= f64::EPSILON {
            return Err("Map Range input range cannot be zero".into());
        }
        let normalized = (value[index] - in_min[index]) / (in_max[index] - in_min[index]);
        result.push(out_min[index] + normalized * (out_max[index] - out_min[index]));
    }
    Ok(numeric_from_components(shape, &result))
}

fn clamp_values(inputs: &[RuntimeValue]) -> Result<RuntimeValue, String> {
    let values = require_inputs::<3>(inputs)?.map(Clone::clone);
    let (shape, values) = aligned_numeric_components(&values)?;
    let [value, minimum, maximum] = values.try_into().map_err(|_| "invalid Clamp input count".to_string())?;
    let result = value
        .iter()
        .zip(minimum.iter().zip(maximum.iter()))
        .map(|(value, (minimum, maximum))| value.clamp(*minimum, *maximum))
        .collect::<Vec<_>>();
    Ok(numeric_from_components(shape, &result))
}
