use super::*;

impl<T: Node> Engine<T> {
    /// Resolves scheduler state when marked dirty.
    ///
    /// Returns `true` when resolution happened.
    pub fn resolve_if_needed(&mut self) -> Result<bool, EngineRuntimeError> {
        if !self.runtime_resolve_pending {
            return Ok(false);
        }

        self.resolve()?;
        Ok(true)
    }

    /// Rebuilds runtime scheduling from current node execution rules.
    pub fn resolve(&mut self) -> Result<(), EngineRuntimeError> {
        let previously_scheduled_nodes: HashSet<NodeId> =
            self.runtime_schedule.bucket_by_node.keys().copied().collect();
        let rules = self.collect_execution_rules();
        let topo_order = self.topological_sort(&rules)?;
        self.runtime_schedule.rebuild(topo_order, &rules)?;
        for node_id in self.runtime_schedule.bucket_by_node.keys().copied() {
            if !previously_scheduled_nodes.contains(&node_id) {
                self.last_update_elapsed_by_node.insert(node_id, self.runtime_elapsed);
            }
        }
        self.runtime_resolve_pending = false;
        Ok(())
    }

    /// Executes one runtime tick with an elapsed wall-clock delta.
    ///
    /// # Phase diagram
    ///
    /// | Phase | Reads | Writes | May emit events | May append edits |
    /// |-------|-------|--------|-----------------|-----------------|
    /// | 1. resolve | schedule state | topo_order, buckets | no | no |
    /// | 2. absorb external edits | external_edits_rx | edits.pending | no | yes |
    /// | 3. apply external edits | edits.pending | nodes, schedule | yes (ChildAdded etc.) | no |
    /// | 4. inbox precompute | inbox.events, nodes | per_node_events | no | no |
    /// | 5. inbox preprocess (on_child_added) | per_node_events | nodes | yes | yes |
    /// | 6. control pass | nodes (param controls) | edits.pending | yes (ParamChanged) | yes |
    /// | 7. scheduled updates (node.update) | nodes, parameter_values_cache | nodes, inbox | yes | yes |
    /// | 8. stabilization rounds | inbox, edits.pending | nodes | yes | yes |
    /// | 9. logger sync | logger state | ui_event_log | no | no |
    pub fn run_tick(&mut self, elapsed: Duration) -> Result<(), EngineRuntimeError> {
        let tick_started = Instant::now();
        self.tick_scratch.clear_stats();
        self.tick_tree_snapshot = None;
        self.tick_scratch.clear_scheduled();

        // Apply structural edits deferred from the previous tick's stabilization rounds.
        // Structural edits cannot run inside stabilization because they reset the schedule
        // and would extend the loop unpredictably.
        for edit in self.deferred_structural_edits.drain(..) {
            self.edits.push(edit);
        }

        let resolve1_started = Instant::now();
        self.resolve_if_needed()?;
        let resolve1_ms = resolve1_started.elapsed().as_millis();

        self.time.tick = self.time.tick.saturating_add(1);
        self.time.micro = 0;
        self.time.seq = 0;
        self.runtime_elapsed = self.runtime_elapsed.saturating_add(elapsed);

        let absorb_started = Instant::now();
        self.absorb_external_edits()?;
        let absorb_external_edits_ms = absorb_started.elapsed().as_millis();

        let pending_edits = self.edits.pending.len();
        let apply_started = Instant::now();
        if !self.edits.pending.is_empty() {
            self.apply_edits_without_history()?;
            self.resolve_if_needed()?;
        }
        let apply_external_edits_ms = apply_started.elapsed().as_millis();

        let inbox_events = self.inbox.events.len();
        let mut inbox_precompute_ms = 0;
        let mut inbox_preprocess_ms = 0;
        if !self.inbox.events.is_empty() {
            let precompute_started = Instant::now();
            let precomputed = self.precompute_inbox_dispatch();
            inbox_precompute_ms = precompute_started.elapsed().as_millis();

            let preprocess_started = Instant::now();
            self.preprocess_precomputed_inbox(ExecutionPhase::EngineTick, precomputed)?;
            if !self.edits.pending.is_empty() {
                self.apply_edits_without_history()?;
                self.resolve_if_needed()?;
            }
            inbox_preprocess_ms = preprocess_started.elapsed().as_millis();
        }

        let control_started = Instant::now();
        self.run_control_pass()?;
        let control_ms = control_started.elapsed().as_millis();

        let scheduled_started = Instant::now();
        self.run_scheduled_updates(elapsed)?;
        let scheduled_ms = scheduled_started.elapsed().as_millis();

        let stabilization_started = Instant::now();
        self.run_stabilization_rounds()?;
        let stabilization_ms = stabilization_started.elapsed().as_millis();

        let sync_started = Instant::now();
        self.sync_logger_ui_events();
        let logger_sync_ms = sync_started.elapsed().as_millis();

        let total_ms = tick_started.elapsed().as_millis();
        if *PERF_TRACE_ENABLED && total_ms >= PERF_LOG_TICK_THRESHOLD_MS {
            eprintln!(
                "[engine] tick total_ms={} resolve1_ms={} absorb_external_edits_ms={} apply_external_edits_ms={} inbox_precompute_ms={} inbox_preprocess_ms={} control_ms={} scheduled_ms={} stabilization_ms={} logger_sync_ms={} pending_edits={} inbox_events={}",
                total_ms,
                resolve1_ms,
                absorb_external_edits_ms,
                apply_external_edits_ms,
                inbox_precompute_ms,
                inbox_preprocess_ms,
                control_ms,
                scheduled_ms,
                stabilization_ms,
                logger_sync_ms,
                pending_edits,
                inbox_events
            );
            if self
                .last_performance_log_tick
                .is_none_or(|last_tick| self.time.tick.saturating_sub(last_tick) >= PERF_LOG_MIN_TICK_INTERVAL)
            {
                self.last_performance_log_tick = Some(self.time.tick);
                let stats = self.tick_stats();
                eprintln!(
                    "[engine] slow_tick total_ms={total_ms} resolve_ms={resolve1_ms} external_edits_ms={apply_external_edits_ms} inbox_precompute_ms={inbox_precompute_ms} inbox_preprocess_ms={inbox_preprocess_ms} control_ms={control_ms} scheduled_ms={scheduled_ms} stabilization_ms={stabilization_ms} logger_sync_ms={logger_sync_ms} pending_edits={pending_edits} inbox_events={inbox_events} nodes_due={} callbacks={} events_emitted={} edits_applied={} stabilization_passes={} snapshot_rebuilds={} snapshot_builds={} snapshot_build_us={} snapshot_nodes_cloned={} dispatch_events_routed={} dispatch_recipient_deliveries={} dispatch_max_fanout={} controls_params_scanned={}",
                    stats.nodes_due,
                    stats.callbacks_fired,
                    stats.events_emitted,
                    stats.edits_applied,
                    stats.stabilization_passes,
                    stats.snapshot_rebuilds,
                    stats.snapshot_builds,
                    stats.snapshot_build_ns / 1_000,
                    stats.snapshot_nodes_cloned,
                    stats.dispatch_events_routed,
                    stats.dispatch_recipient_deliveries,
                    stats.dispatch_max_fanout,
                    stats.controls_params_scanned
                );
            }
        }
        Ok(())
    }

