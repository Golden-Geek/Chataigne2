use super::*;

#[test]
#[ignore = "requires the explicitly built Golden IO process fixture after software prerequisites"]
fn native_post_create_rejection_returns_a_retirement_owner_without_executing_target_code() {
    use crate::owned_process::OwnedProcess;
    use std::time::Duration;

    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("never.txt");
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("golden_io_process_fixture.exe");
    assert!(executable.is_file(), "build golden_io with process-fixture first");
    let mut spec = ProcessSpec::new(executable);
    spec.arguments = vec![
        "report".into(),
        report.clone().into_os_string(),
        directory.path().join("stop").into_os_string(),
    ];
    fn reject(_: &OwnedHandle, _: &OwnedHandle) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(5))
    }
    let error = Process::start(&spec, reject)
        .err()
        .expect("forced post-create rejection");
    assert_eq!(error.error().raw_os_error(), Some(5));
    let (error, owner) = error.map_owner(OwnedProcess).into_parts();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    let owner = owner.expect("a created root must remain owned through failure");
    assert_ne!(owner.id(), 0);
    assert!(owner.wait_tree(Duration::from_secs(3)).unwrap());
    assert!(owner.snapshot().unwrap().tree_exited());
    assert!(!report.exists(), "a rejected creation must never execute target code");
}

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
