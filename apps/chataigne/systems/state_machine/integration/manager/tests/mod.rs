use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use chataigne_alchemist::{
    primitive_node_registry, ANodeId, ANodeInstance, ANodeTypeId, AlchemistFormula, AlchemistGraphDomain, AxisSet,
    CompileCtx, ContextAxisId, ContextItemId, ContextKey, EvaluationCtx, ExecNodeId, FormulaContextContract, FormulaId,
    FormulaPropertySchema, FormulaSurface, InputSocketRef, OutputPreviewStatus, OutputSocketRef, RuntimeInputSnapshot,
    RuntimeIntent, RuntimeOutput, RuntimeRegistries, SocketId, StableRef, TriggerValue, ValueTypeId, ValueTypeRegistry,
};
use chataigne_state_machine::{
    ANodeOutputPreviewSample, DefaultProcessorContextProvider, Processor, ProcessorContextProvider,
    ProcessorDebugCapture, ProcessorExecutionPlan, ProcessorExecutionStrategy, ProcessorId, ProcessorLaneInspectionDto,
    ProcessorLifecycleEvent, ProcessorLifecyclePolicy, ProcessorOverviewLaneSelectionDto, ProcessorRuntime,
    ProcessorRuntimeOverviewDto, StateMachineProcessorOverviewDto,
};
use golden_core::{
    app::ProjectNode,
    edit::Edit,
    engine::{EngineTime, DEFAULT_RUNTIME_LOOP_MAX_FREQUENCY_HZ},
    node::{
        DashboardWidgetTargetDescriptor, DeclId, Folder, Node, NodeId, NodeUuid, PresentationHint,
        USER_CONTEXT_NODE_TYPE,
    },
    parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ParameterControlMode, ParameterControlSpec, ParameterControlState,
    },
    process_ctx::{ExecutionPhase, ProcessCtx, ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
    ui_sync::UiEditIntent,
};
use golden_values::Value as RuntimeValue;

use super::{
    collect_processor_lane_parameter_inspection, compile_processor_runtime_for_cache_rebuild,
    condition_manager_edge_previous, condition_manager_value, dispatch_command_intent,
    formula_default_output_preview_samples, intern_runtime_command_invocation, latest_param_value,
    merge_output_preview_snapshot, output_preview_signature, processor_formula_from_snapshot,
    processor_formula_source_ref, processor_override_value, processor_overview_publish_due, processor_overview_shard,
    processor_overview_topic, processor_preview_needs_hydration, processor_preview_plan,
    processor_requires_forced_recompute, processor_should_evaluate, requested_inactive_processor_nodes,
    requested_processor_overviews, resolve_multiplex_template_value, resolved_output_param_overrides,
    retain_requested_preview_snapshots, runtime_invalidation_for_node, runtime_param_change_requires_snapshot,
    selected_context_includes_lane, set_output_target_param, should_emit_runtime_log, sync_runtime_processor_warning,
    ActivePreviewSelection, ContextKeyCardinalityError, FormulaDefaultPreviewState, FormulaPreviewDemandLease,
    LaneParamResolver, PendingRuntimeCommandBatch, ProcessorContextAxisRuntime, ProcessorContextListRuntime,
    ProcessorContextRuntime, ProcessorContextScopeCache, ProcessorLanePreviewKey, ProcessorOverviewDemandLease,
    RuntimeCommandDispatch, RuntimeCommandDispatchAction, RuntimeCommandDispatchPlan, RuntimeCommandDispatchPlanCache,
    RuntimeCommandTickBudget, RuntimeFormulaPreviewMode, RuntimeInvalidation, RuntimeLogKey, RuntimeProcessor,
    SnapshotProcessorContextProvider, StateMachineManager, CONTEXT_LANE_LIMIT_WARNING, MAX_CACHED_CONTEXT_KEY_SETS,
    MAX_MATERIALIZED_CONTEXT_KEYS, MAX_RETAINED_CONTEXT_KEYS, MAX_RUNTIME_COMMAND_ACTIONS_PER_TICK,
    PROCESSOR_MANAGER_DECL_ID, STATE_ITEM_KIND, STATE_MACHINE_PROCESSOR_OVERVIEW_LANE_TOPIC,
};

mod command_dispatch;
mod context_cache;

fn context_axis(axis: ContextAxisId, name: &str, items: Vec<ContextItemId>) -> ProcessorContextAxisRuntime {
    let default_item = items.first().cloned().expect("test context axes need an item");
    ProcessorContextAxisRuntime {
        axis,
        name: name.to_owned(),
        item_indexes: items
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, item)| (item, index))
            .collect(),
        items,
        default_item,
    }
}

fn context_list(
    axis: ContextAxisId,
    symbol: &str,
    list_id: &str,
    entries: impl IntoIterator<Item = (ContextItemId, RuntimeValue)>,
) -> ProcessorContextListRuntime {
    ProcessorContextListRuntime {
        axis,
        symbol: symbol.to_owned(),
        list_id: list_id.to_owned(),
        entries: entries.into_iter().collect(),
    }
}

fn context_runtime(
    axes: Vec<ProcessorContextAxisRuntime>,
    lists: Vec<ProcessorContextListRuntime>,
) -> Arc<ProcessorContextRuntime> {
    let mut runtime = ProcessorContextRuntime {
        axes,
        lists,
        ..ProcessorContextRuntime::default()
    };
    runtime.rebuild_indexes();
    Arc::new(runtime)
}

fn context_provider(
    processor_id: ProcessorId,
    runtime: Arc<ProcessorContextRuntime>,
) -> SnapshotProcessorContextProvider {
    let mut provider = SnapshotProcessorContextProvider::default();
    provider.insert_processor_runtime(processor_id, runtime);
    provider
}
use crate::app::systems_alchemist_processor::{FormulaCatalog, FormulaSourceRef, PROCESSOR_FORMULA_SOURCE_DECL_ID};

fn context_scope_test_node(
    id: NodeId,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    next_sibling: Option<NodeId>,
    node_type: &str,
) -> ProcessTreeNodeSnapshot {
    ProcessTreeNodeSnapshot {
        id,
        uuid: NodeUuid::nil(),
        parent,
        first_child,
        next_sibling,
        node_type: node_type.to_owned(),
        decl_id: format!("node_{}", id.0),
        short_name: String::new(),
        label: format!("Node {}", id.0),
        tags: Vec::new(),
        presentation: PresentationHint::default(),
        enabled: true,
        can_be_disabled: false,
        child_count: usize::from(first_child.is_some()),
        param_value: None,
        param_constraints: None,
        param_control: None,
        dashboard_widget_target: DashboardWidgetTargetDescriptor::inspector_only(),
        script_properties: HashMap::new(),
        script_methods: Vec::new(),
    }
}

