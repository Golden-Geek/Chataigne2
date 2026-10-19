use super::*;

#[test]
fn log_admission_returns_original_spare_capacity_instead_of_capturing_oversized_storage() {
    let message = String::with_capacity(64 * 1024 + 1);
    let pointer = message.as_ptr();
    let returned = retain_log(message).unwrap_err().into_input();
    assert_eq!(returned.as_ptr(), pointer);
    let mut message = String::with_capacity(4096);
    message.push('x');
    let pointer = message.as_ptr();
    let retained = retain_log(message).unwrap();
    assert_eq!(retained.as_ptr(), pointer);
    assert_eq!(retained.capacity(), 4096);
}
