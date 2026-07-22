use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use arc_swap::ArcSwap;
use golden_values::Value;

use crate::{
    ArtifactId, EffectSlot, InputSlot, KernelId, LaneIndex, ProcessorInstanceId, ProjectRevision, RuntimeGenerationId,
    RuntimeSchedule, StableStateKey, StateSlot, ValueSlot, WorkUnitId,
};

/// App-agnostic compiled domain artifact retained by a generation catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledArtifact {
    /// Dense runtime artifact id.
    pub id: ArtifactId,
    /// Stable authoring identity retained outside hot paths.
    pub stable_id: Arc<str>,
}

/// Compiled statechart layout descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledStatechart {
    /// Catalog artifact.
    pub artifact: CompiledArtifact,
    /// Dense state slots owned by the statechart.
    pub state_slots: Range<usize>,
}

/// Shared compiled processor kernel descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledProcessorKernel {
    /// Dense kernel id used by schedule batches.
    pub id: KernelId,
    /// Stable semantic cache key.
    pub stable_key: Arc<str>,
    /// Number of input values consumed per lane.
    pub inputs_per_lane: u32,
    /// Number of output values produced per lane.
    pub outputs_per_lane: u32,
    /// Number of state values retained per lane.
    pub state_per_lane: u32,
}

/// Dense layout of one processor instance over one or more multiplex lanes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessorInstanceLayout {
    /// Dense instance id.
    pub id: ProcessorInstanceId,
    /// Shared compiled kernel.
    pub kernel: KernelId,
    /// First lane owned by this instance.
    pub first_lane: LaneIndex,
    /// Number of contiguous lanes.
    pub lane_count: u32,
    /// First input value slot.
    pub input_base: ValueSlot,
    /// First persistent state slot.
    pub state_base: StateSlot,
    /// First output value slot.
    pub output_base: ValueSlot,
    /// First staged effect slot.
    pub effect_base: EffectSlot,
}

/// Compiled context and multiplex catalog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompiledContextCatalog {
    /// Total dense lane count.
    pub lane_count: usize,
    /// Stable lane keys in dense order.
    pub lane_keys: Arc<[Arc<str>]>,
}

/// One direct external-input dependency route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputRoute {
    /// Dense input slot written by an I/O adapter.
    pub input: InputSlot,
    /// Dense value slot populated before execution.
    pub target: ValueSlot,
    /// Work unit made dirty by this input.
    pub dependent: WorkUnitId,
}

/// Direct input routes grouped by dense input slot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputRoutingTable {
    /// Routes in deterministic input/dependent order.
    pub routes: Arc<[InputRoute]>,
    /// Start offset per input slot plus one sentinel.
    pub offsets: Arc<[u32]>,
}

impl InputRoutingTable {
    /// Builds and validates a compact adjacency table.
    pub fn new(input_count: usize, mut routes: Vec<InputRoute>) -> Result<Self, RuntimeGenerationError> {
        routes.sort_unstable_by_key(|route| (route.input, route.dependent, route.target));
        if routes.iter().any(|route| route.input.index() >= input_count) {
            return Err(RuntimeGenerationError::RouteOutOfBounds);
        }
        let mut offsets = vec![0_u32; input_count + 1];
        for route in &routes {
            offsets[route.input.index() + 1] += 1;
        }
        for index in 1..offsets.len() {
            offsets[index] += offsets[index - 1];
        }
        Ok(Self {
            routes: routes.into(),
            offsets: offsets.into(),
        })
    }

    /// Returns direct routes for an input without graph traversal or authored-id lookup.
    pub fn routes_for(&self, input: InputSlot) -> &[InputRoute] {
        let Some((&start, &end)) = self.offsets.get(input.index()).zip(self.offsets.get(input.index() + 1)) else {
            return &[];
        };
        &self.routes[start as usize..end as usize]
    }
}

/// One deterministic effect route, preordered at compile time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectRoute {
    /// Staging slot read during commit.
    pub slot: EffectSlot,
    /// State order component.
    pub state_order: u32,
    /// Processor order component.
    pub processor_order: u32,
    /// Lane order component.
    pub lane_order: u32,
    /// Effect order component.
    pub effect_order: u32,
}

