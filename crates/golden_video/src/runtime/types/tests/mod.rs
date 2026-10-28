use super::*;

#[test]
fn reliable_diagnostics_keep_a_valid_bounded_utf8_prefix() {
    let message = "é🎨".repeat(1_000);
    let failure = RuntimeFailure::new(FailureCode::InvalidPlan, &message);
    assert!(failure.message().len() <= 1_024);
    assert!(message.starts_with(failure.message()));
    assert_eq!(failure.code(), FailureCode::InvalidPlan);
    assert!(failure.message().len() > 1_000);
}
