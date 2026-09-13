//! A typed managed stage compiles one ANode graph and applies it to authored channel groups.

use std::{collections::HashSet, sync::Arc};

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistGraphDomain, AlchemistGraphTransaction, AlchemistMemory, ChannelDescriptor,
    ChannelLayout, ChannelLayoutError, CompileCtx, CompiledAlchemistGraph, ContextAxisId, ContextItemId, ContextKey,
    DebugCaptureMode, DebugCaptureSink, DebugValueSample, Diagnostic, EvaluationCtx, EvaluationFrame, ExecNodeId,
    FormulaPropertyDecl, FormulaPropertyId, FormulaPropertySchema, InputSocketRef, LaneRuntimePool,
    MANAGED_GROUPS_FIELD, MANAGED_IMPLICIT_GATE_DEFAULT_FIELD, ManagedApplication, ManagedApplicationError,
    ManagedFilterValueMode, ManagedItemInstance, NodeFlow, OutputPreviewStatus, OutputSocketRef, ParamUiHints,
    PipelineCardinality, PrimitiveNodeKind, RuntimeContextFrame, RuntimeOutput, RuntimePropertyFrame,
    RuntimePropertyFrameError, SignatureCtx, SocketId, StableRef, TypeConstraint, ValueComponent, ValueLaneKey,
    ValueSlotId, ValueTypeId, compile_graph, evaluate_compiled_graph, evaluate_compiled_graph_fresh_reusing,
};
use golden_values::Value as RuntimeValue;
use indexmap::{IndexMap, IndexSet};

use crate::{ChannelFrame, ChannelFrameError, ChannelSlot, ChannelValidity, RuntimeInputBinding};

const STAGE_AXIS: &str = "managed_stage_lane";
const MAX_MAPPING_PREVIEW_ELEMENTS: usize = 64;
const MAX_MAPPING_PREVIEW_BYTES: usize = 16 * 1024;

fn preview_values_fit(slots: &[ChannelSlot]) -> bool {
    if slots.len() > MAX_MAPPING_PREVIEW_ELEMENTS {
        return false;
    }
    let mut stack = slots.iter().filter_map(|slot| slot.value.as_ref()).collect::<Vec<_>>();
    let mut bytes = 0usize;
    while let Some(value) = stack.pop() {
        bytes = bytes.saturating_add(std::mem::size_of::<RuntimeValue>());
        match value {
            RuntimeValue::String(value) => bytes = bytes.saturating_add(value.len()),
            RuntimeValue::Ref(value) => {
                bytes = bytes.saturating_add(value.stable_id.len() + value.value_type.as_str().len());
            }
            RuntimeValue::Extension(value) => bytes = bytes.saturating_add(value.payload.len()),
            RuntimeValue::Array(values) if values.len() > MAX_MAPPING_PREVIEW_ELEMENTS => return false,
            RuntimeValue::Array(values) => stack.extend(values),
            _ => {}
        }
        if bytes > MAX_MAPPING_PREVIEW_BYTES {
            return false;
        }
    }
    true
}

mod cache;
mod chain;

pub use cache::ManagedStageSpecializationCache;
use cache::{StageSpecialization, StageSpecializationKey};
pub use chain::ManagedStageChain;

pub struct ManagedStageRuntime {
    item: ManagedItemInstance,
    input_layout: ChannelLayout,
    compiled: Arc<CompiledAlchemistGraph>,
    compiled_stage_node: chataigne_alchemist::ANodeId,
    compiled_stage_exec: ExecNodeId,
    input_properties: Vec<FormulaPropertyId>,
    auxiliary: Vec<StageAuxiliaryBinding>,
    output_slots: Vec<ValueSlotId>,
    groups: Vec<StageGroup>,
    group_results: Vec<Vec<ChannelSlot>>,
    outputs: Vec<StageOutputBinding>,
    output_frame: ChannelFrame,
    memory: LaneRuntimePool,
    scratch: AlchemistMemory,
    active_temporal_contexts: HashSet<ContextKey>,
    temporal_evaluated: bool,
}

struct StageAuxiliaryBinding {
    socket: SocketId,
    property: FormulaPropertyId,
    value_type: ValueTypeId,
    binding: RuntimeInputBinding,
}

struct StageGroup {
    inputs: Vec<usize>,
    context: ContextKey,
}

enum StageOutputBinding {
    Passthrough(usize),
    Result { group: usize, output: usize },
}

