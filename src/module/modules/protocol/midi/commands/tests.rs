use super::validate_14_bit_controller;

#[test]
fn validate_14_bit_controller_accepts_msb_range() {
    assert_eq!(validate_14_bit_controller(0), Ok(0));
    assert_eq!(validate_14_bit_controller(31), Ok(31));
}

#[test]
fn validate_14_bit_controller_rejects_lsb_range() {
    let error = validate_14_bit_controller(32).expect_err("controller 32 should be reserved for the paired LSB role");
    assert!(error.contains("0-31 range"));
    assert!(error.contains("32"));
}
