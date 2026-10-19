use super::transport::{HttpFailure, HttpResponse};
use crate::app::module::common::received_values::service::ReceivedValueCursor;

pub(super) enum PendingHttpEvent {
    Response(HttpResponse, ReceivedValueCursor),
    Failure(HttpFailure),
    Callback(crate::app::module::script_api::CallbackDelivery),
}
