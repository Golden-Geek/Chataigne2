use super::input::PreparedUdpInput;
use crate::app::{module::script_api::CallbackDelivery, StreamingModuleBase};
use golden_core::process_ctx::ProcessCtx;
use golden_io::RetainedPayload;

pub(super) struct InputDelivery {
    input: RetainedPayload<PreparedUdpInput>,
    next: usize,
    pending: Option<CallbackDelivery>,
}

#[cfg(test)]
mod tests;

impl InputDelivery {
    pub(super) fn new(input: RetainedPayload<PreparedUdpInput>) -> Self {
        Self {
            input,
            next: 0,
            pending: None,
        }
    }

    pub(super) fn service(&mut self, ctx: &mut ProcessCtx, stream: &mut StreamingModuleBase) -> bool {
        while let Some(event) = self.input.callbacks.get(self.next) {
            let delivery = self.pending.get_or_insert_with(|| CallbackDelivery::new(event.clone()));
            if !delivery.service(ctx) {
                return false;
            }
            self.pending = None;
            self.next += 1;
        }
        if let Ok(values) = &self.input.values {
            stream.push_messages(values.clone());
        }
        true
    }
}
