use super::super::{
    configuration::HttpSettings,
    lifecycle::HttpTransports,
    transport::{HttpOutcome, HttpTransportHandle},
};
use super::transport::{config, read_request};
use super::*;
use golden_core::{edit::Edit, node::NodeMetaPatch, parameter::ParameterEventBehaviour};
use golden_io::RetirementPool;
use std::{
    io::Write,
    net::TcpListener,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::Instant,
};

fn module(engine: &crate::app::AppEngine, id: NodeId) -> &HttpModule {
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get(id).unwrap() else {
        panic!("HTTP")
    };
    module
}

fn set(engine: &mut crate::app::AppEngine, id: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node: id,
        value,
        behaviour: ParameterEventBehaviour::Append,
    });
}

fn settle(engine: &mut crate::app::AppEngine, id: NodeId) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        run_http_runtime_ticks(engine, 1);
        if module(engine, id).active_settings.is_some() && !module(engine, id).transport_dirty {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "applied HTTP configuration did not become active"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn applied_snapshot_values_override_cached_handles_and_non_transport_values_do_not_change_identity() {
    let (engine, id) = create_http_module();
    let node = module(&engine, id);
    let snapshot = engine.process_tree_snapshot();
    let initial = HttpSettings::read(node, &snapshot).unwrap().owned();
    let changed = snapshot.with_param_values([(
        node.base_address.id(),
        ParamValue::Str("http://127.0.0.1:1234/changed/".into()),
    )]);
    let applied = HttpSettings::read(node, &changed).unwrap();
    assert_ne!(initial, applied);
    assert_eq!(
        applied.prepare().unwrap().base_address,
        "http://127.0.0.1:1234/changed/"
    );
    let unrelated = snapshot.with_param_values([(node.auto_add.id(), ParamValue::Bool(false))]);
    assert_eq!(initial, HttpSettings::read(node, &unrelated).unwrap());
}

#[test]
fn source_lengths_and_header_count_are_bounded_before_preparation() {
    let (engine, id) = create_http_module();
    let node = module(&engine, id);
    let snapshot = engine.process_tree_snapshot();
    for (parameter, input, limit) in [
        (node.base_address.id(), "x".repeat(8193), "8192"),
        (node.default_headers.id(), "x".repeat(65537), "65536"),
    ] {
        let view = snapshot.with_param_values([(parameter, ParamValue::Str(input))]);
        assert!(HttpSettings::read(node, &view).unwrap_err().contains(limit));
    }
    let many = snapshot.with_param_values([(node.default_headers.id(), ParamValue::Str("X-Test: y\n".repeat(257)))]);
    assert!(HttpSettings::read(node, &many)
        .unwrap()
        .prepare()
        .unwrap_err()
        .contains("256"));
    let valid = snapshot.with_param_values([(
        node.default_headers.id(),
        ParamValue::Str(format!("# ignored\n\n{}", "X-Test: y\n".repeat(256))),
    )]);
    assert_eq!(
        HttpSettings::read(node, &valid)
            .unwrap()
            .prepare()
            .unwrap()
            .default_headers
            .len(),
        256
    );
}

#[test]
fn inactive_auth_fields_are_not_retained_or_part_of_transport_identity() {
    let (mut engine, id) = create_http_module();
    let auth = find_path(&engine, id, "parameters/auth").unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: auth,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    let node = module(&engine, id);
    let view = engine.process_tree_snapshot().with_param_values([
        (node.auth_mode.id(), ParamValue::Enum("bearer".into())),
        (node.token.id(), ParamValue::Str("active-token".into())),
        (node.username.id(), ParamValue::Str("unused".repeat(4096))),
    ]);
    let settings = HttpSettings::read(node, &view).unwrap();
    let config = settings.prepare().unwrap();
    assert_eq!(config.auth.token, "active-token");
    assert!(config.auth.username.is_empty());
    let changed = view.with_param_values([(node.username.id(), ParamValue::Str("other".repeat(8192)))]);
    assert_eq!(settings, HttpSettings::read(node, &changed).unwrap());
    let oversized = view.with_param_values([(node.token.id(), ParamValue::Str("x".repeat(8193)))]);
    assert!(HttpSettings::read(node, &oversized).unwrap_err().contains("8192"));
}

#[test]
fn authored_and_native_numeric_ranges_reject_invalid_settings() {
    let (engine, id) = create_http_module();
    let node = module(&engine, id);
    let snapshot = engine.process_tree_snapshot();
    for (parameter, input) in [
        (node.timeout_ms.id(), 0),
        (node.retry_count.id(), 11),
        (node.retry_delay_ms.id(), 60_001),
    ] {
        let view = snapshot.with_param_values([(parameter, ParamValue::Int(input))]);
        assert!(HttpSettings::read(node, &view).is_err());
    }
    let mut native = config("http://127.0.0.1/".into());
    native.retry_count = 11;
    assert!(native.validate_limits().is_err());
    native.retry_count = 0;
    native.timeout = Duration::ZERO;
    assert!(native.validate_limits().is_err());
}

#[test]
fn native_spare_capacity_is_rejected_before_a_worker_slot_is_reserved() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut base = config("http://127.0.0.1/".into());
    base.base_address.reserve(32769);
    assert!(HttpTransportHandle::spawn_with_lifetimes(base, &pool).is_err());
    let mut auth = config("http://127.0.0.1/".into());
    auth.auth.token = String::with_capacity(8193);
    assert!(HttpTransportHandle::spawn_with_lifetimes(auth, &pool).is_err());
    let mut headers = config("http://127.0.0.1/".into());
    headers.default_headers = Vec::with_capacity(4096);
    assert!(HttpTransportHandle::spawn_with_lifetimes(headers, &pool).is_err());
    assert_eq!(pool.metrics().active, 0);
}

