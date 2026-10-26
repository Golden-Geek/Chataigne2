use super::*;
use golden_engine::{application::ProjectReplacement, define_node_enum};

define_node_enum!(
    enum ReplacementNode {}
);
impl ProjectLifecycle for ReplacementNode {}

#[test]
fn project_generation_resyncs_an_idle_subscription_even_when_event_times_match() {
    let engine = Engine::new(ReplacementNode::from(Folder::new("Before")));
    let runtime = ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap();
    let read_model = runtime.read_model();
    let previous = read_model.current_project_generation();
    let client = client_with_subscription_count(1);
    let outbound = client.outbound.clone();
    let mut clients = HashMap::from([(1, client)]);
    runtime
        .replace_project(ProjectReplacement {
            engine: Engine::new(Folder::new("After").into()),
            project_file: UiProjectFileSpec::default(),
            reason: "same event clock".into(),
            recover: false,
        })
        .unwrap();
    assert_ne!(read_model.current_project_generation(), previous);
    // The cached client boundary can equal the new project's boundary exactly.
    // EventTime is meaningful only within its project generation.
    clients
        .get_mut(&1)
        .unwrap()
        .subscriptions
        .get_mut("subscription-0")
        .unwrap()
        .cursor = Some(read_model.current_revision());
    let mut origins = HashMap::new();
    dispatch_ws_batches(&read_model, &mut clients, &mut origins, false);
    assert!(
        matches!(outbound.pop(), Some(WsOutbound::Message(WsServerMessage::ResyncRequired { reason, .. })) if reason == "project_generation_changed")
    );
    assert!(clients[&1].subscriptions["subscription-0"].awaiting_resync);
    dispatch_ws_batches(&read_model, &mut clients, &mut origins, false);
    assert!(
        outbound.pop().is_none(),
        "An idle replacement must request exactly one resync"
    );
}
