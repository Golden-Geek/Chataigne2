use golden_core::node::NodeUuid;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct TargetPickerQuery {
    #[ts(type = "string")]
    pub parameter: NodeUuid,
    #[ts(type = "string | null")]
    pub folder: Option<NodeUuid>,
    #[ts(type = "string | null")]
    pub cursor: Option<NodeUuid>,
    pub backwards: bool,
    /// Zero reads only the current binding; pages admit at most64 choices/256 siblings.
    pub limit: u16,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub enum TargetPickerKind {
    Media,
    Channel,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct TargetPickerEntry {
    #[ts(type = "string")]
    pub id: NodeUuid,
    pub label: String,
    pub folder: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct TargetPickerCurrent {
    #[ts(type = "string")]
    pub id: NodeUuid,
    pub label: String,
    pub present: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct TargetPickerPage {
    pub kind: TargetPickerKind,
    pub location: String,
    #[ts(type = "string | null")]
    pub folder: Option<NodeUuid>,
    #[ts(type = "string | null")]
    pub parent: Option<NodeUuid>,
    #[ts(type = "string | null")]
    pub before: Option<NodeUuid>,
    #[ts(type = "string | null")]
    pub after: Option<NodeUuid>,
    pub current: Option<TargetPickerCurrent>,
    pub empty_label: String,
    pub entries: Vec<TargetPickerEntry>,
}
