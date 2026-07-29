use super::*;

/// Maximum resolved command actions produced and serialized by one state-machine tick.
///
/// This is eight wire chunks at the current 512-execution chunk size. The hard
/// cap protects the engine thread from pathological multiplex/output fan-out.
/// Actions retain their deterministic lane/intent/target prefix; rejected
/// overflow is counted and surfaced as a throttled runtime warning.
pub(super) const MAX_RUNTIME_COMMAND_ACTIONS_PER_TICK: usize =
    crate::app::module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS * 8;

#[derive(Default)]
pub(super) struct RuntimeCommandDispatchPlanCache {
    pub(super) plans: HashMap<StableRef, RuntimeCommandDispatchPlan>,
    pub(super) dependencies: HashMap<StableRef, RuntimeCommandDependency>,
}

pub(super) struct RuntimeCommandDispatchPlan {
    pub(super) actions: Vec<RuntimeCommandDispatchAction>,
    pub(super) manager_with_children: bool,
    pub(super) truncated_actions: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RuntimeCommandDependency {
    pub(super) target_uuid: NodeUuid,
    pub(super) root: NodeId,
    pub(super) parent: Option<NodeId>,
}

pub(super) enum RuntimeCommandDispatchAction {
    Command {
        node: NodeId,
        contextual_params: Vec<NodeId>,
        batchable: bool,
    },
    Param(NodeId),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RuntimeCommandBudgetRejections {
    pub(super) actions: u64,
    pub(super) unresolved_intents: u64,
}

#[derive(Debug)]
pub(super) struct RuntimeCommandTickBudget {
    remaining_actions: usize,
    rejections: RuntimeCommandBudgetRejections,
}

impl Default for RuntimeCommandTickBudget {
    fn default() -> Self {
        Self {
            remaining_actions: MAX_RUNTIME_COMMAND_ACTIONS_PER_TICK,
            rejections: RuntimeCommandBudgetRejections::default(),
        }
    }
}

impl RuntimeCommandTickBudget {
    pub(super) fn is_exhausted(&self) -> bool {
        self.remaining_actions == 0
    }

    pub(super) fn reject_unresolved_intent(&mut self) {
        self.rejections.unresolved_intents += 1;
    }

    pub(super) fn rejections(&self) -> RuntimeCommandBudgetRejections {
        self.rejections
    }

    #[cfg(test)]
    pub(super) fn remaining_actions(&self) -> usize {
        self.remaining_actions
    }

    fn admit_plan(&mut self, plan: &RuntimeCommandDispatchPlan) -> usize {
        let admitted = plan.actions.len().min(self.remaining_actions);
        self.remaining_actions -= admitted;
        self.rejections.actions += plan
            .actions
            .len()
            .saturating_sub(admitted)
            .saturating_add(plan.truncated_actions) as u64;
        admitted
    }
}

impl RuntimeCommandDispatchPlanCache {
    pub(super) fn plan_for<'a>(
        &'a mut self,
        snapshot: &ProcessTreeSnapshot,
        processor_node: NodeId,
        target: &StableRef,
    ) -> &'a RuntimeCommandDispatchPlan {
        if !self.plans.contains_key(target) {
            let (plan, dependency) = build_runtime_command_dispatch_plan(snapshot, processor_node, target);
            if let Some(dependency) = dependency {
                self.dependencies.insert(target.clone(), dependency);
            }
            self.plans.insert(target.clone(), plan);
        }
        self.plans
            .get(target)
            .expect("a command dispatch plan was inserted for the requested target")
    }

    pub(super) fn invalidate_plans(&mut self) {
        self.plans.clear();
    }

    pub(super) fn reset(&mut self) {
        self.plans.clear();
        self.dependencies.clear();
    }