#[test]
fn processor_context_scope_cache_preserves_nearest_first_order_and_reuses_ancestors() {
    let root = NodeId(1);
    let manager = NodeId(2);
    let root_scope = NodeId(3);
    let first_processor = NodeId(4);
    let second_processor = NodeId(5);
    let manager_scope = NodeId(6);
    let processor_scope = NodeId(7);
    let nodes = HashMap::from([
        (root, context_scope_test_node(root, None, Some(manager), None, "root")),
        (
            manager,
            context_scope_test_node(manager, Some(root), Some(first_processor), Some(root_scope), "manager"),
        ),
        (
            root_scope,
            context_scope_test_node(root_scope, Some(root), None, None, USER_CONTEXT_NODE_TYPE),
        ),
        (
            first_processor,
            context_scope_test_node(
                first_processor,
                Some(manager),
                Some(processor_scope),
                Some(second_processor),
                "processor",
            ),
        ),
        (
            second_processor,
            context_scope_test_node(second_processor, Some(manager), None, Some(manager_scope), "processor"),
        ),
        (
            manager_scope,
            context_scope_test_node(manager_scope, Some(manager), None, None, USER_CONTEXT_NODE_TYPE),
        ),
        (
            processor_scope,
            context_scope_test_node(
                processor_scope,
                Some(first_processor),
                None,
                None,
                USER_CONTEXT_NODE_TYPE,
            ),
        ),
    ]);
    let snapshot = ProcessTreeSnapshot::new(root, nodes);
    let mut cache = ProcessorContextScopeCache::new(&snapshot);

    assert_eq!(
        cache.scopes_for(first_processor).as_ref(),
        &[processor_scope, manager_scope, root_scope]
    );
    let manager_scopes = cache.scopes_for(manager);
    let second_processor_scopes = cache.scopes_for(second_processor);
    assert_eq!(second_processor_scopes.as_ref(), &[manager_scope, root_scope]);
    assert!(Arc::ptr_eq(&manager_scopes, &second_processor_scopes));
}

#[test]
fn processor_context_scope_cache_stops_at_parent_cycles() {
    let first = NodeId(1);
    let second = NodeId(2);
    let first_scope = NodeId(3);
    let second_scope = NodeId(4);
    let nodes = HashMap::from([
        (
            first,
            context_scope_test_node(first, Some(second), Some(first_scope), None, "owner"),
        ),
        (
            second,
            context_scope_test_node(second, Some(first), Some(second_scope), None, "owner"),
        ),
        (
            first_scope,
            context_scope_test_node(first_scope, Some(first), None, None, USER_CONTEXT_NODE_TYPE),
        ),
        (
            second_scope,
            context_scope_test_node(second_scope, Some(second), None, None, USER_CONTEXT_NODE_TYPE),
        ),
    ]);
    let snapshot = ProcessTreeSnapshot::new(first, nodes);
    let mut cache = ProcessorContextScopeCache::new(&snapshot);

    assert_eq!(cache.scopes_for(first).as_ref(), &[first_scope, second_scope]);
}

#[test]
fn state_machine_manager_is_fixed_and_creates_states() {
    let mut manager = StateMachineManager::new();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    manager.init(&mut ctx);

    assert!(!manager.node_data().meta.user_permissions.can_remove_and_duplicate);
    assert_eq!(
        manager.execution_rule().update_rate,
        Some(DEFAULT_RUNTIME_LOOP_MAX_FREQUENCY_HZ),
        "state-machine processing must follow the engine loop instead of imposing a lower cap"
    );
    let items = manager.user_creatable_items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].node_type, crate::app::StateMachineState::NODE_TYPE);
    assert_eq!(items[0].item_kind, STATE_ITEM_KIND);
    assert_eq!(items[0].label, "State");

    let state = manager
        .create_user_item(crate::app::StateMachineState::NODE_TYPE)
        .expect("state machine manager should create state items");
    assert_eq!(state.get_type(), crate::app::StateMachineState::NODE_TYPE);
}

#[test]
fn preview_demand_expires_even_when_no_tree_snapshot_is_needed() {
    let mut manager = StateMachineManager::new();
    manager.runtime_cache.preview_demands.insert(
        "stale-preview".to_owned(),
        FormulaPreviewDemandLease {
            mode: RuntimeFormulaPreviewMode::FormulaDefaults(FormulaId::new("formula")),
            expires_at: Duration::from_secs(1),
        },
    );
    manager.runtime_cache.preview_demand_dirty = false;
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 2,
            micro: 0,
            seq: 0,
        },
    );
    ctx.runtime_elapsed = Duration::from_secs(2);

    manager.run_processors(&mut ctx);

    assert!(manager.runtime_cache.preview_demands.is_empty());
    assert!(manager.runtime_cache.preview_demand_dirty);
}

#[test]
fn processor_overview_demand_expires_without_enabling_debug_preview() {
    let mut manager = StateMachineManager::new();
    manager.runtime_cache.processor_overview_demands.insert(
        "stale-overview".to_owned(),
        ProcessorOverviewDemandLease {
            processor_ids: HashSet::new(),
            expires_at: Duration::from_secs(1),
        },
    );
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 2,
            micro: 0,
            seq: 0,
        },
    );
    ctx.runtime_elapsed = Duration::from_secs(2);

    manager.run_processors(&mut ctx);

    assert!(manager.runtime_cache.processor_overview_demands.is_empty());
    assert!(manager.runtime_cache.processor_overview_demand_dirty);
    assert!(manager.runtime_cache.preview_demands.is_empty());
}

#[test]
fn runtime_and_overview_rebuilds_share_one_tick_scoped_formula_catalog() {
    let root = NodeId(1);
    let snapshot = Arc::new(ProcessTreeSnapshot::new(
        root,
        HashMap::from([(root, context_scope_test_node(root, None, None, None, "root"))]),
    ));
    let mut manager = StateMachineManager::new();
    manager.runtime_cache.topology_dirty = true;
    manager.runtime_cache.context_provider_dirty = true;
    manager.runtime_cache.processor_overview_demands.insert(
        "active-overview".to_owned(),
        ProcessorOverviewDemandLease {
            processor_ids: HashSet::from([ProcessorId::new()]),
            expires_at: Duration::from_secs(1),
        },
    );
    manager.runtime_cache.processor_overview_demand_dirty = true;
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    ctx.set_tree_snapshot(snapshot);

    manager.run_processors(&mut ctx);

    assert_eq!(manager.runtime_cache.perf_stats.runtime_cache_rebuilds, 1);
    assert_eq!(
        manager.runtime_cache.perf_stats.formula_catalog_builds, 1,
        "runtime and overview materialization in one tick must share the catalog"
    );
}

#[test]
fn processor_overview_demands_union_visible_processors_across_panels() {
    let shared = ProcessorId::new();
    let first_only = ProcessorId::new();
    let second_only = ProcessorId::new();
    let leases = HashMap::from([
        (
            "first-panel".to_owned(),
            ProcessorOverviewDemandLease {
                processor_ids: HashSet::from([shared, first_only]),
                expires_at: Duration::from_secs(1),
            },
        ),
        (
            "second-panel".to_owned(),
            ProcessorOverviewDemandLease {
                processor_ids: HashSet::from([shared, second_only]),
                expires_at: Duration::from_secs(1),
            },
        ),
    ]);

    assert_eq!(
        requested_processor_overviews(&leases),
        HashSet::from([shared, first_only, second_only])
    );
}

