use super::super::*;

#[derive(Default)]
struct Host {
    requests: Vec<ScriptAppOperationRequest>,
}
impl ScriptHostBridge for Host {
    fn log(&mut self, _: ScriptLogLevel, _: &str) {}
    fn emit_custom(&mut self, _: ScriptCustomEvent) -> Result<(), String> {
        Ok(())
    }
    fn invoke_app_operation(&mut self, request: ScriptAppOperationRequest) -> Result<(), String> {
        self.requests.push(request);
        Ok(())
    }
}

fn runtime(source: &str) -> QuickJsRuntime {
    let mut runtime = QuickJsRuntime::new(ScriptBudgets {
        max_wall_time_us_per_load: 1_000_000,
        max_wall_time_us_per_callback: 1_000_000,
        ..ScriptBudgets::default()
    })
    .unwrap();
    runtime.load(source, "app-operation.js", None).unwrap();
    runtime
}

#[test]
fn public_gc_api_prepares_typed_names_and_complete_opaque_json() {
    let mut runtime = runtime(
        r#"function update() {
        gc.invokeAppOperation('request-1', 'fixture', 'paste', {capsule:{hidden:[42,null]}});
    }"#,
    );
    let mut host = Host::default();
    runtime.call_on_update(&mut host).unwrap();
    let request = host.requests.pop().unwrap();
    assert_eq!(request.request_id(), "request-1");
    assert_eq!(request.namespace(), "fixture");
    assert_eq!(request.operation(), "paste");
    assert_eq!(request.payload(), &serde_json::json!({"capsule":{"hidden":[42,null]}}));
}

#[test]
fn failed_script_callback_discards_requests_before_they_reach_the_host() {
    let mut runtime = runtime(
        r#"function update() {
        gc.invokeAppOperation('request-1', 'fixture', 'paste', {});
        throw new Error('discard entire callback');
    }"#,
    );
    let mut host = Host::default();
    assert!(runtime.call_on_update(&mut host).is_err());
    assert!(host.requests.is_empty());
    assert!(runtime.host_ops.lock().unwrap().is_empty());
}

#[test]
fn invalid_names_and_malformed_json_fail_before_host_execution() {
    let mut runtime = runtime(
        r#"function update() {
        try { gc.invokeAppOperation('', 'fixture', 'paste', {}); } catch (_) {}
        try { gc.__app_operation_raw('request-1', 'fixture', 'paste', '{bad'); } catch (_) {}
    }"#,
    );
    let mut host = Host::default();
    runtime.call_on_update(&mut host).unwrap();
    assert!(host.requests.is_empty());
}
