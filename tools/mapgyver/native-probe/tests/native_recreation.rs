//! Full application-device reconstruction only; never resets the user's graphics driver.
#![cfg(all(windows, feature = "desktop"))]

use std::io::Write;
use std::process::Command;
use std::process::Stdio;

const PROBE: &str = env!("CARGO_BIN_EXE_mapgyver_native_probe");
mod support;

#[test]
#[ignore = "opens owned windows and reconstructs probe D3D11 devices on available hardware"]
fn both_presenters_and_spout_recover_across_device_generations() {
    let inventory = Command::new(PROBE).arg("inventory").output().unwrap();
    assert!(inventory.status.success());
    let adapters: Vec<serde_json::Value> = serde_json::from_slice(&inventory.stdout).unwrap();
    let mut count = 0;
    for adapter in adapters.into_iter().filter(|a| a["software"] == false) {
        count += 1;
        let index = adapter["index"].as_u64().unwrap().to_string();
        let mut observer = Command::new(PROBE)
            .args(["observe", &index, "MapGyver-uninitialized-recreation", "q01"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        observer
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"command\":\"verify_generation\",\"generation\":1}\n")
            .unwrap();
        let rejected = observer.wait_with_output().unwrap();
        assert!(!rejected.status.success());
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("invalid recreation generation sequence"));
        for (scenario, extent) in [("q01", [1920, 1080]), ("q02", [3840, 2160])] {
            let result = Command::new(PROBE)
                .args(["recreate", &index, scenario, "2"])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{scenario}/{index}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty(), "{}", String::from_utf8_lossy(&result.stderr));
            let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
            support::assert_frame_age(&report);
            support::assert_child_resources(&report);
            assert_eq!(report["observer"]["verified_generations"], 2);
            assert_eq!(report["observer"]["checkpoints"].as_array().unwrap().len(), 4);
            let generations = report["generations"].as_array().unwrap();
            assert_eq!(generations.len(), 3);
            assert!(report["shutdown_seconds"].as_f64().unwrap() < 5.0);
            for (index, generation) in generations.iter().enumerate() {
                support::assert_metrics(generation);
                let scene = &generation["steady_scene"];
                assert_eq!(scene["process"]["process_id"], report["render_process_id"]);
                assert_eq!(scene["allocations"]["live_textures"], 9);
                assert_eq!(scene["allocations"]["retiring_textures"], 0);
                if index == 2 {
                    let settling = &generation["settling"];
                    assert!(settling["elapsed_seconds"].as_f64().unwrap() >= 10.0);
                    assert_eq!(settling["scene"]["process"]["process_id"], report["render_process_id"]);
                    assert_eq!(settling["scene"]["allocations"]["live_textures"], 9);
                    assert_eq!(settling["scene"]["allocations"]["retiring_textures"], 0);
                } else {
                    assert!(generation["settling"].is_null());
                }
                assert_eq!(generation["generation"], index);
                assert_eq!(
                    generation["end"],
                    if index < 2 {
                        "recreated"
                    } else {
                        "final_generation_complete"
                    }
                );
                assert_eq!(generation["adapter_luid"], adapter["luid"]);
                assert_eq!(generation["native_output_extent"], serde_json::json!(extent));
                assert_eq!(generation["fixed_geometry_observed"], true);
                assert!(generation["retirement_fence_ms"].as_f64().unwrap() < 2000.0);
                assert!(generation["setup_ms"].as_f64().unwrap() > 0.0);
                assert_eq!(generation["after_release"]["process_id"], report["render_process_id"]);
                assert!(generation["after_release"]["handle_count"].as_u64().unwrap() > 0);
                assert!(generation["output_presents"].as_u64().unwrap() > 0);
                assert!(generation["preview_presents"].as_u64().unwrap() > 0);
                assert!(generation["spout_sends"].as_u64().unwrap() > 60);
                let timing = &generation["timing"];
                let cpu = timing["cpu_submission_work"]["samples"].as_u64().unwrap();
                assert_eq!(cpu, generation["spout_sends"]);
                assert_eq!(
                    cpu,
                    timing["gpu_execution"]["samples"].as_u64().unwrap()
                        + timing["skipped_gpu_queries"].as_u64().unwrap()
                        + timing["invalid_gpu_queries"].as_u64().unwrap()
                        + timing["pending_gpu_queries"].as_u64().unwrap()
                );
                if index > 0 {
                    assert_eq!(generation["sampled_geometry_mismatches"], 0);
                    assert!(
                        generation["consumer_ready_ms"].as_f64().unwrap() >= generation["setup_ms"].as_f64().unwrap()
                    );
                    let markers = generation["consumer_checkpoint"]["sampled_generation_markers"]
                        .as_array()
                        .unwrap();
                    assert_eq!(markers.last().unwrap(), &serde_json::json!([index, 0, 165, 255]));
                    assert_eq!(
                        generation["consumer_checkpoint"]["sampled_bgra"]
                            .as_array()
                            .unwrap()
                            .last()
                            .unwrap(),
                        &serde_json::json!([[255, 0, 0, 255], [0, 255, 255, 255]])
                    );
                }
            }
            let samples = report["resources"]["samples"].as_array().unwrap();
            let first = &samples[0];
            assert!(
                samples
                    .iter()
                    .filter(|sample| sample["stage"]["settling"] == true)
                    .count()
                    >= 9
            );
            for sample in samples {
                assert_eq!(sample["renderer"]["process_id"], report["render_process_id"]);
                assert_eq!(sample["observer"]["process_id"], report["observer"]["process_id"]);
                for source in 0..2 {
                    assert_eq!(
                        sample["sources"][source]["process_id"],
                        first["sources"][source]["process_id"]
                    );
                }
            }
        }
    }
    assert!(count > 0, "no hardware adapter exercised");
}
