use super::*;
use crate::app::module::common::serial::{spawn_for_test, SerialConnectionConfig, SerialDiscoveryRegistration};
use golden_core::{events::EventKind, process_ctx::ProcessCtx};
use golden_io::RetirementPool;
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

#[derive(Default)]
struct State {
    input: VecDeque<Vec<u8>>,
    output: Vec<u8>,
}
struct Wire(Arc<Mutex<State>>);
impl Read for Wire {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let Some(input) = self.0.lock().unwrap().input.pop_front() else {
            return Err(io::ErrorKind::WouldBlock.into());
        };
        assert!(input.len() <= bytes.len());
        bytes[..input.len()].copy_from_slice(&input);
        Ok(input.len())
    }
}
impl Write for Wire {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().output.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("unbounded flush")
    }
}
fn module_mut(engine: &mut crate::app::AppEngine, id: NodeId) -> &mut SerialModule {
    let crate::app::AppNode::SerialModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("serial")
    };
    node
}
fn tick(engine: &mut crate::app::AppEngine) {
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(10)).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(engine.tick_stats().callbacks_fired, 0);
}
fn wait(engine: &mut crate::app::AppEngine, mut condition: impl FnMut(&crate::app::AppEngine) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        tick(engine);
        if condition(engine) {
            return;
        }
        assert!(Instant::now() < deadline, "passive serial owner did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
fn idle(engine: &crate::app::AppEngine, id: NodeId) -> bool {
    let node = serial_module(engine, id);
    !node.transport_dirty
        && node.retiring_transport.is_none()
        && node.pending_inputs.is_empty()
        && !node.stream.has_pending_messages()
        && !node.projection.pending()
        && !node.projection.needs_snapshot()
        && !node.transport.as_ref().is_some_and(|t| t.has_pending())
}
fn value(engine: &crate::app::AppEngine, id: NodeId, path: &str) -> Option<ParamValue> {
    engine
        .process_tree_snapshot()
        .node(find_path(engine, id, path)?)
        .and_then(|node| node.param_value.clone())
}
fn callbacks(engine: &crate::app::AppEngine, id: NodeId) -> Vec<String> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            let EventKind::Custom(event) = &event.kind else {
                return None;
            };
            let name = event.payload.get("callback")?.as_str()?;
            (event.origin == Some(id) && ["dataReceived", "dataReceive", "textReceived"].contains(&name))
                .then(|| name.into())
        })
        .collect()
}
fn configured() -> (crate::app::AppEngine, NodeId, RetirementPool, Arc<Mutex<State>>) {
    let (mut engine, id) = create_serial_module();
    wait(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let node = module_mut(&mut engine, id);
    node.stream.refresh_config_cache(&snapshot);
    let input = super::super::StreamInputConfig {
        module: id,
        parse: node.stream.parse_config_cached(),
        processing_enabled: true,
        log_incoming: false,
    };
    let config = SerialConnectionConfig {
        port_name: "fixture".into(),
        baud_rate: 115200,
        dtr: false,
        rts: false,
        input,
    };
    let state = Arc::new(Mutex::new(State::default()));
    let native = state.clone();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    node.transport = Some(
        spawn_for_test(
            config.clone(),
            &pool,
            move |_| Ok(Wire(native.clone())),
            node.wake.as_ref().unwrap().notifier.clone(),
        )
        .unwrap(),
    );
    node.last_transport_config = Some(config);
    node.transport_dirty = false;
    wait(&mut engine, |engine| {
        value(engine, id, "connection/connected") == Some(ParamValue::Bool(true)) && idle(engine, id)
    });
    (engine, id, pool, state)
}
#[test]
fn passive_input_delivers_canonical_callbacks_and_values_then_becomes_idle() {
    let (mut engine, id, _, state) = configured();
    assert_eq!(
        serial_module(&engine, id).execution_rule(),
        golden_core::engine::NodeExecutionRule::passive()
    );
    state.lock().unwrap().input.push_back(b"reading 7\n".to_vec());
    wait(&mut engine, |engine| {
        value(engine, id, "values/reading") == Some(ParamValue::Int(7)) && idle(engine, id)
    });
    assert_eq!(callbacks(&engine, id), ["dataReceived", "dataReceive", "textReceived"]);
    assert!(!serial_module(&engine, id).worker_wake_requires_tree_snapshot());
    for _ in 0..5 {
        tick(&mut engine);
    }
    assert_eq!(callbacks(&engine, id).len(), 3);
}
#[test]
fn disabled_owner_finishes_accepted_output_and_prepared_input_without_reconnecting() {
    let (mut engine, id, pool, state) = configured();
    state.lock().unwrap().input.push_back(b"final 8\n".to_vec());
    let deadline = Instant::now() + Duration::from_secs(2);
    while !serial_module(&engine, id).transport.as_ref().unwrap().has_pending() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let payload = vec![7; 100_000];
    serial_module(&engine, id)
        .transport
        .as_ref()
        .unwrap()
        .send(&payload, false)
        .unwrap();
    set_node_enabled(&mut engine, id, false);
    engine.apply_edits().unwrap();
    wait(&mut engine, |engine| {
        value(engine, id, "values/final") == Some(ParamValue::Int(8)) && idle(engine, id)
    });
    assert_eq!(state.lock().unwrap().output, payload);
    assert_eq!(callbacks(&engine, id).len(), 3);
    assert_eq!(
        value(&engine, id, "connection/connected"),
        Some(ParamValue::Bool(false))
    );
    assert!(serial_module(&engine, id).transport.is_none());
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn discarded_feedback_reconciles_without_another_native_transition() {
    let (mut engine, id) = create_serial_module();
    wait(&mut engine, |engine| idle(engine, id));
    for desired in [true, false] {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let node = module_mut(&mut engine, id);
        node.projection.connected(desired);
        node.projection.warning(desired.then_some("fixture warning"));
        node.project_feedback(&mut ctx, &snapshot);
        assert!(!ctx.edits.pending.is_empty());
        drop(ctx);
        wait(&mut engine, |engine| idle(engine, id));
        assert_eq!(
            value(&engine, id, "connection/connected"),
            Some(ParamValue::Bool(desired))
        );
        let snapshot = engine.process_tree_snapshot();
        let port = serial_module(&engine, id).port_name.id();
        assert_eq!(
            snapshot
                .node(port)
                .unwrap()
                .presentation
                .warning(Some(super::super::SERIAL_PORT_CONNECTION_WARNING_ID))
                .map(|warning| warning.message.as_str()),
            desired.then_some("fixture warning")
        );
    }
}
#[test]
fn inflight_catalog_selection_and_warning_projection_keep_newer_desired_state() {
    let (mut engine, id) = create_serial_module();
    wait(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let mut previous = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let mut later = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let node = module_mut(&mut engine, id);
    node.projection.selection_changed();
    node.projection.warning(Some("old"));
    node.project_feedback(&mut previous, &snapshot);
    node.projection.selection_changed();
    node.projection.warning(Some("new"));
    node.project_feedback(&mut later, &snapshot);
    assert!(later.edits.pending.is_empty());
    engine.absorb_edits(&mut previous).unwrap();
    engine.apply_edits().unwrap();
    wait(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let port = serial_module(&engine, id).port_name.id();
    assert_eq!(
        snapshot
            .node(port)
            .unwrap()
            .presentation
            .warning(Some(super::super::SERIAL_PORT_CONNECTION_WARNING_ID))
            .unwrap()
            .message,
        "new"
    );
}

#[test]
fn snapshot_free_callback_receipts_recover_and_undo_fences_old_delivery() {
    use crate::app::module::common::streaming::{
        delivery::InputDelivery,
        input::{self, InputSnapshot, StreamInputConfig},
        parser::StreamingParser,
    };
    let (mut engine, id) = create_serial_module();
    wait(&mut engine, |engine| idle(engine, id));
    let packet = input::prepare(
        &mut StreamingParser::default(),
        &InputSnapshot {
            generation: 0,
            config: StreamInputConfig {
                module: id,
                parse: Default::default(),
                processing_enabled: true,
                log_incoming: false,
            },
        },
        b"stale 9\n",
        "serial",
        None,
        true,
    )
    .unwrap();
    let node = module_mut(&mut engine, id);
    let old = node.wake.as_ref().unwrap().notifier.clone();
    let old_source = node.wake.as_ref().unwrap().source();
    node.pending_inputs.push_back(InputDelivery::new(packet));
    assert!(!node.worker_wake_requires_tree_snapshot());
    for turn in 0..10 {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        module_mut(&mut engine, id).on_worker_wake(&mut ctx);
        if turn == 1 {
            engine.edits.push(Edit::SetParam {
                node: NodeId(u64::MAX),
                value: ParamValue::Int(1),
                behaviour: ParameterEventBehaviour::Append,
            });
        }
        engine.absorb_edits(&mut ctx).unwrap();
        if turn == 1 {
            assert!(engine.apply_edits().is_err());
        } else {
            engine.apply_edits().unwrap();
        }
    }
    assert_eq!(callbacks(&engine, id), ["dataReceived", "dataReceive", "textReceived"]);
    assert!(serial_module(&engine, id).stream.has_pending_messages());
    engine.edits.push(Edit::RemoveNode { node: id });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    assert!(serial_module(&engine, id).pending_inputs.is_empty());
    wait(&mut engine, |engine| idle(engine, id));
    assert!(old_source.is_closed());
    assert_eq!(old_source.wake(), Err(golden_core::engine::NodeWakeError::Closed));
    old.notify();
    tick(&mut engine);
    assert!(value(&engine, id, "values/stale").is_none());
}

#[test]
fn partly_applied_catalog_feedback_reconciles_its_suffix_and_preserves_port_identity() {
    let (mut engine, id) = create_serial_module();
    let node = module_mut(&mut engine, id);
    let notifier = node.wake.as_ref().unwrap().notifier.clone();
    // Projection recovery must not depend on another test's retiring native discovery worker.
    node.port_discovery = Some(SerialDiscoveryRegistration::isolated_empty_for_test(notifier));
    node.request_service();
    wait(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let node = module_mut(&mut engine, id);
    let port = node.port_name.id();
    let mut catalog = node.projection.catalog.clone();
    catalog.version += 1;
    catalog.error = Some("catalog warning".into());
    node.projection.catalog(catalog);
    node.projection.connected(true);
    node.project_feedback(&mut ctx, &snapshot);
    let last = ctx.edits.pending.pop().unwrap();
    ctx.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    ctx.edits.pending.push(last);
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    wait(&mut engine, |engine| idle(engine, id));
    assert_eq!(serial_module(&engine, id).port_name.id(), port);
    assert_eq!(value(&engine, id, "connection/connected"), Some(ParamValue::Bool(true)));
    // The live discovery header supersedes the injected obsolete warning.
    assert!(engine
        .process_tree_snapshot()
        .node(port)
        .unwrap()
        .presentation
        .warning(Some(super::super::SERIAL_PORT_OPTIONS_WARNING_ID))
        .is_none());
}