/// Deterministic effect commit order compiled into a generation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectRoutingTable {
    /// Routes ordered by `(state, processor, lane, effect)`.
    pub routes: Arc<[EffectRoute]>,
}

impl EffectRoutingTable {
    /// Creates a validated deterministic route table.
    pub fn new(mut routes: Vec<EffectRoute>, effect_count: usize) -> Result<Self, RuntimeGenerationError> {
        if routes.iter().any(|route| route.slot.index() >= effect_count) {
            return Err(RuntimeGenerationError::EffectRouteOutOfBounds);
        }
        routes.sort_unstable_by_key(|route| {
            (
                route.state_order,
                route.processor_order,
                route.lane_order,
                route.effect_order,
            )
        });
        Ok(Self { routes: routes.into() })
    }
}

/// One visible/selected runtime observation route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObservationRoute {
    /// Stable catalog key represented outside the semantic loop.
    pub key: u64,
    /// Dense committed value slot.
    pub value: ValueSlot,
}

/// Compile-time observation catalog used by off-thread projection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservationCatalog {
    /// Stable keyed routes.
    pub routes: Arc<[ObservationRoute]>,
}

/// Dense arena sizes fixed for one immutable generation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaLayout {
    /// External input slots.
    pub inputs: usize,
    /// Persistent state slots.
    pub states: usize,
    /// Semantic value/output slots.
    pub values: usize,
    /// Staged effect slots.
    pub effects: usize,
}

/// Stable state binding and generation-specific dense slot.
#[derive(Clone, Debug, PartialEq)]
pub struct StableStateBinding {
    /// Stable typed migration key.
    pub key: StableStateKey,
    /// Dense slot in this generation.
    pub slot: StateSlot,
    /// Default value used when no compatible previous state exists.
    pub default: Value,
}

/// Immutable compiled representation of one valid project revision.
#[derive(Clone, Debug)]
pub struct RuntimeGeneration {
    /// Monotonic runtime generation id.
    pub id: RuntimeGenerationId,
    /// Project revision from which this generation was built.
    pub project_revision: ProjectRevision,
    /// Compiled statechart layouts.
    pub statecharts: Arc<[CompiledStatechart]>,
    /// Shared compiled processor kernels.
    pub processor_kernels: Arc<[CompiledProcessorKernel]>,
    /// Dense processor instance layouts.
    pub processor_instances: Arc<[ProcessorInstanceLayout]>,
    /// Context and multiplex lane catalog.
    pub contexts: Arc<CompiledContextCatalog>,
    /// Direct input dependency routes.
    pub input_routes: Arc<InputRoutingTable>,
    /// Persistent deterministic schedule.
    pub schedule: Arc<RuntimeSchedule>,
    /// Deterministic effect routes.
    pub effects: Arc<EffectRoutingTable>,
    /// Off-thread observation catalog.
    pub observation: Arc<ObservationCatalog>,
    /// Fixed dense arena layout.
    pub arenas: ArenaLayout,
    /// Stable state migration bindings.
    pub state_bindings: Arc<[StableStateBinding]>,
}

/// Builder that validates all generation-owned dense layouts before publication.
#[derive(Clone, Debug)]
pub struct RuntimeGenerationBuilder {
    /// Generation id.
    pub id: RuntimeGenerationId,
    /// Source project revision.
    pub project_revision: ProjectRevision,
    /// Statecharts.
    pub statecharts: Vec<CompiledStatechart>,
    /// Processor kernels.
    pub processor_kernels: Vec<CompiledProcessorKernel>,
    /// Processor instances.
    pub processor_instances: Vec<ProcessorInstanceLayout>,
    /// Context catalog.
    pub contexts: CompiledContextCatalog,
    /// Input routing table.
    pub input_routes: InputRoutingTable,
    /// Runtime schedule.
    pub schedule: RuntimeSchedule,
    /// Effect routing table.
    pub effects: EffectRoutingTable,
    /// Observation catalog.
    pub observation: ObservationCatalog,
    /// Arena layout.
    pub arenas: ArenaLayout,
    /// State migration bindings.
    pub state_bindings: Vec<StableStateBinding>,
}

