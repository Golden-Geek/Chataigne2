use super::super::super::super::mouse_runtime::{self, DiscoveredMouseDevice, MouseInputEvent, MouseRuntimeEvent};
use super::super::*;
use crate::app::{AppEngine, AppNode};
use golden_core::{node::Folder, process_ctx::ExecutionPhase};
use std::{collections::VecDeque, sync::atomic::AtomicUsize};

#[derive(Default)]
pub(super) struct InputControl {
    pub(super) records: Mutex<VecDeque<MouseRuntimeEvent>>,
    pub(super) panic: AtomicBool,
    pub(super) opens: AtomicUsize,
}
struct NativeInput(Arc<InputControl>);
impl mouse_runtime::Provider for NativeInput {
    fn poll(&mut self) -> Result<Vec<MouseRuntimeEvent>, String> {
        assert!(
            !self.0.panic.swap(false, Ordering::AcqRel),
            "controlled combined input failure"
        );
        let mut records = self.0.records.lock().unwrap();
        let count = records.len().min(16);
        Ok(records.drain(..count).collect())
    }
}
pub(super) fn key(pressed: bool) -> MouseRuntimeEvent {
    MouseRuntimeEvent::Input {
        device: "fake:mouse".into(),
        event: MouseInputEvent::ButtonChanged {
            button: MouseButtonKind::Middle,
            pressed,
        },
    }
}
fn catalog() -> MouseRuntimeEvent {
    MouseRuntimeEvent::DevicesChanged(vec![DiscoveredMouseDevice {
        index: 0,
        variant_id: "fake:mouse".into(),
        label: "Fake Mouse".into(),
        details: "fixture".into(),
    }])
}
pub(super) struct Fixture {
    pub(super) engine: AppEngine,
    pub(super) id: NodeId,
    pub(super) input: Arc<InputControl>,
    pub(super) output: Control,
    pub(super) input_slots: RetirementPool,
    pub(super) output_slots: RetirementPool,
    pub(super) output_opens: Arc<AtomicUsize>,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let mut engine = AppEngine::new(Folder::new("root").into());
        let mut node = MouseModule::create();
        node.disable_backends_for_test();
        engine.add_node(node.into(), None);
        for _ in 0..8 {
            engine.apply_edits().unwrap();
        }
        engine.resolve().unwrap();
        let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
        let input = Arc::new(InputControl::default());
        let input_slots = slots(1);
        let output = Control::default();
        let output_slots = slots(1);
        let output_opens = Arc::new(AtomicUsize::new(0));
        let target = module(&mut engine, id);
        let records = input.clone();
        let native = input_slots.clone();
        let source_pool = pool();
        target.input.factory = Some(Box::new(move |owner, notifier, config| {
            let records = records.clone();
            mouse_runtime::MouseInputRuntime::spawn_using(
                owner,
                &native,
                source_pool.clone(),
                notifier,
                config,
                move || {
                    records.opens.fetch_add(1, Ordering::AcqRel);
                    records.records.lock().unwrap().push_front(catalog());
                    Ok(NativeInput(records))
                },
            )
        }));
        let controller = output.clone();
        let native = output_slots.clone();
        let source_pool = pool();
        let coordinator = super::super::super::coordination::Coordinator::new();
        let opens = output_opens.clone();
        target.output.factory = Some(Box::new(move |owner, notifier| {
            let controller = controller.clone();
            let opens = opens.clone();
            Runtime::spawn_shared_using(
                owner,
                &native,
                source_pool.clone(),
                notifier,
                coordinator.clone(),
                move || {
                    opens.fetch_add(1, Ordering::AcqRel);
                    Ok(Fake(controller))
                },
            )
        }));
        target.suppress_input_backend = false;
        target.suppress_output_backend = false;
        target.request_input_service();
        target.request_output_service();
        let mut fixture = Self {
            engine,
            id,
            input,
            output,
            input_slots,
            output_slots,
            output_opens,
        };
        until(|| {
            fixture.tick();
            fixture.target().input.catalog.is_some() && fixture.target().output.is_ready()
        });
        fixture.quiet();
        fixture
    }
    pub(super) fn target(&mut self) -> &mut MouseModule {
        module(&mut self.engine, self.id)
    }
    pub(super) fn tick(&mut self) {
        self.engine.apply_edits().unwrap();
        self.engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        self.engine.apply_edits().unwrap();
        self.engine.run_tick(Duration::from_millis(1)).unwrap();
        self.engine.apply_edits().unwrap();
    }
    pub(super) fn quiet(&mut self) {
        let mut silent = 0;
        until(|| {
            self.tick();
            if self.engine.tick_stats().wake_callbacks_fired == 0 {
                silent += 1;
            } else {
                silent = 0;
            }
            silent >= 20
        });
    }
    pub(super) fn completed_output(&mut self) {
        let (release, gate) = mpsc::channel();
        *self.output.gate.lock().unwrap() = Some(gate);
        engine::invoke(
            &mut self.engine,
            self.id,
            "click",
            vec![golden_core::parameter::ParamValue::Str("left".into())],
        );
        until(|| {
            self.tick();
            self.output.calls.lock().unwrap().len() == 1
        });
        release.send(()).unwrap();
        until(|| self.target().output.receipts.front().is_some_and(Receipt::complete));
    }
    pub(super) fn queued_combined_turn(&mut self) -> golden_core::process_ctx::ProcessCtx {
        self.input.records.lock().unwrap().push_back(key(true));
        until(|| self.target().input.runtime.as_ref().unwrap().has_pending());
        let snapshot = self.engine.process_tree_snapshot();
        self.target()
            .engine_sync_bound_param_handles(&mut |id| snapshot.node(id)?.param_value.clone());
        let mut ctx = engine::context(&self.engine);
        self.target().on_worker_wake(&mut ctx);
        ctx
    }
    pub(super) fn finish(mut self) {
        self.target().stop_input_backend();
        self.target().stop_output_backend();
        assert!(self.input_slots.wait_for_idle(Duration::from_secs(2)));
        assert!(self.output_slots.wait_for_idle(Duration::from_secs(2)));
    }
}
fn module(engine: &mut AppEngine, id: NodeId) -> &mut MouseModule {
    let AppNode::MouseModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Mouse fixture")
    };
    module
}
