use super::*;

#[derive(Default)]
pub(crate) struct ScheduleMgr {
    topo_order: Vec<NodeId>,
    buckets: Vec<ScheduleBucket>,
    pub(super) bucket_by_node: HashMap<NodeId, usize>,
    /// Maps each scheduled NodeId → its position in the global topo_order.
    /// Used for k-way merge order comparison in collect_due_nodes_into.
    /// INVALIDATED BY: rebuild()
    topo_index_by_node: HashMap<NodeId, usize>,
    /// Scratch: due bucket indices collected each tick. Cleared on each call.
    due_bucket_scratch: Vec<usize>,
}

struct ScheduleBucket {
    rate_hz: NodeUpdateRate,
    interval: Duration,
    accumulator: Duration,
    nodes: Vec<NodeId>,
    due_count: u32,
}

impl ScheduleBucket {
    fn new(rate_hz: NodeUpdateRate, nodes: Vec<NodeId>) -> Self {
        let nanos = (1_000_000_000u64 / rate_hz as u64).max(1);
        Self {
            rate_hz,
            interval: Duration::from_nanos(nanos),
            accumulator: Duration::ZERO,
            nodes,
            due_count: 0,
        }
    }
}

impl ScheduleMgr {
    pub(super) fn rebuild(
        &mut self,
        topo_order: Vec<NodeId>,
        rules: &HashMap<NodeId, NodeExecutionRule>,
    ) -> Result<(), EngineRuntimeError> {
        self.bucket_by_node.clear();
        self.buckets.clear();

        let mut buckets_by_rate: BTreeMap<NodeUpdateRate, Vec<NodeId>> = BTreeMap::new();
        for node_id in &topo_order {
            let Some(rule) = rules.get(node_id) else {
                continue;
            };

            let Some(rate_hz) = rule.update_rate else {
                continue;
            };

            if rate_hz == 0 {
                return Err(EngineRuntimeError::InvalidUpdateRate {
                    node: *node_id,
                    rate_hz,
                });
            }

            buckets_by_rate.entry(rate_hz).or_default().push(*node_id);
        }

        for (rate_hz, nodes) in buckets_by_rate {
            let bucket_index = self.buckets.len();
            for node in &nodes {
                self.bucket_by_node.insert(*node, bucket_index);
            }
            self.buckets.push(ScheduleBucket::new(rate_hz, nodes));
        }

        // Build topo_index_by_node for k-way merge order comparison in collect_due_nodes_into.
        self.topo_index_by_node.clear();
        for (idx, node_id) in topo_order.iter().enumerate() {
            if self.bucket_by_node.contains_key(node_id) {
                self.topo_index_by_node.insert(*node_id, idx);
            }
        }

        // Keep only scheduled nodes for the schedule_topology() public API.
        self.topo_order = topo_order
            .into_iter()
            .filter(|node_id| self.bucket_by_node.contains_key(node_id))
            .collect();

        Ok(())
    }

    /// Advances bucket accumulators and merges due nodes into `out` in topological order.
    ///
    /// Zero per-node work when no buckets are due this tick.
    // pub(in crate::engine) so engine::tests can call this directly on runtime_schedule
    pub(in crate::engine) fn collect_due_nodes_into(
        &mut self,
        out: &mut Vec<NodeId>,
        elapsed: Duration,
        max_bucket_catch_up: u32,
    ) {
        out.clear();

        if self.buckets.is_empty() {
            return;
        }

        let catch_up_limit = max_bucket_catch_up.max(1);

        // Advance accumulators; record which bucket indices fired this tick.
        self.due_bucket_scratch.clear();
        let mut max_due = 0u32;
        for (idx, bucket) in self.buckets.iter_mut().enumerate() {
            bucket.accumulator = bucket.accumulator.saturating_add(elapsed);
            bucket.due_count = 0;
            while bucket.due_count < catch_up_limit && bucket.accumulator >= bucket.interval {
                bucket.accumulator = bucket.accumulator.saturating_sub(bucket.interval);
                bucket.due_count += 1;
            }
            if bucket.due_count > 0 {
                self.due_bucket_scratch.push(idx);
                max_due = max_due.max(bucket.due_count);
            }
        }

        // No per-node work when nothing is due.
        if self.due_bucket_scratch.is_empty() {
            return;
        }

        for round in 0..max_due {
            if self.due_bucket_scratch.len() == 1 {
                // Single due bucket — nodes are already in topo order; copy directly.
                let bi = self.due_bucket_scratch[0];
                if self.buckets[bi].due_count > round {
                    out.extend_from_slice(&self.buckets[bi].nodes);
                }
            } else {
                // Multiple due buckets — k-way merge by topo index to preserve global order.
                // Each bucket's nodes vec is pre-sorted by topo index (built from topo_order in rebuild).
                let mut cursors: Vec<(usize, usize)> = self
                    .due_bucket_scratch
                    .iter()
                    .filter(|&&bi| self.buckets[bi].due_count > round)
                    .map(|&bi| (bi, 0usize))
                    .collect();

                while !cursors.is_empty() {
                    let min_pos = {
                        let buckets = &self.buckets;
                        let topo_index_by_node = &self.topo_index_by_node;
                        let mut best_pos = 0;
                        let mut best_topo = usize::MAX;
                        for (pos, (bi, ci)) in cursors.iter().enumerate() {
                            let topo = topo_index_by_node
                                .get(&buckets[*bi].nodes[*ci])
                                .copied()
                                .unwrap_or(usize::MAX);
                            if topo < best_topo {
                                best_topo = topo;
                                best_pos = pos;
                            }
                        }
                        best_pos
                    };
                    let (bi, ci) = cursors[min_pos];
                    out.push(self.buckets[bi].nodes[ci]);
                    if ci + 1 < self.buckets[bi].nodes.len() {
                        cursors[min_pos].1 += 1;
                    } else {
                        cursors.swap_remove(min_pos);
                    }
                }
            }
        }

        for bucket in &mut self.buckets {
            bucket.due_count = 0;
        }
    }

    pub(super) fn bucket_nodes(&self, rate_hz: NodeUpdateRate) -> Option<&[NodeId]> {
        self.buckets
            .iter()
            .find(|bucket| bucket.rate_hz == rate_hz)
            .map(|bucket| bucket.nodes.as_slice())
    }

    pub(super) fn topo_order(&self) -> &[NodeId] {
        &self.topo_order
    }
}