impl RuntimeGenerationBuilder {
    /// Validates and freezes the generation.
    pub fn build(self) -> Result<RuntimeGeneration, RuntimeGenerationError> {
        validate_dense_ids(&self.processor_kernels, &self.processor_instances)?;
        if self.input_routes.offsets.len() != self.arenas.inputs + 1 {
            return Err(RuntimeGenerationError::InputLayoutMismatch);
        }
        if self.schedule.work_count() > self.schedule.units().len() {
            return Err(RuntimeGenerationError::ScheduleLayoutMismatch);
        }
        let mut keys = HashMap::with_capacity(self.state_bindings.len());
        for binding in &self.state_bindings {
            if binding.slot.index() >= self.arenas.states {
                return Err(RuntimeGenerationError::StateSlotOutOfBounds);
            }
            if keys.insert(binding.key.clone(), binding.slot).is_some() {
                return Err(RuntimeGenerationError::DuplicateStateKey(binding.key.clone()));
            }
        }
        Ok(RuntimeGeneration {
            id: self.id,
            project_revision: self.project_revision,
            statecharts: self.statecharts.into(),
            processor_kernels: self.processor_kernels.into(),
            processor_instances: self.processor_instances.into(),
            contexts: Arc::new(self.contexts),
            input_routes: Arc::new(self.input_routes),
            schedule: Arc::new(self.schedule),
            effects: Arc::new(self.effects),
            observation: Arc::new(self.observation),
            arenas: self.arenas,
            state_bindings: self.state_bindings.into(),
        })
    }
}

/// Dense mutable semantic storage owned by one runtime thread.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeArenas {
    inputs: Vec<Value>,
    states: Vec<Value>,
    values: Vec<Value>,
}

impl RuntimeArenas {
    /// Allocates arenas once for a validated generation.
    pub fn for_generation(generation: &RuntimeGeneration) -> Self {
        let mut states = vec![Value::Unit; generation.arenas.states];
        for binding in generation.state_bindings.iter() {
            states[binding.slot.index()] = binding.default.clone();
        }
        Self {
            inputs: vec![Value::Unit; generation.arenas.inputs],
            states,
            values: vec![Value::Unit; generation.arenas.values],
        }
    }

    /// Writes a direct external input slot.
    pub fn set_input(&mut self, slot: InputSlot, value: Value) -> Result<(), RuntimeGenerationError> {
        let target = self
            .inputs
            .get_mut(slot.index())
            .ok_or(RuntimeGenerationError::InputSlotOutOfBounds)?;
        *target = value;
        Ok(())
    }

    /// Reads an input slot.
    pub fn input(&self, slot: InputSlot) -> Option<&Value> {
        self.inputs.get(slot.index())
    }

    /// Reads a persistent state slot.
    pub fn state(&self, slot: StateSlot) -> Option<&Value> {
        self.states.get(slot.index())
    }

    /// Mutably reads a persistent state slot.
    pub fn state_mut(&mut self, slot: StateSlot) -> Option<&mut Value> {
        self.states.get_mut(slot.index())
    }

    /// Reads a committed semantic value.
    pub fn value(&self, slot: ValueSlot) -> Option<&Value> {
        self.values.get(slot.index())
    }

    /// Mutably reads a committed semantic value.
    pub fn value_mut(&mut self, slot: ValueSlot) -> Option<&mut Value> {
        self.values.get_mut(slot.index())
    }
}

/// State migration evidence produced at an atomic generation swap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenerationSwapReport {
    /// Previous generation.
    pub previous: RuntimeGenerationId,
    /// Newly published generation.
    pub current: RuntimeGenerationId,
    /// Compatible state values migrated.
    pub migrated_states: usize,
    /// State values initialized from generation defaults.
    pub initialized_states: usize,
}

/// Semantic owner of one current immutable generation and its dense mutable arenas.
pub struct SemanticRuntime {
    generation: Arc<RuntimeGeneration>,
    published_generation: Arc<ArcSwap<RuntimeGeneration>>,
    arenas: RuntimeArenas,
}

impl SemanticRuntime {
    /// Creates a semantic runtime from its first valid generation.
    pub fn new(generation: Arc<RuntimeGeneration>) -> Self {
        let arenas = RuntimeArenas::for_generation(&generation);
        Self {
            published_generation: Arc::new(ArcSwap::from(generation.clone())),
            generation,
            arenas,
        }
    }

