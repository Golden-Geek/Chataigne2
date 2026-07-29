use golden_audio::{
    AudioDeviceReadiness, AudioDeviceSelection, AudioDirection, AudioStreamStatus, BackendId,
};
use golden_core::{node::Node, process_ctx::ProcessCtx};

use super::{NO_AUDIO_DEVICE, SYSTEM_DEFAULT_DEVICE, SoundCardModule};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConnectionLogLevel {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ConnectionLog {
    pub level: ConnectionLogLevel,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ConnectionWarning {
    Keep,
    Clear,
    Set { message: String, detail: String },
}

pub(super) fn log_for_status(status: &AudioStreamStatus) -> Option<ConnectionLog> {
    let direction = direction_label(status.direction);
    let device = device_label(status);
    match status.readiness {
        AudioDeviceReadiness::Discovering | AudioDeviceReadiness::Recovering => Some(ConnectionLog {
            level: ConnectionLogLevel::Info,
            message: format!("Connecting to {direction} audio device [{device}]..."),
        }),
        AudioDeviceReadiness::Ready => Some(ConnectionLog {
            level: ConnectionLogLevel::Success,
            message: format!("Connected to {direction} audio device [{device}]."),
        }),
        readiness if is_connection_failure(readiness) => {
            let detail = failure_detail(status);
            Some(ConnectionLog {
                level: ConnectionLogLevel::Error,
                message: format!("Could not connect to {direction} audio device [{device}]: {detail}"),
            })
        }
        _ => None,
    }
}

pub(super) fn warning_for_status(status: &AudioStreamStatus) -> ConnectionWarning {
    if matches!(
        status.readiness,
        AudioDeviceReadiness::Ready | AudioDeviceReadiness::Disabled
    ) {
        return ConnectionWarning::Clear;
    }
    if !is_connection_failure(status.readiness) {
        return ConnectionWarning::Keep;
    }

    let direction = direction_label(status.direction);
    let device = device_label(status);
    ConnectionWarning::Set {
        message: format!("Could not connect to {direction} audio device [{device}]"),
        detail: failure_detail(status),
    }
}

pub(super) const fn warning_id(direction: AudioDirection) -> &'static str {
    match direction {
        AudioDirection::Input => "sound-card-input-device-connection",
        AudioDirection::Output => "sound-card-output-device-connection",
    }
}

pub(super) fn status_matches_selection(
    status: &AudioStreamStatus,
    selected_value: &str,
    driver: Option<&BackendId>,
) -> bool {
    let Some(driver) = driver else {
        return false;
    };
    let selected = if selected_value == NO_AUDIO_DEVICE {
        return false;
    } else if selected_value == SYSTEM_DEFAULT_DEVICE {
        AudioDeviceSelection::follow_system_default(driver.clone(), status.direction)
    } else {
        let Ok(selected) = serde_json::from_str::<AudioDeviceSelection>(selected_value) else {
            return false;
        };
        selected
    };
    if selected.target.backend() != driver {
        return false;
    }

    status.selected_target.as_ref() == Some(&selected.target)
        || status.profile_key.as_ref() == Some(&selected.profile_key)
}

impl SoundCardModule {
    pub(super) fn apply_device_connection_feedback(
        &self,
        ctx: &mut ProcessCtx,
        status: &AudioStreamStatus,
    ) {
        let driver = BackendId::new(self.audio_driver.get_ref().0.as_str()).ok();
        let (input_device, output_device) = self.selected_device_values();
        let selected_value = match status.direction {
            AudioDirection::Input => input_device,
            AudioDirection::Output => output_device,
        };
        if !status_matches_selection(status, selected_value, driver.as_ref()) {
            return;
        }

        if let Some(log) = log_for_status(status) {
            match log.level {
                ConnectionLogLevel::Info => {
                    golden_core::log!(origin = self.id(); log.message);
                }
                ConnectionLogLevel::Success => {
                    golden_core::logsuccess!(origin = self.id(); log.message);
                }
                ConnectionLogLevel::Error => {
                    golden_core::logerror!(origin = self.id(); log.message);
                }
            }
        }

        let parameter = self.device_parameter(status.direction);
        let warning_id = warning_id(status.direction);
        match warning_for_status(status) {
            ConnectionWarning::Keep => {}
            ConnectionWarning::Clear => ctx.clear_node_warning(parameter, Some(warning_id)),
            ConnectionWarning::Set { message, detail } => {
                ctx.set_node_warning_with(parameter, Some(warning_id), message, Some(detail.as_str()));
            }
        }
    }

    pub(super) fn clear_device_connection_warning(
        &self,
        ctx: &mut ProcessCtx,
        direction: AudioDirection,
    ) {
        ctx.clear_node_warning(
            self.device_parameter(direction),
            Some(warning_id(direction)),
        );
    }

    pub(super) fn clear_device_connection_warnings(&self, ctx: &mut ProcessCtx) {
        for direction in [AudioDirection::Input, AudioDirection::Output] {
            self.clear_device_connection_warning(ctx, direction);
        }
    }

    fn device_parameter(&self, direction: AudioDirection) -> golden_core::node::NodeId {
        match (self.uses_duplex_device(), direction) {
            (true, _) => self.device.id(),
            (false, AudioDirection::Input) => self.input_device.id(),
            (false, AudioDirection::Output) => self.output_device.id(),
        }
    }
}

const fn direction_label(direction: AudioDirection) -> &'static str {
    match direction {
        AudioDirection::Input => "input",
        AudioDirection::Output => "output",
    }
}

fn device_label(status: &AudioStreamStatus) -> &str {
    status.selected_label.as_deref().unwrap_or("selected device")
}

const fn is_connection_failure(readiness: AudioDeviceReadiness) -> bool {
    matches!(
        readiness,
        AudioDeviceReadiness::Missing
            | AudioDeviceReadiness::Unavailable
            | AudioDeviceReadiness::Busy
            | AudioDeviceReadiness::PermissionDenied
            | AudioDeviceReadiness::Failed
    )
}

fn failure_detail(status: &AudioStreamStatus) -> String {
    if let Some(error) = &status.error {
        return match &error.technical_detail {
            Some(technical_detail) => format!("{}\n{technical_detail}", error.message),
            None => error.message.clone(),
        };
    }

    match status.readiness {
        AudioDeviceReadiness::Missing => "The selected device is unavailable or disconnected.".to_owned(),
        AudioDeviceReadiness::Unavailable => "The audio backend is unavailable.".to_owned(),
        AudioDeviceReadiness::Busy => "The device is busy or already in use.".to_owned(),
        AudioDeviceReadiness::PermissionDenied => "Permission to use the device was denied.".to_owned(),
        AudioDeviceReadiness::Failed => "The audio stream could not be opened.".to_owned(),
        _ => "The audio device connection failed.".to_owned(),
    }
}
