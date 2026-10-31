//! Finite native targets for ordinary-app lifecycle acceptance; never launch user software.
use serde_json::json;
use std::{
    os::windows::{io::FromRawHandle, process::CommandExt},
    path::Path,
    process::Command,
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        System::Threading::{CREATE_BREAKAWAY_FROM_JOB, CREATE_NO_WINDOW, CreateMutexW},
    },
    core::PCWSTR,
};

const LIFETIME: Duration = Duration::from_secs(20);

pub fn run() -> bool {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(mode) = arguments.first().map(String::as_str) else {
        return false;
    };
    match mode {
        "--lifecycle-singleton" => singleton(&arguments[1], Path::new(&arguments[2])),
        "--lifecycle-launcher" => launcher(Path::new(&arguments[1])),
        "--lifecycle-breakaway" => breakaway(Path::new(&arguments[1])),
        "--lifecycle-leaf" => {
            std::fs::write(&arguments[1], std::process::id().to_string()).unwrap();
            std::thread::sleep(LIFETIME);
        }
        _ => return false,
    }
    true
}

fn singleton(name: &str, report: &Path) {
    let name = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    // Keep the named object alive without claiming mutex ownership or inspecting another process.
    let handle = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())).unwrap() };
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let _handle = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(handle.0) };
    std::fs::write(
        report,
        json!({ "pid": std::process::id(), "already_exists": exists }).to_string(),
    )
    .unwrap();
    if exists {
        std::process::exit(17);
    }
    std::thread::sleep(LIFETIME);
}

fn child(report: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args([std::ffi::OsStr::new("--lifecycle-leaf"), report.as_os_str()]);
    command.creation_flags(CREATE_NO_WINDOW.0);
    command
}

fn launcher(report: &Path) {
    let child = child(&report.with_extension("child")).spawn().unwrap();
    std::fs::write(
        report,
        json!({ "root": std::process::id(), "child": child.id() }).to_string(),
    )
    .unwrap();
    // Give the normal controller an opportunity to observe the child before root exit.
    std::thread::sleep(Duration::from_millis(400));
    drop(child); // The app's job retains the descendant; this successful root exit is not readiness.
}

fn breakaway(report: &Path) {
    let mut command = child(&report.with_extension("escaped"));
    command.creation_flags((CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB).0);
    let error = match command.spawn() {
        Err(error) => error.raw_os_error(),
        Ok(mut child) => {
            // A failing ownership test must still clean up its own finite escaped target.
            child.kill().unwrap();
            child.wait().unwrap();
            None
        }
    };
    std::fs::write(
        report,
        json!({ "pid": std::process::id(), "breakaway_error": error }).to_string(),
    )
    .unwrap();
    std::thread::sleep(LIFETIME);
}
