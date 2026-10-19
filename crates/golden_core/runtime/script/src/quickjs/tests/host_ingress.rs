use super::super::*;

#[derive(Default)]
struct Host {
    logs: Vec<String>,
    events: Vec<ScriptCustomEvent>,
    properties: Vec<ScriptPropertyWrite>,
    reject_event: bool,
}
impl ScriptHostBridge for Host {
    fn log(&mut self, _: ScriptLogLevel, message: &str) {
        self.logs.push(message.to_owned());
    }
    fn emit_custom(&mut self, event: ScriptCustomEvent) -> Result<(), String> {
        self.events.push(event);
        if self.reject_event {
            Err("injected event rejection".into())
        } else {
            Ok(())
        }
    }
    fn set_node_script_property(&mut self, node: NodeId, write: ScriptPropertyWrite) -> Result<(), String> {
        assert_eq!(node, NodeId(7));
        self.properties.push(write);
        Ok(())
    }
}

fn runtime(body: &str) -> QuickJsRuntime {
    let mut runtime = QuickJsRuntime::new(ScriptBudgets {
        max_wall_time_us_per_callback: 1_000_000,
        max_wall_time_us_per_load: 1_000_000,
        ..ScriptBudgets::default()
    })
    .unwrap();
    runtime
        .load(&format!("function update() {{ {body} }}"), "host-input.js", None)
        .unwrap();
    runtime
}

#[test]
fn typed_host_inputs_preserve_values_and_host_failure_discards_the_unvisited_suffix() {
    let mut runtime = runtime(
        r#"
        gc.log('info', 'hello');
        gc.__emit_raw('topic', '{"x":42}');
        gc.__tree_set_property_raw(7, 'value', '"original"');
    "#,
    );
    let mut host = Host::default();
    runtime.call_on_update(&mut host).unwrap();
    assert_eq!(host.logs, ["hello"]);
    assert_eq!(host.events[0].topic(), "topic");
    assert_eq!(**host.events[0].payload(), serde_json::json!({"x":42}));
    assert_eq!(host.properties[0].property(), "value");
    assert_eq!(host.properties[0].value(), &ParamValue::Str("original".into()));

    let mut host = Host {
        reject_event: true,
        ..Host::default()
    };
    assert!(runtime.call_on_update(&mut host).is_err());
    assert_eq!(host.logs, ["hello"]);
    assert_eq!(host.events.len(), 1);
    assert!(host.properties.is_empty());
    assert!(runtime.host_ops.lock().unwrap().is_empty());
}

#[test]
fn malformed_wide_inputs_and_oversized_levels_abort_without_committing_the_prefix() {
    for operation in [
        "gc.__emit_raw('topic', '[')",
        "gc.__tree_set_property_raw(7, 'value', '[')",
        "gc.__emit_raw('topic', JSON.stringify(Array(4097).fill(0)))",
        "gc.__tree_set_property_raw(7, 'value', JSON.stringify(Array(4097).fill(0)))",
        "gc.log('x'.repeat(65536), 'small')",
    ] {
        let mut runtime = runtime(&format!("gc.log('info', 'discarded'); {operation};"));
        let mut host = Host::default();
        let error = runtime.call_on_update(&mut host).unwrap_err();
        assert!(error.to_string().len() < 4096);
        assert!(host.logs.is_empty());
        assert!(host.events.is_empty());
        assert!(host.properties.is_empty());
        assert!(runtime.host_ops.lock().unwrap().is_empty());
    }
}

#[test]
fn metadata_only_host_ops_have_a_hard_queue_limit_independent_of_the_configured_call_budget() {
    let mut runtime = QuickJsRuntime::new(ScriptBudgets {
        max_host_calls_per_callback: 8192,
        max_wall_time_us_per_callback: 1_000_000,
        max_wall_time_us_per_load: 1_000_000,
        ..ScriptBudgets::default()
    })
    .unwrap();
    runtime
        .load(
            "function update() { for (let i=0; i<4097; i++) gc.__listeners_clear_raw(); }",
            "bounded-host-queue.js",
            None,
        )
        .unwrap();
    let error = runtime.call_on_update(&mut NoopScriptHostBridge).unwrap_err();
    assert!(
        error.to_string().contains("host-op queue capacity exhausted"),
        "{error}"
    );
    assert!(runtime.host_ops.lock().unwrap().is_empty());
}
