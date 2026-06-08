use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct StateUiLayoutDto {
    pub x: f64,
    pub y: f64,
    pub width: Option<f64>,
    pub height: Option<f64>,
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

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ExposedDeclDto {
    pub id: String,
    pub label: String,
    pub value_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ProcessorUiDto {
    pub id: String,
    pub label: String,
    pub active: bool,
    pub exposed: Vec<ExposedDeclDto>,
    pub diagnostic_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct AlchemistSocketDto {
    pub id: String,
    pub label: String,
    pub value_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct AlchemistNodeDto {
    pub id: String,
    pub type_id: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub inputs: Vec<AlchemistSocketDto>,
    pub outputs: Vec<AlchemistSocketDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct AlchemistEdgeDto {
    pub from_node: String,
    pub from_socket: String,
    pub to_node: String,
    pub to_socket: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct SocketCompatibilityDto {
    pub node_id: String,
    pub socket_id: String,
    pub compatible: bool,
    pub explanation: Option<String>,
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
    pub graph_nodes: Vec<AlchemistNodeDto>,
    pub graph_edges: Vec<AlchemistEdgeDto>,
    pub socket_compatibility: Vec<SocketCompatibilityDto>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub runtime_debug: Vec<RuntimeDebugDeltaDto>,
}

pub fn export_typescript(output_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    const INDEX: &str = "\
export type { AlchemistEdgeDto } from './AlchemistEdgeDto';\n\
export type { AlchemistNodeDto } from './AlchemistNodeDto';\n\
export type { AlchemistSocketDto } from './AlchemistSocketDto';\n\
export type { DiagnosticDto } from './DiagnosticDto';\n\
export type { DiagnosticSeverityDto } from './DiagnosticSeverityDto';\n\
export type { ExposedDeclDto } from './ExposedDeclDto';\n\
export type { ProcessorUiDto } from './ProcessorUiDto';\n\
export type { RuntimeDebugDeltaDto } from './RuntimeDebugDeltaDto';\n\
export type { SocketCompatibilityDto } from './SocketCompatibilityDto';\n\
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
