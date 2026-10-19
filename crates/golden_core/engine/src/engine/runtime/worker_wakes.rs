use super::*;

impl<T: Node> Engine<T> {
    /// Discrete worker service has its own bounded turn; continuous producers cannot extend
    /// this batch or consume the scheduled-node budget. Payloads remain owned by each module.
    pub(super) fn run_worker_wakes(&mut self) -> Result<(), EngineRuntimeError> {
        if self.edit_journal.is_some() || self.rollback_replay {
            return Ok(());
        }
        let turn = self
            .node_wakes
            .take_turn(self.runtime_limits.max_wake_callbacks_per_tick);
        for mut delivery in turn {
            if !delivery.ready() {
                continue;
            }
            let node_id = delivery.node();
            if !self.nodes.contains(node_id) {
                delivery.close();
                continue;
            }
            if !self.is_enabled(node_id, true) {
                delivery.complete();
                continue;
            }
            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.runtime_elapsed = self.runtime_elapsed;
            if self
                .nodes
                .get(node_id)
                .is_some_and(|node| node.worker_wake_requires_tree_snapshot())
            {
                ctx.set_tree_snapshot(self.get_or_build_tick_snapshot());
            }
            if let Some(node) = self.nodes.get_mut(node_id) {
                let values = &self.parameter_values_cache;
                crate::logger::with_node_origin(node_id, || {
                    node.engine_sync_bound_param_handles(&mut |id| values.get(&id).cloned());
                    node.on_worker_wake(&mut ctx);
                });
                self.tick_scratch.stats.wake_callbacks_fired += 1;
            }
            let completion = ctx.edits.completion();
            // The module owns payload/cursor recovery; receipt failure re-arms only its level
            // notification, without replaying the consumed commands or a discarded edit batch.
            self.absorb_edits(&mut ctx)?;
            if completion.status() == crate::edit::EditCompletionStatus::Applied {
                delivery.complete();
            } else {
                delivery.wait_for(completion);
            }
        }
        Ok(())
    }
}
