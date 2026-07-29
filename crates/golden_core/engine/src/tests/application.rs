use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use golden_application::{GraphEditing, Observation, RuntimeValues};

use crate::app::{
    PREFERENCES_DECL_ID, PREFERENCES_ENGINE_DECL_ID, PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID, ProjectLifecycle,
    ensure_preferences_tree,
};
use crate::application::{ProductionRuntime, ProjectReplacement, apply_ui_intent_to_engine};
use crate::define_node_enum;
use crate::engine::Engine;
use crate::node::{DeclId, Folder, Node, NodeId, NodeScriptDescriptor, USER_CONTEXT_NODE_TYPE, UserContextNode};
use crate::parameter::ParamValue;
use crate::ui_sync::{
    UiCreateUserItemInitialParam, UiDuplicateCreateUserItemSpec, UiDuplicateDependentInitialParamValue,
    UiDuplicateDependentUserItem, UiDuplicateDependentUserItemInitialParam, UiDuplicateNodeSpec, UiEditIntent,
    UiNodeDataDto, UiProjectFileSpec, UiSubscriptionScope,
};

static SNAPSHOT_PROBE_DESCRIPTOR_CALLS: AtomicUsize = AtomicUsize::new(0);

#[crate::node("snapshot_probe")]
struct SnapshotProbeNode {}

#[crate::node("snapshot_probe", from_struct)]
impl Node for SnapshotProbeNode {
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        SNAPSHOT_PROBE_DESCRIPTOR_CALLS.fetch_add(1, Ordering::Relaxed);
        NodeScriptDescriptor::default()
    }
}

define_node_enum!(
    enum FacadeTestNode {
        SnapshotProbeNode,
    }
);

impl ProjectLifecycle for FacadeTestNode {}

fn runtime() -> ProductionRuntime<FacadeTestNode> {
    let engine = Engine::new(Folder::new("Root").into());
    ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(FacadeTestNode::project_file_spec(), None),
    )
}

fn runtime_with_preferences(include_snapshot_probe: bool) -> (ProductionRuntime<FacadeTestNode>, NodeId) {
    let root: FacadeTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);
    engine.add_node(Folder::new("Work").into(), None);
    if include_snapshot_probe {
        engine.add_node(SnapshotProbeNode::new().into(), None);
    }
    ensure_preferences_tree(&mut engine, "C:/ChataigneData");
    engine.apply_edits().expect("work tree and preferences should attach");

    let snapshot = engine.process_tree_snapshot();
    let preferences = snapshot
        .find_child_by_decl_id(snapshot.root(), PREFERENCES_DECL_ID)
        .expect("preferences root");
    let engine_preferences = snapshot
        .find_child_by_decl_id(preferences, PREFERENCES_ENGINE_DECL_ID)
        .expect("engine preferences");
    let max_frequency = snapshot
        .find_child_by_decl_id(engine_preferences, PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID)
        .expect("max frequency preference");
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(FacadeTestNode::project_file_spec(), None),
    );

    (runtime, max_frequency)
}

fn atomic_duplicate_engine() -> (Engine<FacadeTestNode>, NodeId, Vec<NodeId>) {
    let root: FacadeTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);
    engine.add_node(UserContextNode::new("Context").into(), None);
    engine.apply_edits().expect("user context should attach");
    let container = engine
        .nodes
        .iter()
        .find_map(|(node_id, node)| (node.get_type() == USER_CONTEXT_NODE_TYPE).then_some(node_id))
        .expect("user context");

    engine
        .queue_catalog_create(container, "float", Some("First".to_string()), None)
        .expect("first item should be creatable");
    engine
        .queue_catalog_create(container, "float", Some("Second".to_string()), None)
        .expect("second item should be creatable");
    engine.apply_edits().expect("source items should attach");
    let items = engine.ui_direct_children(container).expect("container children");
    assert_eq!(items.len(), 2);

    (engine, container, items)
}

fn assert_rejected_intent_is_atomic(engine: &mut Engine<FacadeTestNode>, intent: UiEditIntent) {
    let graph_before = engine
        .to_project_json_with(|node| node.project_encode_data())
        .expect("graph should serialize before rejected edit");
    let history_before = engine.ui_history_state();
    let events_before = engine.ui_event_log().to_vec();

    let acknowledgement = apply_ui_intent_to_engine(engine, intent, Some("atomic-duplicate-test"));

    assert!(!acknowledgement.success);
    assert_eq!(
        engine
            .to_project_json_with(|node| node.project_encode_data())
            .expect("graph should serialize after rejected edit"),
        graph_before
    );
    assert_eq!(engine.ui_history_state(), history_before);
    assert_eq!(engine.ui_event_log(), events_before.as_slice());
}

