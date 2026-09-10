use golden_application::GraphEditing;
use golden_engine::app::ProjectLifecycle;
use golden_engine::application::ProductionRuntime;
use golden_engine::define_node_enum;
use golden_engine::engine::Engine;
use golden_engine::node::Folder;
use golden_engine::ui_sync::{UiEditIntent, UiProjectFileSpec};

define_node_enum!(
    enum ExternalConsumerNode {}
);

impl ProjectLifecycle for ExternalConsumerNode {}

fn apply_through_public_contract<G>(graph: &G, edit: G::Edit) -> Result<G::Revision, G::Error>
where
    G: GraphEditing,
{
    graph.apply_graph_edit(edit)
}

#[test]
fn dependency_consumer_observes_graph_edit_rejection_details() {
    let engine: Engine<ExternalConsumerNode> = Engine::new(Folder::new("Root").into());
    let root = engine.root;
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(ExternalConsumerNode::project_file_spec(), None),
    );

    let error = apply_through_public_contract(&runtime, UiEditIntent::RemoveNode { node: root })
        .expect_err("the dependency-facing contract must not report rejected edits as success");

    assert_eq!(error.code(), Some("cannot_mutate_root"));
    assert!(error.message().is_some_and(|message| message.contains("root")));
    assert_eq!(error.history().current_history_state_id, 0);
    assert_eq!(runtime.history_state(), *error.history());
}
