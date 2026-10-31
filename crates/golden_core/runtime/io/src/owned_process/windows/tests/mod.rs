use super::*;

#[test]
fn native_access_and_unsupported_errors_preserve_their_original_codes_without_a_fallback() {
    for code in [5u32, 50, 740] {
        let failure = ::windows::core::Error::from_hresult(::windows::core::HRESULT::from_win32(code));
        let error = os_error(failure);
        assert_eq!(error.raw_os_error(), Some(code as i32));
        if code == 5 {
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        }
    }
}
