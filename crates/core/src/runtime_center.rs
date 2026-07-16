use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arc_swap::ArcSwap;
use golden_runtime::{
    ArenaLayout, BatchExecutor, CompilationCompletion, CompilationService, CompileRequest, CompiledContextCatalog,
    CompiledProcessorKernel, DirtySet, EffectRoutingTable, GenerationCompiler, InputDelivery, InputIngressConfig,
    InputRoute, InputRoutingTable, InputSlot, KernelId, ObservationCatalog, ObservationRoute, PersistentBatchScheduler,
    ProjectRevision, RuntimeChangeSet, RuntimeGeneration, RuntimeGenerationBuilder, RuntimeGenerationId,
    RuntimeInputHandle, RuntimeInputMailbox, RuntimeInputUpdate, RuntimeMetrics, RuntimeSchedule, ScheduledWork,
    SemanticRuntime, ValueSlot, WorkUnitId,
};
use golden_values::{ColorValue as RuntimeColor, TriggerValue as RuntimeTrigger, Value as RuntimeValue};

use crate::edit::Edit;
use crate::engine::{Engine, EngineRuntimeError, EngineTime};
use crate::events::EventKind;
use crate::node::{Node, NodeId};
use crate::parameter::{ParamValue, ParameterEventBehaviour};

const INPUT_LOSSLESS_CAPACITY: usize = 16_384;
const DOMAIN_NODE_ADAPTER_KERNEL: &str = "golden.runtime.domain-node-adapter";

#[derive(Clone)]
struct CompiledParameter {
    node: NodeId,
    value: ParamValue,
}

#[derive(Clone)]
struct EngineCompileSnapshot {
    parameters: Vec<CompiledParameter>,
    scheduled_nodes: Vec<CompiledScheduledNode>,
}

#[derive(Clone)]
struct CompiledScheduledNode {
    node: NodeId,
    kernel_key: String,
}

impl EngineCompileSnapshot {
    fn capture<T: Node>(engine: &Engine<T>) -> Self {
        let mut parameters = engine
            .nodes
            .iter()
            .filter_map(|(node, value)| {
                value.engine_param_snapshot().map(|snapshot| CompiledParameter {
                    node,
                    value: snapshot.value,
                })
            })
            .collect::<Vec<_>>();
        parameters.sort_unstable_by_key(|parameter| parameter.node.0);
        let scheduled_nodes = engine
            .schedule_topology()
            .iter()
            .map(|node| {
                let kernel_key = engine
                    .nodes
                    .get(*node)
                    .and_then(|value| value.execution_rule().compiled_kernel_key)
                    .unwrap_or(DOMAIN_NODE_ADAPTER_KERNEL)
                    .to_string();
                CompiledScheduledNode {
                    node: *node,
                    kernel_key,
                }
            })
            .collect();
        Self {
            parameters,
            scheduled_nodes,
        }
    }

    fn node_to_slot(&self) -> HashMap<NodeId, InputSlot> {
        self.parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.node, InputSlot(index as u32)))
            .collect()
    }

    fn slot_to_node(&self) -> Vec<NodeId> {
        self.parameters.iter().map(|parameter| parameter.node).collect()
    }

    fn scheduled_node_to_work(&self) -> HashMap<NodeId, WorkUnitId> {
        let input_count = self.parameters.len();
        self.scheduled_nodes
            .iter()
            .enumerate()
            .map(|(index, scheduled)| (scheduled.node, WorkUnitId((input_count + index) as u32)))
            .collect()
    }
}

#[derive(Clone, Copy)]
struct EngineGenerationCompiler;

impl GenerationCompiler<EngineCompileSnapshot> for EngineGenerationCompiler {
    type Error = String;

    fn compile(
        &self,
        generation_id: RuntimeGenerationId,
        request: CompileRequest<EngineCompileSnapshot>,
    ) -> Result<RuntimeGeneration, Self::Error> {
        compile_generation(generation_id, request.revision, &request.project)
    }
}

