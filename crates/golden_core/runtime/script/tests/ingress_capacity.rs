use golden_script::{NoopScriptHostBridge, QuickJsRuntime, ScriptBudgets, ScriptCustomEvent, ScriptRuntime};
use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const CHILD_ENV: &str = "GOLDEN_SCRIPT_INGRESS_CAPACITY_CHILD";

#[test]
fn aggregate_ingress_capacity_and_recovery_run_in_an_isolated_process() {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "ingress_capacity_child", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let output = child.wait_with_output().unwrap();
            assert!(
                status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "ingress watchdog expired: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn ingress_capacity_child() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    let mut runtime = QuickJsRuntime::new(ScriptBudgets {
        max_wall_time_us_per_callback: 5_000_000,
        max_wall_time_us_per_load: 5_000_000,
        max_instructions_per_callback: 1_000_000,
        ..ScriptBudgets::default()
    })
    .unwrap();
    runtime
        .load(
            r#"function update() {
        const value = JSON.stringify('x'.repeat(1024 * 1024 - 128));
        for (let i = 0; i < 100; i++) gc.__emit_raw('capacity', value);
    }"#,
            "capacity.js",
            None,
        )
        .unwrap();
    let error = runtime.call_on_update(&mut NoopScriptHostBridge).unwrap_err();
    assert!(error.to_string().contains("lifetime capacity exhausted"), "{error}");
    drop(runtime);

    // Callback rejection releases its staged data, even though raw Arc cleanup has no notification.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut large = Vec::new();
    for _ in 0..16 {
        loop {
            let payload = serde_json::Value::String("x".repeat(4 * 1024 * 1024 - 256));
            match ScriptCustomEvent::prepare("recovered", payload) {
                Ok(event) => {
                    large.push(event);
                    break;
                }
                Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Err(error) => panic!("admission did not recover: {error}"),
            }
        }
    }
    assert!(
        golden_script::ScriptPropertyWrite::prepare("value", golden_parameters::ParamValue::Str("x".repeat(4096)))
            .is_err(),
        "events and property writes must charge the same lifetime byte budget"
    );
    drop(large);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match golden_script::ScriptPropertyWrite::prepare("value", golden_parameters::ParamValue::Str("x".repeat(4096)))
        {
            Ok(write) => {
                drop(write);
                break;
            }
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Err(error) => panic!("property admission did not recover: {error}"),
        }
    }
    // Earlier routing/replay owners must not exhaust the item limit at the old 4096 threshold.
    let mut routed = Vec::new();
    for _ in 0..13_000 {
        let event = ScriptCustomEvent::prepare("retained", serde_json::Value::Null).unwrap();
        routed.push(event.into_parts().1);
    }
    assert_eq!(routed.len(), 13_000);
}
