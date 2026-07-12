use std::ffi::OsString;

use serde_json::json;

use super::{digest, parse_invocation, run_named_scenario, OSC_LOOPBACK_SCENARIO};

#[test]
fn ordinary_host_arguments_are_not_intercepted() {
    assert_eq!(
        parse_invocation([OsString::from("--dev")]).expect("host args should parse"),
        None
    );
    assert_eq!(
        parse_invocation([OsString::from("--headless"), OsString::from("--no-remote")])
            .expect("host args should parse"),
        None
    );
}

#[test]
fn product_evidence_flag_is_explicit_and_exclusive() {
    let invocation = parse_invocation([
        OsString::from("--product-evidence"),
        OsString::from(OSC_LOOPBACK_SCENARIO),
    ])
    .expect("evidence invocation should parse")
    .expect("evidence invocation should be selected");
    assert_eq!(invocation.scenario, OSC_LOOPBACK_SCENARIO);

    assert!(parse_invocation([
        OsString::from("--dev"),
        OsString::from("--product-evidence"),
        OsString::from(OSC_LOOPBACK_SCENARIO),
    ])
    .is_err());
}

#[test]
fn semantic_digest_uses_a_versionable_stable_algorithm() {
    assert_eq!(digest::digest_bytes(b"hello"), "fnv1a64:a430d84680aabd0b");
    let value = json!({ "input": 42, "order": ["first", "second"] });
    assert_eq!(
        digest::semantic_digest(&value).expect("evidence should encode"),
        digest::semantic_digest(&value).expect("evidence should encode")
    );
}

#[test]
fn unknown_named_scenario_fails() {
    let error = run_named_scenario("not-a-scenario").expect_err("unknown scenarios must fail");
    assert!(error.contains(OSC_LOOPBACK_SCENARIO));
}

#[test]
fn osc_loopback_runs_through_real_app_engine_and_round_trips_semantics() {
    let evidence = run_named_scenario(OSC_LOOPBACK_SCENARIO).expect("OSC loopback evidence should pass");
    assert_eq!(
        digest::semantic_digest(&evidence).expect("scenario evidence should encode"),
        "fnv1a64:9da80781af2c7655"
    );
    assert_eq!(evidence["command_creation_ack"], true);
    assert_eq!(evidence["input"]["value"], 42);
    assert_eq!(
        evidence["effect_order"],
        json!(["/evidence/output/1", "/evidence/output/2"])
    );
    assert_eq!(
        evidence["save_reload"]["state"]["input_value"],
        evidence["input"]["value"]
    );
    assert_eq!(evidence["save_reload"]["semantic_digest"], "fnv1a64:78a1a93d927b4a39");
}
