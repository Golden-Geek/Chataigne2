use super::*;
use std::process::{Command, Stdio};

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
            Ok(mut child) => {
                // Keep native process ownership until reaping. Do not try a
                // second command after a started process has an uncertain result.
                return match child.wait() {
                    Ok(status) if status.success() => Ok(()),
                    Ok(status) => Err(format!("{} exited with {status}", candidate.display())),
                    Err(error) => Err(format!(
                        "{} process completion is unconfirmed: {error}",
                        candidate.display()
                    )),
                };
            }
            Err(error) => errors.push(format!("{}: {error}", candidate.display())),
        }
    }

    Err(format!(
        "failed to {} using any known host command: {}",
        action.as_human_label(),
        errors.join("; "),
    ))
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
        match action {
            HostControlAction::Shutdown => vec![candidate("shutdown", &["/s", "/t", "0"])],
            HostControlAction::Reboot => vec![candidate("shutdown", &["/r", "/t", "0"])],
            HostControlAction::Logout => vec![candidate("shutdown", &["/l"])],
        }
    }

    #[cfg(target_os = "macos")]
    {
        match action {
            HostControlAction::Shutdown => {
                vec![candidate(
                    "osascript",
                    &["-e", "tell application \"System Events\" to shut down"],
                )]
            }
            HostControlAction::Reboot => {
                vec![candidate(
                    "osascript",
                    &["-e", "tell application \"System Events\" to restart"],
                )]
            }
            HostControlAction::Logout => {
                vec![candidate(
                    "osascript",
                    &["-e", "tell application \"System Events\" to log out"],
                )]
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        match action {
            HostControlAction::Shutdown => vec![
                candidate("systemctl", &["poweroff"]),
                candidate("shutdown", &["-h", "now"]),
            ],
            HostControlAction::Reboot => vec![
                candidate("systemctl", &["reboot"]),
                candidate("shutdown", &["-r", "now"]),
            ],
            HostControlAction::Logout => linux_logout_candidates(),
        }
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
    candidates.push(candidate("qdbus", &["org.kde.Shutdown", "/Shutdown", "logout"]));

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
