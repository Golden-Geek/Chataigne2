//! Engine-side latest-only inbox; native producers are absent from non-native builds.
use golden_core::{
    edit::Edit,
    engine::NodeWaker,
    node::{NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEnumOptions},
    process_ctx::ProcessCtx,
};
use std::sync::{Arc, Mutex};

#[cfg(any(test, all(windows, feature = "native-video")))]
mod producer;
#[cfg(any(test, all(windows, feature = "native-video")))]
pub(crate) use producer::Source;
#[cfg(test)]
pub(super) use producer::choices;

#[derive(Default)]
struct State {
    waker: Option<NodeWaker>,
    pending: Option<Update>,
    closed: bool,
}
#[derive(Default)]
pub(super) struct Inbox(Arc<Mutex<State>>);
impl Inbox {
    pub fn register(&self, ctx: &ProcessCtx) -> Result<(), String> {
        self.0.lock().map_err(|_| "GPU catalog inbox poisoned")?.waker =
            Some(ctx.node_waker().map_err(|error| error.to_string())?);
        Ok(())
    }
    pub fn take(&self) -> Option<Update> {
        self.0.lock().ok()?.pending.take()
    }
}
impl Drop for Inbox {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lock() {
            state.closed = true;
            state.pending.take();
        }
    }
}
pub(super) struct Update {
    node: NodeId,
    uuid: NodeUuid,
    value: ParamValue,
    options: ParameterEnumOptions,
}
impl Update {
    pub fn apply(self, ctx: &mut ProcessCtx) {
        ctx.edits.push(Edit::SetParamEnumOptions {
            node: self.node,
            expected_uuid: self.uuid,
            expected_value: self.value,
            selected_value: None,
            options: self.options,
        });
    }
}