    pub(super) fn depends_on_change(
        &self,
        current: Option<&ProcessTreeSnapshot>,
        previous: Option<&ProcessTreeSnapshot>,
        changed: NodeId,
    ) -> bool {
        self.dependencies.values().any(|dependency| {
            current.is_some_and(|snapshot| command_dependency_contains(snapshot, *dependency, changed))
                || previous.is_some_and(|snapshot| command_dependency_contains(snapshot, *dependency, changed))
        })
    }

    pub(super) fn observes_change(&self, snapshot: Option<&ProcessTreeSnapshot>, changed: NodeId) -> bool {
        let Some(snapshot) = snapshot else {
            return false;
        };
        self.dependencies.values().any(|dependency| {
            node_is_within(snapshot, changed, dependency.root)
                || dependency.parent.is_some_and(|parent| {
                    changed == parent || snapshot.node(changed).is_some_and(|node| node.parent == Some(parent))
                })
        })
    }

    pub(super) fn listener_roots(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.dependencies.values().map(|dependency| dependency.root)
    }

    pub(super) fn listener_parents(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.dependencies.values().filter_map(|dependency| dependency.parent)
    }
}

#[derive(Default)]
pub(super) struct PendingRuntimeCommandBatch {
    pub(super) command: Option<NodeId>,
    pub(super) executions: Vec<crate::app::module_command::ModuleCommandExecuteEvent>,
    rejected_executions: u64,
    emission_error: Option<String>,
    #[cfg(test)]
    emitted_events: u64,
    #[cfg(test)]
    emitted_executions: u64,
}

impl PendingRuntimeCommandBatch {
    pub(super) fn push(
        &mut self,
        ctx: &mut ProcessCtx,
        command: NodeId,
        execution: crate::app::module_command::ModuleCommandExecuteEvent,
    ) {
        if self.command.is_some_and(|pending| pending != command) {
            self.flush(ctx);
        }
        self.command = Some(command);
        self.executions.push(execution);
        if self.executions.len() >= crate::app::module_command::MODULE_COMMAND_EXECUTE_BATCH_MAX_EXECUTIONS {
            self.flush(ctx);
        }
    }