fn install_first_publication_gate(
    runtime: &ProductionRuntime<FacadeTestNode>,
) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let calls = Arc::new(AtomicUsize::new(0));
    runtime.set_read_model_publication_hook(Some(Arc::new(move || {
        if calls.fetch_add(1, Ordering::SeqCst) != 0 {
            return;
        }
        let _ = entered_tx.send(());
        let _ = release_rx.lock().expect("publication gate poisoned").recv();
    })));
    (entered_rx, release_tx)
}

fn wait_for_control_queue_depth(runtime: &ProductionRuntime<FacadeTestNode>, depth: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if runtime.runtime_metrics().control_queue_depth >= depth {
            return true;
        }
        thread::yield_now();
    }
    false
}

fn snapshot_parameter_value(runtime: &ProductionRuntime<FacadeTestNode>, node: NodeId) -> ParamValue {
    let snapshot = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .expect("runtime snapshot");
    let node = snapshot
        .nodes
        .iter()
        .find(|candidate| candidate.node_id == node)
        .expect("parameter should remain in the read model");
    let UiNodeDataDto::Parameter { param } = &node.data else {
        panic!("node should remain a parameter");
    };
    param.value.clone()
}

#[test]
fn production_facade_keeps_graph_edits_and_observation_on_one_authoritative_path() {
    let runtime = runtime();
    let revision = runtime.apply_graph_edit(UiEditIntent::ReevaluateGraph).unwrap();
    let snapshot = runtime.snapshot(UiSubscriptionScope::WholeGraph).unwrap();

    assert_eq!(revision, runtime.history_state());
    assert_eq!(snapshot.nodes.len(), 1);
}

#[test]
fn production_runtime_value_facade_reports_non_parameter_inputs() {
    let runtime = runtime();
    let root = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .unwrap()
        .nodes
        .first()
        .expect("root node")
        .node_id;
    let error = runtime.publish_input(root, ParamValue::Int(5), 123).unwrap_err();

    assert!(!error.is_empty());
}

#[test]
fn production_tick_refreshes_runtime_limit_without_snapshotting_unrelated_nodes() {
    let (runtime, max_frequency) = runtime_with_preferences(true);

    let transaction = runtime.apply_ui_transaction(
        UiEditIntent::SetParam {
            node: max_frequency,
            value: ParamValue::Int(125),
            behaviour: Default::default(),
        },
        Some("runtime-limit-test"),
    );
    assert!(transaction.acknowledgement.success);

    SNAPSHOT_PROBE_DESCRIPTOR_CALLS.store(0, Ordering::Relaxed);
    let tick = runtime
        .run_tick(Duration::from_millis(1))
        .expect("runtime tick should succeed");

    assert_eq!(tick.next_interval, Duration::from_millis(8));
    assert_eq!(SNAPSHOT_PROBE_DESCRIPTOR_CALLS.load(Ordering::Relaxed), 0);
}

#[test]
fn concurrent_tick_and_ui_mutation_publish_in_control_actor_order() {
    let (runtime, max_frequency) = runtime_with_preferences(false);
    runtime
        .publish_input(max_frequency, ParamValue::Int(101), 1)
        .expect("runtime input should be accepted");
    let (publication_entered, release_publication) = install_first_publication_gate(&runtime);

    let tick_runtime = runtime.clone();
    let tick = thread::spawn(move || {
        tick_runtime
            .run_tick(Duration::from_millis(1))
            .expect("runtime tick should succeed")
    });
    publication_entered
        .recv_timeout(Duration::from_secs(2))
        .expect("tick should reach read-model publication");

    let (ui_done_tx, ui_done_rx) = mpsc::channel();
    let ui_runtime = runtime.clone();
    let ui = thread::spawn(move || {
        let result = ui_runtime.apply_ui_transaction(
            UiEditIntent::SetParam {
                node: max_frequency,
                value: ParamValue::Int(102),
                behaviour: Default::default(),
            },
            Some("ordered-publication-test"),
        );
        let _ = ui_done_tx.send(result.acknowledgement.success);
        result
    });

    let queued = wait_for_control_queue_depth(&runtime, 1);
    assert!(queued, "the UI mutation should wait behind tick publication");
    assert!(
        ui_done_rx.try_recv().is_err(),
        "the later UI mutation must not complete while the earlier tick capture is unpublished"
    );

    release_publication
        .send(())
        .expect("tick publication gate should still be waiting");
    let tick_result = tick.join().expect("tick thread should finish");
    assert!(
        tick_result.events.events.iter().any(|event| matches!(
            &event.kind,
            crate::ui_sync::UiEventKind::ParamChanged {
                param,
                new_value: ParamValue::Int(101),
                ..
            } if *param == max_frequency
        )),
        "the tick must publish the first parameter mutation"
    );
    let ui_result = ui.join().expect("UI thread should finish");
    assert!(ui_result.acknowledgement.success);
    assert!(
        ui_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("UI completion signal")
    );
    runtime.set_read_model_publication_hook(None);

    assert_eq!(
        snapshot_parameter_value(&runtime, max_frequency),
        ParamValue::Int(102),
        "the projection must retain the later actor mutation"
    );
}

