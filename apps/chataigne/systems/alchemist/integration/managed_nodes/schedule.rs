use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, VecDeque},
    sync::Arc,
};

use golden_core::{node::NodeId, process_ctx::ProcessCtx};

use super::OutputRuntimeCache;
use crate::app::module_command::{
    self, ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandInvocationId,
    ModuleCommandParamOverrides,
};
use crate::app::systems_alchemist_generic_commands::{
    LOG_INVOCATION_CHANGE_MIN_TICKS, LOG_INVOCATION_KEEPALIVE_TICKS, LOG_INVOCATION_RECENCY_TOUCH_TICKS,
    LOG_INVOCATION_STALE_TICKS, MAX_LOG_INVOCATIONS, MAX_LOG_PRUNE_STEPS_PER_EVENT,
};

/// Maximum output fan-out cells expanded or delayed executions promoted in one
/// engine tick.
///
/// This matches the transient command-batch chunk size, keeping both main-thread
/// work and serialized event payloads bounded while overdue work remains queued
/// in stable deadline/FIFO order for following ticks.
pub(crate) const MAX_SCHEDULED_OUTPUT_EXECUTIONS_PER_TICK: usize =
    module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS;
const MAX_PENDING_OUTPUT_FANOUT_JOBS: usize = 64;
const MAX_PENDING_OUTPUT_FANOUT_EXECUTIONS: usize = module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS * 8;
pub(super) const MAX_RETAINED_OUTPUT_FANOUT_TARGETS: usize = 65_536;
const MAX_PENDING_SCHEDULED_OUTPUT_EXECUTIONS: usize = module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS * 8;

/// Runtime-only queue of outputs waiting to fire (not persisted).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct OutputSchedule {
    pending: BinaryHeap<PendingOutput>,
    #[serde(default)]
    pending_fanout: VecDeque<PendingOutputFanout>,
    elapsed_seconds: f64,
    next_sequence: u64,
    #[serde(default)]
    next_fanout_generation: u64,
    #[serde(default)]
    latest_cancel_fanout_generation: Option<u64>,
    log_admission: OutputLogAdmissionCache,
    #[serde(default)]
    work_budget_tick: Option<u64>,
    #[serde(default)]
    work_steps_this_tick: usize,
    #[serde(default)]
    rejected_fanout_executions: u64,
    #[serde(default)]
    last_fanout_rejection_log_tick: Option<u64>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct PendingOutput {
    target: NodeId,
    due_at: f64,
    sequence: u64,
    param_overrides: ModuleCommandParamOverrides,
    invocation_id: Option<ModuleCommandInvocationId>,
    delivery_policy: ModuleCommandDeliveryPolicy,
    #[serde(default)]
    batchable: bool,
}

/// Compact continuation for the execution-major Cartesian traversal of one
/// trigger batch and its immutable output snapshot.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct PendingOutputFanout {
    #[serde(default)]
    generation: u64,
    targets: Arc<[OutputRuntimeTarget]>,
    executions: Vec<ModuleCommandExecuteEvent>,
    execution_index: usize,
    target_index: usize,
    delay: f64,
    stagger: f64,
    cancel_on_trigger: bool,
    trigger_elapsed_seconds: f64,
    trigger_tick: u64,
}

impl PendingOutputFanout {
    fn is_complete(&self) -> bool {
        self.execution_index >= self.executions.len() || self.targets.is_empty()
    }

    fn remaining_execution_count(&self) -> usize {
        self.executions.len().saturating_sub(self.execution_index)
    }

    fn advance(&mut self) {
        self.target_index += 1;
        if self.target_index >= self.targets.len() {
            self.target_index = 0;
            self.execution_index += 1;
        }
    }

    fn has_later_execution(&self) -> bool {
        self.execution_index + 1 < self.executions.len()
    }
}

impl PartialEq for PendingOutput {
    fn eq(&self, other: &Self) -> bool {
        self.due_at.to_bits() == other.due_at.to_bits() && self.sequence == other.sequence
    }
}

impl Eq for PendingOutput {}

