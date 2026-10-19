use super::transport::{HttpResultRejection, PreparedHttpFailure, PreparedHttpResponse};
use crate::app::module::common::received_values::service::ReceivedValueCursor;
use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus, EditQueue},
    events::CustomEvent,
    node::{NodeId, NodeWarning},
    process_ctx::ProcessCtx,
};

pub(super) enum PendingHttpEvent {
    Response(PreparedHttpResponse, ReceivedValueCursor),
    Failure(PreparedHttpFailure),
    Rejected(RejectionDelivery),
    Callback(HttpCallbackDelivery),
}

pub(super) struct RejectionDelivery {
    warning: WarningDelivery,
    logged: bool,
}

pub(super) struct HttpCallbackDelivery {
    callback: crate::app::module::script_api::CallbackDelivery,
    clear_warning: Option<WarningDelivery>,
}

struct WarningDelivery {
    change: WarningChange,
    completion: Option<EditCompletion>,
}

enum WarningChange {
    Set(NodeWarning),
    Clear,
}

impl HttpCallbackDelivery {
    pub(super) fn new(event: CustomEvent, clear_warning: bool) -> Self {
        Self {
            callback: crate::app::module::script_api::CallbackDelivery::new(event),
            clear_warning: clear_warning.then_some(WarningDelivery {
                change: WarningChange::Clear,
                completion: None,
            }),
        }
    }

    pub(super) fn service(&mut self, ctx: &mut ProcessCtx, module: NodeId) -> bool {
        if let Some(clear) = &mut self.clear_warning {
            if !clear.service(ctx, module) {
                return false;
            }
            self.clear_warning = None;
        }
        self.callback.service(ctx)
    }
}

impl From<HttpResultRejection> for RejectionDelivery {
    fn from(rejection: HttpResultRejection) -> Self {
        let message = format!(
            concat!(
                "HTTP result for {} (attempts {}, command {:?}) rejected before ",
                "value/callback delivery: {}. Native requests are not replayed."
            ),
            rejection.description, rejection.attempts, rejection.command_id, rejection.error,
        );
        Self {
            warning: WarningDelivery {
                change: WarningChange::Set(NodeWarning {
                    id: super::HTTP_RESULT_WARNING_ID.into(),
                    message,
                    detail: None,
                }),
                completion: None,
            },
            logged: false,
        }
    }
}

impl RejectionDelivery {
    pub(super) fn service(&mut self, ctx: &mut ProcessCtx, module: NodeId) -> bool {
        if !self.logged {
            if let WarningChange::Set(warning) = &self.warning.change {
                golden_core::logerror!(warning.message.as_str());
            }
            self.logged = true;
        }
        self.warning.service(ctx, module)
    }
}

impl WarningDelivery {
    fn service(&mut self, ctx: &mut ProcessCtx, module: NodeId) -> bool {
        if let Some(completion) = self.completion.take() {
            match completion.status() {
                EditCompletionStatus::Applied => return true,
                EditCompletionStatus::Pending => {
                    self.completion = Some(completion);
                    return false;
                }
                EditCompletionStatus::Failed => {}
            }
        }
        // Set and clear each retain their own receipt; neither may be lost behind
        // another callback's failed edits. Native work is never part of this retry.
        let mut delivery = EditQueue::new();
        delivery.push(match &self.change {
            WarningChange::Set(warning) => Edit::SetNodeWarning {
                node: module,
                warning: warning.clone(),
            },
            WarningChange::Clear => Edit::ClearNodeWarning {
                node: module,
                warning_id: Some(super::HTTP_RESULT_WARNING_ID.into()),
            },
        });
        self.completion = Some(delivery.completion());
        ctx.edits.pending.extend(delivery.drain());
        false
    }
}
