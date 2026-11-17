use golden_core::node::NodeUuid;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MatrixPresetEdit {
    InspectMembers {
        preset: NodeUuid,
        after: Option<NodeUuid>,
        count: u16,
    },
    Capture {
        label: Option<String>,
    },
    Update {
        preset: NodeUuid,
    },
    Recall {
        preset: NodeUuid,
    },
    Rename {
        preset: NodeUuid,
        label: String,
    },
    Delete {
        preset: NodeUuid,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixPresetSummary {
    pub id: NodeUuid,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixPresetMembers {
    pub preset: NodeUuid,
    pub members: Vec<MatrixPresetSummary>,
    pub next: Option<NodeUuid>,
}
