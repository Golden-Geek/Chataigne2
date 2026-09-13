//! A typed managed stage compiles one ANode graph and applies it to authored channel groups.

use std::sync::Arc;

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistGraphDomain, AlchemistGraphTransaction, AlchemistMemory, ChannelDescriptor,
    ChannelLayout, ChannelLayoutError, CompileCtx, CompiledAlchemistGraph, ContextAxisId, ContextItemId, ContextKey,
    Diagnostic, EvaluationCtx, EvaluationFrame, FormulaPropertyDecl, FormulaPropertyId, FormulaPropertySchema,
    InputSocketRef, LaneRuntimePool, MANAGED_GROUPS_FIELD, ManagedApplication, ManagedApplicationError,
    ManagedItemInstance, OutputSocketRef, ParamUiHints, PipelineCardinality, RuntimeContextFrame, RuntimeOutput,
    RuntimePropertyFrame, RuntimePropertyFrameError, SignatureCtx, SocketId, StableRef, TypeConstraint, ValueComponent,
    ValueLaneKey, ValueSlotId, ValueTypeId, compile_graph, evaluate_compiled_graph,
    evaluate_compiled_graph_fresh_reusing,
};
use golden_values::Value as RuntimeValue;
use indexmap::{IndexMap, IndexSet};

use crate::{ChannelFrame, ChannelFrameError, ChannelSlot, ChannelValidity, RuntimeInputBinding};

const STAGE_AXIS: &str = "managed_stage_lane";

mod chain;

pub use chain::ManagedStageChain;

