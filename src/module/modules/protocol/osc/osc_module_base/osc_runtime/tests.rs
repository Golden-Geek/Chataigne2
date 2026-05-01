use std::io;

use super::should_ignore_receive_error;

#[test]
fn ignores_windows_udp_icmp_connreset_receive_error() {
    let error = io::Error::from_raw_os_error(10054);
    assert!(should_ignore_receive_error(&error));
}

#[test]
fn keeps_reporting_unrelated_receive_errors() {
    let error = io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe");
    assert!(!should_ignore_receive_error(&error));
}
