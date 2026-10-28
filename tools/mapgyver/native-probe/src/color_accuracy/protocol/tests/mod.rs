use super::*;
use mapgyver_native_probe::{
    allocation::{Operations, Snapshot},
    color_reference::{Comparison, Difference},
    workload::Scenario,
};

fn consumer() -> Report {
    Report {
        process_id: 123,
        adapter_luid: "adapter".into(),
        role: Role::Consumer,
        fixture: Fixture::Authored,
        initial_pixel_upload_bytes: 0,
        full_image_readbacks: 1,
        sender_unregistered: None,
        device: DeviceReport {
            role: "consumer".into(),
            operations: Operations::default(),
            after_fence: Snapshot::default(),
            retirement_ms: 0.1,
        },
        comparison: Some(Comparison {
            extent: [3840, 2160],
            compared_pixels: 3840 * 2160,
            tolerance_per_component: 2,
            maximum_error_bgra: [255; 4],
            differing_components_bgra: [3840 * 2160; 4],
            mismatched_pixels: 3840 * 2160,
            first_differences: (0..16)
                .map(|_| Difference {
                    position: [3839, 2159],
                    expected_bgra: [255; 4],
                    actual_bgra: [0; 4],
                })
                .collect(),
        }),
    }
}

#[test]
fn mismatching_full_image_reports_still_fit_the_bounded_pipe_and_preserve_failure() {
    let response = Response::Finished {
        report: Box::new(consumer()),
    };
    let bytes = crate::wire::line(&response).unwrap();
    assert!(bytes.len() <= crate::wire::MAX_LINE_BYTES as usize);
    let Response::Finished { report } = crate::wire::read_message::<Response>(&mut &bytes[..]).unwrap().unwrap() else {
        panic!("wrong response");
    };
    assert_eq!(report.comparison.as_ref().unwrap().mismatched_pixels, 3840 * 2160);
    report
        .validate_identity(Role::Consumer, 123, "adapter", Scenario::Q02, Fixture::Authored)
        .unwrap();
}

#[test]
fn child_identity_wrong_role_adapter_pid_and_incomplete_coverage_cannot_pass() {
    let r = consumer();
    for (role, pid, luid, scenario) in [
        (Role::SourceA, 123, "adapter", Scenario::Q02),
        (Role::Consumer, 0, "adapter", Scenario::Q02),
        (Role::Consumer, 124, "adapter", Scenario::Q02),
        (Role::Consumer, 123, "other", Scenario::Q02),
        (Role::Consumer, 123, "adapter", Scenario::Q01),
    ] {
        assert!(
            r.validate_identity(role, pid, luid, scenario, Fixture::Authored)
                .is_err()
        );
    }
    let mut r = consumer();
    r.comparison.as_mut().unwrap().compared_pixels -= 1;
    assert!(
        r.validate_identity(Role::Consumer, 123, "adapter", Scenario::Q02, Fixture::Authored)
            .is_err()
    );
    let mut r = consumer();
    r.initial_pixel_upload_bytes = 4;
    assert!(
        r.validate_identity(Role::Consumer, 123, "adapter", Scenario::Q02, Fixture::Authored)
            .is_err()
    );
}

#[test]
fn a_report_for_a_different_fixture_cannot_satisfy_the_requested_shader_check() {
    let mut r = consumer();
    assert!(
        r.validate_identity(Role::Consumer, 123, "adapter", Scenario::Q02, Fixture::Procedural)
            .is_err()
    );
    r.fixture = Fixture::Procedural;
    r.validate_identity(Role::Consumer, 123, "adapter", Scenario::Q02, Fixture::Procedural)
        .unwrap();
}