#[test]
fn processor_overview_resolves_requested_inactive_processor_nodes_only() {
    let root = NodeId(1);
    let active_node = NodeId(2);
    let inactive_node = NodeId(3);
    let non_processor_node = NodeId(4);
    let active_id = ProcessorId::new();
    let inactive_id = ProcessorId::new();
    let non_processor_id = ProcessorId::new();
    let mut active = context_scope_test_node(active_node, Some(root), None, Some(inactive_node), "state_processor");
    active.uuid = NodeUuid(active_id.as_uuid());
    let mut inactive = context_scope_test_node(
        inactive_node,
        Some(root),
        None,
        Some(non_processor_node),
        "state_processor",
    );
    inactive.uuid = NodeUuid(inactive_id.as_uuid());
    inactive.enabled = false;
    let mut non_processor = context_scope_test_node(non_processor_node, Some(root), None, None, "folder");
    non_processor.uuid = NodeUuid(non_processor_id.as_uuid());
    let snapshot = ProcessTreeSnapshot::new(
        root,
        HashMap::from([
            (
                root,
                context_scope_test_node(root, None, Some(active_node), None, "root"),
            ),
            (active_node, active),
            (inactive_node, inactive),
            (non_processor_node, non_processor),
        ]),
    );

    assert_eq!(
        requested_inactive_processor_nodes(
            &snapshot,
            &[active_node],
            &HashSet::from([active_id, inactive_id, non_processor_id]),
        ),
        vec![inactive_node]
    );
}

#[test]
fn processor_overview_publication_is_capped_to_ui_rate() {
    let first = Duration::from_secs(4);
    assert!(processor_overview_publish_due(None, first));
    assert!(!processor_overview_publish_due(
        Some(first),
        first + Duration::from_millis(32)
    ));
    assert!(processor_overview_publish_due(
        Some(first),
        first + Duration::from_millis(33)
    ));
}

#[test]
fn processor_overview_topics_are_stably_sharded_by_uuid_prefix() {
    let processor_id = ProcessorId::from_uuid(
        "ab000000-0000-0000-0000-000000000000"
            .parse()
            .expect("fixed processor UUID should parse"),
    );

    assert_eq!(processor_overview_shard(processor_id), 0xab);
    assert_eq!(
        processor_overview_topic(0xab),
        "chataigne.state_machine.processor_overview.ab"
    );
}

#[test]
fn processor_overview_publication_emits_only_the_changed_shard() {
    let processor_id = ProcessorId::from_uuid(
        "ab000000-0000-0000-0000-000000000000"
            .parse()
            .expect("fixed processor UUID should parse"),
    );
    let shard = processor_overview_shard(processor_id);
    let overview = ProcessorRuntimeOverviewDto {
        processor_id: processor_id.to_string(),
        multiplex_lane_count: 2,
        preview_context_key: None,
        preview_lane_label: "Device #1".to_owned(),
        preview_index: 1,
        default_preview_index: 1,
        preview_overridden: false,
        condition_states: Vec::new(),
    };
    let mut manager = StateMachineManager::new();
    manager
        .runtime_cache
        .processor_overview_snapshot
        .insert(processor_id, overview.clone());
    manager.runtime_cache.dirty_processor_overview_shards.insert(shard);
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    manager.publish_processor_overview(&mut ctx);

    let Edit::EmitCustomEvent { event } = &ctx
        .edits
        .pending
        .last()
        .expect("changed overview shard should emit one event")
        .edit
    else {
        panic!("overview publication should emit a custom event");
    };
    assert_eq!(event.topic, processor_overview_topic(shard));
    let payload = event
        .payload_as::<StateMachineProcessorOverviewDto>()
        .expect("overview shard payload should decode");
    assert_eq!(payload.processors, vec![overview]);
}

#[test]
fn processor_overview_lane_selection_is_independent_from_formula_debug_demand() {
    let processor_id = ProcessorId::new();
    let payload = ProcessorOverviewLaneSelectionDto {
        processor_id: processor_id.to_string(),
        preview_index: Some(2),
    };
    let event =
        golden_core::events::CustomEvent::from_payload(STATE_MACHINE_PROCESSOR_OVERVIEW_LANE_TOPIC, None, &payload)
            .expect("overview lane selection should serialize");
    let mut manager = StateMachineManager::new();

    manager.apply_processor_overview_lane_selection(event);

    assert_eq!(
        manager
            .runtime_cache
            .processor_overview_lane_selections
            .get(&processor_id),
        Some(&2)
    );
    assert!(manager
        .runtime_cache
        .dirty_processor_overview_lanes
        .contains(&processor_id));
    assert!(manager.runtime_cache.preview_demands.is_empty());
}

#[test]
fn steady_single_lane_preview_captures_only_that_lane_without_rebuilding_catalog() {
    let processor_id = ProcessorId::new();
    let selected_lane = ContextKey::single("device", "selected");
    let selection = ActivePreviewSelection {
        formula_defaults: HashSet::new(),
        processor_lanes: HashMap::from([(processor_id, HashSet::from([selected_lane.clone()]))]),
    };

    let steady_plan = processor_preview_plan(&selection, processor_id, false);
    assert!(!steady_plan.force_evaluation);
    assert!(!steady_plan.refresh_lane_catalog);
    let ProcessorDebugCapture::ProcessorLanes { context_keys, .. } = steady_plan.capture else {
        panic!("selected processor lane must use targeted capture");
    };
    assert_eq!(context_keys.len(), 1);
    assert!(context_keys.contains(&selected_lane));

    let initial_plan = processor_preview_plan(&selection, processor_id, true);
    assert!(initial_plan.force_evaluation);
    assert!(initial_plan.refresh_lane_catalog);
}

#[test]
fn formula_default_preview_does_not_force_matching_processor_instances() {
    let selection = ActivePreviewSelection {
        formula_defaults: HashSet::from([FormulaId::new("shared-formula")]),
        processor_lanes: HashMap::new(),
    };

    for processor_id in (0..128).map(|_| ProcessorId::new()) {
        let plan = processor_preview_plan(&selection, processor_id, true);
        assert_eq!(plan.capture, ProcessorDebugCapture::Off);
        assert!(!plan.force_evaluation);
        assert!(!plan.refresh_lane_catalog);
    }
}

#[test]
fn dirty_source_wakes_only_its_indexed_processors() {
    let source = golden_core::node::NodeId(10);
    let affected = golden_core::node::NodeId(20);
    let unrelated = golden_core::node::NodeId(30);
    let mut manager = StateMachineManager::new();
    manager
        .runtime_cache
        .source_listener_processors
        .insert(source, HashSet::from([affected]));

    manager.mark_source_processors_dirty(source);

    assert_eq!(manager.runtime_cache.dirty_source_processors, HashSet::from([affected]));
    assert!(!manager.runtime_cache.dirty_source_processors.contains(&unrelated));
}

#[test]
fn states_do_not_accept_user_items() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(StateMachineManager::new().into(), None);
    engine.apply_edits().expect("state machine manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateMachineManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("state machine manager should exist");

    engine.add_user_item(crate::app::StateMachineState::new().into(), Some(manager_id));
    engine.add_user_item(crate::app::StateMachineState::new().into(), Some(manager_id));
    engine.apply_edits().expect("states should attach to the manager");

    let states = engine.process_tree_snapshot().child_ids(manager_id);
    assert_eq!(states.len(), 2);

    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: states[1],
        new_parent: states[0],
        new_prev_sibling: None,
    });
    assert!(!ack.success, "states must not accept nested user items");
}

