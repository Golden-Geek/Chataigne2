#![cfg(all(windows, feature = "process-fixture"))]
use golden_io::owned_process::{ProcessSpec, RootProcessState, SuspendedProcess};
use std::{
    ffi::OsString,
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS, WaitForSingleObject},
};

const WAIT: Duration = Duration::from_secs(3);
fn executable() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_golden_io_process_fixture"))
}
fn spec(mode: &str, report: &Path, stop: &Path) -> ProcessSpec {
    let mut spec = ProcessSpec::new(executable());
    spec.arguments = vec![mode.into(), report.as_os_str().into(), stop.as_os_str().into()];
    spec
}
fn wait(mut check: impl FnMut() -> bool) {
    let end = Instant::now() + WAIT;
    while !check() {
        assert!(Instant::now() < end, "owned fixture deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn monitor(id: u32) -> OwnedHandle {
    // SYNCHRONIZE only. Open before releasing the owner, so PID reuse cannot change this observation.
    unsafe { OwnedHandle::from_raw_handle(OpenProcess(PROCESS_ACCESS_RIGHTS(0x0010_0000), false, id).unwrap().0) }
}
fn exited(handle: &OwnedHandle, millis: u32) -> bool {
    unsafe { WaitForSingleObject(HANDLE(handle.as_raw_handle()), millis) == WAIT_OBJECT_0 }
}
fn units(value: &std::ffi::OsStr) -> String {
    value
        .encode_wide()
        .map(|unit| format!("{unit:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}
fn tree_ready(report: &Path) -> (u32, u32) {
    let branch = report.with_extension("branch");
    let leaf = report.with_extension("leaf");
    wait(|| report.exists() && branch.exists() && leaf.exists());
    let ids = std::fs::read_to_string(branch)
        .unwrap()
        .lines()
        .map(|line| line.parse::<u32>().unwrap())
        .collect::<Vec<_>>();
    (ids[0], ids[1])
}

#[test]
fn primary_thread_stays_suspended_and_resume_preserves_argv_environment_and_directory() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("arguments.txt");
    let mut spec = spec("report", &report, &directory.path().join("stop"));
    let values = [
        "",
        "space tab\targument",
        "quote\"value",
        "trailing\\",
        "slash\\\\\"quote",
        "你好🙂",
        "& | > $(literal)",
    ];
    spec.arguments.extend(values.iter().map(OsString::from));
    spec.directory = Some(directory.path().into());
    spec.environment = Some(vec![("GOLDEN_PROCESS_FIXTURE".into(), "unicode value λ".into())]);
    let suspended = SuspendedProcess::create(&spec).unwrap();
    assert_eq!(suspended.snapshot().unwrap().active_processes, 1);
    assert_eq!(suspended.snapshot().unwrap().root, RootProcessState::Running);
    std::thread::sleep(Duration::from_millis(25));
    assert!(!report.exists());
    assert!(!suspended.wait_tree(Duration::ZERO).unwrap());
    let owned = suspended.resume().unwrap();
    assert!(owned.wait_tree(WAIT).unwrap());
    assert_eq!(owned.snapshot().unwrap().root, RootProcessState::Exited(0));
    let expected = format!(
        "cwd={}\nenv=unicode value λ\n{}\n",
        directory.path().display(),
        values
            .iter()
            .map(|value| units(std::ffi::OsStr::new(value)))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(std::fs::read_to_string(report).unwrap(), expected);
}

#[test]
fn dropping_suspended_ownership_cancels_without_executing_user_code() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("never.txt");
    let suspended = SuspendedProcess::create(&spec("report", &report, &directory.path().join("stop"))).unwrap();
    let root = monitor(suspended.id());
    drop(suspended);
    assert!(exited(&root, 3000));
    assert!(!report.exists());
}

#[test]
fn root_exit_does_not_release_descendants_and_termination_waits_for_the_entire_tree() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("tree.txt");
    let owned = SuspendedProcess::create(&spec("root-exit", &report, &directory.path().join("stop")))
        .unwrap()
        .resume()
        .unwrap();
    let (branch, leaf) = tree_ready(&report);
    let branch = monitor(branch);
    let leaf = monitor(leaf);
    wait(|| owned.snapshot().unwrap().root == RootProcessState::Exited(0));
    assert!(owned.snapshot().unwrap().active_processes >= 2);
    assert!(!owned.wait_tree(Duration::ZERO).unwrap());
    assert_eq!(
        owned.request_close().unwrap(),
        0,
        "headless processes do not acknowledge graceful shutdown"
    );
    assert!(!owned.wait_tree(Duration::from_millis(20)).unwrap());
    owned.terminate(73).unwrap();
    let retired = owned.wait_tree(WAIT).unwrap();
    let observation = owned.snapshot().unwrap();
    assert_eq!(observation.active_processes, 0);
    assert!(retired, "{observation:?}");
    assert!(
        exited(&branch, 0) && exited(&leaf, 0),
        "retirement cannot precede known handle completion"
    );
}

#[test]
fn dropping_running_ownership_kills_child_and_grandchild_without_inherited_job_handles() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("tree.txt");
    let owned = SuspendedProcess::create(&spec("tree", &report, &directory.path().join("stop")))
        .unwrap()
        .resume()
        .unwrap();
    let (branch, leaf) = tree_ready(&report);
    let root = monitor(owned.id());
    let branch = monitor(branch);
    let leaf = monitor(leaf);
    drop(owned);
    assert!(exited(&root, 3000) && exited(&branch, 3000) && exited(&leaf, 3000));
}

#[test]
fn app_owned_graceful_signal_and_termination_exit_codes_are_observed_without_false_retirement() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("tree.txt");
    let stop = directory.path().join("stop");
    let owned = SuspendedProcess::create(&spec("tree", &report, &stop))
        .unwrap()
        .resume()
        .unwrap();
    tree_ready(&report);
    assert_eq!(owned.snapshot().unwrap().unobserved_processes, 0);
    std::fs::write(&stop, b"stop").unwrap();
    assert!(owned.wait_tree(WAIT).unwrap());
    assert_eq!(owned.snapshot().unwrap().root, RootProcessState::Exited(0));
    let report = directory.path().join("forced.txt");
    let suspended = SuspendedProcess::create(&spec("report", &report, &stop)).unwrap();
    suspended.terminate(259).unwrap();
    assert!(suspended.wait_tree(WAIT).unwrap());
    assert_eq!(suspended.snapshot().unwrap().root, RootProcessState::Exited(259));
    assert!(!report.exists());
}

#[test]
fn invalid_launches_and_missing_executables_reject_before_any_fixture_executes() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("never.txt");
    let stop = directory.path().join("stop");
    let mut invalid = vec![];
    invalid.push(ProcessSpec::new("relative.exe"));
    invalid.push(ProcessSpec::new(directory.path().join("script.cmd")));
    let mut value = spec("report", &report, &stop);
    value.arguments.push(OsString::from("bad\0argument"));
    invalid.push(value);
    let mut value = spec("report", &report, &stop);
    value.directory = Some("relative".into());
    invalid.push(value);
    let mut value = spec("report", &report, &stop);
    value.environment = Some(vec![("Path".into(), "x".into()), ("PATH".into(), "y".into())]);
    invalid.push(value);
    let mut value = spec("report", &report, &stop);
    value.arguments.push("x".repeat(32_767).into());
    invalid.push(value);
    for value in invalid {
        assert_eq!(
            SuspendedProcess::create(&value).err().unwrap().error().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    let missing = ProcessSpec::new(directory.path().join("missing.exe"));
    assert_eq!(
        SuspendedProcess::create(&missing).err().unwrap().error().raw_os_error(),
        Some(2)
    );
    assert!(!report.exists());
}

struct ExternalFixture(std::process::Child);

#[test]
fn a_close_request_reaches_only_the_owned_hidden_window_and_observes_graceful_exit() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("window.txt");
    let owned = SuspendedProcess::create(&spec("window", &report, &directory.path().join("stop")))
        .unwrap()
        .resume()
        .unwrap();
    wait(|| report.exists());
    assert!(owned.request_close().unwrap() >= 1);
    let retired = owned.wait_tree(WAIT).unwrap();
    let observation = owned.snapshot().unwrap();
    assert_eq!(observation.root, RootProcessState::Exited(0));
    assert_eq!(observation.active_processes, 0);
    assert!(retired, "{observation:?}");
}

#[test]
fn a_transient_child_is_captured_without_any_caller_snapshot_during_its_lifetime() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("transient.txt");
    let stop = directory.path().join("stop");
    let owned = SuspendedProcess::create(&spec("transient", &report, &stop))
        .unwrap()
        .resume()
        .unwrap();
    wait(|| report.exists());
    assert_eq!(owned.snapshot().unwrap().unobserved_processes, 0);
    owned.terminate(1).unwrap();
    assert!(owned.wait_tree(WAIT).unwrap());
    let observation = owned.snapshot().unwrap();
    assert_eq!(observation.active_processes, 0);
    assert!(matches!(observation.root, RootProcessState::Exited(_)));
    assert!(observation.tree_exited());
}

