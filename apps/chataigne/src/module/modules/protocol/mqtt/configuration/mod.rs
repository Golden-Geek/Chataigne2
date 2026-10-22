use super::transport::MqttTransportConfig;
use golden_core::{
    node::{NodeId, NodeUuid},
    process_ctx::{ProcessTreeSnapshot, SubtreeRevision, SubtreeStructureRevision},
};
use golden_io::RetainedPayload;

mod collect;
mod preparation;
mod service;
pub(super) use preparation::Preparation;
#[cfg(test)]
mod tests;

const SCALARS: [&str; 5] = [
    "remote_host",
    "remote_port",
    "client_id",
    "clean_session",
    "keep_alive_secs",
];
const MAX_SCAN: usize = 8192;

/// Configuration inputs exclude Connection/Connected and all Values traffic.
/// Structural identity still fences replaced or formerly missing input paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Key {
    module: NodeId,
    uuid: NodeUuid,
    enabled: bool,
    structure: Option<SubtreeStructureRevision>,
    authored: [Option<(NodeId, SubtreeRevision)>; 7],
}
impl Key {
    pub(super) fn capture(snapshot: &ProcessTreeSnapshot, module: NodeId) -> Option<Self> {
        let node = snapshot.node(module)?;
        let connection = snapshot.find_child_by_decl_id(module, "connection");
        let mut authored = [None; 7];
        if let Some(connection) = connection {
            for (index, name) in SCALARS.into_iter().chain(["credentials", "subscriptions"]).enumerate() {
                authored[index] = snapshot
                    .find_child_by_decl_id(connection, name)
                    .and_then(|id| snapshot.subtree_revision(id).map(|revision| (id, revision)));
            }
        }
        Some(Self {
            module,
            uuid: node.uuid,
            enabled: node.enabled,
            structure: connection.and_then(|id| snapshot.subtree_structure_revision(id)),
            authored,
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct Error {
    pub(super) message: String,
    pub(super) retryable: bool,
}
impl Error {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }
}

struct Ready {
    config: RetainedPayload<MqttTransportConfig>,
    unchanged: bool,
}