#[test]
fn multiplex_template_tokens_resolve_indexes_and_lists_across_named_axes() {
    let processor_id = ProcessorId::new();
    let primary_axis = ContextAxisId::new("primary-axis");
    let secondary_axis = ContextAxisId::new("secondary-axis");
    let primary_items = [ContextItemId::new("p0"), ContextItemId::new("p1")];
    let secondary_items = [ContextItemId::new("s0"), ContextItemId::new("s1")];
    let provider = context_provider(
        processor_id,
        context_runtime(
            vec![
                context_axis(primary_axis.clone(), "Primary", primary_items.to_vec()),
                context_axis(secondary_axis.clone(), "Secondary", secondary_items.to_vec()),
            ],
            vec![
                context_list(
                    primary_axis.clone(),
                    "Names",
                    "primary-names",
                    [
                        (primary_items[0].clone(), RuntimeValue::String("first".into())),
                        (primary_items[1].clone(), RuntimeValue::String("second".into())),
                    ],
                ),
                context_list(
                    secondary_axis.clone(),
                    "Names",
                    "secondary-names",
                    [
                        (secondary_items[0].clone(), RuntimeValue::String("alpha".into())),
                        (secondary_items[1].clone(), RuntimeValue::String("beta".into())),
                    ],
                ),
            ],
        ),
    );
    let key = ContextKey::new([
        chataigne_alchemist::ContextKeyPart::new(primary_axis, primary_items[1].clone()),
        chataigne_alchemist::ContextKeyPart::new(secondary_axis, secondary_items[0].clone()),
    ]);

    let resolved = resolve_multiplex_template_value(
        "{index}/{index0}/{index:2}/{index0:Secondary}/{list:Names}/{list:Secondary:Names}",
        |token| provider.resolve_template_token(processor_id, &key, token),
    );
    assert_eq!(resolved, "2/1/1/0/second/alpha");

    let dto = provider.context_key_dto(processor_id, &key);
    assert_eq!(dto.parts[0].axis_label, "Primary");
    assert_eq!(dto.parts[0].item_label, "#2");
    assert_eq!(dto.parts[0].index, Some(1));
    assert_eq!(dto.parts[1].axis_label, "Secondary");
    assert_eq!(dto.parts[1].item_label, "#1");
    assert_eq!(dto.parts[1].index, Some(0));
    assert_eq!(
        provider.context_key_label(processor_id, &key),
        "Primary #2 × Secondary #1"
    );
}

#[test]
fn processor_preview_indexes_compose_multiplex_defaults_without_materializing_lanes() {
    let processor_id = ProcessorId::new();
    let primary_axis = ContextAxisId::new("primary");
    let secondary_axis = ContextAxisId::new("secondary");
    let primary_items = [ContextItemId::new("p0"), ContextItemId::new("p1")];
    let secondary_items = [
        ContextItemId::new("s0"),
        ContextItemId::new("s1"),
        ContextItemId::new("s2"),
    ];
    let mut primary = context_axis(primary_axis.clone(), "Primary", primary_items.to_vec());
    primary.default_item = primary_items[1].clone();
    let mut secondary = context_axis(secondary_axis.clone(), "Secondary", secondary_items.to_vec());
    secondary.default_item = secondary_items[2].clone();
    let provider = context_provider(processor_id, context_runtime(vec![primary, secondary], Vec::new()));
    let axes = AxisSet::from_iter([primary_axis.clone(), secondary_axis.clone()]);

    let default_key = provider
        .default_context_key(processor_id, &axes)
        .expect("axis defaults should form a processor context key");
    assert_eq!(
        provider.preview_index_for_context_key(processor_id, &axes, &default_key),
        Some(6)
    );
    assert!(provider.context_key_uses_axis_defaults(processor_id, &default_key));

    let override_key = provider
        .context_key_at_preview_index(processor_id, &axes, 2)
        .expect("a valid one-based preview index should resolve directly");
    assert_eq!(
        override_key,
        ContextKey::new([
            chataigne_alchemist::ContextKeyPart::new(primary_axis, primary_items[0].clone()),
            chataigne_alchemist::ContextKeyPart::new(secondary_axis, secondary_items[1].clone()),
        ])
    );
}

#[test]
fn processor_preview_lane_observes_conditions_on_axis_subsets() {
    let selected = ContextKey::new([
        chataigne_alchemist::ContextKeyPart::new("device", "right"),
        chataigne_alchemist::ContextKeyPart::new("channel", "blue"),
    ]);
    let device_condition_lane = ContextKey::single("device", "right");
    let other_device_lane = ContextKey::single("device", "left");

    assert!(selected_context_includes_lane(&selected, &device_condition_lane));
    assert!(selected_context_includes_lane(&selected, &ContextKey::default_lane()));
    assert!(!selected_context_includes_lane(&selected, &other_device_lane));
}

#[test]
fn multiplex_index_context_links_resolve_both_index_bases() {
    let processor_id = ProcessorId::new();
    let axis = ContextAxisId::new("axis");
    let items = [ContextItemId::new("first"), ContextItemId::new("second")];
    let provider = context_provider(
        processor_id,
        context_runtime(vec![context_axis(axis.clone(), "Rows", items.to_vec())], Vec::new()),
    );
    let key = ContextKey::single("axis", "second");

    for (zero_based, expected) in [(false, 2), (true, 1)] {
        let symbol = golden_core::contexts::multiplex_index_context_link_symbol("axis", zero_based);
        let (resolved_axis, path) = provider
            .multiplex_link_for_symbol(processor_id, symbol.as_str())
            .expect("index link should resolve its multiplex axis");
        assert_eq!(
            provider.resolve_context_value(&key, &resolved_axis, &path),
            Some(RuntimeValue::Int(expected))
        );
    }
}

#[test]
fn condition_manager_value_only_fires_transition_edges() {
    let mut next_trigger_edge_id = 0;
    let transition = condition_manager_value(7, true, Some(false), &mut next_trigger_edge_id);
    assert!(transition.on_true.fired);
    assert!(!transition.on_false.fired);

    let steady = condition_manager_value(7, true, Some(true), &mut next_trigger_edge_id);
    assert!(!steady.on_true.fired);
    assert!(!steady.on_false.fired);
}

#[test]
fn initial_condition_observation_without_dirty_source_is_not_an_edge() {
    assert_eq!(condition_manager_edge_previous(None, true, false), Some(true));
    assert_eq!(condition_manager_edge_previous(None, false, false), Some(false));
    assert_eq!(condition_manager_edge_previous(None, true, true), None);
}

#[test]
fn condition_recompute_after_prior_state_can_emit_edge() {
    assert_eq!(condition_manager_edge_previous(Some(false), true, true), Some(false));
    assert_eq!(condition_manager_edge_previous(Some(true), false, true), Some(true));
    assert_eq!(condition_manager_edge_previous(Some(false), true, false), Some(false));
    assert_eq!(condition_manager_edge_previous(Some(true), false, false), Some(true));
}

