use std::{net::UdpSocket, process::{Command, Stdio}};

use sysinfo::{get_current_pid, Networks, Pid, ProcessesToUpdate, System};

use crate::app::module::common::os::WakeOnLanRequest;
use crate::app::module::common::system_metrics;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostControlAction {
    Shutdown,
    Reboot,
    Logout,
}

impl HostControlAction {
    pub(crate) fn as_human_label(self) -> &'static str {
        match self {
            Self::Shutdown => "shutdown",
            Self::Reboot => "reboot",
            Self::Logout => "logout",
        }
    }

    pub(crate) fn as_script_method(self) -> &'static str {
        self.as_human_label()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OsMetricsSnapshot {
    pub(crate) os_version: String,
    pub(crate) global_cpu_ratio: f64,
    pub(crate) app_cpu_ratio: f64,
    pub(crate) system_used_mb: f64,
    pub(crate) system_total_mb: f64,
    pub(crate) app_used_mb: f64,
    pub(crate) app_virtual_mb: f64,
    pub(crate) received_mb_per_sec: f64,
    pub(crate) transmitted_mb_per_sec: f64,
    pub(crate) total_received_mb: f64,
    pub(crate) total_transmitted_mb: f64,
    pub(crate) system_uptime_seconds: f64,
    pub(crate) app_uptime_seconds: f64,
}

pub(crate) struct OsRuntime {
    system: System,
    networks: Networks,
    process_id: Pid,
}

impl OsRuntime {
    pub(crate) fn create() -> Self {
        let process_id = resolve_current_pid();
        let mut system = System::new_all();
        system.refresh_cpu_usage();
        system.refresh_memory();
        system.refresh_processes(ProcessesToUpdate::Some(&[process_id]), true);

        Self {
            system,
            networks: Networks::new_with_refreshed_list(),
            process_id,
        }
    }

    pub(crate) fn refresh(&mut self) -> OsMetricsSnapshot {
        self.system.refresh_memory();
        self.system.refresh_cpu_usage();
        self.system
            .refresh_processes(ProcessesToUpdate::Some(&[self.process_id]), true);
        self.networks.refresh(true);

        let process = self.system.process(self.process_id);
        let cpu_count = self.system.cpus().len();
        let (received_bytes_per_sec, transmitted_bytes_per_sec, total_received_bytes, total_transmitted_bytes) =
            network_counters(&self.networks);

        OsMetricsSnapshot {
            os_version: current_os_version(),
            global_cpu_ratio: system_metrics::percent_to_ratio(self.system.global_cpu_usage() as f64),
            app_cpu_ratio: process
                .map(|process| {
                    system_metrics::process_cpu_percent_to_ratio(process.cpu_usage() as f64, cpu_count)
                })
                .unwrap_or(0.0),
            system_used_mb: system_metrics::bytes_to_mb(self.system.used_memory()),
            system_total_mb: system_metrics::bytes_to_mb(self.system.total_memory()),
            app_used_mb: process
                .map(|process| system_metrics::bytes_to_mb(process.memory()))
                .unwrap_or(0.0),
            app_virtual_mb: process
                .map(|process| system_metrics::bytes_to_mb(process.virtual_memory()))
                .unwrap_or(0.0),
            received_mb_per_sec: system_metrics::bytes_f64_to_mb(received_bytes_per_sec),
            transmitted_mb_per_sec: system_metrics::bytes_f64_to_mb(transmitted_bytes_per_sec),
            total_received_mb: system_metrics::bytes_f64_to_mb(total_received_bytes),
            total_transmitted_mb: system_metrics::bytes_f64_to_mb(total_transmitted_bytes),
            system_uptime_seconds: System::uptime() as f64,
            app_uptime_seconds: process
                .map(|process| system_metrics::uptime_seconds_from_unix_start(process.start_time()))
                .unwrap_or(0.0),
        }
    }
}

pub(crate) fn host_os_type() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "Mac"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        std::env::consts::OS
    }
}

pub(crate) fn host_architecture() -> String {
    let raw = System::cpu_arch();
    match raw.trim().to_ascii_lowercase().as_str() {
        "x86_64" | "amd64" => "64-bit (x86_64)".to_string(),
        "x86" | "i386" | "i586" | "i686" => "32-bit (x86)".to_string(),
        "aarch64" => "arm64".to_string(),
        "arm" | "armv7" | "armv7l" => "arm-v7".to_string(),
        other if !other.is_empty() => other.to_string(),
        _ => std::env::consts::ARCH.to_string(),
    }
}

pub(crate) fn current_os_version() -> String {
    System::long_os_version()
        .or_else(System::os_version)
        .or_else(System::name)
        .unwrap_or_else(|| "Unknown".to_string())
}

pub(crate) fn execute_control_action(action: HostControlAction) -> Result<(), String> {
    let candidates = control_command_candidates(action);
    if candidates.is_empty() {
        return Err(format!(
            "no {} command is configured for this platform",
            action.as_human_label()
        ));
    }

    let mut errors = Vec::new();
    for candidate in candidates {
        let mut command = Command::new(candidate.program.as_str());
        command
            .args(candidate.args.iter().map(String::as_str))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(_) => return Ok(()),
            Err(error) => errors.push(format!("{}: {error}", candidate.display())),
        }
    }

    Err(format!(
        "failed to {} using any known host command: {}",
        action.as_human_label(),
        errors.join("; "),
    ))
}

