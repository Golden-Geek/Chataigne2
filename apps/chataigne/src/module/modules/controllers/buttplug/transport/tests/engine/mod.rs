use super::super::super::ButtplugModule;
use super::super::*;
use crate::app::{AppEngine, AppNode, ButtplugSetOutputCommand};
use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
    script::ScriptMethodCall,
};
use std::{
    sync::{mpsc, Mutex},
    thread,
    time::{Duration, Instant},
};

mod commands;
mod publication;
mod recovery;

#[derive(Clone, Default)]
struct Control {
    actions: Arc<Mutex<Vec<ButtplugWorkerCommand>>>,
    fail_poll: Arc<AtomicBool>,
    input: Arc<Mutex<std::collections::VecDeque<ButtplugWorkerEvent>>>,
    polls: Arc<std::sync::atomic::AtomicUsize>,
    panic_poll: Arc<AtomicBool>,
}
struct Fake {
    control: Control,
    gate: Option<mpsc::Receiver<()>>,
    cleanup: Option<mpsc::Receiver<()>>,
}
impl worker::Provider for Fake {
    fn connect(&mut self, _: &ButtplugTransportConfig) -> Result<Vec<ButtplugWorkerEvent>, String> {
        Ok(vec![ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Connected {
            server_name: "fixture".into(),
        })])
    }
    fn poll(&mut self) -> Result<Vec<ButtplugWorkerEvent>, String> {
        thread::sleep(Duration::from_millis(1));
        self.control.polls.fetch_add(1, Ordering::AcqRel);
        assert!(
            !self.control.panic_poll.load(Ordering::Acquire),
            "controlled provider panic"
        );
        if self.control.fail_poll.swap(false, Ordering::AcqRel) {
            return Err("controlled lost session".into());
        }
        Ok(self.control.input.lock().unwrap().pop_front().into_iter().collect())
    }
    fn execute(&mut self, request: &ButtplugWorkerCommand) -> Result<String, String> {
        self.control.actions.lock().unwrap().push(request.clone());
        if let Some(gate) = self.gate.take() {
            gate.recv().unwrap();
        }
        Ok("completed fixture action".into())
    }
    fn disconnect(&mut self) {}
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(gate) = self.cleanup.take() {
            let _ = gate.recv();
        }
    }
}
fn module(engine: &mut AppEngine, id: NodeId) -> &mut ButtplugModule {
    let AppNode::ButtplugModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!()
    };
    module
}
fn create() -> (AppEngine, NodeId) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    engine.add_node(ButtplugModule::create().into(), None);
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    (engine, id)
}
fn pool() -> PayloadPool {
    PayloadPool::new("buttplug-source-fixture", 256, 64 * 1024 * 1024).unwrap()
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !predicate() {
        assert!(Instant::now() < end, "fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn attach(
    engine: &mut AppEngine,
    id: NodeId,
    control: Control,
    slots: &RetirementPool,
    gate: Option<mpsc::Receiver<()>>,
) {
    install(
        engine,
        id,
        Fake {
            control,
            gate,
            cleanup: None,
        },
        slots,
        pool(),
    );
}
fn install(engine: &mut AppEngine, id: NodeId, provider: Fake, slots: &RetirementPool, events: PayloadPool) {
    let config = module(engine, id).transport_config().unwrap();
    let wake = module(engine, id).service.wake.as_ref().unwrap().notifier.clone();
    let permit = module(engine, id).service.native_slot.try_reserve().unwrap();
    let handle = ButtplugTransportHandle::spawn_using(config.clone(), wake, slots, pool(), events, move || {
        Ok(worker::Exclusive::new(provider, permit))
    })
    .unwrap();
    until(|| handle.shared.connected.load(Ordering::Acquire));
    let module = module(engine, id);
    module.last_transport_config = Some(Box::new(config));
    module.transport = Some(Box::new(handle));
    module.transport_dirty = false;
    module.service.retry_at = None;
    module.request_service();
}
fn turn(engine: &mut AppEngine) {
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    engine.apply_edits().unwrap();
}
fn quiet(engine: &mut AppEngine, id: NodeId) {
    let mut silent = 0;
    until(|| {
        turn(engine);
        let module = module(engine, id);
        if module.service.outgoing.is_empty()
            && module.service.pending.is_empty()
            && engine.tick_stats().wake_callbacks_fired == 0
        {
            silent += 1;
        } else {
            silent = 0;
        }
        silent >= 20
    });
}
fn finish(engine: &mut AppEngine, id: NodeId, slots: &RetirementPool) {
    module(engine, id).stop_transport();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}
fn path(engine: &AppEngine, parent: NodeId, path: &str) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    path.split('/').fold(parent, |parent, key| {
        snapshot
            .find_child_by_decl_id(parent, key)
            .or_else(|| snapshot.find_child(parent, key))
            .unwrap_or_else(|| panic!("missing {key}"))
    })
}
fn set(engine: &mut AppEngine, id: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node: id,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}
fn value(engine: &AppEngine, id: NodeId) -> ParamValue {
    engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value
}
fn invoke(engine: &mut AppEngine, id: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
    engine.apply_edits().unwrap();
}
fn command(engine: &mut AppEngine, id: NodeId) -> NodeId {
    let tester = path(engine, id, "command_tester");
    engine.add_user_item(ButtplugSetOutputCommand::create().into(), Some(tester));
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.nodes.get(tester).unwrap().node_data().first_child.unwrap()
}
fn context(engine: &AppEngine) -> ProcessCtx {
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    ctx
}
fn flush(engine: &mut AppEngine, ctx: &mut ProcessCtx) {
    engine.absorb_edits(ctx).unwrap();
    engine.apply_edits().unwrap();
}
