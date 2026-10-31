//! Desktop-helper launch keeps the owned UI at normal integrity even from an elevated IDE.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::fs::{self, OpenOptions};
use std::io::{self, Error};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(serde::Deserialize)]
struct Launch {
    directory: PathBuf,
}

struct OwnedChild(std::process::Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn record(path: PathBuf, value: serde_json::Value) -> io::Result<()> {
    let pending = path.with_extension("tmp");
    let file = OpenOptions::new().write(true).create_new(true).open(&pending)?;
    serde_json::to_writer(file, &value)?;
    fs::rename(pending, path)
}

fn run() -> io::Result<()> {
    let executable = std::env::current_exe()?;
    let binaries = executable
        .parent()
        .ok_or_else(|| Error::other("missing binary directory"))?;
    let target = binaries
        .parent()
        .ok_or_else(|| Error::other("missing target directory"))?;
    let launch: Launch = serde_json::from_slice(&fs::read(target.join("m05-native-ui-launch.json"))?)?;
    let directory = launch.directory.canonicalize()?;
    if !directory.starts_with(target.canonicalize()?) {
        return Err(Error::other("fixture directory must remain below target"));
    }
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("app.log"))?;
    let mut child = OwnedChild(
        Command::new(binaries.join("MapGyver.exe"))
            .args(["--no-remote", "--automation-shutdown-file"])
            .arg(directory.join("app.stop"))
            .env("APPDATA", directory.join("app-data"))
            .env("LOCALAPPDATA", directory.join("local-data"))
            .env("GC_UI_BIND", "127.0.0.1:7023")
            .env("GC_UI_FRONTEND_URL", "http://127.0.0.1:7023")
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    record(
        directory.join("launcher-start.json"),
        serde_json::json!({"pid":child.0.id()}),
    )?;
    let deadline = Instant::now() + Duration::from_secs(12 * 60);
    let (status, forced) = loop {
        if let Some(status) = child.0.try_wait()? {
            break (status, false);
        }
        if Instant::now() >= deadline {
            child.0.kill()?;
            break (child.0.wait()?, true);
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    record(
        directory.join("launcher-result.json"),
        serde_json::json!({"code":status.code(),"signal":null,"success":status.success(),"forced":forced}),
    )?;
    if forced {
        return Err(Error::other("owned UI fixture exceeded its watchdog"));
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("owned UI launch failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
