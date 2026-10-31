//! Finite, test-owned executable. Never starts an unrelated executable or opens a visible window.
use std::{
    ffi::OsString,
    path::PathBuf,
    time::{Duration, Instant},
};

#[cfg(windows)]
mod owned_window;

fn main() {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let Some(mode) = arguments.first().and_then(|value| value.to_str()) else {
        return;
    };
    let report = PathBuf::from(&arguments[1]);
    let stop = PathBuf::from(&arguments[2]);
    match mode {
        #[cfg(windows)]
        "window" => owned_window::run(&report),
        "report" => {
            let mut text = format!(
                "cwd={}\nenv={}\n",
                std::env::current_dir().unwrap().display(),
                std::env::var("GOLDEN_PROCESS_FIXTURE").unwrap_or_default()
            );
            for value in &arguments[3..] {
                text.push_str(&format!("{}\n", units(value)));
            }
            std::fs::write(report, text).unwrap();
        }
        "tree" | "root-exit" | "branch" => {
            let child_mode = if mode == "branch" { "leaf" } else { "branch" };
            let child_report = report.with_extension(if mode == "branch" { "leaf" } else { "branch" });
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args([
                OsString::from(child_mode),
                child_report.into_os_string(),
                stop.clone().into_os_string(),
            ]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x0800_0000);
            }
            let mut child = command.spawn().unwrap();
            std::fs::write(&report, format!("{}\n{}\n", std::process::id(), child.id())).unwrap();
            if mode == "root-exit" {
                drop(child); // Deliberate root exit: the supervisor's Job Object retains the descendant.
                return;
            }
            wait(stop);
            let _ = child.wait();
        }
        "leaf" => {
            std::fs::write(report, std::process::id().to_string()).unwrap();
            wait(stop);
        }
        "transient" => {
            let child_report = report.with_extension("child");
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args([
                OsString::from("report"),
                child_report.into_os_string(),
                stop.clone().into_os_string(),
            ]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x0800_0000);
            }
            assert!(command.status().unwrap().success());
            std::fs::write(report, b"transient exited").unwrap();
            wait(stop);
        }
        _ => std::process::exit(2),
    }
}
fn wait(stop: PathBuf) {
    let end = Instant::now() + Duration::from_secs(20);
    while !stop.exists() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[cfg(windows)]
fn units(value: &std::ffi::OsStr) -> String {
    use std::os::windows::ffi::OsStrExt;
    value
        .encode_wide()
        .map(|unit| format!("{unit:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}
#[cfg(not(windows))]
fn units(value: &std::ffi::OsStr) -> String {
    value.to_string_lossy().into_owned()
}
