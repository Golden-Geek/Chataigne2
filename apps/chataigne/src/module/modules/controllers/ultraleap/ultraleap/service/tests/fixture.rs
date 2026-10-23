use super::*;
#[derive(Default)]
pub(in super::super::super) struct Control {
    pub(in super::super::super) rows: Mutex<VecDeque<UltraleapRuntimePoll>>,
    pub(super) panic: AtomicBool,
    pub(super) opens: AtomicUsize,
    pub(super) cleanup: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    pub(super) sources: Mutex<Option<PayloadPool>>,
}
struct Fake(Arc<Control>);
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(rx) = self.0.cleanup.lock().unwrap().take() {
            let _ = rx.recv();
        }
    }
}
impl ultraleap_runtime::Provider for Fake {
    fn poll(&mut self) -> Result<Vec<UltraleapRuntimePoll>, String> {
        assert!(
            !self.0.panic.swap(false, Ordering::AcqRel),
            "controlled Ultraleap service failure"
        );
        let mut rows = self.0.rows.lock().unwrap();
        let count = rows.len().min(16);
        Ok(rows.drain(..count).collect())
    }
}
pub(super) fn module(engine: &mut AppEngine, id: NodeId) -> &mut UltraleapModule {
    let AppNode::UltraleapModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Ultraleap fixture")
    };
    module
}
pub(in super::super::super) fn create() -> (AppEngine, NodeId, Arc<Control>, RetirementPool) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    let mut node = UltraleapModule::create();
    node.disable_runtime_for_test();
    engine.add_node(node.into(), None);
    for _ in 0..12 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let control = Arc::new(Control::default());
    let shared = control.clone();
    let native = RetirementPool::new(NonZeroUsize::MIN);
    let slots = native.clone();
    let sources = PayloadPool::new("ultraleap-engine", 64, 64 * ultraleap_runtime::EVENT_WEIGHT).unwrap();
    *control.sources.lock().unwrap() = Some(sources.clone());
    let target = module(&mut engine, id);
    target.service.factory = Some(Box::new(move |owner, notifier| {
        let shared = shared.clone();
        UltraleapRuntime::spawn_using(owner, &slots, sources.clone(), notifier, move || {
            shared.opens.fetch_add(1, Ordering::AcqRel);
            Ok(Fake(shared))
        })
    }));
    target.service.suppressed = false;
    target.request_service();
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).service.runtime.is_some() && control.opens.load(Ordering::Acquire) == 1
    });
    (engine, id, control, native)
}
pub(in super::super::super) fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        assert!(Instant::now() < end, "Ultraleap engine fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
pub(in super::super::super) fn tick(engine: &mut AppEngine) {
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    engine.apply_edits().unwrap();
}
pub(super) fn quiet(engine: &mut AppEngine) {
    let mut silent = 0;
    until(|| {
        tick(engine);
        if engine.tick_stats().wake_callbacks_fired == 0 {
            silent += 1;
        } else {
            silent = 0;
        }
        silent >= 20
    });
}
pub(super) fn finish(engine: &mut AppEngine, id: NodeId, native: &RetirementPool) {
    module(engine, id).stop_runtime();
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
pub(super) fn path(engine: &AppEngine, root: NodeId, path: &str) -> NodeId {
    let mut id = root;
    for name in path.split('/') {
        let mut child = engine.nodes.get(id).unwrap().node_data().first_child;
        id = loop {
            let current = child.expect("fixture path");
            let node = engine.nodes.get(current).unwrap();
            if node.node_data().meta.decl_id.0.rsplit('/').next() == Some(name) {
                break current;
            }
            child = node.node_data().next_sibling;
        };
    }
    id
}
pub(super) fn value(engine: &AppEngine, id: NodeId) -> ParamValue {
    engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value
}
pub(super) fn row(index: usize) -> UltraleapRuntimePoll {
    UltraleapRuntimePoll {
        service_connected: true,
        connected_devices: 1,
        frame: None,
        last_event: Some(format!("status {index}")),
    }
}
