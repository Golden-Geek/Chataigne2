use super::super::*;
use golden_io::PayloadPool;

#[derive(Default)]
struct Host {
    calls: Vec<ScriptMethodCall>,
    reject: bool,
}

impl ScriptHostBridge for Host {
    fn log(&mut self, _: ScriptLogLevel, _: &str) {}
    fn emit_custom(&mut self, _: &str, _: JsonValue) -> Result<(), String> {
        Ok(())
    }
    fn call_node_script_method(&mut self, node: NodeId, call: ScriptMethodCall) -> Result<(), String> {
        assert_eq!(node, NodeId(7));
        self.calls.push(call);
        if self.reject {
            Err("injected host rejection".into())
        } else {
            Ok(())
        }
    }
}

fn new_runtime() -> QuickJsRuntime {
    QuickJsRuntime::new(ScriptBudgets {
        max_wall_time_us_per_callback: 1_000_000,
        max_wall_time_us_per_load: 1_000_000,
        ..ScriptBudgets::default()
    })
    .unwrap()
}

#[test]
fn quickjs_delivers_retained_method_calls_and_discards_a_failed_callback_prefix() {
    let mut runtime = new_runtime();
    runtime
        .load(
            r#"function update() { gc.__tree_call_method_raw(7, "sendText", '["hello",42]'); }"#,
            "retained-method.js",
            None,
        )
        .unwrap();
    let mut host = Host::default();
    runtime.call_on_update(&mut host).unwrap();
    let call = host.calls.pop().unwrap();
    assert_eq!(call.method(), "sendText");
    assert_eq!(&**call.args(), &[ParamValue::Str("hello".into()), ParamValue::Int(42)]);
    let mut runtime = new_runtime();
    runtime
        .load(
            r#"function update() {
            gc.__tree_call_method_raw(7, "sendText", '["retained prefix"]');
            gc.__tree_call_method_raw(7, "sendText", '["partial",{}]');
        }"#,
            "failed-conversion.js",
            None,
        )
        .unwrap();
    assert!(runtime.call_on_update(&mut host).is_err());
    assert!(host.calls.is_empty());
    assert!(runtime.host_ops.lock().unwrap().is_empty());
}

#[test]
fn host_rejection_discards_the_unvisited_retained_suffix_and_releases_local_capacity() {
    let runtime = new_runtime();
    let pool = PayloadPool::new("script-host-suffix", 3, 1024).unwrap();
    for _ in 0..3 {
        let call = ScriptMethodCall::seal(
            "sendText".into(),
            vec![ParamValue::Int(1)],
            pool.try_reserve(256).unwrap(),
            256,
        );
        runtime
            .host_ops
            .lock()
            .unwrap()
            .push(ScriptHostOp::CallNodeScriptMethod { node: NodeId(7), call });
    }
    let mut host = Host {
        reject: true,
        ..Host::default()
    };
    assert!(runtime.flush_host_ops(&mut host).is_err());
    assert_eq!(host.calls.len(), 1);
    assert!(runtime.host_ops.lock().unwrap().is_empty());
    assert!(
        !pool.wait_for_idle(Duration::ZERO),
        "the accepted prefix still owns its arguments"
    );
    host.calls.clear();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn quickjs_rejects_malformed_and_wide_method_inputs_without_queuing_an_edit() {
    for expression in ["'['", "JSON.stringify(Array(4097).fill(0))"] {
        let mut runtime = new_runtime();
        runtime
            .load(
                &format!("function update() {{ gc.__tree_call_method_raw(7, 'sendBytes', {expression}); }}"),
                "rejected-method.js",
                None,
            )
            .unwrap();
        let mut host = Host::default();
        assert!(runtime.call_on_update(&mut host).is_err());
        assert!(host.calls.is_empty());
        assert!(runtime.host_ops.lock().unwrap().is_empty());
    }
}