pub(crate) fn send_wake_on_lan(request: &WakeOnLanRequest) -> Result<(), String> {
    let mac = parse_mac_address(request.mac_address.as_str())?;
    let broadcast_host = request.broadcast_host.trim();
    if broadcast_host.is_empty() {
        return Err("Wake-on-LAN broadcast host cannot be empty".to_string());
    }

    let packet = build_magic_packet(mac);
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|error| format!("failed to bind Wake-on-LAN socket: {error}"))?;
    socket
        .set_broadcast(true)
        .map_err(|error| format!("failed to enable UDP broadcast: {error}"))?;

    let target = format!("{broadcast_host}:{}", request.port);
    let sent = socket
        .send_to(packet.as_slice(), target.as_str())
        .map_err(|error| format!("failed to send Wake-on-LAN packet to {target}: {error}"))?;
    if sent != packet.len() {
        return Err(format!(
            "incomplete Wake-on-LAN packet send: sent {sent} of {} byte(s)",
            packet.len()
        ));
    }

    Ok(())
}

pub(crate) fn parse_mac_address(input: &str) -> Result<[u8; 6], String> {
    let normalized = input.trim();
    let hex: String = normalized.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 12 {
        return Err(format!(
            "invalid Wake-on-LAN MAC address '{normalized}': expected 12 hexadecimal digits"
        ));
    }

    let mut mac = [0u8; 6];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(chunk).map_err(|error| {
            format!("invalid Wake-on-LAN MAC address '{normalized}': {error}")
        })?;
        mac[index] = u8::from_str_radix(pair, 16).map_err(|error| {
            format!("invalid Wake-on-LAN MAC address '{normalized}': {error}")
        })?;
    }

    Ok(mac)
}

pub(crate) fn build_magic_packet(mac: [u8; 6]) -> [u8; 102] {
    let mut packet = [0u8; 102];
    packet[..6].fill(0xFF);
    for chunk in packet[6..].chunks_mut(6) {
        chunk.copy_from_slice(&mac);
    }
    packet
}

fn resolve_current_pid() -> Pid {
    get_current_pid().unwrap_or_else(|_| Pid::from_u32(std::process::id()))
}

fn network_counters(networks: &Networks) -> (f64, f64, f64, f64) {
    let mut received_bytes_per_sec = 0.0;
    let mut transmitted_bytes_per_sec = 0.0;
    let mut total_received_bytes = 0.0;
    let mut total_transmitted_bytes = 0.0;

    for (_interface, data) in networks {
        received_bytes_per_sec += data.received() as f64;
        transmitted_bytes_per_sec += data.transmitted() as f64;
        total_received_bytes += data.total_received() as f64;
        total_transmitted_bytes += data.total_transmitted() as f64;
    }

    (
        received_bytes_per_sec,
        transmitted_bytes_per_sec,
        total_received_bytes,
        total_transmitted_bytes,
    )
}


#[derive(Clone, Debug, Eq, PartialEq)]
struct HostCommandCandidate {
    program: String,
    args: Vec<String>,
}

impl HostCommandCandidate {
    fn new(program: &str, args: Vec<String>) -> Self {
        Self {
            program: program.to_string(),
            args,
        }
    }

    fn display(&self) -> String {
        if self.args.is_empty() {
            return self.program.clone();
        }

        format!("{} {}", self.program, self.args.join(" "))
    }
}

fn control_command_candidates(action: HostControlAction) -> Vec<HostCommandCandidate> {
    #[cfg(windows)]
    {
        return match action {
            HostControlAction::Shutdown => vec![candidate("shutdown", &["/s", "/t", "0"])],
            HostControlAction::Reboot => vec![candidate("shutdown", &["/r", "/t", "0"])],
            HostControlAction::Logout => vec![candidate("shutdown", &["/l"])],
        };
    }

    #[cfg(target_os = "macos")]
    {
        return match action {
            HostControlAction::Shutdown => {
                vec![candidate("osascript", &["-e", "tell application \"System Events\" to shut down"])]
            }
            HostControlAction::Reboot => {
                vec![candidate("osascript", &["-e", "tell application \"System Events\" to restart"])]
            }
            HostControlAction::Logout => {
                vec![candidate("osascript", &["-e", "tell application \"System Events\" to log out"])]
            }
        };
    }

    #[cfg(target_os = "linux")]
    {
        return match action {
            HostControlAction::Shutdown => vec![
                candidate("systemctl", &["poweroff"]),
                candidate("shutdown", &["-h", "now"]),
            ],
            HostControlAction::Reboot => vec![
                candidate("systemctl", &["reboot"]),
                candidate("shutdown", &["-r", "now"]),
            ],
            HostControlAction::Logout => linux_logout_candidates(),
        };
    }

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = action;
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn linux_logout_candidates() -> Vec<HostCommandCandidate> {
    let mut candidates = Vec::new();

    if let Some(session_id) = env_var_non_empty("XDG_SESSION_ID") {
        candidates.push(HostCommandCandidate::new(
            "loginctl",
            vec!["terminate-session".to_string(), session_id],
        ));
    }

    if let Some(user) = env_var_non_empty("USER") {
        candidates.push(HostCommandCandidate::new(
            "loginctl",
            vec!["terminate-user".to_string(), user],
        ));
    }

    candidates.push(candidate("gnome-session-quit", &["--logout", "--no-prompt"]));
    candidates.push(candidate(
        "qdbus",
        &["org.kde.Shutdown", "/Shutdown", "logout"],
    ));

    candidates
}

#[cfg(target_os = "linux")]
fn env_var_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn candidate(program: &str, args: &[&str]) -> HostCommandCandidate {
    HostCommandCandidate::new(program, args.iter().map(|arg| (*arg).to_string()).collect())
}