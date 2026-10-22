use super::*;
use crate::app::module::common::system_metrics;
#[cfg(not(test))]
use std::net::UdpSocket;
#[cfg(not(test))]
mod controls;
use sysinfo::{get_current_pid, Networks, Pid, ProcessesToUpdate, System};

pub(super) struct Native {
    system: System,
    networks: Networks,
    pid: Pid,
}
impl Native {
    pub(super) fn open() -> Result<Self, String> {
        Ok(Self {
            system: System::new(),
            networks: Networks::new_with_refreshed_list(),
            pid: resolve_current_pid(),
        })
    }
}
impl Provider for Native {
    fn sample(&mut self) -> Result<OsMetricsSnapshot, String> {
        let snapshot = collect_metrics_snapshot(&mut self.system, &mut self.networks, self.pid);
        if self.system.cpus().len() > 4096 || self.networks.len() > 256 {
            return Err("OS metrics exceeds 4096 CPUs / 256 network interfaces".into());
        }
        Ok(snapshot)
    }
    fn execute(&mut self, request: &Request) -> Result<(), String> {
        #[cfg(not(test))]
        {
            match request {
                Request::Control(action) => controls::execute_control_action(*action),
                Request::WakeOnLan(request) => send_wake_on_lan(request),
            }
        }
        #[cfg(test)]
        {
            let _ = request;
            Err("Native host-control execution is disabled in tests; use a fake provider".into())
        }
    }
}

fn collect_metrics_snapshot(system: &mut System, networks: &mut Networks, process_id: Pid) -> OsMetricsSnapshot {
    system.refresh_memory();
    system.refresh_cpu_usage();
    system.refresh_processes(ProcessesToUpdate::Some(&[process_id]), true);
    networks.refresh(true);

    let process = system.process(process_id);
    let cpu_count = system.cpus().len();
    let (received_bytes_per_sec, transmitted_bytes_per_sec, total_received_bytes, total_transmitted_bytes) =
        network_counters(networks);

    OsMetricsSnapshot {
        architecture: architecture_label(&System::cpu_arch()),
        os_version: current_os_version(),
        global_cpu_ratio: system_metrics::percent_to_ratio(system.global_cpu_usage() as f64),
        app_cpu_ratio: process
            .map(|process| system_metrics::process_cpu_percent_to_ratio(process.cpu_usage() as f64, cpu_count))
            .unwrap_or(0.0),
        system_used_mb: system_metrics::bytes_to_mb(system.used_memory()),
        system_total_mb: system_metrics::bytes_to_mb(system.total_memory()),
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
    architecture_label(std::env::consts::ARCH)
}
fn architecture_label(raw: &str) -> String {
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

#[cfg(not(test))]
fn send_wake_on_lan(request: &WakeOnLanRequest) -> Result<(), String> {
    let mac = parse_mac_address(request.mac_address.as_str())?;
    let broadcast_host = request.broadcast_host.trim();
    if broadcast_host.is_empty() {
        return Err("Wake-on-LAN broadcast host cannot be empty".to_string());
    }

    let packet = build_magic_packet(mac);
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| format!("failed to bind Wake-on-LAN socket: {error}"))?;
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
    if input.len() > 64 {
        return Err("Wake-on-LAN MAC exceeds 64 bytes".into());
    }
    let normalized = input.trim();
    let hex: String = normalized.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 12 {
        return Err(format!(
            "invalid Wake-on-LAN MAC address '{normalized}': expected 12 hexadecimal digits"
        ));
    }

    let mut mac = [0u8; 6];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(chunk)
            .map_err(|error| format!("invalid Wake-on-LAN MAC address '{normalized}': {error}"))?;
        mac[index] = u8::from_str_radix(pair, 16)
            .map_err(|error| format!("invalid Wake-on-LAN MAC address '{normalized}': {error}"))?;
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