pub struct ManagedStageRuntime {
    item: ManagedItemInstance,
    input_layout: ChannelLayout,
    compiled: Arc<CompiledAlchemistGraph>,
    input_properties: Vec<FormulaPropertyId>,
    auxiliary: Vec<StageAuxiliaryBinding>,
    output_slots: Vec<ValueSlotId>,
    groups: Vec<StageGroup>,
    outputs: Vec<StageOutputBinding>,
    output_frame: ChannelFrame,
    memory: LaneRuntimePool,
    scratch: AlchemistMemory,
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
    ) -> Result<Option<Self>, ManagedStageError> {
        let signature_ctx = SignatureCtx {
            value_types: ctx.value_types,
            properties: ctx.properties,
        };
        let application = ctx
            .nodes
            .resolve_managed_application(&item.anode, input_layout, &signature_ctx)?;
        if application.selection.no_compatible_channels {
            return Ok(None);
        }
        if !matches!(
            application.cardinality,
            PipelineCardinality::Elementwise | PipelineCardinality::Aggregate | PipelineCardinality::Reshape
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
        AlchemistGraphDomain::insert_node(&mut transaction, item.anode.clone());
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
        let compiled = compile_graph(
            &graph,
            &CompileCtx {
                value_types: ctx.value_types,
                nodes: ctx.nodes,
                properties: Some(&properties),
            },
        );
        if compiled.has_errors() {
            return Err(ManagedStageError::Compile(compiled.diagnostics));
        }
        let compiled = compiled.compiled.ok_or(ManagedStageError::MissingCompiledGraph)?;
        let exec = compiled
            .exec_nodes
            .iter()
            .find(|node| node.authored_id == node_id)
            .ok_or(ManagedStageError::MissingCompiledNode)?;
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
        let groups = application
            .groups
            .iter()
            .map(|inputs| StageGroup {
                context: group_context(inputs, input_layout),
                inputs: inputs.clone(),
            })
            .collect();
        let memory = LaneRuntimePool::for_graph(&compiled);
        let scratch = AlchemistMemory::for_graph(&compiled);
        Ok(Some(Self {
            item,
            input_layout: input_layout.clone(),
            compiled,
            input_properties,
            auxiliary,
            output_slots,
            groups,
            outputs,
            output_frame: ChannelFrame::new(Arc::new(output_layout)),
            memory,
            scratch,
        }))
    }

    #[must_use]
    pub fn output_layout(&self) -> &Arc<ChannelLayout> {
        self.output_frame.layout()
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
        if !input.layout().has_same_structure(&self.input_layout) {
            return Err(ManagedStageError::InputLayoutChanged);
        }
        let active = self
            .groups
            .iter()
            .map(|group| group.context.clone())
            .collect::<IndexSet<_>>();
        self.memory.retain_keys(&active);
        let mut output = RuntimeOutput::default();
        let mut group_results = Vec::with_capacity(self.groups.len());
        for group in &self.groups {
            let inputs = group
                .inputs
                .iter()
                .map(|index| input.slots().get(*index).ok_or(ManagedStageError::InputLayoutChanged))
                .collect::<Result<Vec<_>, _>>()?;
            let invalid = inputs
                .iter()
                .find(|slot| slot.validity != ChannelValidity::Valid || slot.value.is_none());
            if let Some(slot) = invalid {
                group_results.push(vec![
                    ChannelSlot {
                        value: None,
                        validity: slot.validity,
                        changed: false,
                        deliver: false,
                    };
                    self.output_slots.len()
                ]);
                continue;
            }
            let mut overrides = IndexMap::new();
            for (property, slot) in self.input_properties.iter().zip(&inputs) {
                overrides.insert(property.clone(), slot.value.clone().expect("validated group input"));
            }
            for auxiliary in &self.auxiliary {
                let value = match &auxiliary.binding {
                    RuntimeInputBinding::Constant(value) => value.clone(),
                    RuntimeInputBinding::Reference(reference) => ctx
                        .inputs
                        .get_context(reference, &group.context)
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
            let context = RuntimeContextFrame::new(group.context.clone());
            let frame = EvaluationFrame {
                ctx,
                properties: &properties,
                context: &context,
                debug: None,
                force_process_unchanged_inputs: false,
                capture_unchanged_outputs: false,
            };
            let (evaluated, results) = match self.memory.memory_for_key(group.context.clone(), &self.compiled) {
                Some(memory) => {
                    let evaluated = evaluate_compiled_graph(&self.compiled, memory, frame);
                    let values = self
                        .output_slots
                        .iter()
                        .map(|slot| memory.value(*slot).cloned())
                        .collect::<Vec<_>>();
                    (evaluated, values)
                }
                None => {
                    let evaluated = evaluate_compiled_graph_fresh_reusing(&self.compiled, &mut self.scratch, frame);
                    let values = self
                        .output_slots
                        .iter()
                        .map(|slot| self.scratch.value(*slot).cloned())
                        .collect::<Vec<_>>();
                    (evaluated, values)
                }
            };
            output.intents.extend(evaluated.intents);
            output.diagnostics.extend(evaluated.diagnostics);
            let deliver = inputs.iter().all(|slot| slot.deliver);
            group_results.push(
                results
                    .into_iter()
                    .map(|value| ChannelSlot {
                        validity: if value.is_some() {
                            ChannelValidity::Valid
                        } else {
                            ChannelValidity::Invalid
                        },
                        value,
                        changed: false,
                        deliver,
                    })
                    .collect::<Vec<_>>(),
            );
        }
        self.output_frame.begin_tick(ctx.logical_tick);
        for (index, binding) in self.outputs.iter().enumerate() {
            let slot = match binding {
                StageOutputBinding::Passthrough(source) => input
                    .slots()
                    .get(*source)
                    .ok_or(ManagedStageError::InputLayoutChanged)?,
                StageOutputBinding::Result { group, output } => &group_results[*group][*output],
            };
            self.output_frame
                .set(index, slot.value.clone(), slot.validity, slot.deliver)
                .map_err(ManagedStageError::Frame)?;
        }
        Ok((&self.output_frame, output))
    }

    #[must_use]
    pub fn item_id(&self) -> chataigne_alchemist::ManagedItemId {
        self.item.id
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
                PipelineCardinality::Elementwise => input.channels()[first].id.clone(),
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
            let mut descriptor = if application.cardinality == PipelineCardinality::Elementwise {
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
