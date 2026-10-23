use super::*;
#[derive(Default)]
pub(super) struct Control {
    pub(super) rows: Mutex<VecDeque<Record>>,
    pub(super) state: Mutex<Option<GamepadStateSnapshot>>,
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
impl gamepad_runtime::Provider for Fake {
    fn poll(&mut self) -> Result<Vec<Record>, String> {
        assert!(
            !self.0.panic.swap(false, Ordering::AcqRel),
            "controlled Gamepad service failure"
        );
        let mut rows = self.0.rows.lock().unwrap();
        let count = rows.len().min(16);
        Ok(rows.drain(..count).collect())
    }
    fn state(&mut self, _selection: &str) -> Result<Option<GamepadStateSnapshot>, String> {
        Ok(self.0.state.lock().unwrap().clone())
    }
}
pub(super) fn module(engine: &mut AppEngine, id: NodeId) -> &mut GamepadModule {
    let AppNode::GamepadModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Gamepad fixture")
    };
    module
}
pub(super) fn device() -> DiscoveredGamepad {
    DiscoveredGamepad {
        index: 0,
        variant_id: "gamepad:0|Pad".into(),
        label: "Pad".into(),
        details: "fixture".into(),
    }
}
pub(super) fn edge(pressed: bool) -> Record {
    Record::Event(if pressed {
        GamepadRuntimeEvent::ButtonPressed {
            device: device(),
            button: GamepadButton::South,
        }
    } else {
        GamepadRuntimeEvent::ButtonReleased {
            device: device(),
            button: GamepadButton::South,
        }
    })
}
pub(super) fn create() -> (AppEngine, NodeId, Arc<Control>, RetirementPool) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    let mut node = GamepadModule::create();
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
    let sources = PayloadPool::new("gamepad-engine", 128, 32 * 1024 * 1024).unwrap();
    *control.sources.lock().unwrap() = Some(sources.clone());
    let target = module(&mut engine, id);
    target.service.factory = Some(Box::new(move |owner, notifier, settings| {
        let shared = shared.clone();
        GamepadRuntime::spawn_using(owner, &slots, sources.clone(), notifier, settings, None, move || {
            shared.opens.fetch_add(1, Ordering::AcqRel);
            shared.rows.lock().unwrap().push_front(Record::Catalog(vec![device()]));
            Ok(Fake(shared))
        })
    }));
    target.service.suppressed = false;
    target.request_service();
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).service.catalog.is_some() && control.opens.load(Ordering::Acquire) == 1
    });
    (engine, id, control, native)
}
pub(super) fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        assert!(Instant::now() < end, "Gamepad engine fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
pub(super) fn tick(engine: &mut AppEngine) {
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
            let current = child.unwrap_or_else(|| panic!("fixture path {path}: {name}"));
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
pub(super) fn set(engine: &mut AppEngine, id: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node: id,
        value,
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}
pub(super) fn stage(engine: &mut AppEngine, id: NodeId, control: &Control) -> ProcessCtx {
    control.rows.lock().unwrap().push_back(edge(true));
    until(|| module(engine, id).service.runtime.as_ref().unwrap().has_pending());
    project(engine, id)
}
pub(super) fn project(engine: &mut AppEngine, id: NodeId) -> ProcessCtx {
    let snapshot = engine.process_tree_snapshot();
    module(engine, id).engine_sync_bound_param_handles(&mut |node| snapshot.node(node)?.param_value.clone());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(snapshot);
    module(engine, id).service_runtime(&mut ctx);
    ctx
}