fn compile_generation(
    generation_id: RuntimeGenerationId,
    revision: ProjectRevision,
    snapshot: &EngineCompileSnapshot,
) -> Result<RuntimeGeneration, String> {
    let input_count = snapshot.parameters.len();
    let routes = (0..input_count)
        .map(|index| InputRoute {
            input: InputSlot(index as u32),
            target: ValueSlot(index as u32),
            dependent: WorkUnitId(index as u32),
        })
        .collect();
    let mut units = (0..input_count)
        .map(|index| ScheduledWork {
            id: WorkUnitId(index as u32),
            kernel: KernelId(0),
            first_lane: index as u32,
            lane_count: 1,
        })
        .collect::<Vec<_>>();
    let mut kernels = Vec::new();
    if !snapshot.parameters.is_empty() {
        kernels.push(CompiledProcessorKernel {
            id: KernelId(0),
            stable_key: "golden.runtime.input-identity".into(),
            inputs_per_lane: 1,
            outputs_per_lane: 1,
            state_per_lane: 0,
        });
    }
    let mut domain_kernels = BTreeMap::<String, KernelId>::new();
    for (index, scheduled) in snapshot.scheduled_nodes.iter().enumerate() {
        let kernel = if let Some(kernel) = domain_kernels.get(&scheduled.kernel_key) {
            *kernel
        } else {
            let kernel = KernelId(kernels.len() as u32);
            kernels.push(CompiledProcessorKernel {
                id: kernel,
                stable_key: scheduled.kernel_key.clone().into(),
                inputs_per_lane: 0,
                outputs_per_lane: 0,
                state_per_lane: 0,
            });
            domain_kernels.insert(scheduled.kernel_key.clone(), kernel);
            kernel
        };
        units.push(ScheduledWork {
            id: WorkUnitId((input_count + index) as u32),
            kernel,
            first_lane: index as u32,
            lane_count: 1,
        });
    }
    let observation = ObservationCatalog {
        routes: snapshot
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| ObservationRoute {
                key: parameter.node.0,
                value: ValueSlot(index as u32),
            })
            .collect::<Vec<_>>()
            .into(),
    };

    RuntimeGenerationBuilder {
        id: generation_id,
        project_revision: revision,
        statecharts: Vec::new(),
        processor_kernels: kernels,
        processor_instances: Vec::new(),
        contexts: CompiledContextCatalog::default(),
        input_routes: InputRoutingTable::new(input_count, routes).map_err(|error| error.to_string())?,
        schedule: RuntimeSchedule::new(units, 0.5).map_err(|error| error.to_string())?,
        effects: EffectRoutingTable::new(Vec::new(), 0).map_err(|error| error.to_string())?,
        observation,
        arenas: ArenaLayout {
            inputs: input_count,
            states: 0,
            values: input_count,
            effects: 0,
        },
        state_bindings: Vec::new(),
    }
    .build()
    .map_err(|error| error.to_string())
}

#[cfg(test)]
pub(crate) fn compiled_kernel_keys<T: Node>(engine: &Engine<T>) -> Result<Vec<String>, String> {
    let snapshot = EngineCompileSnapshot::capture(engine);
    let generation = compile_generation(RuntimeGenerationId(1), ProjectRevision(1), &snapshot)?;
    Ok(generation
        .processor_kernels
        .iter()
        .map(|kernel| kernel.stable_key.to_string())
        .collect())
}

struct InputGeneration {
    lifecycle: Mutex<bool>,
    routes: HashMap<NodeId, InputSlot>,
    handle: RuntimeInputHandle,
}

