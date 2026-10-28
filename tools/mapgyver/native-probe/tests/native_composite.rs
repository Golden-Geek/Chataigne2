//! Native submission/content telemetry only; visual WebView qualification remains manual.

#![cfg(all(windows, feature = "desktop"))]

use std::io::Write;
use std::process::Command;
use std::process::Stdio;

const PROBE: &str = env!("CARGO_BIN_EXE_mapgyver_native_probe");
mod support;

#[test]
#[ignore = "opens owned probe windows; requires Windows hardware GPUs and WebView2"]
fn fixed_workloads_preserve_extents_alpha_and_exclude_warmup() {
    let inventory = Command::new(PROBE).arg("inventory").output().unwrap();
    assert!(inventory.status.success());
    let adapters: Vec<serde_json::Value> = serde_json::from_slice(&inventory.stdout).unwrap();
    for adapter in adapters.into_iter().filter(|adapter| adapter["software"] == false) {
        let index = adapter["index"].as_u64().unwrap().to_string();
        for (scenario, extent) in [("q01", [1920_u32, 1080]), ("q02", [3840, 2160])] {
            let result = Command::new(PROBE)
                .args(["qualify", &index, scenario, "8", "2"])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "adapter {index}/{scenario}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty(), "{}", String::from_utf8_lossy(&result.stderr));
            let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
            support::assert_frame_age(&report);
            support::assert_child_resources(&report);
            let generation = &report["generations"][0];
            support::assert_metrics(generation);
            assert_eq!(generation["end"], "deadline_reached");
            assert_eq!(generation["native_output_extent"], serde_json::json!(extent));
            assert_eq!(generation["fixed_geometry_observed"], true, "{report}");
            assert_eq!(generation["sampled_geometry_mismatches"], 0, "{report}");
            assert_eq!(report["workload"]["scenario"], scenario);
            let cpu = generation["timing"]["cpu_submission_work"]["samples"].as_u64().unwrap();
            let sends = report["status"]["spout_sends"].as_u64().unwrap();
            assert!((350..=500).contains(&cpu), "{report}");
            assert!(sends > cpu + 90, "warmup must be excluded: {report}");
            let checkpoints = report["observer"]["checkpoints"].as_array().unwrap();
            assert_eq!(checkpoints.len(), 4);
            for checkpoint in checkpoints {
                assert_eq!(checkpoint["extent"], serde_json::json!(extent));
                assert!(!checkpoint["sampled_alpha"].as_array().unwrap().is_empty());
            }
            let samples = report["resources"]["samples"].as_array().unwrap();
            assert!((9..=11).contains(&samples.len()));
            assert!(samples.iter().any(|sample| sample["in_sample_window"] == false));
            assert!(samples.iter().any(|sample| sample["in_sample_window"] == true));
            for sample in samples {
                assert!(sample["renderer"]["private_bytes"].as_u64().unwrap() > 0);
                assert!(sample["renderer"]["handle_count"].as_u64().unwrap() > 0);
                assert_eq!(
                    sample["renderer_allocations"]["live_bytes_by_owner"][0],
                    u64::from(extent[0]) * u64::from(extent[1]) * 4 + 4
                );
                assert_eq!(sample["presentation_editor_output"].as_array().unwrap().len(), 2);
                for presentation in sample["presentation_editor_output"].as_array().unwrap() {
                    assert_eq!(presentation["maximum_frame_latency"], serde_json::json!({"Ok": 1}));
                }
            }
            assert!(
                report["resources"]["after_gpu_release"]["handle_count"]
                    .as_u64()
                    .unwrap()
                    > 0
            );
        }
    }
}

#[test]
#[ignore = "opens owned probe windows; requires Windows hardware GPUs and WebView2"]
fn independent_composite_consumer_and_timestamp_queries_on_hardware() {
    let inventory = Command::new(PROBE).arg("inventory").output().unwrap();
    assert!(inventory.status.success());
    let adapters: Vec<serde_json::Value> = serde_json::from_slice(&inventory.stdout).unwrap();
    let mut count = 0;
    for adapter in adapters.into_iter().filter(|adapter| adapter["software"] == false) {
        count += 1;
        let index = adapter["index"].as_u64().unwrap().to_string();
        let result = Command::new(PROBE).args(["underlay", &index, "8"]).output().unwrap();
        assert!(
            result.status.success(),
            "adapter {index}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty(), "{}", String::from_utf8_lossy(&result.stderr));
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["adapter_luid"], adapter["luid"]);
        assert_ne!(report["render_process_id"], report["observer"]["process_id"]);
        assert_eq!(report["observer"]["adapter_luid"], adapter["luid"]);
        assert_eq!(report["observer"]["checkpoints"].as_array().unwrap().len(), 4);
        assert!((4..=16).contains(&report["observer"]["diagnostic_readbacks"].as_u64().unwrap()));
        assert_eq!(report["generations"].as_array().unwrap().len(), 1);
        support::assert_metrics(&report["generations"][0]);
        let timing = &report["generations"][0]["timing"];
        let cpu = timing["cpu_submission_work"]["samples"].as_u64().unwrap();
        let gpu = timing["gpu_execution"]["samples"].as_u64().unwrap();
        assert!(
            cpu > 100 && gpu > 100,
            "timing data must be measured, not an empty/default report"
        );
        assert_eq!(cpu, report["status"]["spout_sends"].as_u64().unwrap());
        assert_eq!(
            gpu + timing["skipped_gpu_queries"].as_u64().unwrap()
                + timing["invalid_gpu_queries"].as_u64().unwrap()
                + timing["pending_gpu_queries"].as_u64().unwrap(),
            cpu
        );
        assert_eq!(timing["truncated_samples"], 0);
        let mut missing = Command::new(PROBE)
            .args([
                "observe",
                &index,
                &format!("MapGyver-missing-output-{}", std::process::id()),
                "diagnostic",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        missing
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"command\":\"sample\",\"phase\":\"initial\"}\n{\"command\":\"finish\"}\n")
            .unwrap();
        let rejected = missing.wait_with_output().unwrap();
        assert!(
            !rejected.status.success(),
            "a missing output must never qualify content"
        );
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("timed out"));
    }
    assert!(count > 0, "no hardware adapter qualified");
}