#[test]
fn endpoint_edit_then_script_call_rejects_the_old_generation_and_recovers_on_the_new_endpoint() {
    let old = TcpListener::bind("127.0.0.1:0").unwrap();
    let new = TcpListener::bind("127.0.0.1:0").unwrap();
    old.set_nonblocking(true).unwrap();
    new.set_nonblocking(true).unwrap();
    let old_address = format!("http://{}/", old.local_addr().unwrap());
    let new_address = format!("http://{}/", new.local_addr().unwrap());
    let (mut engine, id) = create_http_module();
    let base = module(&engine, id).base_address.id();
    set(&mut engine, base, ParamValue::Str(old_address));
    engine.apply_edits().unwrap();
    settle(&mut engine, id);
    set(&mut engine, base, ParamValue::Str(new_address));
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        method: "get".into(),
        args: vec![ParamValue::Str("stale".into())],
    });
    assert!(
        engine.apply_edits().is_err(),
        "an applied endpoint edit must fence admission before callbacks run"
    );
    assert!(matches!(old.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
    settle(&mut engine, id);
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match new.accept() {
                Ok((mut stream, _)) => {
                    read_request(&mut stream);
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .unwrap();
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline);
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("{error}"),
            }
        }
    });
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        method: "get".into(),
        args: vec![ParamValue::Str("new".into())],
    });
    engine.apply_edits().unwrap();
    server.join().unwrap();
    assert!(matches!(old.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
}

#[test]
fn auth_edit_then_script_call_is_fenced_after_an_initial_valid_admission() {
    let (mut engine, id) = create_http_module();
    let auth = find_path(&engine, id, "parameters/auth").unwrap();
    let mode = module(&engine, id).auth_mode.id();
    let token = module(&engine, id).token.id();
    engine.edits.push(Edit::PatchMeta {
        node: auth,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    set(&mut engine, mode, ParamValue::Enum("bearer".into()));
    set(&mut engine, token, ParamValue::Str("old-token".into()));
    engine.apply_edits().unwrap();
    settle(&mut engine, id);
    let calls = install_counted_transport(&mut engine, id);
    request_and_wait(&mut engine, id, &calls, 1);
    set(&mut engine, token, ParamValue::Str("new-token".into()));
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        method: "get".into(),
        args: vec![ParamValue::Str("stale".into())],
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn inactive_auth_edits_and_none_mode_metadata_keep_the_current_generation_open() {
    let (mut engine, id) = create_http_module();
    let base = module(&engine, id).base_address.id();
    set(&mut engine, base, ParamValue::Str("http://127.0.0.1/".into()));
    engine.apply_edits().unwrap();
    settle(&mut engine, id);
    let calls = install_counted_transport(&mut engine, id);
    request_and_wait(&mut engine, id, &calls, 1);
    let username = module(&engine, id).username.id();
    set(&mut engine, username, ParamValue::Str("inactive".into()));
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    request_and_wait(&mut engine, id, &calls, 2);
    let auth = find_path(&engine, id, "parameters/auth").unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: auth,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    assert!(!module(&engine, id).transport_dirty);
    request_and_wait(&mut engine, id, &calls, 3);
}

fn install_counted_transport(engine: &mut crate::app::AppEngine, id: NodeId) -> Arc<AtomicUsize> {
    let calls = Arc::new(AtomicUsize::new(0));
    let native_calls = calls.clone();
    let prepared = HttpSettings::read(module(engine, id), &engine.process_tree_snapshot())
        .unwrap()
        .prepare()
        .unwrap();
    let crate::app::AppNode::HttpModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("HTTP")
    };
    node.transports = HttpTransports::with_lifetimes(RetirementPool::new(NonZeroUsize::new(2).unwrap()));
    node.transports
        .configure_using(
            prepared,
            node.wake.as_ref().unwrap().notifier.clone(),
            move |config, pool, previous, notifier| {
                HttpTransportHandle::spawn_using(config, pool, previous, notifier, move |_, _, request, _, _| {
                    native_calls.fetch_add(1, Ordering::Relaxed);
                    Some(HttpOutcome::Response(test_response(
                        &request.payload.path,
                        None,
                        vec![],
                    )))
                })
            },
        )
        .unwrap();
    calls
}

fn request_and_wait(engine: &mut crate::app::AppEngine, id: NodeId, calls: &AtomicUsize, expected: usize) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        method: "get".into(),
        args: vec![ParamValue::Str("valid".into())],
    });
    engine.apply_edits().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while calls.load(Ordering::Relaxed) < expected {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(calls.load(Ordering::Relaxed), expected);
}
