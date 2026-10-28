//! Explicit hardware qualification; never substitutes a software device or silently skips it.

#![cfg(windows)]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PROBE: &str = env!("CARGO_BIN_EXE_mapgyver_native_probe");

#[test]
#[ignore = "requires local Windows hardware GPUs; closes only its owned fixture's control pipe"]
fn parent_pipe_loss_cancels_owned_fixtures() {
    let inventory = Command::new(PROBE).arg("inventory").output().unwrap();
    assert!(inventory.status.success());
    let adapters: Vec<serde_json::Value> = serde_json::from_slice(&inventory.stdout).unwrap();
    let mut count = 0;
    for adapter in adapters.into_iter().filter(|a| a["software"] == false) {
        count += 1;
        let index = adapter["index"].as_u64().unwrap().to_string();
        let mut fixture = Command::new(PROBE)
            .args([
                "fixture",
                &index,
                &format!("MapGyver-eof-{}", std::process::id()),
                "b",
                "600",
                "diagnostic",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = fixture.stdout.take().unwrap();
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            let mut output = BufReader::new(output);
            let mut line = String::new();
            let result = output.read_line(&mut line).map(|_| line);
            // Retain/drain the report pipe until EOF; closing after Ready would turn the
            // fixture's normal post-fence reporting into a broken-pipe failure.
            let _ = ready_sender.send(result);
            let mut rest = String::new();
            while output.read_line(&mut rest).is_ok_and(|n| n > 0) {
                rest.clear();
            }
        });
        let ready = ready_receiver.recv_timeout(Duration::from_secs(5));
        if !matches!(&ready,Ok(Ok(line)) if serde_json::from_str::<serde_json::Value>(line).is_ok_and(|v| v["event"]=="ready"))
        {
            let _ = fixture.kill();
            let _ = fixture.wait();
        }
        let ready: serde_json::Value = serde_json::from_str(&ready.unwrap().unwrap()).unwrap();
        assert_eq!(ready["event"], "ready");
        drop(fixture.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(3);
        let status = loop {
            if let Some(status) = fixture.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= deadline {
                fixture.kill().unwrap();
                fixture.wait().unwrap();
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(
            status.is_some_and(|status| status.success()),
            "fixture outlived its control pipe"
        );
        reader.join().unwrap();
    }
    assert!(count > 0);
}

#[test]
#[ignore = "requires local Windows hardware GPUs and cross-process Spout sharing"]
fn cross_process_lifecycle_on_each_hardware_adapter() {
    let inventory = Command::new(PROBE).arg("inventory").output().expect("launch inventory");
    assert!(
        inventory.status.success(),
        "{}",
        String::from_utf8_lossy(&inventory.stderr)
    );
    let adapters: Vec<serde_json::Value> = serde_json::from_slice(&inventory.stdout).expect("adapter inventory JSON");
    let mut hardware_count = 0;
    for adapter in adapters {
        let index = adapter["index"].as_u64().expect("adapter index").to_string();
        let result = Command::new(PROBE)
            .args(["smoke", &index])
            .output()
            .expect("launch smoke");
        if adapter["software"] == true {
            assert!(
                !result.status.success(),
                "software adapter must not qualify native sharing"
            );
            assert!(String::from_utf8_lossy(&result.stderr).contains("software adapter"));
            continue;
        }
        hardware_count += 1;
        assert!(
            result.status.success(),
            "adapter {index}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("smoke report JSON");
        assert_eq!(
            report["adapter"]["luid"], adapter["luid"],
            "smoke must use the requested adapter"
        );
        assert_eq!(report["sdk_version"], "2.007.017");
        assert_eq!(report["diagnostic_readbacks"], 4);
        assert_eq!(report["unqualified"].as_array().unwrap().len(), 4);
    }
    assert!(hardware_count > 0, "no hardware adapter was qualified");
}
