use super::*;
use golden_core::{
    edit::Edit,
    node::{Folder, Node},
    process_ctx::ExecutionPhase,
};
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

mod pressure;
mod receipts;

/// Native-only fixture after admission. Retiring its engine must not revoke an
/// already accepted byte prefix; only the module admission boundary checks it.
pub(crate) fn native_payload(pool: &PayloadPool, bytes: Vec<u8>) -> RetainedPayload<PreparedCommand> {
    let engine = golden_core::engine::Engine::new(Folder::new("native fixture"));
    let request = StreamingSendRequest {
        bytes,
        description: "native fixture".into(),
        frame_kind: super::super::StreamingSendFrameKind::Binary,
    };
    let weight =
        std::mem::size_of::<PreparedCommand>() + request.bytes.capacity() + request.description.capacity() + 1024;
    pool.try_retain(
        PreparedCommand {
            module: engine.root,
            request,
            source: Arc::new(AtomicBool::new(false)),
            target: engine.node_waker(engine.root).unwrap(),
        },
        weight,
    )
    .unwrap()
}

type Owner = Arc<Mutex<CommandPreparation>>;
fn fixture() -> (crate::app::AppEngine, NodeId, CommandCapture) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let module = crate::app::SerialModule::create();
    engine.add_node(module.into(), None);
    for _ in 0..5 {
        engine.apply_edits().unwrap();
    }
    let module = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.add_node(crate::app::StreamingSendStringCommand::create().into(), Some(module));
    for _ in 0..5 {
        engine.apply_edits().unwrap();
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    let source = snapshot
        .child_ids_slice(module)
        .iter()
        .copied()
        .find(|id| snapshot.node(*id).unwrap().node_type == "streaming_send_string_command")
        .unwrap();
    let text = snapshot.find_child_by_decl_id(source, "text").unwrap();
    let ending = snapshot.find_child_by_decl_id(source, "line_ending").unwrap();
    (engine, source, CommandCapture::String { text, ending })
}
fn owner(service: Arc<worker::Service>) -> Owner {
    let mut owner = CommandPreparation::default();
    owner.service = Some(service);
    Arc::new(Mutex::new(owner))
}
fn enqueue(
    engine: &mut crate::app::AppEngine,
    owner: &Owner,
    source: NodeId,
    capture: CommandCapture,
) -> Result<(), String> {
    let owner = owner.clone();
    let (send, receive) = mpsc::channel();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.call_node_mutation(source, move |_, ctx| {
        let snapshot = ctx.tree_snapshot_arc().unwrap();
        send.send(owner.lock().unwrap().enqueue(ctx, &snapshot, source, capture, None))
            .unwrap();
        Ok(())
    });
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    receive.recv().unwrap()
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "preparation fixture did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
fn ready(owner: &Owner) -> bool {
    owner
        .lock()
        .unwrap()
        .pending
        .iter()
        .all(|delivery| delivery.ticket.0.result.get().is_some())
}

type Gate = (mpsc::Sender<()>, mpsc::Receiver<()>);
static GATE_LOCK: Mutex<()> = Mutex::new(());
static GATE: Mutex<Option<Gate>> = Mutex::new(None);
fn gate() -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (started, observed) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    *GATE.lock().unwrap() = Some((started, wait));
    (observed, release)
}
fn gated(job: &worker::Job) -> Result<RetainedPayload<PreparedCommand>, String> {
    let gate = GATE.lock().unwrap().take();
    if let Some((started, wait)) = gate {
        started.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    job.run()
}