impl PartialOrd for PendingOutput {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingOutput {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap pops the greatest entry. Reverse the deadline and sequence
        // comparisons so the earliest scheduled output retains stable FIFO order.
        other
            .due_at
            .total_cmp(&self.due_at)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct OutputRuntimeTarget {
    pub(super) node: NodeId,
    pub(super) change_aware_log: bool,
    #[serde(default)]
    pub(super) batchable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct OutputLogInvocationKey {
    target: NodeId,
    invocation_id: ModuleCommandInvocationId,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct OutputLogAdmissionCache {
    records: HashMap<OutputLogInvocationKey, OutputLogAdmissionRecord>,
    recency: VecDeque<(OutputLogInvocationKey, u64)>,
    next_generation: u64,
    budget_tick: Option<u64>,
    emissions_this_tick: usize,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct OutputLogAdmissionRecord {
    param_overrides: ModuleCommandParamOverrides,
    emitted_tick: u64,
    last_seen_tick: u64,
    recency_tick: u64,
    generation: u64,
}

impl OutputLogAdmissionCache {
    fn should_emit(
        &mut self,
        key: OutputLogInvocationKey,
        param_overrides: &ModuleCommandParamOverrides,
        tick: u64,
    ) -> bool {
        if self.budget_tick != Some(tick) {
            self.budget_tick = Some(tick);
            self.emissions_this_tick = 0;
        }
        self.prune_stale(tick);

        let mut touch_recency = false;
        if let Some(previous) = self.records.get_mut(&key) {
            previous.last_seen_tick = tick;
            touch_recency = tick.saturating_sub(previous.recency_tick) >= LOG_INVOCATION_RECENCY_TOUCH_TICKS;
            let minimum_ticks = if previous.param_overrides == *param_overrides {
                LOG_INVOCATION_KEEPALIVE_TICKS
            } else {
                LOG_INVOCATION_CHANGE_MIN_TICKS
            };
            if tick.saturating_sub(previous.emitted_tick) < minimum_ticks {
                if touch_recency {
                    self.touch(key, tick);
                }
                return false;
            }
        }

        if self.emissions_this_tick >= 1 {
            if touch_recency {
                self.touch(key, tick);
            }
            return false;
        }

        self.emissions_this_tick += 1;
        self.record_emission(key, param_overrides, tick);
        true
    }

    fn record_emission(
        &mut self,
        key: OutputLogInvocationKey,
        param_overrides: &ModuleCommandParamOverrides,
        tick: u64,
    ) {
        if !self.records.contains_key(&key) {
            self.make_room();
        }
        let generation = self.allocate_generation();
        let record = self.records.entry(key).or_insert_with(|| OutputLogAdmissionRecord {
            param_overrides: Vec::new(),
            emitted_tick: tick,
            last_seen_tick: tick,
            recency_tick: tick,
            generation,
        });
        record.param_overrides.clone_from(param_overrides);
        record.emitted_tick = tick;
        record.last_seen_tick = tick;
        record.recency_tick = tick;
        record.generation = generation;
        self.recency.push_back((key, generation));
    }

    fn touch(&mut self, key: OutputLogInvocationKey, tick: u64) {
        let generation = self.allocate_generation();
        let Some(record) = self.records.get_mut(&key) else {
            return;
        };
        record.recency_tick = tick;
        record.generation = generation;
        self.recency.push_back((key, generation));
    }

    fn allocate_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("output log admission generation exhausted");
        generation
    }

    fn prune_stale(&mut self, tick: u64) {
        for _ in 0..MAX_LOG_PRUNE_STEPS_PER_EVENT {
            let Some((key, generation)) = self.recency.front().copied() else {
                break;
            };
            let Some(record) = self.records.get(&key) else {
                self.recency.pop_front();
                continue;
            };
            if record.generation != generation {
                self.recency.pop_front();
                continue;
            }
            if tick.saturating_sub(record.last_seen_tick) < LOG_INVOCATION_STALE_TICKS {
                break;
            }
            self.recency.pop_front();
            self.records.remove(&key);
        }
    }

    fn make_room(&mut self) {
        while self.records.len() >= MAX_LOG_INVOCATIONS {
            let Some((key, generation)) = self.recency.pop_front() else {
                self.records.clear();
                return;
            };
            if self
                .records
                .get(&key)
                .is_some_and(|record| record.generation == generation)
            {
                self.records.remove(&key);
            }
        }
    }
}

impl OutputSchedule {
    pub(super) fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.pending_fanout.is_empty()
    }

    #[cfg(test)]
    pub(super) fn pending_len(&self) -> usize {
        self.pending.len()
    }

    #[cfg(test)]
    pub(super) fn pending_fanout_job_count(&self) -> usize {
        self.pending_fanout.len()
    }

    #[cfg(test)]
    pub(super) fn pending_fanout_stored_execution_count(&self) -> usize {
        self.pending_fanout.iter().map(|pending| pending.executions.len()).sum()
    }

    #[cfg(test)]
    pub(super) fn pending_fanout_stored_target_count(&self) -> usize {
        self.retained_target_snapshot_count()
    }

    #[cfg(test)]
    pub(super) fn rejected_fanout_execution_count(&self) -> u64 {
        self.rejected_fanout_executions
    }

    pub(super) fn on_trigger_cached(
        &mut self,
        ctx: &mut ProcessCtx,
        owner: NodeId,
        cache: &OutputRuntimeCache,
        param_overrides: ModuleCommandParamOverrides,
        invocation_id: Option<ModuleCommandInvocationId>,
    ) {
        self.enqueue_fanout(
            ctx,
            owner,
            cache,
            vec![ModuleCommandExecuteEvent {
                command_id: owner,
                param_overrides,
                invocation_id,
                delivery_policy: ModuleCommandDeliveryPolicy::Standard,
            }],
        );
        self.drain_fanout(ctx);
    }

    /// Applies ordered container executions through a bounded execution-major
    /// traversal. Unvisited execution/target pairs remain in one compact
    /// serializable continuation rather than a materialized Cartesian queue.
    pub(super) fn on_trigger_batch_cached(
        &mut self,
        ctx: &mut ProcessCtx,
        owner: NodeId,
        cache: &OutputRuntimeCache,
        executions: Vec<ModuleCommandExecuteEvent>,
    ) {
        self.enqueue_fanout(ctx, owner, cache, executions);
        self.drain_fanout(ctx);
    }

    fn enqueue_fanout(
        &mut self,
        ctx: &mut ProcessCtx,
        owner: NodeId,
        cache: &OutputRuntimeCache,
        executions: Vec<ModuleCommandExecuteEvent>,
    ) {
        if executions.is_empty() {
            return;
        }

        let exceeds_event_boundary = executions.len() > module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS;
        if exceeds_event_boundary {
            self.reject_fanout(ctx, owner, executions.len());
            return;
        }

        let generation = self.allocate_fanout_generation();
        if cache.outputs.is_empty() {
            if cache.cancel_on_trigger {
                self.pending.clear();
                self.latest_cancel_fanout_generation = Some(generation);
            }
            return;
        }

        let queued_executions = self
            .pending_fanout
            .iter()
            .map(PendingOutputFanout::remaining_execution_count)
            .sum::<usize>();
        let exceeds_queue_bound = self.pending_fanout.len() >= MAX_PENDING_OUTPUT_FANOUT_JOBS
            || queued_executions.saturating_add(executions.len()) > MAX_PENDING_OUTPUT_FANOUT_EXECUTIONS;
        let target_snapshot_is_retained = self
            .pending_fanout
            .iter()
            .any(|pending| Arc::ptr_eq(&pending.targets, &cache.outputs));
        let additional_retained_targets = if target_snapshot_is_retained {
            0
        } else {
            cache.outputs.len()
        };
        let exceeds_target_snapshot_bound = self
            .retained_target_snapshot_count()
            .saturating_add(additional_retained_targets)
            > MAX_RETAINED_OUTPUT_FANOUT_TARGETS;
        if exceeds_queue_bound || exceeds_target_snapshot_bound {
            self.reject_fanout(ctx, owner, executions.len());
            return;
        }

        if cache.cancel_on_trigger {
            self.pending.clear();
            self.latest_cancel_fanout_generation = Some(generation);
        }
        self.pending_fanout.push_back(PendingOutputFanout {
            generation,
            targets: Arc::clone(&cache.outputs),
            executions,
            execution_index: 0,
            target_index: 0,
            delay: cache.delay,
            stagger: cache.stagger,
            cancel_on_trigger: cache.cancel_on_trigger,
            trigger_elapsed_seconds: self.elapsed_seconds,
            trigger_tick: ctx.time.tick,
        });
    }

    fn drain_fanout(&mut self, ctx: &mut ProcessCtx) {
        self.reset_work_budget(ctx.time.tick);
        let mut pending_batch: Option<(NodeId, Vec<ModuleCommandExecuteEvent>)> = None;

        while self.remaining_work_budget() > 0 {
            let Some(mut fanout) = self.pending_fanout.pop_front() else {
                break;
            };
            if fanout.is_complete() {
                continue;
            }

            let cancelled_by_later_trigger = self
                .latest_cancel_fanout_generation
                .is_some_and(|generation| fanout.generation < generation);
            let mut blocked_by_delayed_capacity = false;
            while self.remaining_work_budget() > 0 && !fanout.is_complete() {
                if fanout.cancel_on_trigger && fanout.target_index == 0 {
                    self.pending.clear();
                }

                let target = fanout.targets[fanout.target_index];
                let remaining = fanout.delay + (fanout.target_index as f64) * fanout.stagger;
                let delayed = remaining > f64::EPSILON;
                let will_be_cancelled =
                    delayed && (cancelled_by_later_trigger || fanout.cancel_on_trigger && fanout.has_later_execution());
                if delayed && !will_be_cancelled && self.pending.len() >= MAX_PENDING_SCHEDULED_OUTPUT_EXECUTIONS {
                    blocked_by_delayed_capacity = true;
                    break;
                }

                let execution = &fanout.executions[fanout.execution_index];
                let delivery_policy = self.admitted_delivery_policy(
                    target,
                    &execution.param_overrides,
                    execution.invocation_id,
                    fanout.trigger_tick,
                );

                if let Some(delivery_policy) = delivery_policy {
                    if delayed {
                        flush_command_batch(ctx, &mut pending_batch);
                        if !will_be_cancelled {
                            self.schedule_at(
                                target.node,
                                fanout.trigger_elapsed_seconds + remaining,
                                execution.param_overrides.clone(),
                                execution.invocation_id,
                                delivery_policy,
                                target.batchable,
                            );
                        }
                    } else {
                        let forwarded = ModuleCommandExecuteEvent {
                            command_id: target.node,
                            param_overrides: execution.param_overrides.clone(),
                            invocation_id: execution.invocation_id,
                            delivery_policy,
                        };
                        if target.batchable {
                            append_command_batch(ctx, &mut pending_batch, forwarded);
                        } else {
                            flush_command_batch(ctx, &mut pending_batch);
                            let _ = module_command::emit_command_execute_with_invocation(
                                ctx,
                                target.node,
                                forwarded.param_overrides,
                                forwarded.invocation_id,
                                forwarded.delivery_policy,
                            );
                        }
                    }
                }

                fanout.advance();
                self.work_steps_this_tick += 1;
            }

            if !fanout.is_complete() {
                self.pending_fanout.push_front(fanout);
                if blocked_by_delayed_capacity || self.remaining_work_budget() == 0 {
                    break;
                }
            }
        }

        flush_command_batch(ctx, &mut pending_batch);
    }

    fn allocate_fanout_generation(&mut self) -> u64 {
        let generation = self.next_fanout_generation;
        self.next_fanout_generation = self
            .next_fanout_generation
            .checked_add(1)
            .expect("output fan-out generation exhausted");
        generation
    }

    fn retained_target_snapshot_count(&self) -> usize {
        let mut retained: Vec<&Arc<[OutputRuntimeTarget]>> = Vec::with_capacity(self.pending_fanout.len());
        let mut count = 0usize;
        for pending in &self.pending_fanout {
            if retained.iter().any(|targets| Arc::ptr_eq(targets, &pending.targets)) {
                continue;
            }
            count = count.saturating_add(pending.targets.len());
            retained.push(&pending.targets);
        }
        count
    }

    fn reject_fanout(&mut self, ctx: &ProcessCtx, owner: NodeId, execution_count: usize) {
        self.rejected_fanout_executions = self.rejected_fanout_executions.saturating_add(execution_count as u64);
        if self.last_fanout_rejection_log_tick == Some(ctx.time.tick) {
            return;
        }

        self.last_fanout_rejection_log_tick = Some(ctx.time.tick);
        let _ = golden_core::logwarning!(
            origin = owner;
            format!(
                "Output fan-out rejected {execution_count} execution(s): limits are {} executions per event, {} queued executions, {} queued jobs, and {} retained output targets",
                module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS,
                MAX_PENDING_OUTPUT_FANOUT_EXECUTIONS,
                MAX_PENDING_OUTPUT_FANOUT_JOBS,
                MAX_RETAINED_OUTPUT_FANOUT_TARGETS,
            )
        );
    }

    fn schedule_at(
        &mut self,
        target: NodeId,
        due_at: f64,
        param_overrides: ModuleCommandParamOverrides,
        invocation_id: Option<ModuleCommandInvocationId>,
        delivery_policy: ModuleCommandDeliveryPolicy,
        batchable: bool,
    ) {
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("output schedule sequence exhausted");
        self.pending.push(PendingOutput {
            target,
            due_at,
            sequence,
            param_overrides,
            invocation_id,
            delivery_policy,
            batchable,
        });
    }

    fn admitted_delivery_policy(
        &mut self,
        target: OutputRuntimeTarget,
        param_overrides: &ModuleCommandParamOverrides,
        invocation_id: Option<ModuleCommandInvocationId>,
        tick: u64,
    ) -> Option<ModuleCommandDeliveryPolicy> {
        if let (true, Some(invocation_id)) = (target.change_aware_log, invocation_id) {
            let key = OutputLogInvocationKey {
                target: target.node,
                invocation_id,
            };
            self.log_admission
                .should_emit(key, param_overrides, tick)
                .then_some(ModuleCommandDeliveryPolicy::ChangeAwareLogAdmitted)
        } else {
            Some(ModuleCommandDeliveryPolicy::Standard)
        }
    }

    fn reset_work_budget(&mut self, tick: u64) {
        if self.work_budget_tick != Some(tick) {
            self.work_budget_tick = Some(tick);
            self.work_steps_this_tick = 0;
        }
    }

    fn remaining_work_budget(&self) -> usize {
        MAX_SCHEDULED_OUTPUT_EXECUTIONS_PER_TICK.saturating_sub(self.work_steps_this_tick)
    }

    /// Advances compact fan-out continuations and delayed outputs while sharing
    /// one strict per-tick work budget between both queues.
    pub(super) fn tick(&mut self, ctx: &mut ProcessCtx, delta_seconds: f64) {
        if self.is_empty() {
            return;
        }

        if delta_seconds.is_finite() && delta_seconds > 0.0 {
            self.elapsed_seconds += delta_seconds;
        }
        self.reset_work_budget(ctx.time.tick);
        self.drain_fanout(ctx);

        let mut pending_batch: Option<(NodeId, Vec<ModuleCommandExecuteEvent>)> = None;
        while self.remaining_work_budget() > 0 {
            if !self
                .pending
                .peek()
                .is_some_and(|pending| pending.due_at <= self.elapsed_seconds + f64::EPSILON)
            {
                break;
            }
            let pending = self.pending.pop().expect("peeked pending output must remain available");
            if pending.batchable {
                let execution = ModuleCommandExecuteEvent {
                    command_id: pending.target,
                    param_overrides: pending.param_overrides,
                    invocation_id: pending.invocation_id,
                    delivery_policy: pending.delivery_policy,
                };
                append_command_batch(ctx, &mut pending_batch, execution);
            } else {
                flush_command_batch(ctx, &mut pending_batch);
                let _ = module_command::emit_command_execute_with_invocation(
                    ctx,
                    pending.target,
                    pending.param_overrides,
                    pending.invocation_id,
                    pending.delivery_policy,
                );
            }
            self.work_steps_this_tick += 1;
        }
        flush_command_batch(ctx, &mut pending_batch);
    }
}

fn append_command_batch(
    ctx: &mut ProcessCtx,
    pending: &mut Option<(NodeId, Vec<ModuleCommandExecuteEvent>)>,
    execution: ModuleCommandExecuteEvent,
) {
    match pending.as_mut() {
        Some((target, executions)) if *target == execution.command_id => executions.push(execution),
        Some(_) => {
            flush_command_batch(ctx, pending);
            *pending = Some((execution.command_id, vec![execution]));
        }
        None => *pending = Some((execution.command_id, vec![execution])),
    }
}

fn flush_command_batch(ctx: &mut ProcessCtx, pending: &mut Option<(NodeId, Vec<ModuleCommandExecuteEvent>)>) {
    let Some((command_id, executions)) = pending.take() else {
        return;
    };
    let _ = module_command::emit_command_execute_batch(ctx, command_id, executions);
}
