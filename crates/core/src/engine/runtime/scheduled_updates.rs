use super::*;

impl<T: Node> Engine<T> {
    pub(super) fn run_scheduled_updates(&mut self, elapsed: Duration) -> Result<(), EngineRuntimeError> {
        // Reuse scratch buffer to avoid per-tick Vec allocation.
        let mut due_nodes = std::mem::take(&mut self.tick_scratch.due_nodes);
        self.runtime_schedule.collect_due_nodes_into(
            &mut due_nodes,
            elapsed,
            self.runtime_limits.max_bucket_catch_up_per_tick,
        );
        self.tick_scratch.stats.nodes_due = due_nodes.len();

        if due_nodes.is_empty() {
            self.tick_scratch.due_nodes = due_nodes;
            return Ok(());
        }

        // Skip all expensive setup when every due node has nothing to do this tick.
        if !due_nodes
            .iter()
            .any(|id| self.nodes.get(*id).is_some_and(|n| n.needs_update()))
        {
            due_nodes.clear();
            self.tick_scratch.due_nodes = due_nodes;
            return Ok(());
        }

        let needs_tree_snapshot = due_nodes.iter().any(|node_id| {
            self.nodes
                .get(*node_id)
                .is_some_and(|node| node.needs_update() && node.update_requires_tree_snapshot())
        });
        let tree_snapshot = needs_tree_snapshot.then(|| self.get_or_build_tick_snapshot());

        let mut parameter_values = std::mem::take(&mut self.parameter_values_cache);
        let mut callback_count = 0usize;
        // Take scratch HashMap buffers to avoid per-tick allocation.
        let mut due_counts = std::mem::take(&mut self.tick_scratch.due_counts);
        let mut remaining_delta_by_node = std::mem::take(&mut self.tick_scratch.remaining_delta_by_node);
        let mut seen_by_node = std::mem::take(&mut self.tick_scratch.seen_by_node);

        for node_id in &due_nodes {
            *due_counts.entry(*node_id).or_default() += 1;
        }

        for node_id in due_counts.keys() {
            let previous = self
                .last_update_elapsed_by_node
                .get(node_id)
                .copied()
                .unwrap_or(Duration::ZERO);
            remaining_delta_by_node.insert(*node_id, self.runtime_elapsed.saturating_sub(previous));
        }

        for node_id in &due_nodes {
            let node_id = *node_id;
            if !self.is_enabled(node_id, true) {
                continue;
            }

            let seen = seen_by_node.entry(node_id).or_default();
            *seen += 1;

            let total_occurrences = due_counts.get(&node_id).copied().unwrap_or(1);
            let remaining_occurrences = total_occurrences.saturating_sub(*seen - 1).max(1);
            let remaining_delta = remaining_delta_by_node.entry(node_id).or_insert(Duration::ZERO);

            let delta_time = if remaining_occurrences == 1 {
                let dt = *remaining_delta;
                *remaining_delta = Duration::ZERO;
                dt
            } else {
                let dt = *remaining_delta / remaining_occurrences as u32;
                *remaining_delta = remaining_delta.saturating_sub(dt);
                dt
            };

            // Skip expensive context setup when the node has nothing to do this tick.
            if !self.nodes.get(node_id).is_some_and(|n| n.needs_update()) {
                continue;
            }

            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.delta_time = delta_time;
            ctx.runtime_elapsed = self.runtime_elapsed;
            if let Some(tree_snapshot) = &tree_snapshot
                && self
                    .nodes
                    .get(node_id)
                    .is_some_and(|node| node.update_requires_tree_snapshot())
            {
                ctx.set_tree_snapshot(Arc::clone(tree_snapshot));
            }
            let mut did_update = false;
            let events_before_update = self.inbox.events.len();
            if let Some(node) = self.nodes.get_mut(node_id) {
                let mut resolve = |param_id: NodeId| parameter_values.get(&param_id).cloned();
                crate::logger::with_node_origin(node_id, || {
                    node.engine_sync_bound_param_handles(&mut resolve);
                    node.update(&mut ctx);
                });
                did_update = true;
                callback_count += 1;
                self.tick_scratch.stats.callbacks_fired += 1;
                if callback_count > self.runtime_limits.max_update_callbacks_per_tick {
                    return Err(EngineRuntimeError::UpdateBudgetExceeded {
                        tick: self.time.tick,
                        callbacks: callback_count,
                        limit: self.runtime_limits.max_update_callbacks_per_tick,
                    });
                }
            }
            self.absorb_edits(&mut ctx)?;
            for event in self.inbox.events.iter().skip(events_before_update) {
                match &event.kind {
                    EventKind::ParamChanged { param, new_value, .. } => {
                        parameter_values.insert(*param, new_value.clone());
                    }
                    EventKind::NodeCreated { node } => {
                        if let Some(n) = self.nodes.get(*node)
                            && let Some(snapshot) = n.engine_param_snapshot()
                        {
                            parameter_values.insert(*node, snapshot.value);
                        }
                    }
                    EventKind::NodeDeleted { node } => {
                        parameter_values.remove(node);
                    }
                    _ => {}
                }
            }

            if did_update {
                let last = self
                    .last_update_elapsed_by_node
                    .entry(node_id)
                    .or_insert(Duration::ZERO);
                *last = last.saturating_add(delta_time);
            }
        }

        // Return scratch buffers and the updated param cache for reuse next tick.
        due_nodes.clear();
        due_counts.clear();
        remaining_delta_by_node.clear();
        seen_by_node.clear();
        self.tick_scratch.due_nodes = due_nodes;
        self.tick_scratch.due_counts = due_counts;
        self.tick_scratch.remaining_delta_by_node = remaining_delta_by_node;
        self.tick_scratch.seen_by_node = seen_by_node;
        self.parameter_values_cache = parameter_values;

        Ok(())
    }
}
