use super::*;
use crate::app::module::common::serial::{spawn_for_test, SerialConnectionEvent, SerialConnectionStatus};
use golden_io::RetirementPool;
use std::{
    io::{self, Read, Write},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

struct Wire(Arc<Mutex<Vec<u8>>>);
impl Read for Wire {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::WouldBlock.into())
    }
}
impl Write for Wire {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("unbounded flush")
    }
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline, "serial module fixture did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
fn send(engine: &mut crate::app::AppEngine, node: NodeId, text: &str) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node,
        method: "sendText".into(),
        args: vec![ParamValue::Str(text.into())],
    });
}
struct Fixture {
    engine: crate::app::AppEngine,
    id: NodeId,
    pool: RetirementPool,
    output: Arc<Mutex<Vec<u8>>>,
}
fn configured() -> Fixture {
    let (mut engine, id) = create_serial_module();
    set_node_enabled(&mut engine, id, false);
    engine.apply_edits().unwrap();
    let port = serial_module(&engine, id).port_name.id();
    allow_serial_port_variant(&mut engine, port, "fixture");
    allow_serial_port_variant(&mut engine, port, "replacement");
    set_param(&mut engine, port, ParamValue::Enum("fixture".into()));
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    set_node_enabled(&mut engine, id, true);
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    let config = serial_module(&engine, id)
        .transport_config(&engine.process_tree_snapshot())
        .unwrap()
        .unwrap();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let output = Arc::new(Mutex::new(Vec::new()));
    let native_output = output.clone();
    let notifier = serial_module(&engine, id).wake.as_ref().unwrap().notifier.clone();
    let handle = spawn_for_test(
        config.clone(),
        &pool,
        move |_| Ok(Wire(native_output.clone())),
        notifier,
    )
    .unwrap();
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        events.iter().any(|event| {
            matches!(
                event,
                SerialConnectionEvent::Status(SerialConnectionStatus::Connected { .. })
            )
        })
    });
    let crate::app::AppNode::SerialModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("serial module")
    };
    node.stop_transport();
    node.transport = Some(handle);
    node.last_transport_config = Some(config);
    node.transport_dirty = false;
    node.retry_at = None;
    Fixture {
        engine,
        id,
        pool,
        output,
    }
}

#[test]
fn applied_port_baud_control_lines_and_disable_fence_script_sends_before_callbacks() {
    for field in ["port", "baud", "dtr", "rts", "enabled"] {
        let Fixture {
            mut engine,
            id,
            pool,
            output,
        } = configured();
        send(&mut engine, id, "valid");
        engine.apply_edits().unwrap();
        wait(|| output.lock().unwrap().as_slice() == b"valid");
        let node = serial_module(&engine, id);
        let changed = match field {
            "port" => Some((node.port_name.id(), ParamValue::Enum("replacement".into()))),
            "baud" => Some((node.baud_rate.id(), ParamValue::Int(57600))),
            "dtr" => Some((node.dtr.id(), ParamValue::Bool(true))),
            "rts" => Some((node.rts.id(), ParamValue::Bool(true))),
            _ => None,
        };
        if let Some((param, value)) = &changed {
            set_param(&mut engine, *param, value.clone());
        } else {
            set_node_enabled(&mut engine, id, false);
        }
        send(&mut engine, id, "stale");
        assert!(engine.apply_edits().is_err());
        if let Some((param, value)) = changed {
            assert_eq!(
                engine.process_tree_snapshot().node(param).unwrap().param_value.as_ref(),
                Some(&value)
            );
        } else {
            assert!(!engine.process_tree_snapshot().node(id).unwrap().enabled);
        }
        thread::sleep(Duration::from_millis(25));
        assert_eq!(&*output.lock().unwrap(), b"valid");
        drop(engine);
        assert!(pool.wait_for_idle(Duration::from_secs(2)));
    }
}

#[test]
fn applied_transport_config_uses_snapshot_values_and_rejects_oversized_names_before_cloning() {
    let (mut engine, id) = create_serial_module();
    let port = serial_module(&engine, id).port_name.id();
    let oversized = "x".repeat(1025);
    allow_serial_port_variant(&mut engine, port, &oversized);
    set_param(&mut engine, port, ParamValue::Enum(oversized));
    engine.apply_edits().unwrap();
    assert!(serial_module(&engine, id)
        .transport_config(&engine.process_tree_snapshot())
        .unwrap_err()
        .contains("1024"));
}

#[test]
fn native_capacity_retry_avoids_owner_work_and_snapshots_until_its_deadline() {
    let mut node = SerialModule::create();
    node.retry_at = Some(Instant::now() + Duration::from_secs(1));
    assert!(!node.needs_update());
    assert!(!node.worker_wake_requires_tree_snapshot());
    node.retry_at = Some(Instant::now() - Duration::from_millis(1));
    assert!(!node.needs_update());
    assert!(node.worker_wake_requires_tree_snapshot());
}