#[test]
fn project_replace_cannot_be_overtaken_by_an_old_capture_or_project_file_update() {
    let (runtime, max_frequency) = runtime_with_preferences(false);
    let (publication_entered, release_publication) = install_first_publication_gate(&runtime);

    let old_runtime = runtime.clone();
    let old_mutation = thread::spawn(move || {
        old_runtime.apply_ui_transaction(
            UiEditIntent::SetParam {
                node: max_frequency,
                value: ParamValue::Int(111),
                behaviour: Default::default(),
            },
            Some("old-project-publication-test"),
        )
    });
    publication_entered
        .recv_timeout(Duration::from_secs(2))
        .expect("old-project mutation should reach read-model publication");

    let replacement_root: FacadeTestNode = Folder::new("Replacement Root").into();
    let replacement_engine = Engine::new(replacement_root);
    let replacement_file = UiProjectFileSpec {
        display_name: "Replacement".to_string(),
        extension: "noisette".to_string(),
        current_path: Some("replacement.noisette".to_string()),
    };
    let replacement_runtime = runtime.clone();
    let replacement = thread::spawn(move || {
        replacement_runtime.replace_project(ProjectReplacement {
            engine: replacement_engine,
            project_file: replacement_file,
            reason: "ordered-publication-test".to_string(),
            recover: false,
        })
    });
    assert!(
        wait_for_control_queue_depth(&runtime, 1),
        "project replacement should queue behind old-project publication"
    );

    let metadata_runtime = runtime.clone();
    let metadata = thread::spawn(move || {
        metadata_runtime.set_project_file(UiProjectFileSpec {
            display_name: "Replacement".to_string(),
            extension: "noisette".to_string(),
            current_path: Some("saved-after-replacement.noisette".to_string()),
        });
    });
    assert!(
        wait_for_control_queue_depth(&runtime, 2),
        "project-file metadata should queue behind project replacement"
    );

    release_publication
        .send(())
        .expect("old-project publication gate should still be waiting");
    assert!(
        old_mutation
            .join()
            .expect("old-project mutation thread")
            .acknowledgement
            .success
    );
    replacement
        .join()
        .expect("replacement thread")
        .expect("replacement should succeed");
    metadata.join().expect("metadata thread");
    runtime.set_read_model_publication_hook(None);

    let snapshot = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .expect("replacement snapshot");
    assert!(snapshot.nodes.iter().any(|node| node.meta.label == "Replacement Root"));
    assert!(!snapshot.nodes.iter().any(|node| node.meta.label == "Work"));
    assert_eq!(
        snapshot.project_file.current_path.as_deref(),
        Some("saved-after-replacement.noisette")
    );
    let replay = runtime
        .changes((None, UiSubscriptionScope::WholeGraph))
        .expect("replacement replay");
    assert!(
        replay
            .events
            .iter()
            .all(|event| !matches!(&event.kind, crate::ui_sync::UiEventKind::ParamChanged { param, .. } if *param == max_frequency)),
        "old-project parameter events must not re-enter retention after replacement"
    );
}