impl InputGeneration {
    fn publish(&self, update: RuntimeNodeInput) -> Result<(), PublishInputError> {
        let active = self.lifecycle.lock().map_err(|_| PublishInputError::Disconnected)?;
        if !*active {
            return Err(PublishInputError::StaleGeneration);
        }
        let slot = self
            .routes
            .get(&update.node)
            .copied()
            .ok_or(PublishInputError::UnknownInput(update.node))?;
        self.handle
            .publish(RuntimeInputUpdate {
                slot,
                value: param_to_runtime_value(&update.value, update.revision, update.source_time_ns)
                    .map_err(PublishInputError::Rejected)?,
                source_time_ns: update.source_time_ns,
                revision: update.revision,
                delivery: update.delivery,
            })
            .map_err(|error| PublishInputError::Rejected(error.to_string()))
    }

    fn deactivate(&self) -> Result<(), String> {
        let mut active = self
            .lifecycle
            .lock()
            .map_err(|_| "runtime input generation lock was poisoned".to_string())?;
        *active = false;
        Ok(())
    }
}

struct RuntimeInputPlane {
    current: ArcSwap<InputGeneration>,
    next_revision: AtomicU64,
}

/// Cloneable, generation-aware production module input adapter.
#[derive(Clone)]
pub struct ProductionInputPort {
    plane: Arc<RuntimeInputPlane>,
}

impl ProductionInputPort {
    /// Publishes one parsed and timestamped module value into the current dense input generation.
    pub fn publish(&self, node: NodeId, value: ParamValue, source_time_ns: u64) -> Result<(), String> {
        let revision = self.plane.next_revision.fetch_add(1, Ordering::Relaxed);
        let delivery = if matches!(value, ParamValue::Trigger()) {
            InputDelivery::LosslessOrdered
        } else {
            InputDelivery::LatestValue
        };
        let update = RuntimeNodeInput {
            node,
            value,
            source_time_ns,
            revision,
            delivery,
        };

        loop {
            let generation = self.plane.current.load_full();
            match generation.publish(update.clone()) {
                Ok(()) => return Ok(()),
                Err(PublishInputError::StaleGeneration) => std::thread::yield_now(),
                Err(error) => return Err(error.to_string()),
            }
        }
    }
}

#[derive(Clone)]
struct RuntimeNodeInput {
    node: NodeId,
    value: ParamValue,
    source_time_ns: u64,
    revision: u64,
    delivery: InputDelivery,
}

enum PublishInputError {
    StaleGeneration,
    UnknownInput(NodeId),
    Rejected(String),
    Disconnected,
}

impl std::fmt::Display for PublishInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleGeneration => formatter.write_str("runtime input generation changed"),
            Self::UnknownInput(node) => write!(formatter, "node {node:?} is not a compiled runtime input"),
            Self::Rejected(error) => formatter.write_str(error),
            Self::Disconnected => formatter.write_str("runtime input plane disconnected"),
        }
    }
}

#[derive(Clone, Copy)]
struct InputIdentityExecutor;

impl BatchExecutor for InputIdentityExecutor {
    type Output = WorkUnitId;

    fn execute(&self, work: ScheduledWork) -> Self::Output {
        work.id
    }
}

struct PendingCompilation {
    snapshot: Arc<EngineCompileSnapshot>,
}

pub(crate) struct ProductionState<T: Node> {
    pub(crate) engine: Engine<T>,
    semantic: SemanticRuntime,
    compiler: CompilationService<EngineCompileSnapshot, EngineGenerationCompiler>,
    pending_compilations: BTreeMap<u64, PendingCompilation>,
    project_revision: ProjectRevision,
    input_plane: Arc<RuntimeInputPlane>,
    input_generation: Arc<InputGeneration>,
    input_mailbox: Arc<RuntimeInputMailbox>,
    slot_to_node: Vec<NodeId>,
    scheduled_node_to_work: HashMap<NodeId, WorkUnitId>,
    scheduled_work_nodes: Vec<NodeId>,
    input_work_count: usize,
    input_scratch: Vec<RuntimeInputUpdate>,
    dirty: DirtySet,
    scheduler: PersistentBatchScheduler<InputIdentityExecutor>,
    scheduler_outputs: Vec<(WorkUnitId, WorkUnitId)>,
    metrics: Arc<RuntimeMetrics>,
    shadow_expected: Vec<(NodeId, ParamValue)>,
    last_runtime_plane_error: Option<String>,
}

