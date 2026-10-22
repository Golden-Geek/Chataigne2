use super::*;
use crate::app::{AppControlLaunchProcessCommand, AppControlModule, AppEngine, AppNode};
use app_control_runtime::{
    AppObservation, Event as RuntimeEvent, Execution, FolderObservation, FolderWatchUpdate, Observation, PathKind,
    Provider, Request, WatchedAppMetrics,
};
use golden_core::{
    node::{Folder, Node, NodeMetaPatch},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    script::ScriptMethodCall,
};
use golden_io::PayloadPool;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};
mod commands;
mod publication;
mod recovery;

type Gate = (mpsc::SyncSender<()>, mpsc::Receiver<()>);
#[derive(Clone, Default)]
struct Control {
    actions: Arc<Mutex<Vec<Request>>>,
    samples: Arc<AtomicUsize>,
    configs: Arc<Mutex<Vec<Config>>>,
    fail_sample: Arc<AtomicBool>,
    panic_sample: Arc<AtomicBool>,
    fail_action: Arc<AtomicBool>,
    folder_changes: Arc<AtomicBool>,
}
struct Fake {
    control: Control,
    sample_gate: Option<Gate>,
    execute_gate: Option<Gate>,
    cleanup: Option<Gate>,
}
impl Provider for Fake {
    fn sample(&mut self, config: &Config) -> Result<Observation, String> {
        if let Some((entered, blocked)) = self.sample_gate.take() {
            let _ = entered.send(());
            let _ = blocked.recv();
        }
        let n = self.control.samples.fetch_add(1, Ordering::AcqRel) + 1;
        assert!(
            !self.control.panic_sample.load(Ordering::Acquire),
            "injected App Control provider panic"
        );
        if self.control.fail_sample.load(Ordering::Acquire) {
            return Err("injected App Control observation failure".into());
        }
        self.control.configs.lock().unwrap().push(config.clone());
        Ok(Observation {
            apps: config
                .apps
                .iter()
                .map(|entry| AppObservation {
                    metrics: WatchedAppMetrics {
                        cpu_ratio: 0.1 + (n % 10) as f64 * 0.01,
                        ..WatchedAppMetrics::idle(&entry.target_path)
                    },
                    kind: PathKind::File,
                })
                .collect(),
            folders: config
                .folders
                .iter()
                .map(|entry| FolderObservation {
                    update: FolderWatchUpdate {
                        path: entry.target_path.clone(),
                        exists: true,
                        created: if self.control.folder_changes.load(Ordering::Acquire) {
                            vec![format!("change-{n}")]
                        } else {
                            vec![]
                        },
                        timestamp_ms: n as u64,
                        ..Default::default()
                    },
                    kind: PathKind::Directory,
                })
                .collect(),
        })
    }
    fn execute(&mut self, request: &Request) -> Result<Execution, String> {
        if let Some((entered, blocked)) = self.execute_gate.take() {
            let _ = entered.send(());
            let _ = blocked.recv();
        }
        self.control.actions.lock().unwrap().push(request.clone());
        if self.control.fail_action.load(Ordering::Acquire) {
            return Err("injected App Control action failure".into());
        }
        let method = match request {
            Request::Launch(..) => "launch",
            Request::Kill(..) => "kill",
            Request::Window(..) => "controlWindow",
            Request::Running(..) => "setRunning",
        };
        Ok(Execution {
            attempted: true,
            method,
            details: serde_json::json!({"fixture":true}),
        })
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some((entered, blocked)) = self.cleanup.take() {
            let _ = entered.send(());
            let _ = blocked.recv();
        }
    }
}
fn fake(control: Control) -> Fake {
    Fake {
        control,
        sample_gate: None,
        execute_gate: None,
        cleanup: None,
    }
}
fn payloads(items: usize) -> PayloadPool {
    PayloadPool::new("app-control-engine-test", items, 64 * 1024 * 1024).unwrap()
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "controlled App Control work did not progress"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn module(engine: &mut AppEngine, id: NodeId) -> &mut AppControlModule {
    let AppNode::AppControlModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!()
    };
    module
}
fn create() -> (AppEngine, NodeId) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    engine.add_node(AppControlModule::create().into(), None);
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    (engine, id)
}
fn install(
    engine: &mut AppEngine,
    id: NodeId,
    provider: Fake,
    slots: &RetirementPool,
    reports: PayloadPool,
    commands: PayloadPool,
) {
    let wake = module(engine, id).service.wake.as_ref().unwrap().notifier.clone();
    let runtime = AppControlRuntime::spawn_using(wake, slots, reports, commands, move || Ok(provider)).unwrap();
    module(engine, id).service.retry_at = None;
    module(engine, id).service.runtime = Some(runtime);
    module(engine, id).watch_config_dirty = true;
    module(engine, id).request_service();
}
fn attach(engine: &mut AppEngine, id: NodeId, control: Control, slots: &RetirementPool) {
    install(engine, id, fake(control), slots, payloads(32), payloads(256));
}
fn turn(engine: &mut AppEngine) {
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    engine.apply_edits().unwrap();
}
fn quiet(engine: &mut AppEngine, id: NodeId) {
    let mut settled = 0;
    let deadline = Instant::now() + Duration::from_secs(4);
    while settled < 70 {
        turn(engine);
        let state = &module(engine, id).service;
        let pending = state.report.is_some()
            || state.catalog.is_some()
            || state.structure_pending
            || !state.pending.is_empty()
            || !state.outgoing.is_empty()
            || state.runtime.as_ref().is_some_and(AppControlRuntime::pending)
            || module(engine, id).watch_config_dirty;
        let stats = engine.tick_stats();
        settled = if !pending && stats.wake_callbacks_fired == 0 {
            settled + 1
        } else {
            0
        };
        assert!(Instant::now() < deadline, "App Control did not settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn call(engine: &mut AppEngine, id: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
    engine.apply_edits().unwrap();
}
fn invoke(engine: &mut AppEngine, id: NodeId, name: &str) {
    call(engine, id, "launchApp", vec![ParamValue::Str(name.into())]);
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
fn path(engine: &AppEngine, parent: NodeId, name: &str) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    name.split('/').fold(parent, |parent, key| {
        snapshot
            .find_child_by_decl_id(parent, key)
            .or_else(|| snapshot.find_child(parent, key))
            .unwrap_or_else(|| panic!("missing {key}"))
    })
}
fn value(engine: &AppEngine, id: NodeId) -> ParamValue {
    engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value
}
fn set(engine: &mut AppEngine, id: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node: id,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}
fn watch_apps(engine: &mut AppEngine, id: NodeId, count: usize) -> Vec<NodeId> {
    let root = path(engine, id, "parameters/watched_apps_targets");
    for n in 0..count {
        engine.add_user_item(
            Parameter::new(
                &format!("Watch{n}"),
                ParamValue::File(format!("fixture/{n}.exe")),
                ParameterChangeCheck::ValueChange,
            )
            .into(),
            Some(root),
        );
    }
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.process_tree_snapshot().child_ids(root)
}
fn take_report(engine: &mut AppEngine, id: NodeId) -> RetainedPayload<Report> {
    let mut found = None;
    until(|| {
        if let Some(RuntimeEvent::Report(report)) = module(engine, id).service.runtime.as_ref().unwrap().event() {
            found = Some(report);
        }
        found.is_some()
    });
    found.unwrap()
}
fn callbacks(events: &[golden_core::events::Event], name: &str) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(
                &event.kind,
                golden_core::events::EventKind::Custom(event)
                    if event.payload.get("callback").and_then(serde_json::Value::as_str) == Some(name)
            )
        })
        .count()
}
fn finish(engine: &mut AppEngine, id: NodeId, slots: &RetirementPool) {
    module(engine, id).service.runtime = None;
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}
