use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub struct OwnedChild {
    label: String,
    child: Option<Child>,
    relays: Vec<JoinHandle<()>>,
}

impl OwnedChild {
    pub fn spawn(label: &str, command: &mut Command) -> Result<Self, String> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_process_group(command);

        let mut child = command
            .spawn()
            .map_err(|error| format!("failed to start {label}: {error}"))?;
        let mut relays = Vec::with_capacity(2);
        if let Some(stdout) = child.stdout.take() {
            relays.push(spawn_relay(label, "stdout", stdout));
        }
        if let Some(stderr) = child.stderr.take() {
            relays.push(spawn_relay(label, "stderr", stderr));
        }

        eprintln!("[watch][{label}] started pid={}", child.id());
        Ok(Self {
            label: label.to_string(),
            child: Some(child),
            relays,
        })
    }

    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, String> {
        let Some(child) = self.child.as_mut() else {
            return Ok(None);
        };
        child
            .try_wait()
            .map_err(|error| format!("failed to inspect {} process: {error}", self.label))
    }

    pub fn terminate(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };

        if child.try_wait().ok().flatten().is_none() {
            eprintln!("[watch][{}] stopping pid={}", self.label, child.id());
            terminate_process_tree(&mut child);
        }
        let _ = child.wait();
        for relay in self.relays.drain(..) {
            let _ = relay.join();
        }
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn spawn_relay<R>(label: &str, stream: &str, reader: R) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    let prefix = format!("[watch][{label}][{stream}]");
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut bytes = Vec::new();
        loop {
            bytes.clear();
            match reader.read_until(b'\n', &mut bytes) {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&bytes);
                    eprint!("{prefix} {}", line);
                    if !line.ends_with('\n') {
                        eprintln!();
                    }
                }
                Err(error) => {
                    eprintln!("{prefix} log relay stopped: {error}");
                    break;
                }
            }
        }
    })
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn configure_process_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(windows)]
fn terminate_process_tree(child: &mut Child) {
    let pid = child.id().to_string();
    let _ = Command::new("taskkill")
        .args(["/PID", &pid, "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    wait_then_kill(child);
}

#[cfg(unix)]
fn terminate_process_tree(child: &mut Child) {
    signal_process_group("-TERM", child.id());
    if !wait_for_exit(child, Duration::from_secs(2)) {
        signal_process_group("-KILL", child.id());
    }
    wait_then_kill(child);
}

#[cfg(unix)]
fn signal_process_group(signal: &str, pid: u32) {
    let group = format!("-{pid}");
    let _ = Command::new("kill")
        .args([signal, &group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_tree(child: &mut Child) {
    wait_then_kill(child);
}

fn wait_then_kill(child: &mut Child) {
    if !wait_for_exit(child, Duration::from_secs(2)) {
        let _ = child.kill();
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            _ => return false,
        }
    }
}

pub fn command_display(program: &OsStr, args: &[&str]) -> String {
    let mut display = program.to_string_lossy().into_owned();
    for argument in args {
        display.push(' ');
        display.push_str(argument);
    }
    display
}