#[test]
fn an_observer_that_has_not_stopped_cannot_authorize_retirement() {
    use golden_io::owned_process::ProcessSnapshot;
    let mut snapshot = ProcessSnapshot {
        root_id: 42,
        root: RootProcessState::Exited(0),
        active_processes: 0,
        pending_observed_processes: 0,
        unobserved_processes: 0,
        observation_complete: false,
    };
    assert!(!snapshot.tree_exited());
    snapshot.observation_complete = true;
    assert!(snapshot.tree_exited());
    snapshot.unobserved_processes = 1;
    assert!(!snapshot.tree_exited());
}

#[test]
fn creation_and_observation_can_move_between_caller_threads() {
    let directory = tempfile::tempdir().unwrap();
    let specification = spec(
        "report",
        &directory.path().join("report"),
        &directory.path().join("stop"),
    );
    let suspended = std::thread::spawn(move || SuspendedProcess::create(&specification).unwrap())
        .join()
        .unwrap();
    let owned = suspended.resume().unwrap();
    assert!(
        std::thread::spawn(move || owned.wait_tree(WAIT).unwrap())
            .join()
            .unwrap()
    );
}

#[test]
fn sixteen_transient_lifetimes_and_root_exit_complete_without_caller_polling() {
    let directory = tempfile::tempdir().unwrap();
    let report = directory.path().join("burst");
    let owned = SuspendedProcess::create(&spec("transient-burst-exit", &report, &directory.path().join("stop")))
        .unwrap()
        .resume()
        .unwrap();
    wait(|| report.exists());
    assert!(owned.wait_tree(WAIT).unwrap());
    assert!(owned.snapshot().unwrap().tree_exited());
}

