use cpal::{Error, ErrorKind, HostId};

use crate::{AudioBackendState, AudioError, AudioErrorCategory, BackendId};

pub(super) fn map_cpal_error(error: Error, operation: &'static str, backend: Option<BackendId>) -> AudioError {
    let category = match error.kind() {
        ErrorKind::DeviceBusy => AudioErrorCategory::DeviceBusy,
        ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated => AudioErrorCategory::DeviceMissing,
        ErrorKind::HostUnavailable => AudioErrorCategory::BackendUnavailable,
        ErrorKind::PermissionDenied => AudioErrorCategory::PermissionDenied,
        ErrorKind::InvalidInput | ErrorKind::UnsupportedConfig | ErrorKind::UnsupportedOperation => {
            AudioErrorCategory::UnsupportedFormat
        }
        ErrorKind::ResourceExhausted => AudioErrorCategory::CapacityExceeded,
        ErrorKind::DeviceChanged
        | ErrorKind::RealtimeDenied
        | ErrorKind::Xrun
        | ErrorKind::BackendError
        | ErrorKind::Other => AudioErrorCategory::StreamNegotiationFailed,
        _ => AudioErrorCategory::StreamNegotiationFailed,
    };
    let mut mapped = AudioError::new(category, format!("{operation}: {error}"))
        .with_context("cpal_error_kind", format!("{:?}", error.kind()).to_lowercase());
    if let Some(backend) = backend {
        mapped = mapped.with_context("backend", backend.to_string());
    }
    mapped
}

pub(super) fn backend_state_for(host_id: HostId, error: &Error) -> (AudioBackendState, Option<String>) {
    let state = match error.kind() {
        ErrorKind::HostUnavailable if host_id.to_string() == "asio" => AudioBackendState::MissingDriver,
        ErrorKind::HostUnavailable if matches!(host_id.to_string().as_str(), "jack" | "pipewire") => {
            AudioBackendState::MissingServer
        }
        ErrorKind::HostUnavailable => AudioBackendState::Unavailable,
        _ => AudioBackendState::Failed,
    };
    (state, Some(error.to_string()))
}