impl<T: Node> ProductionState<T> {
    pub(crate) fn new(
        mut engine: Engine<T>,
        metrics: Arc<RuntimeMetrics>,
    ) -> Result<(Self, ProductionInputPort), String> {
        engine
            .resolve()
            .map_err(|error| format!("failed to resolve initial runtime schedule: {error}"))?;
        let snapshot = Arc::new(EngineCompileSnapshot::capture(&engine));
        let revision = ProjectRevision(1);
        let generation = Arc::new(compile_generation(RuntimeGenerationId(1), revision, &snapshot)?);
        let work_count = generation.schedule.work_count();
        let mut semantic = SemanticRuntime::new(generation);
        seed_semantic_inputs(&mut semantic, &snapshot)?;
        let (input_mailbox, input_generation) = make_input_generation(&snapshot)?;
        let input_plane = Arc::new(RuntimeInputPlane {
            current: ArcSwap::new(input_generation.clone()),
            next_revision: AtomicU64::new(1),
        });
        let input_port = ProductionInputPort {
            plane: input_plane.clone(),
        };
        let compiler = CompilationService::spawn(EngineGenerationCompiler, 2, metrics.clone())
            .map_err(|error| format!("failed to start runtime compiler: {error}"))?;
        let worker_count = std::thread::available_parallelism().map_or(1, usize::from).clamp(1, 8);
        let scheduler = PersistentBatchScheduler::new(worker_count, InputIdentityExecutor, metrics.clone())
            .map_err(|error| format!("failed to start runtime scheduler: {error}"))?;

        Ok((
            Self {
                engine,
                semantic,
                compiler,
                pending_compilations: BTreeMap::new(),
                project_revision: revision,
                input_plane,
                input_generation,
                input_mailbox,
                slot_to_node: snapshot.slot_to_node(),
                scheduled_node_to_work: snapshot.scheduled_node_to_work(),
                scheduled_work_nodes: snapshot
                    .scheduled_nodes
                    .iter()
                    .map(|scheduled| scheduled.node)
                    .collect(),
                input_work_count: snapshot.parameters.len(),
                input_scratch: Vec::new(),
                dirty: DirtySet::new(work_count),
                scheduler,
                scheduler_outputs: Vec::new(),
                metrics,
                shadow_expected: Vec::new(),
                last_runtime_plane_error: None,
            },
            input_port,
        ))
    }

    pub(crate) fn run_tick(&mut self, elapsed: Duration) -> Result<(), EngineRuntimeError> {
        self.apply_completed_compilations();
        self.apply_dense_inputs();
        let previous_event_time = self.engine.ui_event_log().last().map(|event| event.time);
        let generation = self.semantic.current_generation();
        let scheduled_node_to_work = &self.scheduled_node_to_work;
        let scheduled_work_nodes = &self.scheduled_work_nodes;
        let input_work_count = self.input_work_count;
        let dirty = &mut self.dirty;
        let scheduler = &self.scheduler;
        let scheduler_outputs = &mut self.scheduler_outputs;
        let runtime_error = &mut self.last_runtime_plane_error;
        let tick_result = self
            .engine
            .run_tick_with_compiled_schedule(elapsed, |due_nodes, ordered_nodes| {
                dirty.clear();
                for node in due_nodes {
                    if let Some(work) = scheduled_node_to_work.get(node).copied()
                        && let Err(error) = dirty.mark(work)
                    {
                        *runtime_error = Some(error.to_string());
                    }
                }

                match scheduler.execute_into(&generation.schedule, dirty, scheduler_outputs) {
                    Ok(_) => {
                        for (work, _) in scheduler_outputs.iter().copied() {
                            let Some(index) = work.index().checked_sub(input_work_count) else {
                                continue;
                            };
                            if let Some(node) = scheduled_work_nodes.get(index).copied() {
                                ordered_nodes.push(node);
                            }
                        }
                    }
                    Err(error) => {
                        *runtime_error = Some(error.to_string());
                        for node in scheduled_work_nodes {
                            if due_nodes.contains(node) {
                                ordered_nodes.push(*node);
                            }
                        }
                    }
                }

                // A structural edit may become runnable while its replacement generation is still
                // compiling. Preserve previous-generation continuity by appending only those new
                // nodes; the next successful atomic swap assigns their stable work positions.
                for node in due_nodes {
                    if !scheduled_node_to_work.contains_key(node) && !ordered_nodes.contains(node) {
                        ordered_nodes.push(*node);
                    }
                }
                dirty.clear();
            });
        if let Err(error) = tick_result {
            self.shadow_expected.clear();
            return Err(error);
        }
        self.compare_shadow_results();
        if has_structural_events_since(&self.engine, previous_event_time) {
            self.request_compilation("runtime.structure");
        }
        Ok(())
    }