    /// Runs the runtime loop for `duration`, capped by `RuntimeLimits::max_loop_frequency_hz`.
    ///
    /// When `runtime_limits.fixed_step` is `Some`, uses the fixed-step accumulator:
    /// wall-clock time is absorbed in exact `step`-sized logical ticks so nodes always
    /// receive uniform `delta_time` regardless of frame-rate jitter.
    pub fn run_for(&mut self, duration: Duration) -> Result<(), EngineRuntimeError> {
        let start = Instant::now();
        let mut previous_tick_start = start;

        while start.elapsed() < duration {
            let tick_start = Instant::now();
            let elapsed = tick_start.saturating_duration_since(previous_tick_start);
            previous_tick_start = tick_start;

            if self.runtime_limits.fixed_step.is_some() {
                self.drain_fixed_step_accumulator(elapsed)?;
            } else {
                self.run_tick(elapsed)?;
            }

            let tick_elapsed = tick_start.elapsed();
            let cap_interval = self.runtime_limits.loop_cap_interval();
            if tick_elapsed < cap_interval {
                let sleep_for = cap_interval - tick_elapsed;
                let remaining_total = duration.saturating_sub(start.elapsed());
                if !remaining_total.is_zero() {
                    thread::sleep(sleep_for.min(remaining_total));
                }
            }
        }

        Ok(())
    }