impl ManagedStageRuntime {
    pub fn compile(
        item: ManagedItemInstance,
        input_layout: &ChannelLayout,
        ctx: &CompileCtx<'_>,
        mode: ManagedFilterValueMode,
    ) -> Result<Option<Self>, ManagedStageError> {
        Self::compile_with_cache(item, input_layout, ctx, mode, None)
    }

    pub fn compile_with_cache(
        item: ManagedItemInstance,
        input_layout: &ChannelLayout,
        ctx: &CompileCtx<'_>,
        mode: ManagedFilterValueMode,
        mut cache: Option<&mut ManagedStageSpecializationCache>,
    ) -> Result<Option<Self>, ManagedStageError> {
        let signature_ctx = SignatureCtx {
            value_types: ctx.value_types,
            properties: ctx.properties,
        };
        let application = match mode {
            ManagedFilterValueMode::Routed => {
                ctx.nodes
                    .resolve_managed_application(&item.anode, input_layout, &signature_ctx)?
            }
            ManagedFilterValueMode::Tuple => {
                ctx.nodes
                    .resolve_mapping_application(&item.anode, input_layout, &signature_ctx)?
            }
        };
        if application.selection.no_compatible_channels {
            return match mode {
                ManagedFilterValueMode::Routed => Ok(None),
                ManagedFilterValueMode::Tuple => Err(ManagedStageError::EmptyTuple),
            };
        }
        if !matches!(
            application.cardinality,
            PipelineCardinality::Elementwise
                | PipelineCardinality::Aggregate
                | PipelineCardinality::Reshape
                | PipelineCardinality::WholeSet
        ) {
            return Err(ManagedStageError::UnsupportedCardinality(application.cardinality));
        }
        if item.anode.config.get(MANAGED_GROUPS_FIELD).is_some() && application.groups.is_empty() {
            return Err(ManagedStageError::EmptyGroups);
        }
        let first_group = application.groups.first().ok_or(ManagedStageError::EmptyGroups)?;
        if first_group.len() != application.primary_inputs.len() {
            return Err(ManagedStageError::GroupArity {
                expected: application.primary_inputs.len(),
                actual: first_group.len(),
            });
        }
        let primary_types = group_types(first_group, input_layout)?;
        for group in &application.groups {
            let candidate = group_types(group, input_layout)?;
            if candidate != primary_types {
                return Err(ManagedStageError::MixedGroupSpecializations);
            }
        }

        let declaration = ctx
            .nodes
            .get(&item.anode.type_id)
            .ok_or_else(|| ManagedStageError::MissingDeclaration(item.anode.type_id.clone()))?;
        let signature = declaration.signature(&signature_ctx, &item.anode, &item.anode.type_bindings);
        let mut properties = FormulaPropertySchema::default();
        let mut graph = AlchemistGraphDomain::new_document();
        let mut transaction = AlchemistGraphTransaction::for_document(&graph);
        let node_id = item.anode.id;
        let mut stage_node = item.anode.clone();
        if stage_node.type_id.as_str() == PrimitiveNodeKind::ConditionGate.type_name()
            && !stage_node.input_defaults.contains_key(&SocketId::new("default_value"))
        {
            stage_node
                .config
                .set(MANAGED_IMPLICIT_GATE_DEFAULT_FIELD, RuntimeValue::Bool(true));
        }
        AlchemistGraphDomain::insert_node(&mut transaction, stage_node);
        let mut input_properties = Vec::with_capacity(application.primary_inputs.len());
        let mut auxiliary = Vec::with_capacity(application.auxiliary_inputs.len());
        let mut connections = Vec::with_capacity(signature.inputs.len());
        for input in &signature.inputs {
            let primary_index = application.primary_inputs.iter().position(|socket| *socket == input.id);
            let (value_type, binding) = if let Some(index) = primary_index {
                (primary_types[index].clone(), None)
            } else {
                let authored = item
                    .anode
                    .input_defaults
                    .get(&input.id)
                    .cloned()
                    .or_else(|| input.default_value.clone());
                let value_type = auxiliary_type(&input.constraint, authored.as_ref(), &primary_types)?;
                let binding = match authored {
                    Some(RuntimeValue::Ref(reference)) => RuntimeInputBinding::Reference(reference),
                    Some(value) => RuntimeInputBinding::Constant(value),
                    None => RuntimeInputBinding::Constant(
                        ctx.value_types
                            .default_value(&value_type)
                            .ok_or_else(|| ManagedStageError::MissingDefault(value_type.clone()))?,
                    ),
                };
                (value_type, Some(binding))
            };
            let default_value = ctx
                .value_types
                .default_value(&value_type)
                .ok_or_else(|| ManagedStageError::MissingDefault(value_type.clone()))?;
            let property = FormulaPropertyId::new(format!("managed:{}", input.id.as_str()));
            properties.insert(FormulaPropertyDecl {
                id: property.clone(),
                label: input.label.clone(),
                description: None,
                value_type: value_type.clone(),
                default_value,
                ui: ParamUiHints::default(),
            });
            let mut source = ANodeInstance::new(ANodeTypeId::new("property"), format!("{} input", input.label));
            source.config.set(
                "property_id",
                RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), property.as_str())),
            );
            let source_id = source.id;
            AlchemistGraphDomain::insert_node(&mut transaction, source);
            connections.push((source_id, input.id.clone()));
            if let Some(binding) = binding {
                auxiliary.push(StageAuxiliaryBinding {
                    socket: input.id.clone(),
                    property,
                    value_type,
                    binding,
                });
            } else {
                input_properties.push(property);
            }
        }
        for (source, socket) in connections {
            AlchemistGraphDomain::connect(
                &mut transaction,
                &graph,
                OutputSocketRef::new(source, "value"),
                InputSocketRef::new(node_id, socket),
            );
        }
        let domain = AlchemistGraphDomain::new(ctx.nodes.clone(), ctx.value_types.clone(), Some(properties.clone()));
        transaction
            .commit(&mut graph, &domain)
            .map_err(ManagedStageError::AuthoringEdit)?;
        let key = StageSpecializationKey::new(&item, primary_types, ctx.properties);
        let specialization = if let Some(cached) = cache.as_ref().and_then(|cache| cache.get(&key)) {
            cached
        } else {
            let result = compile_graph(
                &graph,
                &CompileCtx {
                    value_types: ctx.value_types,
                    nodes: ctx.nodes,
                    properties: Some(&properties),
                },
            );
            if result.has_errors() {
                return Err(ManagedStageError::Compile(result.diagnostics));
            }
            let compiled = result.compiled.ok_or(ManagedStageError::MissingCompiledGraph)?;
            let specialization = StageSpecialization {
                compiled,
                authored_node: node_id,
            };
            if let Some(cache) = cache.as_mut() {
                cache.insert(key, specialization.clone());
            }
            specialization
        };
        let compiled = specialization.compiled;
        let exec = compiled
            .exec_nodes
            .iter()
            .find(|node| node.authored_id == specialization.authored_node)
            .ok_or(ManagedStageError::MissingCompiledNode)?;
        let compiled_stage_exec = exec.exec_id;
        let mut output_slots = Vec::with_capacity(application.outputs.len());
        let mut output_types = Vec::with_capacity(application.outputs.len());
        for socket in &application.outputs {
            let index = exec
                .output_sockets
                .iter()
                .position(|candidate| candidate == socket)
                .ok_or_else(|| ManagedStageError::MissingOutputSocket(socket.clone()))?;
            output_slots.push(exec.outputs[index]);
            output_types.push(
                exec.output_types[index]
                    .clone()
                    .ok_or_else(|| ManagedStageError::UnresolvedOutputType(socket.clone()))?,
            );
        }
        let (output_layout, outputs) = output_layout(&item, &application, input_layout, &output_types)?;
        if mode == ManagedFilterValueMode::Tuple
            && outputs
                .iter()
                .any(|binding| matches!(binding, StageOutputBinding::Passthrough(_)))
        {
            return Err(ManagedStageError::PartialTupleApplication);
        }
        let groups = application
            .groups
            .iter()
            .map(|inputs| StageGroup {
                context: group_context(inputs, input_layout),
                inputs: inputs.clone(),
            })
            .collect();
        let group_results = vec![vec![ChannelSlot::default(); output_slots.len()]; application.groups.len()];
        let memory = LaneRuntimePool::for_graph(&compiled);
        let scratch = AlchemistMemory::for_graph(&compiled);
        Ok(Some(Self {
            item,
            input_layout: input_layout.clone(),
            compiled,
            compiled_stage_node: specialization.authored_node,
            compiled_stage_exec,
            input_properties,
            auxiliary,
            output_slots,
            groups,
            group_results,
            outputs,
            output_frame: ChannelFrame::new(Arc::new(output_layout)),
            memory,
            scratch,
            active_temporal_contexts: HashSet::new(),
            temporal_evaluated: false,
        }))
    }

    #[must_use]
    pub fn output_layout(&self) -> &Arc<ChannelLayout> {
        self.output_frame.layout()
    }

    #[must_use]
    pub fn shares_compiled_plan_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.compiled, &other.compiled)
    }

    #[must_use]
    pub fn needs_continuous_evaluation(&self) -> bool {
        self.compiled.analysis.has_always_process_nodes
            && (!self.temporal_evaluated || !self.active_temporal_contexts.is_empty())
    }

    pub fn update_runtime_input(
        &mut self,
        socket: &SocketId,
        binding: RuntimeInputBinding,
    ) -> Result<(), ManagedStageError> {
        let slot = self
            .auxiliary
            .iter_mut()
            .find(|slot| slot.socket == *socket)
            .ok_or_else(|| ManagedStageError::MissingAuxiliarySocket(socket.clone()))?;
        let actual = match &binding {
            RuntimeInputBinding::Constant(value) => value.value_type(),
            RuntimeInputBinding::Reference(reference) => reference.value_type.clone(),
        };
        if actual != slot.value_type {
            return Err(ManagedStageError::AuxiliaryTypeMismatch {
                socket: socket.clone(),
                expected: slot.value_type.clone(),
                actual,
            });
        }
        slot.binding = binding;
        Ok(())
    }

    pub fn evaluate<'a>(
        &'a mut self,
        input: &ChannelFrame,
        ctx: &EvaluationCtx<'_>,
    ) -> Result<(&'a ChannelFrame, RuntimeOutput), ManagedStageError> {
        self.evaluate_with_capture(input, ctx, DebugCaptureMode::Off)
    }

    pub fn evaluate_with_capture<'a>(
        &'a mut self,
        input: &ChannelFrame,
        ctx: &EvaluationCtx<'_>,
        capture_mode: DebugCaptureMode,
    ) -> Result<(&'a ChannelFrame, RuntimeOutput), ManagedStageError> {
        self.evaluate_with_capture_for_context(input, ctx, capture_mode, &ContextKey::default_lane())
    }

    pub fn evaluate_with_capture_for_context<'a>(
        &'a mut self,
        input: &ChannelFrame,
        ctx: &EvaluationCtx<'_>,
        capture_mode: DebugCaptureMode,
        context_key: &ContextKey,
    ) -> Result<(&'a ChannelFrame, RuntimeOutput), ManagedStageError> {
        if !input.layout().has_same_structure(&self.input_layout) {
            return Err(ManagedStageError::InputLayoutChanged);
        }
        let mut output = RuntimeOutput::default();
        let mut active_temporal_work = false;
        let mut stage_failed = false;
        for (group_index, group) in self.groups.iter().enumerate() {
            let group_context = ContextKey::new(context_key.iter().cloned().chain(group.context.iter().cloned()));
            let inputs = group
                .inputs
                .iter()
                .map(|index| input.slots().get(*index).ok_or(ManagedStageError::InputLayoutChanged))
                .collect::<Result<Vec<_>, _>>()?;
            let invalid = inputs
                .iter()
                .find(|slot| slot.validity != ChannelValidity::Valid || slot.value.is_none());
            if let Some(slot) = invalid {
                for result in &mut self.group_results[group_index] {
                    *result = ChannelSlot {
                        value: None,
                        validity: slot.validity,
                        changed: false,
                        deliver: false,
                    };
                }
                continue;
            }
            active_temporal_work = true;
            let mut overrides = IndexMap::new();
            for (property, slot) in self.input_properties.iter().zip(&inputs) {
                overrides.insert(property.clone(), slot.value.clone().expect("validated group input"));
            }
            for auxiliary in &self.auxiliary {
                let value = match &auxiliary.binding {
                    RuntimeInputBinding::Constant(value) => value.clone(),
                    RuntimeInputBinding::Reference(reference) => ctx
                        .inputs
                        .get_context(reference, &group_context)
                        .or_else(|| ctx.inputs.get(reference))
                        .cloned()
                        .ok_or_else(|| ManagedStageError::MissingReference(reference.clone()))?,
                };
                if value.value_type() != auxiliary.value_type {
                    return Err(ManagedStageError::AuxiliaryTypeMismatch {
                        socket: auxiliary.socket.clone(),
                        expected: auxiliary.value_type.clone(),
                        actual: value.value_type(),
                    });
                }
                overrides.insert(auxiliary.property.clone(), value);
            }
            let properties = RuntimePropertyFrame::with_overrides(&self.compiled.properties, &overrides)
                .map_err(ManagedStageError::PropertyFrame)?;
            let context = RuntimeContextFrame::new(group_context.clone());
            let graph_capture_mode = if matches!(capture_mode, DebugCaptureMode::SelectedNodes { .. }) {
                DebugCaptureMode::Off
            } else {
                capture_mode.clone()
            };
            let mut debug = (!graph_capture_mode.is_off()).then(|| DebugCaptureSink::new(graph_capture_mode));
            let frame = EvaluationFrame {
                ctx,
                properties: &properties,
                context: &context,
                debug: debug.as_mut(),
                force_process_unchanged_inputs: false,
                capture_unchanged_outputs: false,
            };
            let (evaluated, results, flows) = match self.memory.memory_for_key(group_context, &self.compiled) {
                Some(memory) => {
                    let evaluated = evaluate_compiled_graph(&self.compiled, memory, frame);
                    let values = self
                        .output_slots
                        .iter()
                        .map(|slot| memory.value(*slot).cloned())
                        .collect::<Vec<_>>();
                    let flows = self
                        .output_slots
                        .iter()
                        .map(|slot| memory.slot_flow(*slot))
                        .collect::<Vec<_>>();
                    (evaluated, values, flows)
                }
                None => {
                    let evaluated = evaluate_compiled_graph_fresh_reusing(&self.compiled, &mut self.scratch, frame);
                    let values = self
                        .output_slots
                        .iter()
                        .map(|slot| self.scratch.value(*slot).cloned())
                        .collect::<Vec<_>>();
                    let flows = self
                        .output_slots
                        .iter()
                        .map(|slot| self.scratch.slot_flow(*slot))
                        .collect::<Vec<_>>();
                    (evaluated, values, flows)
                }
            };
            if !evaluated.diagnostics.is_empty() {
                output.diagnostics.extend(evaluated.diagnostics);
                self.scratch.reset_for_fresh_evaluation(&self.compiled);
                stage_failed = true;
                break;
            }
            output.intents.extend(evaluated.intents.into_iter().map(|mut intent| {
                if intent.source_node == Some(self.compiled_stage_node) {
                    intent.source_node = Some(self.item.anode.id);
                }
                intent
            }));
            output.diagnostics.extend(evaluated.diagnostics);
            output
                .debug_samples
                .extend(evaluated.debug_samples.into_iter().filter_map(|mut sample| {
                    if sample.author_node_id != self.compiled_stage_node {
                        return None;
                    }
                    sample.author_node_id = self.item.anode.id;
                    Some(sample)
                }));
            let inputs_deliver = inputs.iter().all(|slot| slot.deliver);
            for ((result, value), flow) in self.group_results[group_index].iter_mut().zip(results).zip(flows) {
                *result = ChannelSlot {
                    validity: if flow == NodeFlow::Suppress {
                        ChannelValidity::Suppressed
                    } else if value.is_some() {
                        ChannelValidity::Valid
                    } else {
                        ChannelValidity::Invalid
                    },
                    value: (flow == NodeFlow::Deliver).then_some(value).flatten(),
                    changed: false,
                    deliver: flow == NodeFlow::Deliver && inputs_deliver,
                };
            }
        }
        self.temporal_evaluated = true;
        if stage_failed {
            self.active_temporal_contexts.remove(context_key);
        } else if active_temporal_work {
            self.active_temporal_contexts.insert(context_key.clone());
        } else {
            self.active_temporal_contexts.remove(context_key);
        }
        self.output_frame.begin_tick(ctx.logical_tick);
        if stage_failed {
            let failed_contexts = self
                .groups
                .iter()
                .map(|group| ContextKey::new(context_key.iter().cloned().chain(group.context.iter().cloned())))
                .collect::<HashSet<_>>();
            self.memory.retain_where(|key| !failed_contexts.contains(key));
            for index in 0..self.outputs.len() {
                self.output_frame
                    .set(index, None, ChannelValidity::Invalid, false)
                    .map_err(ManagedStageError::Frame)?;
            }
            output.intents.clear();
            output.debug_samples.clear();
            return Ok((&self.output_frame, output));
        }
        for (index, binding) in self.outputs.iter().enumerate() {
            let slot = match binding {
                StageOutputBinding::Passthrough(source) => input
                    .slots()
                    .get(*source)
                    .ok_or(ManagedStageError::InputLayoutChanged)?,
                StageOutputBinding::Result { group, output } => &self.group_results[*group][*output],
            };
            self.output_frame
                .set(index, slot.value.clone(), slot.validity, slot.deliver)
                .map_err(ManagedStageError::Frame)?;
        }
        if let Some(sample) = self.selected_stage_preview(&capture_mode, context_key, ctx.logical_tick) {
            output.debug_samples.push(sample);
        }
        Ok((&self.output_frame, output))
    }

    fn selected_stage_preview(
        &self,
        capture_mode: &DebugCaptureMode,
        context_key: &ContextKey,
        logical_tick: u64,
    ) -> Option<DebugValueSample> {
        let DebugCaptureMode::SelectedNodes {
            formula_id,
            context_key: selected_context,
            nodes,
            history_len,
        } = capture_mode
        else {
            return None;
        };
        if *history_len == 0
            || !nodes.contains(&self.item.anode.id)
            || !selected_context
                .as_ref()
                .map_or_else(|| context_key.is_default_lane(), |selected| selected == context_key)
        {
            return None;
        }
        let slots = self.output_frame.slots();
        let status = if slots.iter().any(|slot| slot.validity == ChannelValidity::Suppressed) {
            OutputPreviewStatus::Suppressed
        } else if slots.is_empty()
            || slots
                .iter()
                .any(|slot| slot.validity != ChannelValidity::Valid || slot.value.is_none())
            || !preview_values_fit(slots)
        {
            OutputPreviewStatus::Unavailable
        } else {
            OutputPreviewStatus::Live
        };
        let value = if status == OutputPreviewStatus::Live {
            let mut values = slots.iter().filter_map(|slot| slot.value.clone()).collect::<Vec<_>>();
            if values.len() == 1 {
                values.pop().expect("one valid stage output")
            } else {
                RuntimeValue::Array(values)
            }
        } else if slots
            .iter()
            .all(|slot| slot.validity == ChannelValidity::Valid && slot.value.is_some())
        {
            RuntimeValue::String("Preview exceeds 64 elements or 16 KiB".into())
        } else {
            RuntimeValue::Unit
        };
        Some(DebugValueSample {
            formula_id: formula_id.clone(),
            context_key: (!context_key.is_default_lane()).then(|| context_key.clone()),
            author_node_id: self.item.anode.id,
            exec_node: self.compiled_stage_exec,
            output_socket: SocketId::new("mapping_result"),
            output_slot: ValueSlotId::new(u32::MAX),
            value_type: value.value_type(),
            value,
            logical_tick,
            status,
        })
    }

    #[must_use]
    pub fn item_id(&self) -> chataigne_alchemist::ManagedItemId {
        self.item.id
    }

    pub fn reset_memory(&mut self) {
        self.memory.clear();
        self.scratch.reset_for_fresh_evaluation(&self.compiled);
        self.active_temporal_contexts.clear();
        self.temporal_evaluated = false;
    }

    pub fn suspend_context(&mut self, context_key: &ContextKey) {
        self.active_temporal_contexts.remove(context_key);
    }

    pub fn retain_context_keys(&mut self, active: &IndexSet<ContextKey>) {
        self.active_temporal_contexts.retain(|key| active.contains(key));
        if active.is_empty() {
            self.temporal_evaluated = true;
        }
        self.memory.retain_where(|key| {
            let processor_key = ContextKey::new(key.iter().filter(|part| part.axis.as_str() != STAGE_AXIS).cloned());
            active.contains(&processor_key)
        });
    }

    /// A renamed or reordered elementwise source keeps its identity. A changed operation or
    /// changed source provenance cannot inherit the old stage's temporal history.
    pub fn migrate_memory_from(&mut self, mut previous: Self) -> bool {
        if self.item.id != previous.item.id
            || self.item.anode.type_id != previous.item.anode.type_id
            || self.item.anode.config != previous.item.anode.config
            || self.item.anode.type_bindings != previous.item.anode.type_bindings
            || self.item.anode.forced_type_bindings != previous.item.anode.forced_type_bindings
            || self
                .auxiliary
                .iter()
                .map(|binding| (&binding.socket, &binding.value_type))
                .ne(previous
                    .auxiliary
                    .iter()
                    .map(|binding| (&binding.socket, &binding.value_type)))
            || self.output_slots.len() != previous.output_slots.len()
            || !previous.memory.is_compatible_with_graph(&self.compiled)
        {
            return false;
        }
        let old = previous.input_layout.channels();
        let new = self.input_layout.channels();
        let reorderable = self.groups.iter().all(|group| group.inputs.len() == 1)
            && previous.groups.iter().all(|group| group.inputs.len() == 1);
        let compatible = if reorderable {
            old.iter().all(|channel| {
                new.iter()
                    .find(|candidate| candidate.id == channel.id)
                    .is_none_or(|candidate| {
                        candidate.value_type == channel.value_type
                            && candidate.port == channel.port
                            && candidate.provenance == channel.provenance
                    })
            })
        } else {
            self.input_layout.has_same_structure(&previous.input_layout)
        };
        if !compatible {
            return false;
        }
        let active = self
            .groups
            .iter()
            .map(|group| group.context.parts[0].item.clone())
            .collect::<HashSet<_>>();
        previous.memory.retain_where(|key| {
            key.iter()
                .find(|part| part.axis.as_str() == STAGE_AXIS)
                .is_some_and(|part| active.contains(&part.item))
        });
        self.memory = previous.memory;
        self.active_temporal_contexts = previous.active_temporal_contexts;
        self.temporal_evaluated = previous.temporal_evaluated;
        true
    }
}