#[test]
fn an_application_exception_is_forwarded_and_its_real_exit_code_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let owned = SuspendedProcess::create(&spec(
        "exception",
        &directory.path().join("report"),
        &directory.path().join("stop"),
    ))
    .unwrap()
    .resume()
    .unwrap();
    assert!(owned.wait_tree(WAIT).unwrap());
    assert_eq!(owned.snapshot().unwrap().root, RootProcessState::Exited(0xe042_0001));
}

#[cfg(target_arch = "x86_64")]
#[test]
#[ignore = "retained WOW64 retirement qualification; see mapgyver-m08-retirement.md before replay"]
fn a_wow64_system_tool_preserves_its_ordinary_exit_code() {
    // Exercise the installed 32-bit loader without changing an OS file or launching a shell.
    // WHERE /Q only tests existence; it neither executes CMD nor writes identifying output.
    let directory = PathBuf::from(std::env::var_os("SystemRoot").unwrap());
    let mut specification = ProcessSpec::new(directory.join("SysWOW64").join("where.exe"));
    // WHERE documents a plain filename searched in its current directory. Keep the Windows
    // directory out of its pattern grammar (which has a separate Path:Pattern form).
    specification.directory = Some(directory.join("System32"));
    specification.arguments = vec!["/q".into(), "cmd.exe".into()];
    let owned = SuspendedProcess::create(&specification).unwrap().resume().unwrap();
    assert!(owned.wait_tree(WAIT).unwrap(), "{:?}", owned.snapshot());
    assert_eq!(owned.snapshot().unwrap().root, RootProcessState::Exited(0));
}
impl Drop for ExternalFixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn an_existing_same_name_process_is_never_adopted_or_killed() {
    use std::os::windows::process::CommandExt;
    let directory = tempfile::tempdir().unwrap();
    let stop = directory.path().join("stop");
    let outside_report = directory.path().join("outside.txt");
    let mut outside = ExternalFixture(
        std::process::Command::new(executable())
            .creation_flags(0x0800_0000)
            .args([
                OsString::from("leaf"),
                outside_report.clone().into_os_string(),
                stop.clone().into_os_string(),
            ])
            .spawn()
            .unwrap(),
    );
    wait(|| outside_report.exists());
    let report = directory.path().join("owned.txt");
    let owned = SuspendedProcess::create(&spec("leaf", &report, &stop))
        .unwrap()
        .resume()
        .unwrap();
    wait(|| report.exists());
    owned.terminate(1).unwrap();
    assert!(owned.wait_tree(WAIT).unwrap());
    assert!(outside.0.try_wait().unwrap().is_none());
    assert!(!matches!(
        unsafe { WaitForSingleObject(HANDLE(outside.0.as_raw_handle()), 0) },
        WAIT_OBJECT_0
    ));
    assert_eq!(
        unsafe { WaitForSingleObject(HANDLE(outside.0.as_raw_handle()), 0) },
        WAIT_TIMEOUT
    );
}
