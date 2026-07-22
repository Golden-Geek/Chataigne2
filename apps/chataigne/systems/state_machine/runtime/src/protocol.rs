use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

use chataigne_alchemist::{
    ContextKey, ManagedItemInstance, ManagedItemUiState, ManagedRegionDefinition, ManagedRegionInstance,
    ManagedRegionKind, ManagedSocketRef, OutputPreviewStatus, SurfaceItemKind, ValueTypeSpec,
};
use golden_values::Value as RuntimeValue;

use crate::{ANodeOutputPreviewSample, ProcessorFormulaSourceKind, ProcessorUiModel};

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct StateUiLayoutDto {
    pub position: [f64; 2],
    pub size: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum StateUiKind {
    Leaf,
    Composite,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct StateUiNodeDto {
    pub id: String,
    pub label: String,
    pub active: bool,
    pub kind: StateUiKind,
    pub layout: StateUiLayoutDto,
    pub child_region_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum StatechartDeltaDto {
    Upsert { state: StateUiNodeDto },
    Remove { state_id: String },
    ActiveChanged { state_id: String, active: bool },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum FormulaSurfaceItemKindDto {
    Parameter,
    Condition,
    Consequence,
    Input,
    Filter,
    Output,
    Command,
}

impl From<SurfaceItemKind> for FormulaSurfaceItemKindDto {
    fn from(value: SurfaceItemKind) -> Self {
        match value {
            SurfaceItemKind::Parameter => Self::Parameter,
            SurfaceItemKind::Condition => Self::Condition,
            SurfaceItemKind::Consequence => Self::Consequence,
            SurfaceItemKind::Input => Self::Input,
            SurfaceItemKind::Filter => Self::Filter,
            SurfaceItemKind::Output => Self::Output,
            SurfaceItemKind::Command => Self::Command,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct FormulaSurfaceItemDto {
    pub id: String,
    pub label: String,
    pub path: Vec<String>,
    pub kind: FormulaSurfaceItemKindDto,
    pub value_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct FormulaSurfaceSectionDto {
    pub id: String,
    pub label: String,
    pub items: Vec<FormulaSurfaceItemDto>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ManagedRegionKindDto {
    InputSet,
    FilterPipeline,
    OutputSet,
    TriggerInput,
    CommandSet,
}

impl From<ManagedRegionKind> for ManagedRegionKindDto {
    fn from(value: ManagedRegionKind) -> Self {
        match value {
            ManagedRegionKind::InputSet => Self::InputSet,
            ManagedRegionKind::FilterPipeline => Self::FilterPipeline,
            ManagedRegionKind::OutputSet => Self::OutputSet,
            ManagedRegionKind::TriggerInput => Self::TriggerInput,
            ManagedRegionKind::CommandSet => Self::CommandSet,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ManagedSocketRefDto {
    pub node_id: String,
    pub socket_id: String,
}

impl From<&ManagedSocketRef> for ManagedSocketRefDto {
    fn from(value: &ManagedSocketRef) -> Self {
        Self {
            node_id: value.node.to_string(),
            socket_id: value.socket.to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ManagedRegionDefinitionDto {
    pub id: String,
    pub kind: ManagedRegionKindDto,
    pub label: String,
    pub input_socket: Option<ManagedSocketRefDto>,
    pub output_socket: Option<ManagedSocketRefDto>,
    pub accepted_roles: Vec<FormulaSurfaceItemKindDto>,
}

impl From<&ManagedRegionDefinition> for ManagedRegionDefinitionDto {
    fn from(value: &ManagedRegionDefinition) -> Self {
        Self {
            id: value.id.to_string(),
            kind: value.kind.into(),
            label: value.label.clone(),
            input_socket: value.input_socket.as_ref().map(ManagedSocketRefDto::from),
            output_socket: value.output_socket.as_ref().map(ManagedSocketRefDto::from),
            accepted_roles: value
                .accepted_roles
                .iter()
                .copied()
                .map(FormulaSurfaceItemKindDto::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ManagedItemUiStateDto {
    pub collapsed: bool,
}

impl From<&ManagedItemUiState> for ManagedItemUiStateDto {
    fn from(value: &ManagedItemUiState) -> Self {
        Self {
            collapsed: value.collapsed,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ManagedItemDto {
    pub id: String,
    pub anode_id: String,
    pub anode_type_id: String,
    pub label: String,
    pub enabled: bool,
    pub anode_enabled: bool,
    pub ui_state: ManagedItemUiStateDto,
}

impl From<&ManagedItemInstance> for ManagedItemDto {
    fn from(value: &ManagedItemInstance) -> Self {
        Self {
            id: value.id.to_string(),
            anode_id: value.anode.id.to_string(),
            anode_type_id: value.anode.type_id.to_string(),
            label: value.anode.label.clone(),
            enabled: value.enabled,
            anode_enabled: value.anode.enabled,
            ui_state: ManagedItemUiStateDto::from(&value.ui_state),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ManagedRegionInstanceDto {
    pub region_id: String,
    pub items: Vec<ManagedItemDto>,
}

impl From<&ManagedRegionInstance> for ManagedRegionInstanceDto {
    fn from(value: &ManagedRegionInstance) -> Self {
        Self {
            region_id: value.region_id.to_string(),
            items: value.items.iter().map(ManagedItemDto::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProcessorFormulaSourceKindDto {
    Project,
    Builtin,
}

impl From<ProcessorFormulaSourceKind> for ProcessorFormulaSourceKindDto {
    fn from(value: ProcessorFormulaSourceKind) -> Self {
        match value {
            ProcessorFormulaSourceKind::Project => Self::Project,
            ProcessorFormulaSourceKind::Builtin => Self::Builtin,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ProcessorUiDto {
    pub id: String,
    pub label: String,
    pub active: bool,
    pub formula_id: String,
    pub formula_label: String,
    pub formula_source_key: Option<String>,
    pub formula_source_kind: ProcessorFormulaSourceKindDto,
    pub formula_open_readonly_from_processor: bool,
    pub formula_can_duplicate_to_library: bool,
    pub surface: Vec<FormulaSurfaceSectionDto>,
    pub managed_regions: Vec<ManagedRegionDefinitionDto>,
    pub managed_region_instances: Vec<ManagedRegionInstanceDto>,
    pub diagnostic_ids: Vec<String>,
    pub multiplex_lane_count: usize,
}

impl From<&ProcessorUiModel> for ProcessorUiDto {
    fn from(value: &ProcessorUiModel) -> Self {
        Self {
            id: value.id.to_string(),
            label: value.label.clone(),
            active: value.active,
            formula_id: value.formula_id.clone(),
            formula_label: value.formula_label.clone(),
            formula_source_key: value.formula_source_key.clone(),
            formula_source_kind: value.formula_source.source_kind.into(),
            formula_open_readonly_from_processor: value.formula_source.open_readonly_from_processor,
            formula_can_duplicate_to_library: value.formula_source.can_duplicate_to_library,
            surface: value
                .surface
                .sections
                .iter()
                .map(|section| FormulaSurfaceSectionDto {
                    id: section.id.to_string(),
                    label: section.label.clone(),
                    items: section
                        .items
                        .iter()
                        .map(|item| FormulaSurfaceItemDto {
                            id: item.id.to_string(),
                            label: item.label.clone(),
                            path: item.path.clone(),
                            kind: item.kind.into(),
                            value_type: item.value_type.as_ref().map(value_type_spec_label),
                        })
                        .collect(),
                })
                .collect(),
            managed_regions: value
                .surface
                .managed_regions
                .iter()
                .map(ManagedRegionDefinitionDto::from)
                .collect(),
            managed_region_instances: value
                .managed_region_instances
                .regions
                .values()
                .map(ManagedRegionInstanceDto::from)
                .collect(),
            diagnostic_ids: value
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.to_string())
                .collect(),
            multiplex_lane_count: 0,
        }
    }
}

fn value_type_spec_label(value: &ValueTypeSpec) -> String {
    match value {
        ValueTypeSpec::Exact(id) => id.to_string(),
        ValueTypeSpec::Facet(id) => id.to_string(),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum DiagnosticSeverityDto {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct DiagnosticDto {
    pub id: String,
    pub severity: DiagnosticSeverityDto,
    pub message: String,
    pub node_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct RuntimeDebugDeltaDto {
    pub logical_tick: u64,
    pub node_id: String,
    pub socket_id: Option<String>,
    pub value: String,
    pub execution_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ContextKeyPartDto {
    pub axis_id: String,
    pub axis_label: String,
    pub item_id: String,
    pub item_label: String,
    pub index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ContextKeyDto {
    pub parts: Vec<ContextKeyPartDto>,
}

impl From<&ContextKey> for ContextKeyDto {
    fn from(value: &ContextKey) -> Self {
        Self {
            parts: value
                .iter()
                .map(|part| ContextKeyPartDto {
                    axis_id: part.axis.as_str().to_owned(),
                    axis_label: part.axis.as_str().to_owned(),
                    item_id: part.item.as_str().to_owned(),
                    item_label: part.item.as_str().to_owned(),
                    index: None,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ProcessorLaneCatalogEntryDto {
    pub processor_id: String,
    pub context_key: Option<ContextKeyDto>,
    pub label: String,
    pub has_memory: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ProcessorLaneParameterPreviewDto {
    pub node_id: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ProcessorLaneConditionPreviewDto {
    pub node_id: String,
    pub valid: bool,
}

/// One processor's inexpensive state-machine canvas preview.
///
/// Unlike `ProcessorLaneInspectionDto`, this never carries resolved parameters or formula output
/// samples. The runtime keeps exactly one lane per processor so a state-machine surface can show
/// multiplexing and condition validity without enabling Alchemist debug capture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ProcessorRuntimeOverviewDto {
    pub processor_id: String,
    pub multiplex_lane_count: usize,
    pub preview_context_key: Option<ContextKeyDto>,
    pub preview_lane_label: String,
    pub condition_states: Vec<ProcessorLaneConditionPreviewDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ProcessorOverviewDemandDto {
    pub subscription_id: String,
    /// Processors whose expanded state cards intersect the settled state-machine viewport.
    /// An empty list releases this subscription immediately.
    pub processor_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ProcessorOverviewLaneSelectionDto {
    pub processor_id: String,
    pub context_key: Option<ContextKeyDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct StateMachineProcessorOverviewDto {
    /// The processors in one UUID-prefix shard. Sharding keeps a changing processor from
    /// retransmitting every processor's overview in very large state machines.
    pub processors: Vec<ProcessorRuntimeOverviewDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ProcessorLaneInspectionDto {
    pub processor_id: String,
    pub context_key: Option<ContextKeyDto>,
    pub parameter_values: Vec<ProcessorLaneParameterPreviewDto>,
    pub condition_states: Vec<ProcessorLaneConditionPreviewDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum FormulaPreviewModeDto {
    FormulaDefaults {
        formula_id: String,
    },
    ProcessorDefaultLane {
        processor_id: String,
    },
    ProcessorLane {
        processor_id: String,
        context_key: ContextKeyDto,
    },
}

/// One leased formula-preview observation requested by a UI surface.
///
/// Sending the same `subscription_id` refreshes or replaces the request. A `None` mode releases
/// it immediately; runtimes also expire requests that stop receiving heartbeats.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct FormulaPreviewDemandDto {
    pub subscription_id: String,
    pub mode: Option<FormulaPreviewModeDto>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum OutputPreviewStatusDto {
    Live,
    DefaultPreview,
    Stale,
    Error,
    Suppressed,
    Unavailable,
}

impl From<OutputPreviewStatus> for OutputPreviewStatusDto {
    fn from(value: OutputPreviewStatus) -> Self {
        match value {
            OutputPreviewStatus::Live => Self::Live,
            OutputPreviewStatus::DefaultPreview => Self::DefaultPreview,
            OutputPreviewStatus::Stale => Self::Stale,
            OutputPreviewStatus::Error => Self::Error,
            OutputPreviewStatus::Suppressed => Self::Suppressed,
            OutputPreviewStatus::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeValueDto {
    Unit,
    Bool {
        value: bool,
    },
    Trigger {
        fired: bool,
        edge_id: u64,
        logical_tick: u64,
    },
    Int {
        value: i64,
    },
    Float {
        value: f64,
    },
    String {
        value: String,
    },
    Vec2 {
        value: [f64; 2],
    },
    Vec3 {
        value: [f64; 3],
    },
    Color {
        red: f64,
        green: f64,
        blue: f64,
        alpha: f64,
    },
    Duration {
        seconds: f64,
    },
    Array {
        values: Vec<RuntimeValueDto>,
    },
    Ref {
        value_type: String,
        stable_id: String,
    },
    Extension {
        value_type: String,
        payload: Vec<u8>,
    },
}

impl From<&RuntimeValue> for RuntimeValueDto {
    fn from(value: &RuntimeValue) -> Self {
        match value {
            RuntimeValue::Unit => Self::Unit,
            RuntimeValue::Bool(value) => Self::Bool { value: *value },
            RuntimeValue::Trigger(value) => Self::Trigger {
                fired: value.fired,
                edge_id: value.edge_id,
                logical_tick: value.logical_tick,
            },
            RuntimeValue::Int(value) => Self::Int { value: *value },
            RuntimeValue::Float(value) => Self::Float { value: *value },
            RuntimeValue::String(value) => Self::String {
                value: value.to_string(),
            },
            RuntimeValue::Vec2(value) => Self::Vec2 { value: *value },
            RuntimeValue::Vec3(value) => Self::Vec3 { value: *value },
            RuntimeValue::Color(value) => Self::Color {
                red: value.red,
                green: value.green,
                blue: value.blue,
                alpha: value.alpha,
            },
            RuntimeValue::Duration(value) => Self::Duration {
                seconds: value.as_secs_f64(),
            },
            RuntimeValue::Array(values) => Self::Array {
                values: values.iter().map(Self::from).collect(),
            },
            RuntimeValue::Ref(value) => Self::Ref {
                value_type: value.value_type.to_string(),
                stable_id: value.stable_id.to_string(),
            },
            RuntimeValue::Extension(value) => Self::Extension {
                value_type: value.value_type.to_string(),
                payload: value.payload.to_vec(),
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ANodeOutputPreviewSampleDto {
    pub formula_id: String,
    pub processor_id: Option<String>,
    pub context_key: Option<ContextKeyDto>,
    pub node_id: String,
    pub exec_node_id: String,
    pub output_socket_id: String,
    pub value_type: String,
    pub value: RuntimeValueDto,
    pub logical_tick: u64,
    pub status: OutputPreviewStatusDto,
}

impl From<&ANodeOutputPreviewSample> for ANodeOutputPreviewSampleDto {
    fn from(value: &ANodeOutputPreviewSample) -> Self {
        Self {
            formula_id: value.formula_id.to_string(),
            processor_id: value.processor_id.map(|id| id.to_string()),
            context_key: value.context_key.as_ref().map(ContextKeyDto::from),
            node_id: value.author_node_id.to_string(),
            exec_node_id: value.exec_node.to_string(),
            output_socket_id: value.output_socket.to_string(),
            value_type: value.value_type.to_string(),
            value: RuntimeValueDto::from(&value.value),
            logical_tick: value.logical_tick,
            status: OutputPreviewStatusDto::from(value.status),
        }
    }
}

/// Stable discovery data for the processors currently observed by preview clients.
///
/// This payload is retained independently from live samples so a high-frequency preview never
/// needs to resend every multiplex lane. A new value is published only when demand or the
/// processor/context topology changes.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct StateMachinePreviewCatalogDto {
    pub processors: Vec<ProcessorUiDto>,
    pub processor_lanes: Vec<ProcessorLaneCatalogEntryDto>,
}

/// High-frequency data for the explicitly observed processor lanes or formula defaults.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct StateMachineRuntimePreviewDto {
    pub processor_lane_inspections: Vec<ProcessorLaneInspectionDto>,
    pub output_preview: Vec<ANodeOutputPreviewSampleDto>,
}

pub fn export_typescript(output_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    const INDEX: &str = "\
export type { ANodeOutputPreviewSampleDto } from './ANodeOutputPreviewSampleDto';\n\
export type { ContextKeyDto } from './ContextKeyDto';\n\
export type { ContextKeyPartDto } from './ContextKeyPartDto';\n\
export type { DiagnosticDto } from './DiagnosticDto';\n\
export type { DiagnosticSeverityDto } from './DiagnosticSeverityDto';\n\
export type { FormulaPreviewDemandDto } from './FormulaPreviewDemandDto';\n\
export type { FormulaPreviewModeDto } from './FormulaPreviewModeDto';\n\
export type { FormulaSurfaceItemDto } from './FormulaSurfaceItemDto';\n\
export type { FormulaSurfaceItemKindDto } from './FormulaSurfaceItemKindDto';\n\
export type { FormulaSurfaceSectionDto } from './FormulaSurfaceSectionDto';\n\
export type { ManagedItemDto } from './ManagedItemDto';\n\
export type { ManagedItemUiStateDto } from './ManagedItemUiStateDto';\n\
export type { ManagedRegionDefinitionDto } from './ManagedRegionDefinitionDto';\n\
export type { ManagedRegionInstanceDto } from './ManagedRegionInstanceDto';\n\
export type { ManagedRegionKindDto } from './ManagedRegionKindDto';\n\
export type { ManagedSocketRefDto } from './ManagedSocketRefDto';\n\
export type { OutputPreviewStatusDto } from './OutputPreviewStatusDto';\n\
export type { ProcessorOverviewDemandDto } from './ProcessorOverviewDemandDto';\n\
export type { ProcessorOverviewLaneSelectionDto } from './ProcessorOverviewLaneSelectionDto';\n\
export type { ProcessorLaneCatalogEntryDto } from './ProcessorLaneCatalogEntryDto';\n\
export type { ProcessorLaneInspectionDto } from './ProcessorLaneInspectionDto';\n\
export type { ProcessorLaneParameterPreviewDto } from './ProcessorLaneParameterPreviewDto';\n\
export type { ProcessorLaneConditionPreviewDto } from './ProcessorLaneConditionPreviewDto';\n\
export type { ProcessorRuntimeOverviewDto } from './ProcessorRuntimeOverviewDto';\n\
export type { ProcessorUiDto } from './ProcessorUiDto';\n\
export type { RuntimeDebugDeltaDto } from './RuntimeDebugDeltaDto';\n\
export type { RuntimeValueDto } from './RuntimeValueDto';\n\
export type { StatechartDeltaDto } from './StatechartDeltaDto';\n\
export type { StateMachinePreviewCatalogDto } from './StateMachinePreviewCatalogDto';\n\
export type { StateMachineProcessorOverviewDto } from './StateMachineProcessorOverviewDto';\n\
export type { StateMachineRuntimePreviewDto } from './StateMachineRuntimePreviewDto';\n\
export type { StateUiKind } from './StateUiKind';\n\
export type { StateUiLayoutDto } from './StateUiLayoutDto';\n\
export type { StateUiNodeDto } from './StateUiNodeDto';\n";
    let output_dir = output_dir.as_ref();
    let config = Config::new().with_out_dir(output_dir.to_path_buf());
    StateMachinePreviewCatalogDto::export_all(&config)?;
    StateMachineProcessorOverviewDto::export_all(&config)?;
    StateMachineRuntimePreviewDto::export_all(&config)?;
    ProcessorOverviewDemandDto::export_all(&config)?;
    ProcessorOverviewLaneSelectionDto::export_all(&config)?;
    FormulaPreviewDemandDto::export_all(&config)?;
    StatechartDeltaDto::export_all(&config)?;
    DiagnosticDto::export_all(&config)?;
    RuntimeDebugDeltaDto::export_all(&config)?;
    std::fs::write(output_dir.join("index.ts"), INDEX)?;
    Ok(())
}
