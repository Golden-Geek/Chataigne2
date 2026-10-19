use super::transport::{config, read_request, request};
use super::*;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

#[test]
fn http_reconfiguration_preserves_accepted_requests_execution_and_completion_order() {
    let old = TcpListener::bind("127.0.0.1:0").unwrap();
    let new = TcpListener::bind("127.0.0.1:0").unwrap();
    old.set_nonblocking(true).unwrap();
    new.set_nonblocking(true).unwrap();
    let old_address = format!("http://{}/", old.local_addr().unwrap());
    let new_address = format!("http://{}/", new.local_addr().unwrap());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (new_tx, new_rx) = mpsc::channel();
    let old_server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut served = 0;
        while served < 2 && Instant::now() < deadline {
            match old.accept() {
                Ok((mut stream, _)) => {
                    read_request(&mut stream);
                    if served == 0 {
                        entered_tx.send(()).unwrap();
                        let _ = release_rx.recv_timeout(Duration::from_secs(3));
                    }
                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    served += 1;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(1)),
                Err(error) => panic!("old endpoint: {error}"),
            }
        }
        served
    });
    let new_server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match new.accept() {
                Ok((mut stream, _)) => {
                    read_request(&mut stream);
                    new_tx.send(()).unwrap();
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .unwrap();
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(1)),
                Err(error) => panic!("new endpoint: {error}"),
            }
        }
        panic!("new request never reached its endpoint");
    });
    let (mut engine, module_id) = create_http_module();
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP")
    };
    module.install_transport_for_test(config(old_address));
    for path in ["old first", "old second"] {
        let mut next = request();
        next.payload.path = path.into();
        module.send_for_test(next).unwrap();
    }
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let address = find_path(&engine, module_id, "parameters/base_address").unwrap();
    engine.edits.push(golden_core::edit::Edit::SetParam {
        node: address,
        value: ParamValue::Str(new_address),
        behaviour: Default::default(),
    });
    engine.apply_edits().unwrap();
    run_http_runtime_ticks(&mut engine, 1);
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP")
    };
    let mut next = request();
    next.payload.path = "new".into();
    module.send_for_test(next).unwrap();
    let premature = new_rx.recv_timeout(Duration::from_millis(200)).is_ok();
    release_tx.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while completions(&engine).len() < 3 && Instant::now() < deadline {
        run_http_runtime_ticks(&mut engine, 1);
        thread::sleep(Duration::from_millis(2));
    }
    let served = old_server.join().unwrap();
    new_server.join().unwrap();
    assert!(
        !premature,
        "the replacement must not execute ahead of older admitted requests"
    );
    assert_eq!(served, 2, "both old requests must retain an owner");
    assert_eq!(completions(&engine), ["old first", "old second", "new"]);
}

fn completions(engine: &crate::app::AppEngine) -> Vec<&str> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            let EventKind::Custom(custom) = &event.kind else {
                return None;
            };
            if custom.topic != crate::app::module::script_api::MODULE_SCRIPT_CALLBACK_TOPIC {
                return None;
            }
            if custom.payload.get("callback").and_then(serde_json::Value::as_str) != Some("responseReceived") {
                return None;
            }
            custom.payload.get("args")?.get(2)?.get("path")?.as_str()
        })
        .collect()
}

#[test]
fn http_disable_and_reenable_preserve_the_bounded_completion_backlog() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = format!("http://{}/", listener.local_addr().unwrap());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let served = Arc::new(AtomicUsize::new(0));
    let count = served.clone();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while count.load(Ordering::Acquire) < 17 && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    read_request(&mut stream);
                    if count.load(Ordering::Acquire) == 0 {
                        entered_tx.send(()).unwrap();
                        let _ = release_rx.recv_timeout(Duration::from_secs(3));
                    }
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .unwrap();
                    count.fetch_add(1, Ordering::AcqRel);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(1)),
                Err(error) => panic!("endpoint: {error}"),
            }
        }
    });
    let (mut engine, module_id) = create_http_module();
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP")
    };
    module.install_transport_for_test(config(address));
    for index in 0..17 {
        let mut next = request();
        next.payload.path = format!("request {index}");
        module.send_for_test(next).unwrap();
    }
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    engine.edits.push(golden_core::edit::Edit::PatchMeta {
        node: module_id,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.resolve().unwrap();
    run_http_runtime_ticks(&mut engine, 1);
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get(module_id).unwrap() else {
        panic!("HTTP")
    };
    assert!(
        module.send_for_test(request()).is_err(),
        "disabled admission is explicit rejection"
    );
    release_tx.send(()).unwrap();
    for _ in 0..20 {
        run_http_runtime_ticks(&mut engine, 1);
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        completions(&engine).is_empty(),
        "disabled consumers retain their completions"
    );
    assert!(
        served.load(Ordering::Acquire) <= 5,
        "four replies plus one in flight bound the paused producer"
    );
    engine.edits.push(golden_core::edit::Edit::PatchMeta {
        node: module_id,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.resolve().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while completions(&engine).len() < 17 && Instant::now() < deadline {
        run_http_runtime_ticks(&mut engine, 1);
        thread::sleep(Duration::from_millis(1));
    }
    server.join().unwrap();
    assert_eq!(served.load(Ordering::Acquire), 17);
    let expected = (0..17).map(|index| format!("request {index}")).collect::<Vec<_>>();
    assert_eq!(completions(&engine), expected);
}

#[test]
fn http_invalid_settings_reject_new_admission_without_discarding_old_completions() {
    let (mut engine, module_id) = create_http_module();
    let pool = golden_io::RetirementPool::new(std::num::NonZeroUsize::MIN);
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        panic!("HTTP")
    };
    module.transports = super::super::lifecycle::HttpTransports::with_lifetimes(pool.clone());
    module
        .transports
        .configure_using(
            config("http://127.0.0.1/".into()),
            module.wake.as_ref().unwrap().notifier.clone(),
            |config, pool, previous, notifier| {
                super::super::transport::HttpTransportHandle::spawn_using(
                    config,
                    pool,
                    previous,
                    notifier,
                    |_, _, next, _, _| {
                        Some(super::super::transport::HttpOutcome::Response(test_response(
                            &next.payload.path,
                            None,
                            Vec::new(),
                        )))
                    },
                )
            },
        )
        .unwrap();
    module.transport_dirty = false;
    for path in ["first", "second"] {
        let mut next = request();
        next.payload.path = path.into();
        module.send_for_test(next).unwrap();
    }
    let address = find_path(&engine, module_id, "parameters/base_address").unwrap();
    engine.edits.push(golden_core::edit::Edit::SetParam {
        node: address,
        value: ParamValue::Str("not an endpoint".into()),
        behaviour: Default::default(),
    });
    engine.apply_edits().unwrap();
    run_http_runtime_ticks(&mut engine, 1);
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get(module_id).unwrap() else {
        panic!("HTTP")
    };
    assert!(module.send_for_test(request()).unwrap_err().contains("not admitted"));
    let deadline = Instant::now() + Duration::from_secs(3);
    while completions(&engine).len() < 2 && Instant::now() < deadline {
        run_http_runtime_ticks(&mut engine, 1);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(completions(&engine), ["first", "second"]);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
