use golden_script::{
    NoopScriptHostBridge, QuickJsRuntime, ScriptBudgets, ScriptRuntime, ScriptRuntimeError, ScriptValue,
};

#[test]
fn public_script_runtime_loads_manifest_and_calls_exports() {
    let mut runtime = QuickJsRuntime::new(ScriptBudgets::default()).expect("public script runtime should initialize");
    let manifest = runtime
        .load(
            "export function echo(value) { return value; }",
            "runtime-contract.js",
            None,
        )
        .expect("script should load through the golden_script package");

    assert_eq!(manifest.exports.len(), 1);
    assert_eq!(manifest.exports[0].name, "echo");
    let mut host = NoopScriptHostBridge;
    assert_eq!(
        runtime
            .call_export("echo", &[ScriptValue::Str("ready".to_string())], &mut host,)
            .expect("export should execute"),
        ScriptValue::Str("ready".to_string())
    );
}

#[test]
fn public_script_runtime_enforces_host_call_budget() {
    let mut runtime = QuickJsRuntime::new(ScriptBudgets {
        max_host_calls_per_callback: 1,
        ..ScriptBudgets::default()
    })
    .expect("budgeted script runtime should initialize");
    runtime
        .load(
            "export function noisy() { log('first'); log('second'); }",
            "budget-contract.js",
            None,
        )
        .expect("script should load before callback budget enforcement");

    let mut host = NoopScriptHostBridge;
    let error = runtime
        .call_export("noisy", &[], &mut host)
        .expect_err("the second host call must exceed the callback budget");
    assert!(
        matches!(error, ScriptRuntimeError::BudgetViolation(_)),
        "expected budget violation, got {error:?}"
    );
}

#[test]
fn public_script_runtime_reload_replaces_cached_manifest() {
    let mut runtime = QuickJsRuntime::new(ScriptBudgets::default()).expect("public script runtime should initialize");
    runtime
        .load(
            "export function beforeReload() { return 1; }",
            "cache-contract.js",
            None,
        )
        .expect("initial script should load");
    assert_eq!(runtime.export_names(), vec!["beforeReload".to_string()]);

    runtime
        .reload("export function afterReload() { return 2; }", "cache-contract.js", None)
        .expect("reloaded script should replace cached runtime state");
    assert_eq!(runtime.export_names(), vec!["afterReload".to_string()]);

    let mut host = NoopScriptHostBridge;
    assert!(matches!(
        runtime.call_export("beforeReload", &[], &mut host),
        Err(ScriptRuntimeError::MissingExport(name)) if name == "beforeReload"
    ));
}