    /// Runs the runtime loop indefinitely, capped by `RuntimeLimits::max_loop_frequency_hz`.
    ///
    /// When `runtime_limits.fixed_step` is `Some`, uses the fixed-step accumulator.
    pub fn run_loop(&mut self) -> Result<(), EngineRuntimeError> {
        let mut previous_tick_start = Instant::now();

        loop {
            let tick_start = Instant::now();
            let elapsed = tick_start.saturating_duration_since(previous_tick_start);
            previous_tick_start = tick_start;

            if self.runtime_limits.fixed_step.is_some() {
                self.drain_fixed_step_accumulator(elapsed)?;
            } else {
                self.run_tick(elapsed)?;
            }

            let tick_elapsed = tick_start.elapsed();
            let cap_interval = self.runtime_limits.loop_cap_interval();
            if tick_elapsed < cap_interval {
                thread::sleep(cap_interval - tick_elapsed);
            }
        }
    }

    /// Absorbs `wall_elapsed` into the fixed-step accumulator and fires `run_tick(step)`
    /// for each complete step. Increments `late_ticks` when `wall_elapsed` exceeds
    /// `max_catchup` and clamping discards time to prevent the spiral-of-death.
    ///
    /// No-op when `runtime_limits.fixed_step` is `None`.
    pub(in crate::engine) fn drain_fixed_step_accumulator(
        &mut self,
        wall_elapsed: Duration,
    ) -> Result<(), EngineRuntimeError> {
        let Some(cfg) = self.runtime_limits.fixed_step else {
            return Ok(());
        };
        let clamped = wall_elapsed.min(cfg.max_catchup);
        if clamped < wall_elapsed {
            self.late_ticks = self.late_ticks.saturating_add(1);
        }
        self.tick_accumulator = self.tick_accumulator.saturating_add(clamped);
        while self.tick_accumulator >= cfg.step {
            self.run_tick(cfg.step)?;
            self.tick_accumulator = self.tick_accumulator.saturating_sub(cfg.step);
        }
        Ok(())
    }

    pub(super) fn collect_execution_rules(&self) -> HashMap<NodeId, NodeExecutionRule> {
        self.nodes
            .iter()
            .filter_map(|(node_id, node)| {
                self.is_enabled(node_id, true)
                    .then_some((node_id, node.execution_rule()))
            })
            .collect()
    }

    pub(super) fn topological_sort(
        &self,
        rules: &HashMap<NodeId, NodeExecutionRule>,
    ) -> Result<Vec<NodeId>, EngineRuntimeError> {
        let mut indegree: HashMap<NodeId, usize> = self.nodes.keys().map(|node_id| (node_id, 0usize)).collect(); // PERF-EXCEPTION: resolve only, gated by runtime_resolve_pending; never called in steady-state ticks.
        let mut outgoing: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

        for (node_id, rule) in rules {
            let mut dedupe = HashSet::new();
            for dependency in &rule.dependencies {
                if !indegree.contains_key(dependency) {
                    return Err(EngineRuntimeError::MissingDependency {
                        node: *node_id,
                        dependency: *dependency,
                    });
                }
                if !dedupe.insert(*dependency) {
                    continue;
                }

                outgoing.entry(*dependency).or_default().push(*node_id);
                if let Some(indegree_value) = indegree.get_mut(node_id) {
                    *indegree_value += 1;
                }
            }
        }

        // Vec stack: initial ready set sorted descending so pop() yields ascending node-id order,
        // preserving the same deterministic tiebreaker as the old BTreeSet approach.
        // Nodes that become ready mid-traversal are pushed to the back and popped next (LIFO).
        let mut ready: Vec<NodeId> = indegree
            .iter()
            .filter_map(|(node_id, &deg)| (deg == 0).then_some(*node_id))
            .collect();
        ready.sort_unstable_by_key(|node| std::cmp::Reverse(node.0));

        let mut sorted = Vec::with_capacity(indegree.len());

        while let Some(node_id) = ready.pop() {
            sorted.push(node_id);

            if let Some(dependents) = outgoing.get(&node_id) {
                for dependent in dependents {
                    if let Some(indegree_value) = indegree.get_mut(dependent) {
                        *indegree_value -= 1;
                        if *indegree_value == 0 {
                            ready.push(*dependent);
                        }
                    }
                }
            }
        }

        if sorted.len() == indegree.len() {
            return Ok(sorted);
        }

        let mut cycle_nodes: Vec<NodeId> = indegree
            .into_iter()
            .filter_map(|(node, indegree)| (indegree > 0).then_some(node))
            .collect();
        cycle_nodes.sort_by_key(|node| node.0);

        Err(EngineRuntimeError::DependencyCycle { nodes: cycle_nodes })
    }
}
