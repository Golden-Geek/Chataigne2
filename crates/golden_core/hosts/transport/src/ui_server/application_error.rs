use super::*;
use golden_engine::application::ControlErrorKind;
use golden_protocol::{UiControlErrorCode, UiErrorResponse};

pub(super) fn application_error_code(error: &ApplicationError) -> Option<UiControlErrorCode> {
    error.control_error().map(|error| match error.kind() {
        ControlErrorKind::Overloaded => UiControlErrorCode::Overloaded,
        ControlErrorKind::Disconnected => UiControlErrorCode::Disconnected,
        ControlErrorKind::Cancelled => UiControlErrorCode::Cancelled,
    })
}

pub(super) fn write_application_error(stream: &mut HttpResponse<'_>, error: &ApplicationError) -> std::io::Result<()> {
    let code = application_error_code(error);
    let status = if code.is_some() {
        "503 Service Unavailable"
    } else {
        "400 Bad Request"
    };
    write_json(
        stream,
        status,
        &UiErrorResponse {
            error: error.to_string(),
            code,
        },
    )
}

pub(super) fn send_application_error(
    clients: &mut HashMap<u64, WsClientState>,
    client: u64,
    request: String,
    error: &ApplicationError,
) {
    send_to_client(
        clients,
        client,
        WsServerMessage::Error {
            code: application_error_code(error),
            message: error.to_string(),
            request_id: Some(request),
        },
    );
}