fn group_types(indices: &[usize], layout: &ChannelLayout) -> Result<Vec<ValueTypeId>, ManagedStageError> {
    indices
        .iter()
        .map(|index| {
            layout
                .channels()
                .get(*index)
                .ok_or(ManagedStageError::InputLayoutChanged)?
                .value_type
                .clone()
                .ok_or(ManagedStageError::UnresolvedInputType)
        })
        .collect()
}

fn auxiliary_type(
    constraint: &TypeConstraint,
    authored: Option<&RuntimeValue>,
    primary: &[ValueTypeId],
) -> Result<ValueTypeId, ManagedStageError> {
    match constraint {
        TypeConstraint::Exact(value_type) => Ok(value_type.clone()),
        _ => match authored {
            Some(RuntimeValue::Ref(reference)) => Ok(reference.value_type.clone()),
            Some(value) => Ok(value.value_type()),
            None => primary.first().cloned().ok_or(ManagedStageError::UnresolvedInputType),
        },
    }
}

fn group_context(indices: &[usize], layout: &ChannelLayout) -> ContextKey {
    let key = indices
        .iter()
        .map(|index| {
            let id = layout.channels()[*index].id.as_str();
            format!("{}:{id}", id.len())
        })
        .collect::<String>();
    ContextKey::single(ContextAxisId::new(STAGE_AXIS), ContextItemId::new(key))
}

