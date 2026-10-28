use serde_json::Value;

pub fn assert_child_resources(report: &Value) {
    let sources = report["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 2);
    let observer = &report["observer"];
    for child in sources.iter().chain(std::iter::once(observer)) {
        let resources = &child["resources"];
        let samples = resources["samples"].as_array().unwrap();
        assert_eq!(resources["sample_count"], samples.len());
        assert!(samples.len() >= 8);
        assert_eq!(resources["after_fence"]["live_textures"], 0);
        assert_eq!(resources["after_fence"]["retiring_textures"], 0);
        for key in ["live_bytes_by_owner", "retiring_bytes_by_owner"] {
            assert_eq!(resources["after_fence"][key], serde_json::json!([0, 0, 0, 0]));
        }
        assert_eq!(resources["skipped_completion_markers"], 0);
        assert_eq!(child["adapter_luid"], report["adapter_luid"]);
    }
    let extent = report["generations"][0]["native_output_extent"].as_array().unwrap();
    let first_renderer_sample = &report["resources"]["samples"][0];
    for (channel, source) in sources.iter().enumerate() {
        assert_eq!(
            source["process_id"],
            first_renderer_sample["sources"][channel]["process_id"]
        );
        assert_eq!(source["channel"], if channel == 0 { "a" } else { "b" });
        let frames = source["frames_sent"].as_u64().unwrap();
        let resources = &source["resources"];
        let ops = &resources["operations"];
        assert_eq!(ops["sdk_send_calls"], frames);
        assert_eq!(ops["shader_draws"], frames);
        assert_eq!(ops["uniform_upload_bytes"], frames * 32);
        assert_eq!(ops["diagnostic_readback_pixels"], 0);
        assert_eq!(ops["explicit_gpu_copy_calls"], 0);
        let phases: Vec<_> = resources["samples"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|sample| {
                sample["source_phase"]
                    .as_u64()
                    .map(|phase| (phase, &sample["allocations"]))
            })
            .collect();
        assert_eq!(
            phases.iter().map(|p| p.0).collect::<Vec<_>>(),
            if channel == 0 { vec![0, 1, 2, 3] } else { vec![0] }
        );
        let full = if channel == 0 {
            extent[0].as_u64().unwrap() * extent[1].as_u64().unwrap() * 4
        } else {
            1280 * 720 * 4
        };
        for (phase, allocations) in phases {
            let bytes = if phase == 2 {
                0
            } else if phase == 1 {
                full / 4
            } else {
                full
            };
            assert_eq!(
                allocations["live_bytes_by_owner"],
                serde_json::json!([bytes, 0, 0, bytes])
            );
            assert_eq!(allocations["live_textures"], if phase == 2 { 0 } else { 2 });
        }
    }
    let age = &observer["frame_age"];
    let lifecycle_pixels = observer["checkpoints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["sampled_bgra"].as_array().unwrap().len() as u64 * 4)
        .sum::<u64>();
    let generation_pixels = report["generations"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|g| g["consumer_checkpoint"]["sampled_bgra"].as_array())
        .map(|s| s.len() as u64 * 5)
        .sum::<u64>();
    let diagnostic_pixels = lifecycle_pixels + generation_pixels;
    let marker_pixels = age["marker_copy_pixels"].as_u64().unwrap();
    let ops = &observer["resources"]["operations"];
    assert_eq!(ops["diagnostic_readback_pixels"], diagnostic_pixels + marker_pixels);
    assert_eq!(ops["explicit_gpu_copy_calls"], diagnostic_pixels + marker_pixels + 1);
    assert_eq!(
        ops["explicit_gpu_copy_bytes"],
        (diagnostic_pixels + marker_pixels) * 4 + age["final_log_copy_bytes"].as_u64().unwrap()
    );
}

pub fn assert_frame_age(report: &Value) {
    use mapgyver_native_probe::frame_age::{self, Config, Sample};
    let age = &report["observer"]["frame_age"];
    let samples: Vec<Sample> = serde_json::from_value(age["samples"].clone()).unwrap();
    let config: Config = serde_json::from_value(age["config"].clone()).unwrap();
    assert!(
        samples.len() >= 300,
        "independent age sampling must cover operating time"
    );
    assert_eq!(age["captured_samples"], samples.len());
    assert_eq!(age["marker_copy_pixels"], samples.len() * 10);
    assert_eq!(age["log_readbacks"], 1);
    assert_eq!(age["ring_full"], 0);
    let channels = frame_age::analyze(config, &samples).unwrap();
    for channel in &channels {
        assert_eq!(channel.invalid, 0);
        assert!(channel.valid >= 200);
        assert!(channel.unique_frames >= 100);
        assert_eq!(channel.frame_age.as_ref().unwrap().samples, channel.valid);
    }
    assert_eq!(channels[1].absent, 0);
}

pub fn assert_metrics(generation: &Value) {
    for (index, name) in ["preview_presents", "output_presents"].iter().enumerate() {
        let admission = &generation["admission_editor_output"][index];
        assert_eq!(admission["accepted_presents"], generation[*name]);
        assert_eq!(
            admission["readiness_polls"].as_u64().unwrap(),
            admission["ready_signals"].as_u64().unwrap() + admission["not_ready"].as_u64().unwrap()
        );
        assert!(admission["accepted_presents"].as_u64().unwrap() <= admission["ready_signals"].as_u64().unwrap());
    }
    let retired = &generation["allocations_after_fence"];
    assert_eq!(retired["live_textures"], 0);
    assert_eq!(retired["retiring_textures"], 0);
    assert_eq!(retired["live_bytes_by_owner"], serde_json::json!([0, 0, 0, 0]));
    assert_eq!(retired["retiring_bytes_by_owner"], serde_json::json!([0, 0, 0, 0]));
    assert!(retired["peak_live_plus_retiring_textures"].as_u64().unwrap() >= 9);
    let operations = &generation["operations"];
    assert_eq!(operations["explicit_gpu_copy_calls"], 0);
    assert_eq!(operations["diagnostic_readback_pixels"], 0);
    assert_eq!(operations["uniform_upload_bytes"], 0);
    let sends = generation["spout_sends"].as_u64().unwrap();
    assert_eq!(operations["sdk_send_calls"], sends);
    assert_eq!(operations["sdk_receive_calls"], 0);
    let queries = operations["sdk_texture_queries"].as_u64().unwrap();
    assert!(queries <= 2 * sends);
    assert!(operations["sdk_texture_observations"].as_u64().unwrap() <= queries);
    let workers = generation["source_workers"].as_array().unwrap();
    assert_eq!(workers.len(), 2);
    for worker in workers {
        assert_eq!(worker["maximum_surfaces"], 3);
        assert!((1..=3).contains(&worker["peak_surfaces"].as_u64().unwrap()));
        let resources = &worker["resources"];
        assert_eq!(resources["after_fence"]["live_textures"], 0);
        assert_eq!(resources["after_fence"]["retiring_textures"], 0);
        assert_eq!(resources["operations"]["sdk_receive_calls"], worker["receive_calls"]);
        assert_eq!(
            resources["operations"]["explicit_gpu_copy_calls"],
            worker["publications"]
        );
        assert_eq!(resources["operations"]["diagnostic_readback_pixels"], 0);
        assert_eq!(resources["operations"]["uniform_upload_bytes"], 0);
        assert_eq!(worker["truncated_receive_samples"], 0);
        let gate = &worker["gpu_admission"];
        assert_eq!(gate["maximum_in_flight"], 2);
        assert!(gate["peak_in_flight"].as_u64().unwrap() <= 2);
        assert_eq!(gate["pending"], 0);
        assert_eq!(gate["submitted"], worker["receive_calls"]);
        assert_eq!(gate["completed"], gate["submitted"]);
    }
    let samples = generation["timing"]["cpu_submission_work"]["samples"].as_u64().unwrap();
    let pipeline = &generation["gpu_admission"];
    assert_eq!(pipeline["maximum_in_flight"], 2);
    assert!(pipeline["peak_in_flight"].as_u64().unwrap() <= 2);
    assert_eq!(pipeline["pending"], 0);
    assert_eq!(pipeline["reserved"], false);
    assert_eq!(pipeline["submitted"], pipeline["completed"]);
    assert_eq!(pipeline["submitted"].as_u64().unwrap(), sends);
    assert_eq!(pipeline["sampled"]["admitted"].as_u64().unwrap(), samples);
    let attempts = pipeline["sampled"]["attempts"].as_u64().unwrap();
    let trace = &generation["operation_timing"];
    assert_eq!(trace["sampled"]["frames"].as_u64().unwrap(), attempts);
    assert_eq!(
        trace["sampled"]["frames"].as_u64().unwrap() + trace["warmup"]["frames"].as_u64().unwrap(),
        pipeline["sampled"]["attempts"].as_u64().unwrap() + pipeline["warmup"]["attempts"].as_u64().unwrap()
    );
    let names = trace["operation_names"].as_array().unwrap();
    assert_eq!(names.len(), 17);
    assert_eq!(names[4], "source_poll_a");
    assert_eq!(names[5], "source_poll_b");
    for window in ["warmup", "sampled"] {
        let outliers = &trace[window]["outliers"];
        assert_eq!(
            outliers["events"].as_array().unwrap().len() as u64 + outliers["omitted"].as_u64().unwrap(),
            outliers["total"].as_u64().unwrap()
        );
        assert!(outliers["events"].as_array().unwrap().len() <= 128);
        for event in outliers["events"].as_array().unwrap() {
            let elapsed = event["elapsed_ns"].as_u64().unwrap();
            let mut previous_end = 0;
            // Registry order is execution order for these disjoint measured operations.
            for span in event["spans"].as_array().unwrap().iter().filter(|span| !span.is_null()) {
                let offset = span["offset_ns"].as_u64().unwrap();
                let duration = span["elapsed_ns"].as_u64().unwrap();
                assert!(offset >= previous_end && offset + duration <= elapsed);
                previous_end = offset + duration;
            }
        }
    }
    for cadence in generation["submission_editor_output_spout"].as_array().unwrap() {
        assert_eq!(cadence["attempts"].as_u64().unwrap(), attempts);
        let sum = ["accepted_calls", "busy", "occluded", "no_target"]
            .iter()
            .map(|name| cadence[name].as_u64().unwrap())
            .sum::<u64>();
        assert_eq!(sum, attempts);
        assert!(cadence["late_attempts"].as_u64().unwrap() <= attempts);
        assert!(cadence["busy"].as_u64().unwrap() >= pipeline["sampled"]["busy"].as_u64().unwrap());
    }
    assert_eq!(
        generation["submission_editor_output_spout"][2]["accepted_calls"],
        samples
    );
}