#[test]
fn processor_evaluation_requires_runtime_or_signal_reason() {
    assert!(!processor_should_evaluate(false, false, false, false, false));
    assert!(processor_should_evaluate(true, false, false, false, false));
    assert!(processor_should_evaluate(false, true, false, false, false));
    assert!(processor_should_evaluate(false, false, true, false, false));
    assert!(processor_should_evaluate(false, false, false, true, false));
    assert!(processor_should_evaluate(false, false, false, false, true));
}

#[test]
fn processor_override_only_forces_its_own_runtime() {
    let changed = NodeId(41);
    let unchanged = NodeId(42);
    let dirty_overrides = HashSet::from([changed]);

    assert!(processor_requires_forced_recompute(false, changed, &dirty_overrides));
    assert!(!processor_requires_forced_recompute(false, unchanged, &dirty_overrides));
    assert!(processor_requires_forced_recompute(true, unchanged, &HashSet::new()));
}

#[test]
fn pending_transient_condition_reset_reuses_runtime_snapshot() {
    let mut manager = StateMachineManager::new();
    manager.runtime_cache.topology_dirty = false;

    assert!(!manager.update_requires_tree_snapshot());

    manager
        .runtime_cache
        .transient_condition_valid_resets
        .insert(manager.id(), 1);

    assert!(!manager.update_requires_tree_snapshot());
}

#[test]
fn live_source_cache_reads_the_latest_value_from_the_inbox_batch() {
    let param = NodeId(42);
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );
    for value in [1.0, 2.0] {
        ctx.events.push_shared(Arc::new(golden_core::events::Event {
            time: ctx.time,
            kind: golden_core::events::EventKind::ParamChanged {
                param,
                old_value: ParamValue::Float(value - 1.0),
                new_value: ParamValue::Float(value),
            },
        }));
    }

    assert_eq!(latest_param_value(&ctx, param), Some(ParamValue::Float(2.0)));
}

#[test]
fn continuous_processor_aggregate_tracks_runtime_cache_replacement() {
    let formula = continuous_formula();
    let processor = Processor::from_formula("Continuous processor", &formula);
    let mut runtime = ProcessorRuntime::new(processor.id);
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    assert!(runtime.compile(&processor, &formula, &compile_ctx));

    let mut manager = StateMachineManager::new();
    manager.runtime_cache.topology_dirty = false;
    manager.runtime_cache.replace_processors(HashMap::from([(
        NodeId(42),
        RuntimeProcessor {
            processor,
            runtime,
            compile_warning: None,
            formula,
            formula_node: None,
            formula_ui: chataigne_state_machine::ProcessorFormulaUiState::project(),
            formula_source_key: "test".to_owned(),
            command_dispatch_plans: Default::default(),
        },
    )]));

    assert_eq!(manager.runtime_cache.continuous_processor_count, 1);
    assert!(!manager.update_requires_tree_snapshot());

    manager.runtime_cache.replace_processors(HashMap::new());

    assert_eq!(manager.runtime_cache.continuous_processor_count, 0);
    assert!(!manager.update_requires_tree_snapshot());
}

#[test]
fn removed_formula_default_lease_prunes_continuous_runtime_while_other_demand_remains() {
    let formula = continuous_formula();
    let formula_id = formula.id.clone();
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut manager = StateMachineManager::new();
    manager.runtime_cache.topology_dirty = false;
    let compiled = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("continuous formula should compile");
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let eval_ctx = EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let provider = DefaultProcessorContextProvider;
    let capture = ProcessorDebugCapture::Off;
    let runtime_cache = &mut manager.runtime_cache;
    let _ = formula_default_output_preview_samples(
        FormulaDefaultPreviewState {
            cache: &mut runtime_cache.formula_default_previews,
            continuous_count: &mut runtime_cache.continuous_formula_default_preview_count,
        },
        compiled,
        &formula,
        &eval_ctx,
        &provider,
        &capture,
        false,
    );

    assert_eq!(manager.runtime_cache.continuous_formula_default_preview_count, 1);
    assert!(!manager.update_requires_tree_snapshot());

    manager.runtime_cache.preview_demands.insert(
        "formula-default".to_owned(),
        FormulaPreviewDemandLease {
            mode: RuntimeFormulaPreviewMode::FormulaDefaults(formula_id),
            expires_at: Duration::from_secs(60),
        },
    );
    manager.runtime_cache.preview_demands.insert(
        "other-lane".to_owned(),
        FormulaPreviewDemandLease {
            mode: RuntimeFormulaPreviewMode::ProcessorLane {
                processor_id: ProcessorId::new(),
                context_key: ContextKey::default_lane(),
            },
            expires_at: Duration::from_secs(60),
        },
    );
    manager.runtime_cache.preview_demands.remove("formula-default");
    let selection = ActivePreviewSelection::from_leases(&manager.runtime_cache.preview_demands);
    assert!(!selection.is_empty());
    assert!(selection.formula_defaults.is_empty());

    manager
        .runtime_cache
        .retain_formula_default_previews(&selection.formula_defaults);

    assert!(manager.runtime_cache.formula_default_previews.is_empty());
    assert_eq!(manager.runtime_cache.continuous_formula_default_preview_count, 0);
    assert!(!manager.update_requires_tree_snapshot());
}

#[test]
fn topology_rebuild_reuses_unchanged_compiled_formulas() {
    let formula = continuous_formula();
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut manager = StateMachineManager::new();
    let compiled_before = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("formula should compile");
    manager.runtime_cache.formula_catalog_initialized = true;
    manager.runtime_cache.topology_dirty = true;

    let root: crate::app::AppNode = Folder::new("root").into();
    let engine = crate::app::AppEngine::new(root);
    manager.refresh_formula_cache(&engine.process_tree_snapshot());

    let compiled_after = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("cached formula should remain compiled");
    assert!(Arc::ptr_eq(&compiled_before, &compiled_after));
    assert_eq!(manager.runtime_cache.perf_stats.formula_compiles, 1);
}

#[test]
fn project_formula_catalog_refresh_preserves_compiled_builtin_formulas() {
    let formula = continuous_formula();
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut manager = StateMachineManager::new();
    let compiled_before = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("formula should compile");
    manager.runtime_cache.formula_catalog_initialized = true;
    manager.runtime_cache.formula_catalog_dirty = true;

    let root: crate::app::AppNode = Folder::new("root").into();
    let engine = crate::app::AppEngine::new(root);
    manager.refresh_formula_cache(&engine.process_tree_snapshot());

    let compiled_after = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("built-in formula should remain compiled");
    assert!(Arc::ptr_eq(&compiled_before, &compiled_after));
    assert_eq!(manager.runtime_cache.perf_stats.formula_compiles, 1);
}