fn output_layout(
    item: &ManagedItemInstance,
    application: &ManagedApplication,
    input: &ChannelLayout,
    output_types: &[ValueTypeId],
) -> Result<(ChannelLayout, Vec<StageOutputBinding>), ManagedStageError> {
    let mut replacements = (0..input.channels().len()).map(|_| Vec::new()).collect::<Vec<_>>();
    let mut consumed = vec![false; input.channels().len()];
    for (group_index, group) in application.groups.iter().enumerate() {
        let first = *group.iter().min().ok_or(ManagedStageError::EmptyGroups)?;
        for index in group {
            if std::mem::replace(&mut consumed[*index], true) {
                return Err(ManagedStageError::DuplicateGroupChannel);
            }
        }
        let mut produced = Vec::with_capacity(application.outputs.len());
        for (output_index, (socket, value_type)) in application.outputs.iter().zip(output_types).enumerate() {
            let id = match application.cardinality {
                PipelineCardinality::Elementwise | PipelineCardinality::WholeSet => input.channels()[first].id.clone(),
                PipelineCardinality::Reshape if group.len() == 1 && application.outputs.len() > 1 => {
                    let source = &input.channels()[first].id;
                    ValueComponent::parse(socket.as_str())
                        .map(|component| source.extracted(component))
                        .unwrap_or_else(|| {
                            ValueLaneKey::new(format!(
                                "extract:{}:{}:{socket}",
                                source.as_str().len(),
                                source.as_str()
                            ))
                            .expect("derived identity")
                        })
                }
                _ if application.groups.len() == 1 => ValueLaneKey::output(item.id, socket),
                _ => ValueLaneKey::new(format!(
                    "group-output:{}:{}:{}",
                    item.id,
                    group_context(group, input).parts[0].item.as_str(),
                    socket.as_str()
                ))
                .expect("derived identity"),
            };
            let mut descriptor = if matches!(
                application.cardinality,
                PipelineCardinality::Elementwise | PipelineCardinality::WholeSet
            ) {
                let mut descriptor = input.channels()[first].clone();
                descriptor.value_type = Some(value_type.clone());
                descriptor
            } else {
                ChannelDescriptor::derived(
                    id,
                    format!("{} {}", item.anode.label, socket.as_str()),
                    value_type.clone(),
                    item.id,
                    socket.clone(),
                    group.iter().map(|index| input.channels()[*index].id.clone()).collect(),
                )
            };
            descriptor.port = Some(socket.clone());
            produced.push((
                descriptor,
                StageOutputBinding::Result {
                    group: group_index,
                    output: output_index,
                },
            ));
        }
        replacements[first].extend(produced);
    }
    let mut channels = Vec::new();
    let mut bindings = Vec::new();
    for (index, channel) in input.channels().iter().enumerate() {
        for (descriptor, binding) in std::mem::take(&mut replacements[index]) {
            channels.push(descriptor);
            bindings.push(binding);
        }
        if !consumed[index] {
            channels.push(channel.clone());
            bindings.push(StageOutputBinding::Passthrough(index));
        }
    }
    Ok((input.reconcile(channels)?, bindings))
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedStageError {
    #[error("{0}")]
    Application(#[from] ManagedApplicationError),
    #[error("managed stage `{0}` is absent from the compiled chain")]
    MissingStage(chataigne_alchemist::ManagedItemId),
    #[error("{0}")]
    Layout(#[from] ChannelLayoutError),
    #[error("ANode declaration `{0}` is not registered")]
    MissingDeclaration(ANodeTypeId),
    #[error("the managed stage requires resolved input channel types")]
    UnresolvedInputType,
    #[error("managed stage input layout changed without a structural rebuild")]
    InputLayoutChanged,
    #[error("managed stage groups have incompatible type specializations")]
    MixedGroupSpecializations,
    #[error("managed stage has no selected groups")]
    EmptyGroups,
    #[error("a standard Mapping filter requires at least one typed input value")]
    EmptyTuple,
    #[error("a standard Mapping filter would pass through part of the input tuple")]
    PartialTupleApplication,
    #[error("managed stage group has {actual} inputs, expected {expected}")]
    GroupArity { expected: usize, actual: usize },
    #[error("managed stage group includes one channel twice")]
    DuplicateGroupChannel,
    #[error("managed stage cardinality `{0:?}` is not supported by typed stages yet")]
    UnsupportedCardinality(PipelineCardinality),
    #[error("value type `{0}` has no registered default")]
    MissingDefault(ValueTypeId),
    #[error("managed stage graph edit failed: {0}")]
    AuthoringEdit(chataigne_alchemist::AlchemistGraphTransactionError),
    #[error("managed stage graph failed to compile: {0:?}")]
    Compile(Vec<Diagnostic>),
    #[error("managed stage graph produced no compiled plan")]
    MissingCompiledGraph,
    #[error("managed stage ANode was removed during compilation")]
    MissingCompiledNode,
    #[error("managed stage output `{0}` was not compiled")]
    MissingOutputSocket(SocketId),
    #[error("managed stage output `{0}` has unresolved type")]
    UnresolvedOutputType(SocketId),
    #[error("managed stage has no auxiliary socket `{0}`")]
    MissingAuxiliarySocket(SocketId),
    #[error("managed stage auxiliary `{socket}` expects `{expected}`, got `{actual}`")]
    AuxiliaryTypeMismatch {
        socket: SocketId,
        expected: ValueTypeId,
        actual: ValueTypeId,
    },
    #[error("managed stage cannot resolve runtime reference `{0:?}`")]
    MissingReference(StableRef),
    #[error("{0}")]
    PropertyFrame(RuntimePropertyFrameError),
    #[error("{0}")]
    Frame(ChannelFrameError),
}