#[test]
fn ui_transactions_report_only_authoritative_preferences_changes() {
    let (runtime, max_frequency) = runtime_with_preferences(false);

    let unrelated = runtime.apply_ui_transaction(UiEditIntent::ReevaluateGraph, Some("preferences-test"));
    assert!(unrelated.acknowledgement.success);
    assert!(!unrelated.preferences_changed);

    let snapshot = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .expect("runtime snapshot");
    let root = snapshot
        .nodes
        .iter()
        .find(|node| node.meta.label == "Root")
        .expect("root node")
        .node_id;
    let work = snapshot
        .nodes
        .iter()
        .find(|node| node.meta.label == "Work")
        .expect("ordinary work folder")
        .node_id;
    let duplicated = runtime.apply_ui_transaction(
        UiEditIntent::DuplicateNode {
            source: work,
            new_parent: root,
            new_prev_sibling: Some(work),
            initial_params: Vec::new(),
        },
        Some("preferences-test"),
    );
    assert!(duplicated.acknowledgement.success);
    assert!(!duplicated.preferences_changed);

    let changed = runtime.apply_ui_transaction(
        UiEditIntent::SetParam {
            node: max_frequency,
            value: ParamValue::Int(125),
            behaviour: Default::default(),
        },
        Some("preferences-test"),
    );
    assert!(changed.acknowledgement.success);
    assert!(changed.preferences_changed);

    let batch = runtime.apply_ui_transaction_batch(
        vec![
            UiEditIntent::ReevaluateGraph,
            UiEditIntent::SetParam {
                node: max_frequency,
                value: ParamValue::Int(144),
                behaviour: Default::default(),
            },
        ],
        Some("preferences-test"),
        true,
    );
    assert!(!batch.transactions[0].preferences_changed);
    assert!(batch.transactions[1].preferences_changed);
    assert!(batch.preferences_changed);
}

#[test]
fn nonstructural_ui_batch_collects_preferences_subtree_once() {
    let (runtime, max_frequency) = runtime_with_preferences(false);
    let intents = (0..512)
        .map(|index| UiEditIntent::SetParam {
            node: max_frequency,
            value: ParamValue::Int(100 + index % 2),
            behaviour: Default::default(),
        })
        .collect();

    runtime.reset_preferences_subtree_collection_count();
    let batch = runtime.apply_ui_transaction_batch(intents, Some("preference-scan-test"), true);

    assert!(
        batch
            .transactions
            .iter()
            .all(|transaction| transaction.acknowledgement.success)
    );
    assert_eq!(runtime.preferences_subtree_collection_count(), 1);
}

#[test]
fn rejected_duplicate_node_initializer_leaves_graph_history_and_events_unchanged() {
    let (mut engine, container, items) = atomic_duplicate_engine();

    assert_rejected_intent_is_atomic(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source: items[0],
            new_parent: container,
            new_prev_sibling: Some(items[1]),
            initial_params: vec![UiCreateUserItemInitialParam {
                decl_id: DeclId("missing".to_string()),
                value: ParamValue::Float(1.0),
            }],
        },
    );
}

#[test]
fn rejected_duplicate_nodes_dependency_leaves_all_planned_roots_uncommitted() {
    let (mut engine, container, items) = atomic_duplicate_engine();

    assert_rejected_intent_is_atomic(
        &mut engine,
        UiEditIntent::DuplicateNodes {
            nodes: vec![UiDuplicateNodeSpec {
                source: items[0],
                new_parent: container,
                new_prev_sibling: Some(items[1]),
                initial_params: Vec::new(),
            }],
            created_items: vec![UiDuplicateCreateUserItemSpec {
                source: items[1],
                parent: container,
                node_type: "float".to_string(),
                label: Some("Created".to_string()),
                initial_params: Vec::new(),
            }],
            dependent_items: vec![UiDuplicateDependentUserItem {
                parent: container,
                node_type: "float".to_string(),
                label: Some("Dependent".to_string()),
                initial_params: vec![UiDuplicateDependentUserItemInitialParam {
                    decl_id: DeclId("value".to_string()),
                    value: UiDuplicateDependentInitialParamValue::DuplicatedNodeReference {
                        source: NodeId(u64::MAX),
                    },
                }],
            }],
        },
    );
}

#[test]
fn failed_transaction_batch_closes_matching_edit_session_and_skips_later_work() {
    let runtime = runtime();
    let root = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .unwrap()
        .nodes
        .first()
        .expect("root node")
        .node_id;
    let batch = runtime.apply_ui_transaction_batch(
        vec![
            UiEditIntent::BeginEdit {
                client_edit_id: "batch-edit".to_string(),
                label: Some("Batch".to_string()),
            },
            UiEditIntent::SetParam {
                node: root,
                value: ParamValue::Int(1),
                behaviour: Default::default(),
            },
            UiEditIntent::EndEdit {
                client_edit_id: "batch-edit".to_string(),
            },
            UiEditIntent::ReevaluateGraph,
        ],
        Some("client"),
        true,
    );

    assert!(batch.transactions[0].acknowledgement.success);
    assert!(!batch.transactions[1].acknowledgement.success);
    assert!(batch.transactions[2].acknowledgement.success);
    assert_eq!(
        batch.transactions[3].acknowledgement.error_code.as_deref(),
        Some("intent_batch_cancelled")
    );
}

#[test]
fn facade_trait_errors_remain_explicit() {
    fn assert_infallible(_: Infallible) {}
    let _ = assert_infallible;
}
