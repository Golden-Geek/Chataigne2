use chataigne_alchemist::{
    ANodeId, ContextKey, DebugCaptureMode, DebugValueSample, Diagnostic, FormulaId, FormulaSurface,
    ManagedRegionInstances, OutputPreviewStatus, SocketId, ValueTypeId,
};
use golden_values::Value as RuntimeValue;
use indexmap::IndexSet;

use super::{ProcessorId, ProcessorLaneOutput};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProcessorDebugCapture {
    #[default]
    Off,
    All {
        history_len: usize,
    },
    ProcessorLane {
        context_key: Option<ContextKey>,
        history_len: usize,
    },
    ProcessorLanes {
        context_keys: IndexSet<ContextKey>,
        history_len: usize,
    },
    SelectedNodes {
        context_key: Option<ContextKey>,
        nodes: IndexSet<ANodeId>,
        history_len: usize,
    },
}

impl ProcessorDebugCapture {
    pub(super) fn debug_capture_mode(&self, formula_id: &FormulaId, context_key: &ContextKey) -> DebugCaptureMode {
        match self {
            Self::Off => DebugCaptureMode::Off,
            Self::All { history_len } => DebugCaptureMode::All {
                history_len: *history_len,
            },
            Self::ProcessorLane {
                context_key: requested,
                history_len,
            } if requested
                .as_ref()
                .map_or_else(|| context_key.is_default_lane(), |requested| requested == context_key) =>
            {
                DebugCaptureMode::ProcessorLane {
                    formula_id: formula_id.clone(),
                    context_key: (!context_key.is_default_lane()).then(|| context_key.clone()),
                    history_len: *history_len,
                }
            }
            Self::ProcessorLanes {
                context_keys,
                history_len,
            } if context_keys.contains(context_key) => DebugCaptureMode::ProcessorLane {
                formula_id: formula_id.clone(),
                context_key: (!context_key.is_default_lane()).then(|| context_key.clone()),
                history_len: *history_len,
            },
            Self::SelectedNodes {
                context_key: requested,
                nodes,
                history_len,
            } if requested
                .as_ref()
                .map_or_else(|| context_key.is_default_lane(), |requested| requested == context_key) =>
            {
                DebugCaptureMode::SelectedNodes {
                    formula_id: Some(formula_id.clone()),
                    context_key: (!context_key.is_default_lane()).then(|| context_key.clone()),
                    nodes: nodes.clone(),
                    history_len: *history_len,
                }
            }
            Self::ProcessorLane { .. } | Self::ProcessorLanes { .. } | Self::SelectedNodes { .. } => {
                DebugCaptureMode::Off
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ANodeOutputPreviewSample {
    pub formula_id: FormulaId,
    pub processor_id: Option<ProcessorId>,
    pub context_key: Option<ContextKey>,
    pub author_node_id: ANodeId,
    pub exec_node: chataigne_alchemist::ExecNodeId,
    pub output_socket: SocketId,
    pub value_type: ValueTypeId,
    pub value: RuntimeValue,
    pub logical_tick: u64,
    pub status: OutputPreviewStatus,
}

impl ANodeOutputPreviewSample {
    fn from_debug_sample(
        processor_id: Option<ProcessorId>,
        fallback_formula_id: &FormulaId,
        sample: DebugValueSample,
    ) -> Self {
        Self {
            formula_id: sample.formula_id.unwrap_or_else(|| fallback_formula_id.clone()),
            processor_id,
            context_key: sample.context_key,
            author_node_id: sample.author_node_id,
            exec_node: sample.exec_node,
            output_socket: sample.output_socket,
            value_type: sample.value_type,
            value: sample.value,
            logical_tick: sample.logical_tick,
            status: sample.status,
        }
    }
}

pub fn processor_output_preview_samples(
    processor_id: ProcessorId,
    formula_id: &FormulaId,
    lanes: Vec<ProcessorLaneOutput>,
) -> Vec<ANodeOutputPreviewSample> {
    lanes
        .into_iter()
        .flat_map(|lane| {
            lane.output
                .debug_samples
                .into_iter()
                .map(move |sample| ANodeOutputPreviewSample::from_debug_sample(Some(processor_id), formula_id, sample))
        })
        .collect()
}

pub fn processor_output_preview_samples_from_lanes(
    processor_id: ProcessorId,
    formula_id: &FormulaId,
    lanes: &[ProcessorLaneOutput],
) -> Vec<ANodeOutputPreviewSample> {
    lanes
        .iter()
        .flat_map(|lane| {
            lane.output
                .debug_samples
                .iter()
                .cloned()
                .map(move |sample| ANodeOutputPreviewSample::from_debug_sample(Some(processor_id), formula_id, sample))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct ProcessorUiModel {
    pub id: ProcessorId,
    pub label: String,
    pub active: bool,
    pub formula_id: String,
    pub formula_label: String,
    pub formula_source_key: Option<String>,
    pub surface: FormulaSurface,
    pub managed_region_instances: ManagedRegionInstances,
    pub diagnostics: Vec<Diagnostic>,
    pub formula_source: ProcessorFormulaUiState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessorFormulaSourceKind {
    Project,
    Builtin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessorFormulaUiState {
    pub source_kind: ProcessorFormulaSourceKind,
    pub open_readonly_from_processor: bool,
    pub can_duplicate_to_library: bool,
}

impl ProcessorFormulaUiState {
    #[must_use]
    pub fn project() -> Self {
        Self {
            source_kind: ProcessorFormulaSourceKind::Project,
            open_readonly_from_processor: false,
            can_duplicate_to_library: false,
        }
    }

    #[must_use]
    pub fn builtin(open_readonly_from_processor: bool, can_duplicate_to_library: bool) -> Self {
        Self {
            source_kind: ProcessorFormulaSourceKind::Builtin,
            open_readonly_from_processor,
            can_duplicate_to_library,
        }
    }
}

impl Default for ProcessorFormulaUiState {
    fn default() -> Self {
        Self::project()
    }
}
