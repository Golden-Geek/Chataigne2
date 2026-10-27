use super::ProcessTreeNodeSnapshot;
use crate::node::{NodeId, NodeUuid};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use golden_io::{PayloadPool, RetainedPayload};

use super::ProcessTreeSnapshot;
use super::snapshot_map::SnapshotLayer;

const SNAPSHOT_ITEMS: usize = 384;
const SNAPSHOT_BYTES: usize = 1024 * 1024 * 1024;

/// Failure to reserve a background snapshot's complete retained allocation budget.
#[derive(Debug)]
pub enum SnapshotLeaseError {
    /// The bounded disposer thread could not start; later calls may retry.
    WorkerStart(std::io::Error),
    /// The snapshot could not reserve an item and its complete allocation weight.
    Capacity {
        /// Conservative bytes required by this immutable view.
        requested_bytes: usize,
    },
}

impl std::fmt::Display for SnapshotLeaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WorkerStart(error) => write!(f, "snapshot disposer could not start: {error}"),
            Self::Capacity { requested_bytes } => write!(
                f,
                "snapshot lease of {requested_bytes} bytes was rejected by the 384-allocation / 1 GiB reservation budget"
            ),
        }
    }
}

impl std::error::Error for SnapshotLeaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WorkerStart(error) => Some(error),
            Self::Capacity { .. } => None,
        }
    }
}

// Guard the actual shared allocations as well as the outer view. A non-leased clone can
// keep these Arcs alive, but can never become the final owner of already leased storage.
type LayerLeases<K, V> = Vec<RetainedPayload<SnapshotLayer<K, V>>>;

#[derive(Debug)]
pub(super) struct StorageLeases {
    _nodes: LayerLeases<NodeId, Box<ProcessTreeNodeSnapshot>>,
    _overlays: RetainedPayload<HashMap<NodeId, ProcessTreeNodeSnapshot>>,
    _uuids: LayerLeases<NodeUuid, NodeId>,
    _child_orders: LayerLeases<NodeId, Box<[NodeId]>>,
    _child_declarations: LayerLeases<NodeId, HashMap<Box<str>, NodeId>>,
    _child_keys: LayerLeases<NodeId, HashMap<Box<str>, NodeId>>,
    _revisions: LayerLeases<NodeId, crate::engine::tree_revisions::NodeRevisions>,
}

impl ProcessTreeSnapshot {
    /// Reserves this view and its shared allocations for background consumption without
    /// cloning node records. One canonical 384-allocation / 1 GiB pool deduplicates identical
    /// Arcs, including storage shared by overlays. Ordinary engine/view clones keep their
    /// reservation alive; final storage destruction runs on the disposer after they release.
    pub fn try_lease(self: &Arc<Self>) -> Result<RetainedPayload<Self>, SnapshotLeaseError> {
        static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
        let mut pool = POOL.lock().unwrap_or_else(|p| p.into_inner());
        if pool.is_none() {
            *pool = Some(
                PayloadPool::new("golden-snapshot-disposer", SNAPSHOT_ITEMS, SNAPSHOT_BYTES)
                    .map_err(SnapshotLeaseError::WorkerStart)?,
            );
        }
        let pool = pool.as_ref().expect("initialized snapshot pool");
        let reject = || SnapshotLeaseError::Capacity {
            requested_bytes: self.retained_bytes(),
        };
        if self.storage_leases.get().is_none() {
            let leases = StorageLeases {
                _nodes: self.nodes.try_lease(pool).map_err(|_| reject())?,
                _overlays: pool
                    .try_retain_arc(self.node_overrides.clone(), self.weights.overlays)
                    .map_err(|_| reject())?,
                _uuids: self.node_ids_by_uuid.try_lease(pool).map_err(|_| reject())?,
                _child_orders: self.child_indexes.ids_by_parent.try_lease(pool).map_err(|_| reject())?,
                _child_declarations: self
                    .child_indexes
                    .ids_by_decl_id
                    .try_lease(pool)
                    .map_err(|_| reject())?,
                _child_keys: self
                    .child_indexes
                    .ids_by_child_key
                    .try_lease(pool)
                    .map_err(|_| reject())?,
                _revisions: self.revisions.try_lease(pool).map_err(|_| reject())?,
            };
            let _ = self.storage_leases.set(leases);
        }
        // Component storage has its own deduplicated reservation; the view itself owns
        // only fixed-size links, cached weights, and retained handles.
        let own = std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>() + self.lease_storage_bytes();
        pool.try_retain_arc(self.clone(), own).map_err(|_| reject())
    }
}