    /// Returns a lock-free immutable generation snapshot for observation and diagnostics.
    pub fn current_generation(&self) -> Arc<RuntimeGeneration> {
        self.published_generation.load_full()
    }

    /// Returns dense arenas to the single semantic owner.
    pub fn arenas(&self) -> &RuntimeArenas {
        &self.arenas
    }

    /// Returns mutable dense arenas to the single semantic owner.
    pub fn arenas_mut(&mut self) -> &mut RuntimeArenas {
        &mut self.arenas
    }

    /// Migrates compatible state and atomically publishes a new generation at a semantic boundary.
    pub fn swap_generation(&mut self, next: Arc<RuntimeGeneration>) -> GenerationSwapReport {
        let previous = self.generation.id;
        let old_slots: HashMap<_, _> = self
            .generation
            .state_bindings
            .iter()
            .map(|binding| (binding.key.clone(), binding.slot))
            .collect();
        let mut next_arenas = RuntimeArenas::for_generation(&next);
        let mut migrated_states = 0;
        for binding in next.state_bindings.iter() {
            let Some(old_slot) = old_slots.get(&binding.key) else {
                continue;
            };
            let Some(value) = self.arenas.state(*old_slot) else {
                continue;
            };
            next_arenas.states[binding.slot.index()] = value.clone();
            migrated_states += 1;
        }
        self.arenas = next_arenas;
        self.generation = next.clone();
        self.published_generation.store(next.clone());
        GenerationSwapReport {
            previous,
            current: next.id,
            migrated_states,
            initialized_states: next.state_bindings.len().saturating_sub(migrated_states),
        }
    }
}

/// Invalid compiled runtime generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeGenerationError {
    /// Direct input route references a missing input.
    RouteOutOfBounds,
    /// Effect route references a missing effect slot.
    EffectRouteOutOfBounds,
    /// Input route offsets do not match arena layout.
    InputLayoutMismatch,
    /// Schedule metadata is inconsistent.
    ScheduleLayoutMismatch,
    /// A state binding references a missing slot.
    StateSlotOutOfBounds,
    /// A direct input write references a missing slot.
    InputSlotOutOfBounds,
    /// Two state slots claim the same stable migration key.
    DuplicateStateKey(StableStateKey),
    /// Kernel ids are not dense and stable.
    KernelIdsNotDense,
    /// Processor instance ids or kernel references are invalid.
    ProcessorLayoutInvalid,
}

impl fmt::Display for RuntimeGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RouteOutOfBounds => formatter.write_str("input route is out of bounds"),
            Self::EffectRouteOutOfBounds => formatter.write_str("effect route is out of bounds"),
            Self::InputLayoutMismatch => formatter.write_str("input route offsets do not match the arena layout"),
            Self::ScheduleLayoutMismatch => formatter.write_str("schedule layout is inconsistent"),
            Self::StateSlotOutOfBounds => formatter.write_str("state slot is out of bounds"),
            Self::InputSlotOutOfBounds => formatter.write_str("input slot is out of bounds"),
            Self::DuplicateStateKey(key) => write!(formatter, "duplicate stable state key {key}"),
            Self::KernelIdsNotDense => formatter.write_str("processor kernel ids are not dense"),
            Self::ProcessorLayoutInvalid => formatter.write_str("processor instance layout is invalid"),
        }
    }
}

impl std::error::Error for RuntimeGenerationError {}

fn validate_dense_ids(
    kernels: &[CompiledProcessorKernel],
    instances: &[ProcessorInstanceLayout],
) -> Result<(), RuntimeGenerationError> {
    if kernels
        .iter()
        .enumerate()
        .any(|(index, kernel)| kernel.id.index() != index)
    {
        return Err(RuntimeGenerationError::KernelIdsNotDense);
    }
    if instances
        .iter()
        .enumerate()
        .any(|(index, instance)| instance.id.index() != index || instance.kernel.index() >= kernels.len())
    {
        return Err(RuntimeGenerationError::ProcessorLayoutInvalid);
    }
    Ok(())
}
