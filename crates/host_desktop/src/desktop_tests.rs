#![cfg(target_os = "windows")]

use std::env;
use std::fs;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::desktop::DevFrontendProcess;

const HELPER_TEST_NAME: &str = "desktop_tests::dev_frontend_process_tree_helper";
const ROLE_ENV: &str = "GOLDEN_HOST_PROCESS_TREE_TEST_ROLE";
const READY_PATH_ENV: &str = "GOLDEN_HOST_PROCESS_TREE_TEST_READY_PATH";
const EXIT_ACK_PATH_ENV: &str = "GOLDEN_HOST_PROCESS_TREE_TEST_EXIT_ACK_PATH";

#[test]
fn dev_frontend_job_terminates_nested_processes_on_drop() {
    let ready_path = unique_test_path("drop-ready");
    let exit_ack_path = unique_test_path("drop-ack");
    let mut command = helper_command("parent", &ready_path, &exit_ack_path);
    let process =
        DevFrontendProcess::spawn(&mut command).expect("frontend process tree should start inside a Windows job");

    let port = wait_for_ready_port(&ready_path);
    assert_port_reachable(port);
    drop(process);
    assert_port_closes(port);

    remove_test_file(&ready_path);
    remove_test_file(&exit_ack_path);
}

#[test]
fn dev_frontend_job_terminates_nested_processes_when_owner_exits_without_drop() {
    let ready_path = unique_test_path("owner-exit-ready");
    let exit_ack_path = unique_test_path("owner-exit-ack");
    let mut owner = ChildGuard::new(
        helper_command("owner", &ready_path, &exit_ack_path)
            .spawn()
            .expect("job-owner helper should start"),
    );

    let port = wait_for_ready_port(&ready_path);
    assert_port_reachable(port);
    fs::write(&exit_ack_path, b"exit").expect("owner-exit helper acknowledgement should be written");
    let status = owner.wait_for_exit(Duration::from_secs(10));
    assert!(status.success(), "job-owner helper failed with {status}");
    assert_port_closes(port);

    remove_test_file(&ready_path);
    remove_test_file(&exit_ack_path);
}

#[test]
#[ignore = "subprocess helper invoked by the Windows process-job tests"]
fn dev_frontend_process_tree_helper() {
    let role = env::var(ROLE_ENV).expect("process-tree helper role should be set");
    let ready_path = PathBuf::from(env::var(READY_PATH_ENV).expect("process-tree helper ready path should be set"));
    let exit_ack_path =
        PathBuf::from(env::var(EXIT_ACK_PATH_ENV).expect("process-tree helper exit-ack path should be set"));

    match role.as_str() {
        "leaf" => run_leaf_helper(&ready_path),
        "parent" => {
            let status = helper_command("leaf", &ready_path, &exit_ack_path)
                .spawn()
                .expect("leaf helper should start")
                .wait()
                .expect("leaf helper should remain waitable");
            std::process::exit(status.code().unwrap_or(1));
        }
        "owner" => {
            let mut command = helper_command("parent", &ready_path, &exit_ack_path);
            let _process =
                DevFrontendProcess::spawn(&mut command).expect("owner helper should create a guarded process tree");
            let _ = wait_for_ready_port(&ready_path);
            wait_for_file(&exit_ack_path, Duration::from_secs(10));
            // Deliberately skip Rust destructors. Windows closes the job handle at process exit and
            // terminates every descendant just as it does after Ctrl-C or abrupt host exit.
            std::process::exit(0);
        }
        other => panic!("unknown process-tree helper role '{other}'"),
    }
}

fn helper_command(role: &str, ready_path: &Path, exit_ack_path: &Path) -> Command {
    let mut command = Command::new(env::current_exe().expect("test executable path should resolve"));
    command
        .args(["--exact", HELPER_TEST_NAME, "--ignored", "--nocapture"])
        .env(ROLE_ENV, role)
        .env(READY_PATH_ENV, ready_path)
        .env(EXIT_ACK_PATH_ENV, exit_ack_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn run_leaf_helper(ready_path: &Path) {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .expect("leaf helper should bind a loopback listener");
    let port = listener
        .local_addr()
        .expect("leaf helper should expose its address")
        .port();
    fs::write(ready_path, port.to_string()).expect("leaf helper should publish its port");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn wait_for_ready_port(path: &Path) -> u16 {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(value) = fs::read_to_string(path)
            && let Ok(port) = value.trim().parse::<u16>()
        {
            return port;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("process-tree helper did not publish a port at {}", path.display());
}

fn wait_for_file(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.is_file() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for {}", path.display());
}

fn assert_port_reachable(port: u16) {
    TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        Duration::from_millis(500),
    )
    .unwrap_or_else(|err| panic!("nested helper port {port} should be reachable: {err}"));
}

fn assert_port_closes(port: u16) {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_err() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("nested helper port {port} remained reachable after job closure");
}

fn unique_test_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should follow the Unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "golden-host-desktop-{label}-{}-{nonce}.txt",
        std::process::id()
    ))
}

fn remove_test_file(path: &Path) {
    if let Err(err) = fs::remove_file(path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        panic!("failed to remove test file {}: {err}", path.display());
    }
}

struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child }
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> std::process::ExitStatus {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait().expect("owner helper should be waitable") {
                return status;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("owner helper did not exit before timeout");
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
