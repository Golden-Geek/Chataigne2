use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use golden_audio::SampleRate;
use golden_core::{
    node::{Folder, Node},
    ui_sync::UiEditIntent,
};

use super::*;

#[test]
fn ui_sound_card_creation_is_immediately_acknowledged() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    crate::app::module::register_module_reference_filters(&mut engine);
    engine.add_node(crate::app::ModuleManager::new().into(), None);
    engine
        .apply_edits()
        .expect("Module Manager should materialize");
    let manager = engine
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.get_type() == crate::app::ModuleManager::NODE_TYPE).then_some(id)
        })
        .expect("Module Manager");

    let mut existing_graph = NodeTree::new(Folder::new("existing graph"));
    for index in 0..10_000 {
        existing_graph.push_child(NodeTree::new(Folder::new(format!("node {index}"))));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree: existing_graph,
        parent: engine.root,
        prev_sibling: None,
    });
    engine
        .apply_edits()
        .expect("existing large graph should materialize");
    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .expect("existing large graph events should settle");

    engine.clear_ui_event_log();
    let started = Instant::now();
    let acknowledgement = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: manager,
        node_type: SoundCardModule::NODE_TYPE.to_owned(),
        label: None,
        initial_params: Vec::new(),
    });
    let elapsed = started.elapsed();
    eprintln!(
        "Sound Card creation acknowledgement: {elapsed:?}, graph nodes: {}, UI events: {}",
        engine.nodes.len(),
        engine.ui_event_log().len()
    );

    assert!(
        acknowledgement.success,
        "Sound Card creation failed: {acknowledgement:?}"
    );
    assert!(
        elapsed < Duration::from_millis(250),
        "Sound Card creation acknowledgement blocked for {elapsed:?} while materializing {} nodes and {} UI events",
        engine.nodes.len(),
        engine.ui_event_log().len(),
    );
    let sound_card = engine
        .nodes
        .get(manager)
        .and_then(|node| node.node_data().first_child)
        .expect("created Sound Card");
    let sound_card_insertions = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            golden_core::events::EventKind::GraphTransaction { transaction } => {
                Some(transaction.ops.as_slice())
            }
            _ => None,
        })
        .flatten()
        .filter(|operation| {
            matches!(
                operation,
                golden_core::ui_sync::UiGraphOp::SubtreeInserted { root, .. }
                    if *root == sound_card
            )
        })
        .count();
    assert_eq!(
        sound_card_insertions, 1,
        "Sound Card defaults must be published as one completed subtree"
    );
}

#[test]
fn blocked_runtime_initialization_stays_off_the_engine_thread() {
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let mut worker = runtime::SoundCardRuntimeWorker::spawn_with(move |_| {
        entered_sender
            .send(())
            .expect("test should observe runtime startup");
        release_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("test should release blocked runtime startup");
        Err("intentional startup failure".to_owned())
    })
    .expect("runtime worker");
    let sample_rate = SampleRate::new(48_000).expect("sample rate");

    let request_started = Instant::now();
    let request = worker
        .request_start(sample_rate)
        .expect("runtime request should be queued");
    assert!(
        request_started.elapsed() < Duration::from_millis(250),
        "queuing runtime initialization must not execute it on the caller"
    );
    entered_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("runtime initialization should begin on the worker");
    assert!(matches!(
        worker.poll(),
        runtime::SoundCardRuntimeWorkerPoll::Pending
    ));

    release_sender
        .send(())
        .expect("blocked runtime should be released");
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match worker.poll() {
            runtime::SoundCardRuntimeWorkerPoll::Pending => {
                assert!(
                    Instant::now() < deadline,
                    "runtime worker did not publish its result"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            runtime::SoundCardRuntimeWorkerPoll::Started(started) => {
                assert_eq!(started.request, request);
                assert_eq!(
                    started.result.expect_err("startup should fail"),
                    "intentional startup failure"
                );
                break;
            }
            runtime::SoundCardRuntimeWorkerPoll::Disconnected => {
                panic!("runtime worker disconnected")
            }
        }
    }
}

#[test]
fn dropping_a_module_runtime_worker_never_joins_blocked_initialization() {
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let mut worker = runtime::SoundCardRuntimeWorker::spawn_with(move |sample_rate| {
        entered_sender
            .send(())
            .expect("test should observe runtime startup");
        release_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("test should release blocked runtime startup");
        runtime::SoundCardRuntime::start(sample_rate)
    })
    .expect("runtime worker");
    worker
        .request_start(SampleRate::new(48_000).expect("sample rate"))
        .expect("runtime request should be queued");
    entered_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("runtime initialization should begin on the worker");

    let drop_started = Instant::now();
    drop(worker);
    assert!(
        drop_started.elapsed() < Duration::from_millis(250),
        "dropping a Sound Card module must detach blocked runtime initialization"
    );
    release_sender
        .send(())
        .expect("detached runtime worker should still finish cleanup");
}