    pub(crate) fn request_compilation(&mut self, affected: &str) {
        if let Err(error) = self.request_compilation_inner(affected) {
            self.last_runtime_plane_error = Some(error);
        }
    }

    pub(crate) fn recompile_blocking(&mut self, affected: &str) -> Result<(), String> {
        let ticket = self.request_compilation_inner(affected)?;
        loop {
            let completion = self.compiler.complete().map_err(|error| error.to_string())?;
            let completed_ticket = completion.ticket;
            self.apply_completion(completion);
            if completed_ticket == ticket {
                return self.last_runtime_plane_error.take().map_or(Ok(()), Err);
            }
        }
    }

    fn request_compilation_inner(&mut self, affected: &str) -> Result<u64, String> {
        if self.engine.is_resolve_pending() {
            self.engine
                .resolve()
                .map_err(|error| format!("failed to resolve runtime schedule before compilation: {error}"))?;
        }
        self.project_revision.0 = self.project_revision.0.saturating_add(1);
        let snapshot = Arc::new(EngineCompileSnapshot::capture(&self.engine));
        let mut changes = RuntimeChangeSet::new();
        changes.mark(affected);
        let ticket = self
            .compiler
            .handle()
            .request(CompileRequest {
                project: snapshot.clone(),
                revision: self.project_revision,
                changes,
                previous: Some(self.semantic.current_generation()),
            })
            .map_err(|error| error.to_string())?;
        self.pending_compilations
            .insert(ticket, PendingCompilation { snapshot });
        Ok(ticket)
    }

    fn apply_completed_compilations(&mut self) {
        loop {
            match self.compiler.try_complete() {
                Ok(Some(completion)) => self.apply_completion(completion),
                Ok(None) => break,
                Err(error) => {
                    self.last_runtime_plane_error = Some(error.to_string());
                    break;
                }
            }
        }
    }

    fn apply_completion(&mut self, completion: CompilationCompletion) {
        let Some(pending) = self.pending_compilations.remove(&completion.ticket) else {
            return;
        };
        if completion.revision != self.project_revision {
            return;
        }
        let generation = match completion.result {
            Ok(generation) => generation,
            Err(error) => {
                self.last_runtime_plane_error = Some(error.to_string());
                return;
            }
        };
        if let Err(error) = self.install_generation(generation, &pending.snapshot) {
            self.last_runtime_plane_error = Some(error);
        }
    }