#[test]
fn deleted_state_uses_previous_snapshot_and_preserves_compiled_formulas() {
    let formula = continuous_formula();
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut manager = StateMachineManager::new();
    let compiled_before = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("formula should compile");
    manager.runtime_cache.formula_catalog_initialized = true;
    Arc::make_mut(&mut manager.runtime_cache.formulas).insert(NodeUuid::nil(), formula.clone());

    let root = NodeId(100);
    let manager_id = manager.id();
    let state = NodeId(101);
    let previous = ProcessTreeSnapshot::new(
        root,
        HashMap::from([
            (
                root,
                context_scope_test_node(root, None, Some(manager_id), None, "root"),
            ),
            (
                manager_id,
                context_scope_test_node(
                    manager_id,
                    Some(root),
                    Some(state),
                    None,
                    StateMachineManager::NODE_TYPE,
                ),
            ),
            (
                state,
                context_scope_test_node(
                    state,
                    Some(manager_id),
                    None,
                    None,
                    crate::app::StateMachineState::NODE_TYPE,
                ),
            ),
        ]),
    );
    let current = ProcessTreeSnapshot::new(
        root,
        HashMap::from([
            (
                root,
                context_scope_test_node(root, None, Some(manager_id), None, "root"),
            ),
            (
                manager_id,
                context_scope_test_node(manager_id, Some(root), None, None, StateMachineManager::NODE_TYPE),
            ),
        ]),
    );
    manager.runtime_cache.runtime_snapshot = Some(Arc::new(previous));

    let invalidation = manager.runtime_invalidation_for_change(&current, state);
    assert_eq!(invalidation, RuntimeInvalidation::Topology);
    manager.apply_runtime_invalidation(invalidation);
    assert!(!manager.runtime_cache.formula_catalog_dirty);
    manager.refresh_formula_cache(&current);

    let compiled_after = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("ordinary topology deletion must preserve compiled formulas");
    assert!(Arc::ptr_eq(&compiled_before, &compiled_after));
    assert_eq!(manager.runtime_cache.perf_stats.formula_compiles, 1);
}

#[test]
fn deleted_formula_uses_previous_snapshot_and_evicts_its_compiled_program() {
    let formula = continuous_formula();
    let formula_uuid = NodeUuid(uuid::Uuid::from_u128(0xfeed));
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut manager = StateMachineManager::new();
    let compiled_before = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("formula should compile");
    manager.runtime_cache.formula_catalog_initialized = true;
    Arc::make_mut(&mut manager.runtime_cache.formulas).insert(formula_uuid, formula.clone());

    let root = NodeId(200);
    let formula_node = NodeId(201);
    let mut formula_snapshot = context_scope_test_node(formula_node, Some(root), None, None, "alchemist_formula");
    formula_snapshot.uuid = formula_uuid;
    let previous = ProcessTreeSnapshot::new(
        root,
        HashMap::from([
            (
                root,
                context_scope_test_node(root, None, Some(formula_node), None, "root"),
            ),
            (formula_node, formula_snapshot),
        ]),
    );
    let current = ProcessTreeSnapshot::new(
        root,
        HashMap::from([(root, context_scope_test_node(root, None, None, None, "root"))]),
    );
    manager.runtime_cache.runtime_snapshot = Some(Arc::new(previous));

    let invalidation = manager.runtime_invalidation_for_change(&current, formula_node);
    assert_eq!(invalidation, RuntimeInvalidation::Formula(formula_uuid));
    manager.apply_runtime_invalidation(invalidation);
    assert!(!manager.runtime_cache.formula_catalog_dirty);
    manager.refresh_formula_cache(&current);

    assert!(!manager.runtime_cache.formulas.contains_key(&formula_uuid));
    let compiled_after = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("deleted formula should compile again if requested");
    assert!(!Arc::ptr_eq(&compiled_before, &compiled_after));
    assert_eq!(manager.runtime_cache.perf_stats.formula_compiles, 2);
}

#[test]
fn processor_root_invalidation_rebuilds_topology_but_descendants_are_local() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(StateMachineManager::new().into(), None);
    engine.apply_edits().expect("state machine manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateMachineManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("state machine manager should exist");

    engine.add_user_item(crate::app::StateMachineState::new().into(), Some(manager_id));
    engine.apply_edits().expect("state should attach to the manager");
    let snapshot = engine.process_tree_snapshot();
    let state_id = snapshot
        .child_ids(manager_id)
        .into_iter()
        .find(|state| {
            snapshot
                .node(*state)
                .is_some_and(|node| node.node_type == crate::app::StateMachineState::NODE_TYPE)
        })
        .expect("state should exist");
    let processor_manager_id = snapshot
        .find_child_by_decl_id(state_id, PROCESSOR_MANAGER_DECL_ID)
        .expect("state should have a processor manager");

    engine.add_user_item(crate::app::StateProcessor::new().into(), Some(processor_manager_id));
    engine
        .apply_edits()
        .expect("processor should attach to the processor manager");
    let snapshot = engine.process_tree_snapshot();
    let processor_id = snapshot
        .child_ids(processor_manager_id)
        .into_iter()
        .find(|processor| {
            snapshot
                .node(*processor)
                .is_some_and(|node| node.node_type == crate::app::StateProcessor::NODE_TYPE)
        })
        .expect("processor should exist");
    let formula_source_key = snapshot
        .find_child_by_decl_id(processor_id, PROCESSOR_FORMULA_SOURCE_DECL_ID)
        .expect("processor should have a formula source key child");

    assert_eq!(
        runtime_invalidation_for_node(&snapshot, manager_id, processor_id),
        RuntimeInvalidation::Topology
    );
    assert_eq!(
        runtime_invalidation_for_node(&snapshot, manager_id, formula_source_key),
        RuntimeInvalidation::Processor(processor_id)
    );
}

#[test]
fn duplicated_processor_marks_manager_topology_dirty_after_queued_structure_dispatch() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(StateMachineManager::new().into(), None);
    engine.apply_edits().expect("state machine manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateMachineManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("state machine manager should exist");

    engine.add_user_item(crate::app::StateMachineState::new().into(), Some(manager_id));
    engine.apply_edits().expect("state should attach to the manager");
    let snapshot = engine.process_tree_snapshot();
    let state_id = snapshot
        .child_ids(manager_id)
        .into_iter()
        .find(|state| {
            snapshot
                .node(*state)
                .is_some_and(|node| node.node_type == crate::app::StateMachineState::NODE_TYPE)
        })
        .expect("state should exist");
    let processor_manager_id = snapshot
        .find_child_by_decl_id(state_id, PROCESSOR_MANAGER_DECL_ID)
        .expect("state should have a processor manager");

    engine.add_user_item(crate::app::StateProcessor::new().into(), Some(processor_manager_id));
    engine
        .apply_edits()
        .expect("processor should attach to the processor manager");
    run_manager_runtime_tick(&mut engine);
    assert!(
        !manager_topology_dirty(&engine, manager_id),
        "initial processor topology should be rebuilt before duplication",
    );

    let snapshot = engine.process_tree_snapshot();
    let processor_id = snapshot
        .child_ids(processor_manager_id)
        .into_iter()
        .find(|processor| {
            snapshot
                .node(*processor)
                .is_some_and(|node| node.node_type == crate::app::StateProcessor::NODE_TYPE)
        })
        .expect("processor should exist");
    engine
        .duplicate_subtree_with(
            processor_id,
            processor_manager_id,
            Some(processor_id),
            Some("Processor Copy".to_owned()),
            |node| node.project_encode_data(),
            <crate::app::AppNode as ProjectNode>::project_decode_node,
        )
        .expect("processor duplicate should succeed");

    assert!(
        !manager_topology_dirty(&engine, manager_id),
        "duplicating a processor should not synchronously rebuild state-machine topology",
    );

    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EngineTick)
        .expect("queued duplicate structure events should dispatch");

    assert!(
        manager_topology_dirty(&engine, manager_id),
        "queued duplicate structure events must wake the state-machine runtime before evaluation",
    );
}