    pub(super) fn flush(&mut self, ctx: &mut ProcessCtx) {
        let Some(command) = self.command.take() else {
            return;
        };
        let executions = std::mem::take(&mut self.executions);
        match crate::app::module_command::emit_command_execute_batch(ctx, command, executions) {
            Ok(emission) => {
                self.rejected_executions += emission.rejected_execution_count as u64;
                #[cfg(test)]
                {
                    self.emitted_events += emission.event_count as u64;
                    self.emitted_executions += emission.execution_count as u64;
                }
            }
            Err(error) => {
                self.emission_error.get_or_insert(error);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn take_emission_counts(&mut self) -> (u64, u64) {
        let counts = (self.emitted_events, self.emitted_executions);
        self.emitted_events = 0;
        self.emitted_executions = 0;
        counts
    }

    pub(super) fn take_emission_issue(&mut self) -> (u64, Option<String>) {
        (
            std::mem::take(&mut self.rejected_executions),
            self.emission_error.take(),
        )
    }

    pub(super) fn reject_execution(&mut self, error: String) {
        self.rejected_executions += 1;
        self.emission_error.get_or_insert(error);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct RuntimeCommandInvocationKey {
    context_key: ContextKey,
    source_node: Option<ANodeId>,
    source_socket: Option<SocketId>,
    target: Option<StableRef>,
}

pub(super) fn intern_runtime_command_invocation(
    streams_by_processor: &mut HashMap<NodeId, HashMap<RuntimeCommandInvocationKey, u64>>,
    next_stream: &mut u64,
    emitter: NodeId,
    processor_node: NodeId,
    context_key: Option<&ContextKey>,
    intent: &RuntimeIntent,
) -> crate::app::module_command::ModuleCommandInvocationId {
    let key = RuntimeCommandInvocationKey {
        context_key: context_key.cloned().unwrap_or_default(),
        source_node: intent.source_node,
        source_socket: intent.source_socket.clone(),
        target: intent.target.clone(),
    };
    let stream = streams_by_processor
        .entry(processor_node)
        .or_default()
        .entry(key)
        .or_insert_with(|| {
            let stream = *next_stream;
            *next_stream = next_stream
                .checked_add(1)
                .expect("state-machine command invocation stream exhausted");
            stream
        });
    crate::app::module_command::ModuleCommandInvocationId::new(emitter, *stream)
}

/// Dispatches one lane's command intent through its processor-scoped structural plan.
///
/// Target discovery and contextual-parameter collection are cached because the same
/// formula output commonly fires across thousands of multiplex lanes.
pub(super) struct RuntimeCommandDispatch<'a> {
    pub(super) processor_node: NodeId,
    pub(super) processor_id: ProcessorId,
    pub(super) context_key: Option<&'a ContextKey>,
    pub(super) context_provider: &'a SnapshotProcessorContextProvider,
    pub(super) live_param_values: &'a mut HashMap<NodeId, ParamValue>,
    pub(super) invocation_id: crate::app::module_command::ModuleCommandInvocationId,
    pub(super) intent: &'a RuntimeIntent,
    pub(super) plan: &'a RuntimeCommandDispatchPlan,
    pub(super) pending_batch: &'a mut PendingRuntimeCommandBatch,
}

pub(super) fn dispatch_command_intent(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    dispatch: RuntimeCommandDispatch<'_>,
    budget: &mut RuntimeCommandTickBudget,
) {
    let RuntimeCommandDispatch {
        processor_node,
        processor_id,
        context_key,
        context_provider,
        live_param_values,
        invocation_id,
        intent,
        plan,
        pending_batch,
    } = dispatch;
    let mut command_count = 0usize;
    let lane_resolver = context_key.map(|context_key| LaneParamResolver {
        processor_id,
        context_key,
        context_provider,
    });
    let admitted_actions = budget.admit_plan(plan);
    for action in plan.actions.iter().take(admitted_actions) {
        match action {
            RuntimeCommandDispatchAction::Command {
                node,
                contextual_params,
                batchable,
            } => {
                let param_overrides = resolved_output_param_overrides_for_params(
                    snapshot,
                    live_param_values,
                    contextual_params,
                    lane_resolver.as_ref(),
                );
                if *batchable {
                    pending_batch.push(
                        ctx,
                        *node,
                        crate::app::module_command::ModuleCommandExecuteEvent {
                            command_id: *node,
                            param_overrides,
                            invocation_id: Some(invocation_id),
                            delivery_policy: crate::app::module_command::ModuleCommandDeliveryPolicy::Standard,
                        },
                    );
                } else {
                    pending_batch.flush(ctx);
                    if let Err(error) = crate::app::module_command::emit_command_execute_with_invocation(
                        ctx,
                        *node,
                        param_overrides,
                        Some(invocation_id),
                        crate::app::module_command::ModuleCommandDeliveryPolicy::Standard,
                    ) {
                        pending_batch.reject_execution(error);
                    }
                }
                command_count += 1;
            }
            RuntimeCommandDispatchAction::Param(node) => {
                pending_batch.flush(ctx);
                match runtime_value_to_param(&intent.payload) {
                    Ok(value) => {
                        set_output_target_param(ctx, snapshot, live_param_values, *node, value);
                    }
                    Err(error) => pending_batch.reject_execution(error),
                }
            }
        }
    }

    // Only warn when an Outputs manager actually holds items but none fired — an
    // empty branch (e.g. an unused "On False") is a normal, silent no-op.
    if command_count == 0 && plan.actions.is_empty() && plan.truncated_actions == 0 && plan.manager_with_children {
        let target = intent
            .target
            .as_ref()
            .expect("a dispatch plan is only built for targeted command intents");
        log!(
            origin = processor_node;
            format!("Output dispatch: no command fired for target '{}'", target.stable_id)
        );
    }
}

/// Resolves the stable formula target into ordered, deduplicated live actions.
fn build_runtime_command_dispatch_plan(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    target: &StableRef,
) -> (RuntimeCommandDispatchPlan, Option<RuntimeCommandDependency>) {
    let mut roots = SmallVec::<[NodeId; 2]>::new();
    let direct = resolve_stable_ref_node(snapshot, target);
    if let Some(direct) = direct {
        roots.push(direct);
    }
    let dependency = direct
        .filter(|direct| !node_is_within(snapshot, *direct, processor_node))
        .and_then(|direct| external_command_dependency(snapshot, direct));
    let surface_decl_id = format!("surface/{}", target.stable_id);
    if let Some(surface) = find_descendant_by_decl_id(snapshot, processor_node, &surface_decl_id) {
        roots.push(surface);
    }

    let manager_with_children = roots.iter().copied().any(|node_id| {
        snapshot
            .node(node_id)
            .is_some_and(|node| node.node_type == crate::app::OutputsManager::NODE_TYPE)
            && !snapshot.child_ids_slice(node_id).is_empty()
    });
    let mut seen = HashSet::new();
    let mut actions = Vec::new();
    let mut truncated_actions = 0usize;
    for root in roots {
        if !seen.insert(root) {
            continue;
        }
        let batchable = crate::app::systems_alchemist_managed_nodes::is_output_container(snapshot, root);
        if node_is_command(snapshot, root) || batchable {
            push_dispatch_action(
                &mut actions,
                &mut truncated_actions,
                RuntimeCommandDispatchAction::Command {
                    node: root,
                    contextual_params: contextual_output_params(snapshot, root),
                    batchable,
                },
            );
            continue;
        }

        let mut has_enabled_commands = false;
        for child in snapshot.child_ids_slice(root).iter().copied() {
            if snapshot.node(child).is_some_and(|child| child.enabled) && node_is_command(snapshot, child) {
                has_enabled_commands = true;
                if actions.len() >= MAX_RUNTIME_COMMAND_ACTIONS_PER_TICK {
                    truncated_actions += 1;
                } else if seen.insert(child) {
                    push_dispatch_action(
                        &mut actions,
                        &mut truncated_actions,
                        RuntimeCommandDispatchAction::Command {
                            node: child,
                            contextual_params: contextual_output_params(snapshot, child),
                            batchable: false,
                        },
                    );
                }
            }
        }
        if !has_enabled_commands && snapshot.node(root).is_some_and(|node| node.param_value.is_some()) {
            push_dispatch_action(
                &mut actions,
                &mut truncated_actions,
                RuntimeCommandDispatchAction::Param(root),
            );
        }
    }
    (
        RuntimeCommandDispatchPlan {
            actions,
            manager_with_children,
            truncated_actions,
        },
        dependency,
    )
}

fn push_dispatch_action(
    actions: &mut Vec<RuntimeCommandDispatchAction>,
    truncated_actions: &mut usize,
    action: RuntimeCommandDispatchAction,
) {
    if actions.len() < MAX_RUNTIME_COMMAND_ACTIONS_PER_TICK {
        actions.push(action);
    } else {
        *truncated_actions += 1;
    }
}

fn external_command_dependency(snapshot: &ProcessTreeSnapshot, target: NodeId) -> Option<RuntimeCommandDependency> {
    let target_uuid = snapshot.node(target)?.uuid;
    Some(RuntimeCommandDependency {
        target_uuid,
        root: target,
        parent: snapshot.node(target)?.parent,
    })
}

fn command_dependency_contains(
    snapshot: &ProcessTreeSnapshot,
    dependency: RuntimeCommandDependency,
    changed: NodeId,
) -> bool {
    snapshot
        .node(changed)
        .is_some_and(|node| node.uuid == dependency.target_uuid)
        || node_is_within(snapshot, changed, dependency.root)
}

fn node_is_within(snapshot: &ProcessTreeSnapshot, node: NodeId, ancestor: NodeId) -> bool {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if candidate == ancestor {
            return true;
        }
        current = snapshot.node(candidate).and_then(|node| node.parent);
    }
    false
}

fn contextual_output_params(snapshot: &ProcessTreeSnapshot, root: NodeId) -> Vec<NodeId> {
    let mut params = Vec::new();
    collect_contextual_output_params(snapshot, root, &mut params);
    params
}

fn collect_contextual_output_params(snapshot: &ProcessTreeSnapshot, node_id: NodeId, params: &mut Vec<NodeId>) {
    let Some(node) = snapshot.node(node_id) else {
        return;
    };
    if node.param_value.is_some()
        && node
            .param_control
            .as_ref()
            .is_some_and(|control| control.mode != ParameterControlMode::Manual)
    {
        params.push(node_id);
    }
    for child in snapshot.child_ids_slice(node_id).iter().copied() {
        collect_contextual_output_params(snapshot, child, params);
    }
}

#[cfg(test)]
pub(super) fn resolved_output_param_overrides(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    lane_resolver: Option<&LaneParamResolver<'_>>,
) -> crate::app::module_command::ModuleCommandParamOverrides {
    let params = contextual_output_params(snapshot, root);
    resolved_output_param_overrides_for_params(snapshot, &HashMap::new(), &params, lane_resolver)
}

fn resolved_output_param_overrides_for_params(
    snapshot: &ProcessTreeSnapshot,
    live_param_values: &HashMap<NodeId, ParamValue>,
    params: &[NodeId],
    lane_resolver: Option<&LaneParamResolver<'_>>,
) -> crate::app::module_command::ModuleCommandParamOverrides {
    let Some(lane_resolver) = lane_resolver else {
        return Vec::new();
    };
    params
        .iter()
        .filter_map(|param_id| {
            lane_resolver
                .param_value_with_live(snapshot, live_param_values, *param_id)
                .map(|value| crate::app::module_command::ModuleCommandParamOverride {
                    param_id: *param_id,
                    value,
                })
        })
        .collect()
}

pub(super) fn set_output_target_param(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    live_param_values: &mut HashMap<NodeId, ParamValue>,
    node: NodeId,
    value: ParamValue,
) -> bool {
    let current = live_param_values
        .get(&node)
        .or_else(|| snapshot.node(node).and_then(|node| node.param_value.as_ref()));
    let Some(current) = current else {
        return false;
    };
    if !matches!(value, ParamValue::Trigger()) && current == &value {
        return false;
    }
    ctx.set_param(node, value.clone());
    live_param_values.insert(node, value);
    true
}

/// A command is a node whose type is a declared module- or generic-command item.
/// (Detecting by a `trigger` child is unreliable — triggers aren't persisted, so
/// a freshly-loaded command may not expose one until re-materialized.)
fn node_is_command(snapshot: &ProcessTreeSnapshot, node: NodeId) -> bool {
    snapshot.node(node).is_some_and(|node| {
        crate::app::declared_user_item_type_matches(
            &node.node_type,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
        ) || crate::app::declared_user_item_type_matches(
            &node.node_type,
            crate::app::systems_alchemist_generic_commands::GENERIC_COMMAND_ITEM_KIND,
        )
    })
}

fn resolve_stable_ref_node(snapshot: &ProcessTreeSnapshot, target: &StableRef) -> Option<NodeId> {
    let uuid = target.stable_id.parse::<uuid::Uuid>().map(NodeUuid).ok()?;
    snapshot.node_id_by_uuid(uuid)
}

fn find_descendant_by_decl_id(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    for child in snapshot.child_ids(parent) {
        if snapshot.node(child).is_some_and(|child| child.decl_id == decl_id) {
            return Some(child);
        }
        if let Some(found) = find_descendant_by_decl_id(snapshot, child, decl_id) {
            return Some(found);
        }
    }
    None
}
