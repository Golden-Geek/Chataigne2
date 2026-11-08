use super::*;

type BindingCallback = dyn FnMut(&dyn Node) -> Result<bool, String> + Send;
pub(crate) struct RuntimeBinding {
    pub source: NodeId,
    pub callback: Box<BindingCallback>,
}
impl ProcessCtx {
    /// Register a shared runtime handle with an owning ancestor, without authored edits or a
    /// tree snapshot. The callback receives read-only nodes, nearest ancestor first, and returns
    /// true once bound. It must perform no IO or waiting. Registrations settle before ready
    /// callbacks return to the host, including live project activation and history restoration.
    pub fn register_ancestor_runtime(
        &mut self,
        source: NodeId,
        callback: impl FnMut(&dyn Node) -> Result<bool, String> + Send + 'static,
    ) {
        self.runtime_bindings.push(RuntimeBinding {
            source,
            callback: Box::new(callback),
        });
    }
    pub(crate) fn take_runtime_bindings(&mut self) -> Vec<RuntimeBinding> {
        std::mem::take(&mut self.runtime_bindings)
    }
}