#[test]
fn cache_rebuild_compile_preserves_stateful_trigger_memory() {
    let formula = stateful_trigger_formula();
    let mut processor = Processor::from_formula("Processor", &formula);
    processor.lifecycle = ProcessorLifecyclePolicy::AlwaysActive;
    let mut runtime = ProcessorRuntime::new(processor.id);
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    assert!(compile_processor_runtime_for_cache_rebuild(
        &mut runtime,
        &processor,
        &formula,
        &compile_ctx,
    ));
    runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::ProjectStart);

    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let provider = DefaultProcessorContextProvider;
    let capture = ProcessorDebugCapture::All { history_len: 64 };
    let first_ctx = EvaluationCtx {
        logical_tick: 1,
        delta_time: std::time::Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let first = runtime
        .evaluate_processor_with_context_provider_and_runtime_capture(&processor, &first_ctx, &provider, &capture);
    assert_eq!(first.len(), 1);
    assert!(runtime_output_trigger_fired(&first[0].output));

    assert!(compile_processor_runtime_for_cache_rebuild(
        &mut runtime,
        &processor,
        &formula,
        &compile_ctx,
    ));
    let second_ctx = EvaluationCtx {
        logical_tick: 2,
        delta_time: std::time::Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let second = runtime.evaluate_processor_with_context_provider_and_runtime_capture(
        &processor,
        &second_ctx,
        &provider,
        &capture,
    );

    assert_eq!(second.len(), 1);
    assert!(!runtime_output_trigger_fired(&second[0].output));
    assert_eq!(runtime.lanes.memory_count(), 1);
}

#[test]
fn manager_shared_compile_cache_compiles_formula_once() {
    let formula = stateful_trigger_formula();
    let mut manager = StateMachineManager::new();
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };

    let first = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("formula should compile");
    let second = manager
        .shared_compiled_formula(&formula, &compile_ctx)
        .expect("formula should come from cache");

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(manager.runtime_perf_stats().formula_compiles, 1);
}

fn run_manager_runtime_tick(engine: &mut crate::app::AppEngine) {
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("state-machine inbox should dispatch");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("state-machine runtime tick should run");
    engine.apply_edits().expect("state-machine runtime edits should apply");
}

fn manager_topology_dirty(engine: &crate::app::AppEngine, manager_id: golden_core::node::NodeId) -> bool {
    let crate::app::AppNode::StateMachineManager(manager) = engine
        .nodes
        .get(manager_id)
        .expect("state machine manager should exist")
    else {
        panic!("expected StateMachineManager node");
    };
    manager.runtime_topology_dirty()
}

fn runtime_output_trigger_fired(output: &RuntimeOutput) -> bool {
    output
        .debug_samples
        .iter()
        .any(|sample| matches!(&sample.value, RuntimeValue::Trigger(trigger) if trigger.fired))
}

fn stateful_trigger_formula() -> AlchemistFormula {
    trigger_formula(true)
}

fn continuous_formula() -> AlchemistFormula {
    trigger_formula(false)
}

