use super::*;
use uuid::Uuid;

struct CacheEntry {
    snapshot: Arc<ProcessTreeSnapshot>,
    store_epoch: Uuid,
    revision: (Uuid, u64),
    root: NodeId,
}

/// Retains one canonical view across ticks. The existing retirement pool also owns
/// cache eviction; neither project replacement nor engine drop adds a cleanup queue.
pub(super) struct SnapshotCache {
    entry: Option<CacheEntry>,
    retirements: RetirementPool,
}

impl SnapshotCache {
    pub(super) fn new(retirements: RetirementPool) -> Self {
        Self {
            entry: None,
            retirements,
        }
    }

    fn replace(&mut self, entry: CacheEntry) {
        self.clear();
        self.entry = Some(entry);
    }

    fn clear(&mut self) {
        if let Some(entry) = self.entry.take() {
            retire(&self.retirements, entry.snapshot);
        }
    }
}

impl Drop for SnapshotCache {
    fn drop(&mut self) {
        self.clear();
    }
}

pub(super) fn retire(pool: &RetirementPool, snapshot: Arc<ProcessTreeSnapshot>) {
    const RETIRE_NODE_THRESHOLD: usize = 10_000;
    if snapshot.node_count() < RETIRE_NODE_THRESHOLD || Arc::strong_count(&snapshot) != 1 {
        return;
    }
    if let Err(error) = pool.try_retire("golden-process-snapshot-retire", snapshot, drop) {
        drop(error.into_value());
    }
}

impl<T: Node> Engine<T> {
    /// Unlike a tick-scoped read, this checks all mutable-store accesses since the
    /// last capture. Inbox dispatch must observe edits preceding its callbacks.
    #[track_caller]
    pub(crate) fn canonical_process_snapshot(&mut self) -> Arc<ProcessTreeSnapshot> {
        let revision = self.tree_revisions.version();
        let store_epoch = self.nodes.snapshot_epoch();
        let compatible = self.process_snapshot_cache.entry.as_ref().filter(|entry| {
            entry.store_epoch == store_epoch && entry.root == self.root && entry.revision.0 == revision.0
        });
        if let Some(entry) = compatible
            && entry.revision == revision
            && self.nodes.snapshot_changes().is_some_and(HashSet::is_empty)
        {
            return entry.snapshot.clone();
        }

        let started = Instant::now();
        let refreshed = compatible.and_then(|entry| {
            let changes = self.nodes.snapshot_changes()?;
            let updates = self.capture_snapshot_changes(&entry.snapshot, changes)?;
            let captured = updates.len();
            entry
                .snapshot
                .refreshed(updates, self.tree_revisions.snapshot())
                .map(|(snapshot, compacted)| (Arc::new(snapshot), captured, compacted))
        });
        let snapshot = if let Some((snapshot, captured, compacted)) = refreshed {
            self.tick_scratch.stats.snapshot_refreshes += 1;
            self.tick_scratch.stats.snapshot_nodes_cloned += captured + compacted;
            if self.nodes.len() >= 10_000 && *runtime::PERF_TRACE_ENABLED {
                eprintln!(
                    "[engine] process_snapshot_refresh nodes={} captured={captured} compacted={compacted} total_us={} caller={}",
                    self.nodes.len(),
                    started.elapsed().as_micros(),
                    std::panic::Location::caller()
                );
            }
            snapshot
        } else {
            self.tick_scratch.stats.snapshot_builds += 1;
            self.tick_scratch.stats.snapshot_nodes_cloned += self.nodes.len();
            self.build_process_tree_snapshot()
        };
        self.tick_scratch.stats.snapshot_build_ns += started.elapsed().as_nanos();
        self.nodes.acknowledge_snapshot();
        self.process_snapshot_cache.replace(CacheEntry {
            snapshot: snapshot.clone(),
            store_epoch,
            revision,
            root: self.root,
        });
        snapshot
    }

    pub(super) fn evict_invalid_process_snapshot(&mut self) {
        if self.process_snapshot_cache.entry.as_ref().is_some_and(|entry| {
            entry.store_epoch != self.nodes.snapshot_epoch()
                || entry.root != self.root
                || self.nodes.snapshot_changes().is_none()
        }) {
            self.process_snapshot_cache.clear();
        }
    }

    /// Count actual parent relations, matching full capture even if a raw caller
    /// changes parent pointers without repairing the sibling chain.
    fn capture_snapshot_changes(
        &self,
        before: &ProcessTreeSnapshot,
        changes: &HashSet<NodeId>,
    ) -> Option<HashMap<NodeId, Box<ProcessTreeNodeSnapshot>>> {
        let mut updates = HashMap::with_capacity(changes.len());
        let mut child_deltas: HashMap<NodeId, isize> = HashMap::new();
        for id in changes {
            let mut next = self.capture_process_node(*id)?;
            let prior = before.node(*id);
            next.child_count = prior.map_or(0, |node| node.child_count);
            let old_parent = prior.and_then(|node| node.parent);
            if next.parent != old_parent {
                if let Some(parent) = old_parent {
                    *child_deltas.entry(parent).or_default() -= 1;
                }
                if let Some(parent) = next.parent {
                    *child_deltas.entry(parent).or_default() += 1;
                }
            }
            updates.insert(*id, next);
        }
        for (parent, delta) in child_deltas {
            // Full capture ignores parent IDs which are not in the store.
            if !self.nodes.contains(parent) {
                continue;
            }
            let node = updates.entry(parent).or_insert_with(|| {
                let mut node = self.capture_process_node(parent).expect("existing parent");
                node.child_count = before.node(parent).map_or(0, |node| node.child_count);
                node
            });
            node.child_count = node.child_count.checked_add_signed(delta)?;
        }
        Some(updates)
    }

    pub(super) fn capture_process_node(&self, id: NodeId) -> Option<Box<ProcessTreeNodeSnapshot>> {
        let node = self.nodes.get(id)?;
        let data = node.node_data();
        let descriptor = node.engine_script_descriptor();
        let (param_value, param_constraints, param_control) = match node.engine_process_tree_parameter_state() {
            Some(state) => (Some(state.value), Some(state.constraints), Some(state.control)),
            None => (None, None, None),
        };
        Some(Box::new(ProcessTreeNodeSnapshot {
            id,
            uuid: data.meta.uuid,
            parent: data.parent,
            first_child: data.first_child,
            next_sibling: data.next_sibling,
            node_type: node.get_type().to_string(),
            decl_id: data.meta.decl_id.0.clone(),
            short_name: data.meta.short_name.clone(),
            label: data.meta.label.clone(),
            tags: data.meta.tags.clone(),
            presentation: data.meta.presentation.clone(),
            enabled: data.effective_enabled,
            can_be_disabled: data.meta.can_be_disabled,
            child_count: 0,
            param_value,
            param_constraints,
            param_control,
            dashboard_widget_target: node.engine_dashboard_widget_target_descriptor(),
            script_properties: descriptor.properties,
            script_methods: descriptor.methods,
        }))
    }
}