    fn install_generation(
        &mut self,
        generation: Arc<RuntimeGeneration>,
        snapshot: &EngineCompileSnapshot,
    ) -> Result<(), String> {
        let (next_mailbox, next_input_generation) = make_input_generation(snapshot)?;
        let work_count = generation.schedule.work_count();
        self.input_generation.deactivate()?;
        self.input_plane.current.store(next_input_generation.clone());
        self.apply_dense_inputs();
        self.semantic.swap_generation(generation);
        seed_semantic_inputs(&mut self.semantic, snapshot)?;
        self.input_mailbox = next_mailbox;
        self.input_generation = next_input_generation;
        self.slot_to_node = snapshot.slot_to_node();
        self.scheduled_node_to_work = snapshot.scheduled_node_to_work();
        self.scheduled_work_nodes = snapshot
            .scheduled_nodes
            .iter()
            .map(|scheduled| scheduled.node)
            .collect();
        self.input_work_count = snapshot.parameters.len();
        self.dirty = DirtySet::new(work_count);
        self.scheduler_outputs.clear();
        Ok(())
    }

    fn apply_dense_inputs(&mut self) {
        let generation = self.semantic.current_generation();
        let drain_result = self.input_mailbox.drain_into(
            self.semantic.arenas_mut(),
            &generation.input_routes,
            &mut self.dirty,
            &mut self.input_scratch,
        );
        if let Err(error) = drain_result {
            self.last_runtime_plane_error = Some(error.to_string());
            self.dirty.clear();
            return;
        }
        if self.input_scratch.is_empty() {
            self.dirty.clear();
            return;
        }
        if let Err(error) = self
            .scheduler
            .execute_into(&generation.schedule, &self.dirty, &mut self.scheduler_outputs)
        {
            self.last_runtime_plane_error = Some(error.to_string());
            self.dirty.clear();
            return;
        }
        for update in &self.input_scratch {
            let Some(node) = self.slot_to_node.get(update.slot.index()).copied() else {
                self.last_runtime_plane_error = Some("compiled input slot has no node route".to_string());
                continue;
            };
            let value = match runtime_to_param_value(
                &update.value,
                self.engine
                    .nodes
                    .get(node)
                    .and_then(Node::engine_param_snapshot)
                    .as_ref()
                    .map(|snapshot| &snapshot.value),
            ) {
                Ok(value) => value,
                Err(error) => {
                    self.last_runtime_plane_error = Some(error);
                    continue;
                }
            };
            self.shadow_expected.push((node, value.clone()));
            self.engine.edits.push(Edit::SetParam {
                node,
                value,
                behaviour: match update.delivery {
                    InputDelivery::LatestValue => ParameterEventBehaviour::Coalesce,
                    InputDelivery::LosslessOrdered => ParameterEventBehaviour::Append,
                },
            });
        }
        self.dirty.clear();
    }

    fn compare_shadow_results(&mut self) {
        let comparisons = self.shadow_expected.len();
        let mismatches = self
            .shadow_expected
            .drain(..)
            .filter(|(node, expected)| {
                self.engine
                    .nodes
                    .get(*node)
                    .and_then(Node::engine_param_snapshot)
                    .is_none_or(|snapshot| snapshot.value != *expected)
            })
            .count();
        self.metrics.shadow_compared(comparisons, mismatches);
    }
}

fn make_input_generation(
    snapshot: &EngineCompileSnapshot,
) -> Result<(Arc<RuntimeInputMailbox>, Arc<InputGeneration>), String> {
    let (mailbox, handle) = RuntimeInputMailbox::new(InputIngressConfig {
        input_count: snapshot.parameters.len(),
        lossless_capacity: INPUT_LOSSLESS_CAPACITY,
    })
    .map_err(|error| error.to_string())?;
    let generation = Arc::new(InputGeneration {
        lifecycle: Mutex::new(true),
        routes: snapshot.node_to_slot(),
        handle,
    });
    Ok((mailbox, generation))
}

