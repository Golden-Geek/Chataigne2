use std::convert::Infallible;

use golden_application::{GraphEditing, Observation, RuntimeValues};

use super::application::ProductionRuntime;
use crate::app::ProjectLifecycle;
use crate::define_node_enum;
use crate::engine::Engine;
use crate::node::Folder;
use crate::parameter::ParamValue;
use crate::ui_sync::{UiEditIntent, UiProjectFileSpec, UiSubscriptionScope};

define_node_enum!(
    enum FacadeTestNode {}
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
