use super::*;
#[derive(Default)]
pub(in super::super::super) struct Control {
    pub(in super::super::super) rows: Mutex<VecDeque<KinectRuntimePoll>>,
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
impl kinect2_runtime::Provider for Fake {
    fn poll(&mut self) -> Result<Vec<KinectRuntimePoll>, String> {
        assert!(
            !self.0.panic.swap(false, Ordering::AcqRel),
            "controlled Kinect service failure"
        );
        let mut rows = self.0.rows.lock().unwrap();
        let count = rows.len().min(16);
        Ok(rows.drain(..count).collect())
    }
}
pub(super) fn module(engine: &mut AppEngine, id: NodeId) -> &mut Kinect2Module {
    let AppNode::Kinect2Module(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Kinect fixture")
    };
    module
}
pub(in super::super::super) fn create() -> (AppEngine, NodeId, Arc<Control>, RetirementPool) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    let mut node = Kinect2Module::create();
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
    let sources = PayloadPool::new("kinect2-engine", 64, 64 * kinect2_runtime::EVENT_WEIGHT).unwrap();
    *control.sources.lock().unwrap() = Some(sources.clone());
    let target = module(&mut engine, id);
    target.service.factory = Some(Box::new(move |owner, notifier, space| {
        let shared = shared.clone();
        Kinect2Runtime::spawn_using(owner, &slots, sources.clone(), notifier, space, move || {
            shared.opens.fetch_add(1, Ordering::AcqRel);
            shared.rows.lock().unwrap().push_front(KinectRuntimePoll {
                sensor_available: false,
                frame: None,
            });
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
        assert!(Instant::now() < end, "Kinect engine fixture deadline");
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
pub(super) fn project(engine: &mut AppEngine, id: NodeId) -> ProcessCtx {
    let snapshot = engine.process_tree_snapshot();
    module(engine, id).engine_sync_bound_param_handles(&mut |node| snapshot.node(node)?.param_value.clone());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(snapshot);
    module(engine, id).service_runtime(&mut ctx);
    ctx
}
pub(super) fn stage(engine: &mut AppEngine, id: NodeId, control: &Control) -> ProcessCtx {
    control.rows.lock().unwrap().push_back(frame(7));
    until(|| module(engine, id).service.runtime.as_ref().unwrap().has_pending());
    project(engine, id)
}
pub(super) fn emit(engine: &mut AppEngine, id: NodeId, control: &Control, row: KinectRuntimePoll) {
    quiet(engine);
    control.rows.lock().unwrap().push_back(row);
    until(|| module(engine, id).service.runtime.as_ref().unwrap().has_pending());
    tick(engine);
    quiet(engine);
}
pub(super) fn set_space(engine: &mut AppEngine, id: NodeId, space: &str) {
    let target = path(engine, id, "parameters/reference_space");
    engine.edits.push(Edit::SetParam {
        node: target,
        value: ParamValue::Enum(space.into()),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}
