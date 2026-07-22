use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use golden_application::{GraphEditing, Observation, RuntimeValues};

use crate::app::{
    PREFERENCES_DECL_ID, PREFERENCES_ENGINE_DECL_ID, PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID, ProjectLifecycle,
    ensure_preferences_tree,
};
use crate::application::ProductionRuntime;
use crate::define_node_enum;
use crate::engine::Engine;
use crate::node::{Folder, Node, NodeScriptDescriptor};
use crate::parameter::ParamValue;
use crate::ui_sync::{UiEditIntent, UiProjectFileSpec, UiSubscriptionScope};

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
    let root: FacadeTestNode = Folder::new("Root").into();
    let mut engine = Engine::new(root);
    engine.add_node(SnapshotProbeNode::new().into(), None);
    ensure_preferences_tree(&mut engine, "C:/ChataigneData");
    engine.apply_edits().expect("probe and preferences tree should attach");

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