fn seed_semantic_inputs(semantic: &mut SemanticRuntime, snapshot: &EngineCompileSnapshot) -> Result<(), String> {
    for (index, parameter) in snapshot.parameters.iter().enumerate() {
        semantic
            .arenas_mut()
            .set_input(InputSlot(index as u32), param_to_runtime_value(&parameter.value, 0, 0)?)
            .map_err(|error| error.to_string())?;
        let value = semantic
            .arenas_mut()
            .value_mut(ValueSlot(index as u32))
            .ok_or_else(|| "compiled value slot is out of bounds".to_string())?;
        *value = param_to_runtime_value(&parameter.value, 0, 0)?;
    }
    Ok(())
}

fn param_to_runtime_value(value: &ParamValue, revision: u64, source_time_ns: u64) -> Result<RuntimeValue, String> {
    Ok(match value {
        ParamValue::Trigger() => RuntimeValue::Trigger(RuntimeTrigger {
            fired: true,
            edge_id: revision,
            logical_tick: source_time_ns,
        }),
        ParamValue::Int(value) => RuntimeValue::Int(i64::from(*value)),
        ParamValue::Float(value) => RuntimeValue::Float(*value),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => {
            RuntimeValue::String(value.clone().into())
        }
        ParamValue::Bool(value) => RuntimeValue::Bool(*value),
        ParamValue::CssValue(_) | ParamValue::Reference(_) => RuntimeValue::String(
            serde_json::to_string(value)
                .map_err(|error| format!("failed to encode runtime parameter value: {error}"))?
                .into(),
        ),
        ParamValue::Vec2(x, y) => RuntimeValue::Vec2([*x, *y]),
        ParamValue::Vec3(x, y, z) => RuntimeValue::Vec3([*x, *y, *z]),
        ParamValue::Color(red, green, blue, alpha) => RuntimeValue::Color(RuntimeColor {
            red: *red,
            green: *green,
            blue: *blue,
            alpha: *alpha,
        }),
    })
}

fn runtime_to_param_value(value: &RuntimeValue, current: Option<&ParamValue>) -> Result<ParamValue, String> {
    match value {
        RuntimeValue::Bool(value) => Ok(ParamValue::Bool(*value)),
        RuntimeValue::Trigger(_) => Ok(ParamValue::Trigger()),
        RuntimeValue::Int(value) => i32::try_from(*value)
            .map(ParamValue::Int)
            .map_err(|_| format!("runtime integer {value} is outside the parameter range")),
        RuntimeValue::Float(value) => Ok(ParamValue::Float(*value)),
        RuntimeValue::String(value) => match current {
            Some(ParamValue::File(_)) => Ok(ParamValue::File(value.to_string())),
            Some(ParamValue::Enum(_)) => Ok(ParamValue::Enum(value.to_string())),
            Some(ParamValue::CssValue(_)) | Some(ParamValue::Reference(_)) => serde_json::from_str(value)
                .map_err(|error| format!("failed to decode runtime parameter value: {error}")),
            _ => Ok(ParamValue::Str(value.to_string())),
        },
        RuntimeValue::Vec2(value) => Ok(ParamValue::Vec2(value[0], value[1])),
        RuntimeValue::Vec3(value) => Ok(ParamValue::Vec3(value[0], value[1], value[2])),
        RuntimeValue::Color(value) => Ok(ParamValue::Color(value.red, value.green, value.blue, value.alpha)),
        other => Err(format!("runtime value {other:?} cannot be applied to a parameter")),
    }
}

fn has_structural_events_since<T: Node>(engine: &Engine<T>, previous: Option<EngineTime>) -> bool {
    engine
        .ui_event_log()
        .iter()
        .filter(|event| previous.is_none_or(|time| event.time > time))
        .any(|event| {
            matches!(
                event.kind,
                EventKind::ChildAdded { .. }
                    | EventKind::ChildRemoved { .. }
                    | EventKind::ChildReplaced { .. }
                    | EventKind::ChildMoved { .. }
                    | EventKind::ChildReordered { .. }
                    | EventKind::NodeCreated { .. }
                    | EventKind::NodeDeleted { .. }
                    | EventKind::GraphTransaction { .. }
            )
        })
}