fn trigger_formula(process_on_input_change_only: bool) -> AlchemistFormula {
    let mut graph = AlchemistGraphDomain::new_document();
    let mut transaction = chataigne_alchemist::AlchemistGraphTransaction::for_document(&graph);
    let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    constant.config.set("value", RuntimeValue::Bool(true));
    constant.config.set(
        chataigne_alchemist::PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG,
        RuntimeValue::Bool(process_on_input_change_only),
    );
    let source = constant.id;
    AlchemistGraphDomain::insert_node(&mut transaction, constant);
    let edge = ANodeInstance::new(ANodeTypeId::new("trigger_on_off"), "Trigger On/Off");
    let edge_id = edge.id;
    AlchemistGraphDomain::insert_node(&mut transaction, edge);
    AlchemistGraphDomain::connect(
        &mut transaction,
        &graph,
        OutputSocketRef::new(source, "value"),
        InputSocketRef::new(edge_id, "value"),
    );
    transaction
        .commit(&mut graph, &AlchemistGraphDomain::with_primitives())
        .unwrap();
    AlchemistFormula {
        id: FormulaId::new("test"),
        version: 1,
        label: "Test".into(),
        description: None,
        tags: Vec::new(),
        graph,
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

fn preview_sample(
    formula_id: FormulaId,
    processor_id: ProcessorId,
    author_node_id: ANodeId,
    socket: &str,
    value: RuntimeValue,
    logical_tick: u64,
) -> ANodeOutputPreviewSample {
    ANodeOutputPreviewSample {
        formula_id,
        processor_id: Some(processor_id),
        context_key: None,
        author_node_id,
        exec_node: ExecNodeId::new(0),
        output_socket: SocketId::new(socket),
        value_type: value.value_type(),
        value,
        logical_tick,
        status: OutputPreviewStatus::Live,
    }
}

#[test]
fn output_preview_snapshot_retains_latest_values_absent_from_next_delta() {
    let formula_id = FormulaId::new("formula");
    let processor_id = ProcessorId::new();
    let condition_node = ANodeId::new();
    let value_node = ANodeId::new();
    let mut snapshot = HashMap::new();

    let condition_valid = preview_sample(
        formula_id.clone(),
        processor_id,
        condition_node,
        "valid",
        RuntimeValue::Bool(true),
        10,
    );
    let first_frame = merge_output_preview_snapshot(&mut snapshot, vec![condition_valid.clone()]);
    assert_eq!(first_frame, vec![condition_valid.clone()]);

    let value_update = preview_sample(
        formula_id,
        processor_id,
        value_node,
        "value",
        RuntimeValue::Float(0.75),
        11,
    );
    let second_frame = merge_output_preview_snapshot(&mut snapshot, vec![value_update.clone()]);

    assert_eq!(second_frame.len(), 2);
    assert!(second_frame.contains(&condition_valid));
    assert!(second_frame.contains(&value_update));
}

#[test]
fn requested_lane_returned_then_suppressed_clears_retained_preview_state() {
    let processor_id = ProcessorId::new();
    let context_key = ContextKey::single("device", "selected");
    let mut sample = preview_sample(
        FormulaId::new("formula"),
        processor_id,
        ANodeId::new(),
        "value",
        RuntimeValue::Float(0.75),
        10,
    );
    sample.context_key = Some(context_key.clone());
    let mut output_snapshot = HashMap::new();
    merge_output_preview_snapshot(&mut output_snapshot, vec![sample]);

    let inspection_key = ProcessorLanePreviewKey::new(processor_id, Some(&context_key));
    let mut inspection_snapshot = HashMap::from([(
        inspection_key,
        ProcessorLaneInspectionDto {
            processor_id: processor_id.to_string(),
            context_key: Some((&context_key).into()),
            parameter_values: Vec::new(),
            condition_states: Vec::new(),
        },
    )]);
    let selection = ActivePreviewSelection {
        formula_defaults: HashSet::new(),
        processor_lanes: HashMap::from([(processor_id, HashSet::from([context_key.clone()]))]),
    };
    let mut evaluated_lanes = HashMap::from([(processor_id, HashSet::from([context_key.clone()]))]);
    assert!(!processor_preview_needs_hydration(
        &inspection_snapshot,
        processor_id,
        selection.processor_lanes(processor_id),
    ));

    retain_requested_preview_snapshots(
        &mut output_snapshot,
        &mut inspection_snapshot,
        &selection,
        &evaluated_lanes,
    );
    assert_eq!(output_snapshot.len(), 1);
    assert_eq!(inspection_snapshot.len(), 1);

    evaluated_lanes.insert(processor_id, HashSet::new());
    retain_requested_preview_snapshots(
        &mut output_snapshot,
        &mut inspection_snapshot,
        &selection,
        &evaluated_lanes,
    );
    assert!(output_snapshot.is_empty());
    assert!(inspection_snapshot.is_empty());
    assert!(processor_preview_needs_hydration(
        &inspection_snapshot,
        processor_id,
        selection.processor_lanes(processor_id),
    ));
}

#[test]
fn output_preview_signature_is_order_independent_and_trigger_sensitive() {
    let formula_id = FormulaId::new("formula");
    let processor_id = ProcessorId::new();
    let value_node = ANodeId::new();
    let trigger_node = ANodeId::new();
    let value = preview_sample(
        formula_id.clone(),
        processor_id,
        value_node,
        "value",
        RuntimeValue::Float(0.75),
        10,
    );
    let trigger = preview_sample(
        formula_id.clone(),
        processor_id,
        trigger_node,
        "trigger",
        RuntimeValue::Trigger(TriggerValue::fired(7, 10)),
        10,
    );
    let value_at_next_tick = preview_sample(
        formula_id.clone(),
        processor_id,
        value_node,
        "value",
        RuntimeValue::Float(0.75),
        11,
    );
    let changed_trigger = preview_sample(
        formula_id,
        processor_id,
        trigger_node,
        "trigger",
        RuntimeValue::Trigger(TriggerValue::fired(8, 10)),
        10,
    );

    let signature = output_preview_signature(&[value.clone(), trigger.clone()]);
    assert_eq!(signature, output_preview_signature(&[trigger.clone(), value.clone()]));
    assert_eq!(
        signature,
        output_preview_signature(&[value_at_next_tick, trigger.clone()])
    );
    assert_ne!(signature, output_preview_signature(&[value, changed_trigger]));
}

#[test]
fn runtime_log_dedupe_uses_typed_processor_and_kind_key() {
    let manager = StateMachineManager::new();
    let processor_node = manager.id();
    let mut last_values = HashMap::new();

    assert!(should_emit_runtime_log(
        &mut last_values,
        10,
        RuntimeLogKey::processor_compile(processor_node),
        "same diagnostic",
    ));
    assert!(!should_emit_runtime_log(
        &mut last_values,
        40,
        RuntimeLogKey::processor_compile(processor_node),
        "same diagnostic",
    ));
    assert!(should_emit_runtime_log(
        &mut last_values,
        40,
        RuntimeLogKey::processor_runtime(processor_node),
        "same diagnostic",
    ));
}

#[test]
fn command_invocation_interner_reuses_exact_streams_without_collapsing_lanes() {
    let manager = StateMachineManager::new();
    let emitter = manager.id();
    let processor = NodeId(42);
    let source_node = ANodeId::new();
    let intent = RuntimeIntent {
        kind: chataigne_state_machine::COMMAND_INTENT_KIND.into(),
        source_node: Some(source_node),
        source_socket: Some(SocketId::new("command")),
        target: None,
        payload: RuntimeValue::String("value".into()),
        logical_tick: 1,
    };
    let first_lane = ContextKey::single("device", "first");
    let second_lane = ContextKey::single("device", "second");
    let mut streams = HashMap::new();
    let mut next_stream = 0;

    let first = intern_runtime_command_invocation(
        &mut streams,
        &mut next_stream,
        emitter,
        processor,
        Some(&first_lane),
        &intent,
    );
    let repeated = intern_runtime_command_invocation(
        &mut streams,
        &mut next_stream,
        emitter,
        processor,
        Some(&first_lane),
        &intent,
    );
    let second = intern_runtime_command_invocation(
        &mut streams,
        &mut next_stream,
        emitter,
        processor,
        Some(&second_lane),
        &intent,
    );

    assert_eq!(first, repeated);
    assert_ne!(first, second);
    assert_eq!(first.emitter, emitter);
    assert_eq!(next_stream, 2);
}

#[test]
fn processor_override_value_reads_direct_parameter_nodes() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let mut parameter = Parameter::new("Amount", ParamValue::Float(7.5), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId("surface/amount".to_owned());
    engine.add_node(parameter.into(), None);
    engine.apply_edits().expect("override parameter should attach");
    let parameter_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.decl_id.0 == "surface/amount")
        .map(|(id, _)| id)
        .expect("override parameter should exist");
    let snapshot = engine.process_tree_snapshot();

    assert_eq!(
        processor_override_value(&snapshot, parameter_id),
        Some(&ParamValue::Float(7.5))
    );
}

#[test]
fn processor_formula_resolver_reads_project_source_key() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(crate::app::FormulaLibrary::new().into(), None);
    engine.apply_edits().expect("formula library should attach");
    let library_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == crate::app::FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("formula library should exist");
    let mut formula = crate::app::AlchemistFormulaDefinition::new();
    formula.node_data_mut().meta.label = "Project Formula".to_owned();
    let formula_uuid = formula.node_data().meta.uuid;
    engine.add_user_item(formula.into(), Some(library_id));
    engine.apply_edits().expect("project formula should attach");

    engine.add_node(crate::app::StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("processor manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == crate::app::StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: manager_id,
        node_type: format!("state_processor:project:{}", formula_uuid.0),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(ack.success, "project formula processor should attach: {ack:?}");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == crate::app::StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor should exist");
    let snapshot = engine.process_tree_snapshot();
    let source = processor_formula_source_ref(&snapshot, processor_id).expect("processor source should resolve");

    assert!(matches!(
        &source,
        FormulaSourceRef::ProjectNode(reference) if reference.uuid() == formula_uuid
    ));

    let catalog = FormulaCatalog::from_snapshot(&snapshot);
    let formula_id = snapshot
        .node_id_by_uuid(formula_uuid)
        .expect("project formula should exist");
    let formula_map = HashMap::from([(
        formula_uuid,
        crate::app::systems_alchemist_formula::formula_from_snapshot(&snapshot, formula_id)
            .expect("project formula should materialize"),
    )]);
    let (formula_node, formula, formula_ui, formula_source_key) =
        processor_formula_from_snapshot(&snapshot, processor_id, &formula_map, &catalog)
            .expect("project formula should resolve from catalog");
    assert!(formula_node.is_some());
    assert_eq!(formula.label, "Project Formula");
    assert_eq!(
        formula_source_key,
        format!("state_processor:project:{}", formula_uuid.0)
    );
    assert_eq!(
        formula_ui.source_kind,
        chataigne_state_machine::ProcessorFormulaSourceKind::Project
    );
    assert!(!formula_ui.open_readonly_from_processor);
    assert!(!formula_ui.can_duplicate_to_library);
}
