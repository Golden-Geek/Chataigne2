use std::{
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use golden_script::{
    QuickJsRuntime, ScriptBudgets, ScriptEvent, ScriptHostBridge, ScriptLogLevel, ScriptRuntime, ScriptRuntimeError,
};
use serde_json::Value as JsonValue;

const CHILD_ENV: &str = "GOLDEN_SCRIPT_SAFETY_CHILD";

#[derive(Default)]
struct RecordingHost {
    effects: Vec<String>,
}

impl ScriptHostBridge for RecordingHost {
    fn log(&mut self, _level: ScriptLogLevel, message: &str) {
        self.effects.push(format!("log:{message}"));
    }

    fn emit_custom(&mut self, topic: &str, _payload: JsonValue) -> Result<(), String> {
        self.effects.push(format!("emit:{topic}"));
        Ok(())
    }
}

fn safety_budgets() -> ScriptBudgets {
    ScriptBudgets {
        max_instructions_per_callback: 20_000,
        max_instructions_per_load: 100_000,
        max_wall_time_us_per_callback: 20_000,
        max_wall_time_us_per_load: 100_000,
        max_memory_bytes: 8 * 1024 * 1024,
        ..ScriptBudgets::default()
    }
}

fn assert_budget_or_vm_failure(error: ScriptRuntimeError, phase: &str) {
    assert!(
        matches!(
            error,
            ScriptRuntimeError::BudgetViolation(_) | ScriptRuntimeError::QuickJs(_)
        ),
        "{phase} should fail through the VM safety boundary, got {error:?}"
    );
}

#[test]
fn script_safety_cases_finish_under_an_external_watchdog() {
    let executable = std::env::current_exe().expect("test executable should be discoverable");
    let mut child = Command::new(executable)
        .args([
            "--exact",
            "subprocess_script_safety_cases",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("script safety subprocess should start");
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        if let Some(status) = child.try_wait().expect("watchdog should poll child status") {
            let output = child
                .wait_with_output()
                .expect("completed safety subprocess output should be readable");
            assert!(
                status.success(),
                "script safety subprocess failed\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        if Instant::now() >= deadline {
            child.kill().expect("watchdog should terminate a hung VM test");
            let output = child
                .wait_with_output()
                .expect("terminated safety subprocess output should be readable");
            panic!(
                "script safety subprocess exceeded its external watchdog\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn subprocess_script_safety_cases() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }

    let mut top_level = QuickJsRuntime::new(safety_budgets()).expect("top-level runtime should initialize");
    let error = top_level
        .load("while (true) {}", "infinite-top-level.js", None)
        .expect_err("an infinite top-level evaluation must be interrupted");
    assert!(matches!(error, ScriptRuntimeError::BudgetViolation(_)));
    drop(top_level);

    let mut callback = QuickJsRuntime::new(safety_budgets()).expect("callback runtime should initialize");
    callback
        .load("function update() { while (true) {} }", "infinite-callback.js", None)
        .expect("callback source should load");
    let mut host = RecordingHost::default();
    let error = callback
        .call_on_update(&mut host)
        .expect_err("an infinite callback must be interrupted");
    assert!(matches!(error, ScriptRuntimeError::BudgetViolation(_)));
    assert!(
        callback
            .call_on_update(&mut host)
            .expect_err("a failed context must require reload")
            .to_string()
            .contains("reload is required")
    );
    drop(callback);

    let mut cancellable = QuickJsRuntime::new(ScriptBudgets {
        max_wall_time_us_per_callback: 5_000_000,
        ..safety_budgets()
    })
    .expect("cancellable runtime should initialize");
    cancellable
        .load("function update() { while (true) {} }", "cancelled-callback.js", None)
        .expect("cancellable source should load");
    let cancellation = cancellable.cancellation_handle();
    let (started_tx, started_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut host = RecordingHost::default();
        started_tx.send(()).expect("cancellation observer should be alive");
        result_tx
            .send(cancellable.call_on_update(&mut host))
            .expect("cancellation result observer should be alive");
    });
    started_rx.recv().expect("cancellable callback should start");
    let cancellation_deadline = Instant::now() + Duration::from_secs(1);
    let cancellation_error = loop {
        cancellation.cancel();
        match result_rx.try_recv() {
            Ok(result) => break result.expect_err("external cancellation must interrupt the callback"),
            Err(mpsc::TryRecvError::Empty) => {
                assert!(
                    Instant::now() < cancellation_deadline,
                    "external cancellation did not return control"
                );
                thread::yield_now();
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("cancellation worker disconnected without a result")
            }
        }
    };
    assert!(
        matches!(cancellation_error, ScriptRuntimeError::BudgetViolation(ref message) if message.contains("cancelled"))
    );
    worker.join().expect("cancellation worker should exit");

    let mut init = QuickJsRuntime::new(safety_budgets()).expect("init runtime should initialize");
    init.load("function init() { while (true) {} }", "infinite-init.js", None)
        .expect("init source should load");
    let error = init
        .call_on_init(&mut host)
        .expect_err("an infinite init callback must be interrupted");
    assert!(matches!(error, ScriptRuntimeError::BudgetViolation(_)));
    drop(init);

    let mut event = QuickJsRuntime::new(safety_budgets()).expect("event runtime should initialize");
    event
        .load("function event(_) { while (true) {} }", "infinite-event.js", None)
        .expect("event source should load");
    let error = event
        .call_on_event(
            &ScriptEvent {
                kind: "custom".to_string(),
                origin: None,
                old_value: None,
                payload: JsonValue::Null,
            },
            &mut host,
        )
        .expect_err("an infinite event callback must be interrupted");
    assert!(matches!(error, ScriptRuntimeError::BudgetViolation(_)));
    drop(event);

    let mut reload = QuickJsRuntime::new(safety_budgets()).expect("reload runtime should initialize");
    reload
        .load("function update() {}", "reload.js", None)
        .expect("initial reload source should load");
    let error = reload
        .reload("while (true) {}", "reload.js", None)
        .expect_err("an infinite reload evaluation must be interrupted");
    assert!(matches!(error, ScriptRuntimeError::BudgetViolation(_)));
    drop(reload);

    let mut finite = QuickJsRuntime::new(safety_budgets()).expect("finite-work runtime should initialize");
    finite
        .load(
            "export function expensive() { let value = 0; for (let i = 0; i < 1_000_000_000; i++) { value += i; } return value; }",
            "excessive-finite-work.js",
            None,
        )
        .expect("finite-work source should load");
    let error = finite
        .call_export("expensive", &[], &mut host)
        .expect_err("excessive finite work must be interrupted");
    assert!(matches!(error, ScriptRuntimeError::BudgetViolation(_)));
    drop(finite);

    let mut memory = QuickJsRuntime::new(ScriptBudgets {
        max_memory_bytes: 3 * 1024 * 1024,
        max_wall_time_us_per_callback: 500_000,
        ..safety_budgets()
    })
    .expect("memory-limited runtime should initialize");
    memory
        .load(
            "export function exhaust() { const blocks = []; while (true) { blocks.push(new Array(32768).fill(1)); } }",
            "memory-exhaustion.js",
            None,
        )
        .expect("memory exhaustion source should load");
    let error = memory
        .call_export("exhaust", &[], &mut host)
        .expect_err("memory exhaustion must return control");
    assert_budget_or_vm_failure(error, "memory exhaustion");
    drop(memory);

    let mut recursion = QuickJsRuntime::new(safety_budgets()).expect("recursion runtime should initialize");
    recursion
        .load("export function recurse() { return recurse(); }", "recursion.js", None)
        .expect("recursive source should load");
    let error = recursion
        .call_export("recurse", &[], &mut host)
        .expect_err("runaway recursion must return control");
    assert_budget_or_vm_failure(error, "recursion");
    drop(recursion);

    let mut nested = QuickJsRuntime::new(safety_budgets()).expect("nested runtime should initialize");
    nested
        .load(
            "function inner() { while (true) {} }\nexport function outer() { inner(); }",
            "nested.js",
            None,
        )
        .expect("nested source should load");
    let error = nested
        .call_export("outer", &[], &mut host)
        .expect_err("nested calls must not escape the outer deadline");
    assert!(matches!(error, ScriptRuntimeError::BudgetViolation(_)));
    drop(nested);

    let mut jobs = QuickJsRuntime::new(safety_budgets()).expect("job runtime should initialize");
    jobs.load(
        "export function schedule() { Promise.resolve().then(() => { while (true) {} }); return 1; }",
        "pending-job.js",
        None,
    )
    .expect("pending-job source should load");
    let error = jobs
        .call_export("schedule", &[], &mut host)
        .expect_err("the synchronous host must explicitly reject queued jobs");
    assert!(matches!(error, ScriptRuntimeError::BudgetViolation(_)));
    drop(jobs);

    let mut teardown = QuickJsRuntime::new(safety_budgets()).expect("teardown runtime should initialize");
    teardown
        .load("function destroy() { while (true) {} }", "teardown.js", None)
        .expect("teardown source should load");
    let error = teardown
        .call_on_destroy(&mut host)
        .expect_err("script teardown must be interruptible");
    assert!(matches!(error, ScriptRuntimeError::BudgetViolation(_)));
    drop(teardown);
}

#[test]
fn failed_invocations_discard_effects_and_reload_restores_a_clean_context() {
    let mut runtime = QuickJsRuntime::new(safety_budgets()).expect("runtime should initialize");
    let mut host = RecordingHost::default();
    let error = runtime
        .load(
            "emit('load-effect', 1); throw new Error('load failed');",
            "failed-load.js",
            Some(&mut host),
        )
        .expect_err("failed load should report its exception");
    assert!(matches!(error, ScriptRuntimeError::QuickJs(_)));
    assert!(host.effects.is_empty(), "failed load effects must be discarded");

    runtime
        .reload(
            "function update() { log('before'); emit('staged', 1); throw new Error('callback failed'); }",
            "failed-callback.js",
            None,
        )
        .expect("clean source should reload after the failed load");
    let error = runtime
        .call_on_update(&mut host)
        .expect_err("failed callback should report its exception");
    assert!(matches!(error, ScriptRuntimeError::QuickJs(_)));
    assert!(
        host.effects.is_empty(),
        "no effect staged by a failed callback may escape"
    );

    runtime
        .reload(
            "function update() { log('one'); emit('two', 2); log('three'); }",
            "successful-callback.js",
            None,
        )
        .expect("successful source should reinitialize the quarantined runtime");
    runtime
        .call_on_update(&mut host)
        .expect("successful callback should commit its journal");
    assert_eq!(host.effects, ["log:one", "emit:two", "log:three"]);
}

#[test]
fn oversized_host_inputs_are_rejected_without_committing_effects() {
    let mut runtime = QuickJsRuntime::new(safety_budgets()).expect("runtime should initialize");
    runtime
        .load(
            "function update() { log('accepted'); emit('x'.repeat(2048), 1); }",
            "oversized-host-input.js",
            None,
        )
        .expect("oversized host input source should load");
    let mut host = RecordingHost::default();
    runtime
        .call_on_update(&mut host)
        .expect_err("oversized host input should reject the entire effect journal");
    assert!(
        host.effects.is_empty(),
        "an earlier staged effect must not commit when later host input validation fails"
    );
}
