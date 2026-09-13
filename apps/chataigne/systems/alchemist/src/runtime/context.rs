use super::*;

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

#[derive(Clone, Debug)]
pub struct RuntimeInputSnapshot {
    shared_values: Arc<HashMap<StableRef, RuntimeValue>>,
    values: HashMap<StableRef, RuntimeValue>,
    context_values: HashMap<StableRef, ContextualRuntimeInputValues>,
}

#[derive(Clone, Debug)]
struct ContextualRuntimeInputValues {
    axes: AxisSet,
    values: HashMap<ContextKey, RuntimeValue>,
}

impl Default for RuntimeInputSnapshot {
    fn default() -> Self {
        Self {
            shared_values: Arc::new(HashMap::new()),
            values: HashMap::new(),
            context_values: HashMap::new(),
        }
    }
}

impl RuntimeInputSnapshot {
    /// Creates a snapshot with a shared immutable value base and an empty local overlay.
    ///
    /// Per-evaluation inputs added through [`Self::insert`] stay local to this snapshot, so many
    /// processor lanes can reuse a large common input set without cloning it.
    #[must_use]
    pub fn with_shared_values(shared_values: Arc<HashMap<StableRef, RuntimeValue>>) -> Self {
        Self {
            shared_values,
            values: HashMap::new(),
            context_values: HashMap::new(),
        }
    }

    pub fn insert(&mut self, reference: StableRef, value: RuntimeValue) -> Option<RuntimeValue> {
        let shared_value = self.shared_values.get(&reference).cloned();
        self.values.insert(reference, value).or(shared_value)
    }

    #[must_use]
    pub fn get(&self, reference: &StableRef) -> Option<&RuntimeValue> {
        self.values.get(reference).or_else(|| self.shared_values.get(reference))
    }

    pub fn insert_context(
        &mut self,
        reference: StableRef,
        axes: &AxisSet,
        context_key: ContextKey,
        value: RuntimeValue,
    ) -> Option<RuntimeValue> {
        let contextual_values = self
            .context_values
            .entry(reference)
            .or_insert_with(|| ContextualRuntimeInputValues {
                axes: axes.clone(),
                values: HashMap::new(),
            });
        if contextual_values.axes != *axes {
            debug_assert!(
                contextual_values.values.is_empty(),
                "one runtime input source must use one stable context-axis projection"
            );
            contextual_values.axes.clone_from(axes);
            contextual_values.values.clear();
        }
        let context_key = if context_key.iter().map(|part| &part.axis).eq(axes.iter()) {
            context_key
        } else {
            context_key_projected_in_axis_order(&context_key, axes)
        };
        contextual_values.values.insert(context_key, value)
    }

    #[must_use]
    pub fn get_context(&self, reference: &StableRef, context_key: &ContextKey) -> Option<&RuntimeValue> {
        let contextual_values = self.context_values.get(reference)?;
        if let Some(value) = contextual_values.values.get(context_key) {
            return Some(value);
        }
        let projected = context_key_projected_in_axis_order(context_key, &contextual_values.axes);
        contextual_values.values.get(&projected)
    }
}

fn context_key_projected_in_axis_order(context_key: &ContextKey, axes: &AxisSet) -> ContextKey {
    ContextKey::new(
        axes.iter()
            .filter_map(|axis| context_key.iter().find(|part| &part.axis == axis).cloned()),
    )
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DebugCaptureMode {
    #[default]
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

    pub(super) fn sample_status(&self) -> Option<OutputPreviewStatus> {
        match self {
            Self::Off => None,
            Self::All { .. } | Self::ProcessorLane { .. } | Self::SelectedNodes { .. } => {
                Some(OutputPreviewStatus::Live)
            }
            Self::FormulaDefaults { .. } => Some(OutputPreviewStatus::DefaultPreview),
        }
    }

    pub(super) fn formula_id(&self) -> Option<&FormulaId> {
        match self {
            Self::FormulaDefaults { formula_id, .. } | Self::ProcessorLane { formula_id, .. } => Some(formula_id),
            Self::SelectedNodes { formula_id, .. } => formula_id.as_ref(),
            Self::Off | Self::All { .. } => None,
        }
    }

    pub(super) fn accepts(&self, sample: &DebugValueSample) -> bool {
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
