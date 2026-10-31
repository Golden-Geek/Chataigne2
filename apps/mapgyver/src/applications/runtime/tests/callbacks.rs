use super::*;
use golden_core::{
    application::apply_project_intent,
    edit::Edit,
    events::EventKind,
    node::Node,
    script::{ScriptNode, ScriptNodeConfig, ScriptSource},
    ui_sync::UiEditIntent,
};

fn fixture() -> (AppEngine, ApplicationCallbacks, uuid::Uuid) {
    let mut engine: AppEngine = golden_core::app::create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let Some(AppNode::MediaManager(node)) = engine.nodes.get(manager) else {
        panic!()
    };
    let source = ApplicationCallbacks::new(node.instance, node.project_guard(), node.applications.clone());
    let owner = uuid::Uuid::new_v4();
    let status = agent().status;
    source
        .view
        .statuses
        .lock()
        .unwrap()
        .insert(status.application, (owner, status));
    (engine, source, owner)
}

fn phases(source: &ApplicationCallbacks, owner: uuid::Uuid) {
    let mut status = source.view.snapshot(source.project()).applications[0].clone();
    let mut permit = source.view.callbacks.reserve().unwrap();
    status.launch = Some(uuid::Uuid::new_v4());
    for (index, phase) in [
        ApplicationPhase::Starting,
        ApplicationPhase::WaitingForSender,
        ApplicationPhase::Ready,
        ApplicationPhase::Stopping,
        ApplicationPhase::Idle,
    ]
    .into_iter()
    .enumerate()
    {
        status.phase = phase;
        source.view.publish(
            status.application,
            owner,
            &status,
            Some(&mut permit),
            source.project(),
            index as u64,
        );
    }
}

fn invoke(engine: &mut AppEngine, ticket: ApplicationCallbackTicket) -> golden_core::ui_sync::UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: crate::media::protocol::MEDIA_NAMESPACE.into(),
            operation: crate::media::protocol::MEDIA_OPERATION.into(),
            payload: serde_json::to_value(crate::media::protocol::MediaEdit::Applications {
                edit: ApplicationEdit::DispatchCallbacks { ticket },
            })
            .unwrap(),
        },
    )
}

#[test]
fn hosted_scripts_receive_every_phase_in_order_without_authored_history_or_replay() {
    let (mut engine, source, owner) = fixture();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let mut script = ScriptNode::new("Lifecycle listener", ScriptNodeConfig {
        source: ScriptSource::Inline { text: "function init(){local.listen({level:1});} function onApplicationLifecycle(event){gc.emit('lifecycle-test',event);}".into() },
    });
    script.budgets.max_wall_time_us_per_load = 1_000_000;
    script.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(script.into(), Some(manager));
    engine.apply_edits().unwrap();
    let history = engine.undo_len();
    phases(&source, owner);
    assert_eq!(
        source.view.snapshot(source.project()).applications[0].phase,
        ApplicationPhase::Idle
    );
    assert_eq!(source.view.callback_state().pending, 5);
    let ticket = source.prepare().unwrap().unwrap();
    let ack = invoke(&mut engine, ticket);
    assert!(ack.success, "{ack:?}");
    source.settle();
    let events = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "lifecycle-test" => Some((*event.payload).clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 5, "{events:?}");
    for (index, phase) in ["Starting", "WaitingForSender", "Ready", "Stopping", "Idle"]
        .into_iter()
        .enumerate()
    {
        assert_eq!(events[index]["sequence"], (index + 1) as u64);
        assert_eq!(events[index]["status"]["phase"], phase);
        assert_eq!(events[index]["project"], source.project().to_string());
    }
    assert_eq!(engine.undo_len(), history);
    assert_eq!(source.view.callback_state().pending, 0);
    assert_eq!(source.view.callback_state().acknowledged, 5);
    assert!(source.prepare().unwrap().is_none());
    assert!(!invoke(&mut engine, ticket).success);
    phases(&source, owner);
    let ticket = source.prepare().unwrap().unwrap();
    let listener = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Lifecycle listener")
        .unwrap()
        .0;
    engine.edits.push(Edit::RemoveNode { node: listener });
    engine.apply_edits().unwrap();
    assert!(invoke(&mut engine, ticket).success);
    source.settle();
    assert_eq!(source.view.callback_state().acknowledged, 10);
    assert_eq!(
        engine
            .ui_event_log()
            .iter()
            .filter(|event| matches!(&event.kind,
        EventKind::Custom(event) if event.topic == "lifecycle-test"))
            .count(),
        5
    );
}

#[test]
fn cancelled_consumer_keeps_exclusive_ownership_until_its_blocked_sink_returns() {
    use std::{sync::mpsc, time::Duration};
    let (_engine, source, owner) = fixture();
    phases(&source, owner);
    let (entered, observed) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let subscription = source
        .start(move |_| {
            entered.send(()).unwrap();
            blocked.recv_timeout(Duration::from_secs(2)).unwrap();
            Err("temporary host backpressure".into())
        })
        .unwrap();
    observed.recv_timeout(Duration::from_secs(2)).unwrap();
    drop(subscription);
    assert!(source.start(|_| Ok(())).is_err());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match source.start(|_| Err("no actor needed".into())) {
            Ok(subscription) => {
                drop(subscription);
                break;
            }
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Err(error) => panic!("consumer failed to retire: {error}"),
        }
    }
    assert_eq!(source.view.callback_state().pending, 5);
    assert!(source.view.callback_state().fault.is_none());
}

#[test]
fn callback_capacity_reserves_cleanup_and_returns_unused_launch_credits() {
    let journal = Arc::new(super::super::callbacks::Journal::default());
    let mut permits = (0..128).map(|_| journal.reserve().unwrap()).collect::<Vec<_>>();
    assert!(journal.reserve().is_none());
    journal.append(Some(&mut permits[0]), uuid::Uuid::new_v4(), 0, agent().status);
    assert_eq!(journal.state.lock().unwrap().snapshot().pending, 1);
    // A queued event keeps its credit; dropping a reservation returns only its unused credits.
    drop(permits.remove(0));
    assert!(journal.reserve().is_none());
    drop(permits.remove(0));
    assert!(journal.reserve().is_some());
}

#[test]
fn stale_project_and_failed_receipt_cannot_replay_a_partially_delivered_batch() {
    let (mut engine, source, owner) = fixture();
    phases(&source, owner);
    let ticket = source.prepare().unwrap().unwrap();
    let mut stale = ticket;
    stale.project = uuid::Uuid::new_v4();
    assert!(!invoke(&mut engine, stale).success);
    super::super::callbacks::dispatch(&mut engine, ticket).unwrap();
    assert!(super::super::callbacks::dispatch(&mut engine, ticket).is_err());
    engine.edits.pending.clear();
    source.settle();
    let state = source.view.callback_state();
    assert_eq!(state.pending, 5);
    assert_eq!(state.acknowledged, 0);
    assert!(state.fault.unwrap().contains("partial delivery cannot be replayed"));
    assert!(matches!(
        source.view.snapshot(source.project()).supervision.callbacks,
        ApplicationServiceStatus::Halted { .. }
    ));
    assert!(source.prepare().is_err());
    assert!(source.view.callbacks.reserve().is_none());
    // The host may continue processing ordinary edits after callback admission halts.
    engine.apply_edits().unwrap();
}
