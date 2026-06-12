use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

use golden_alchemist::SurfaceItemKind;

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
    Action,
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
            SurfaceItemKind::Action => Self::Action,
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

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ProcessorUiDto {
    pub id: String,
    pub label: String,
    pub active: bool,
    pub formula_id: String,
    pub formula_label: String,
    pub surface: Vec<FormulaSurfaceSectionDto>,
    pub diagnostic_ids: Vec<String>,
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

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct StateMachineProtocolBundle {
    pub statechart_deltas: Vec<StatechartDeltaDto>,
    pub processors: Vec<ProcessorUiDto>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub runtime_debug: Vec<RuntimeDebugDeltaDto>,
}

pub fn export_typescript(output_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    const INDEX: &str = "\
export type { DiagnosticDto } from './DiagnosticDto';\n\
export type { DiagnosticSeverityDto } from './DiagnosticSeverityDto';\n\
export type { FormulaSurfaceItemDto } from './FormulaSurfaceItemDto';\n\
export type { FormulaSurfaceItemKindDto } from './FormulaSurfaceItemKindDto';\n\
export type { FormulaSurfaceSectionDto } from './FormulaSurfaceSectionDto';\n\
export type { ProcessorUiDto } from './ProcessorUiDto';\n\
export type { RuntimeDebugDeltaDto } from './RuntimeDebugDeltaDto';\n\
export type { StatechartDeltaDto } from './StatechartDeltaDto';\n\
export type { StateMachineProtocolBundle } from './StateMachineProtocolBundle';\n\
export type { StateUiKind } from './StateUiKind';\n\
export type { StateUiLayoutDto } from './StateUiLayoutDto';\n\
export type { StateUiNodeDto } from './StateUiNodeDto';\n";
    let output_dir = output_dir.as_ref();
    let config = Config::new().with_out_dir(output_dir.to_path_buf());
    StateMachineProtocolBundle::export_all(&config)?;
    std::fs::write(output_dir.join("index.ts"), INDEX)?;
    Ok(())
}
