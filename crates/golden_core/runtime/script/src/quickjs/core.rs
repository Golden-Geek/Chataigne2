use super::*;

impl QuickJsRuntime {
    /// Creates a new QuickJS runtime with budget guardrails.
    pub fn new(budgets: ScriptBudgets) -> Result<Self, ScriptRuntimeError> {
        let runtime = QuickJsRuntimeHandle::new()?;
        runtime.set_memory_limit(budgets.max_memory_bytes);
        runtime.set_max_stack_size(SCRIPT_MAX_STACK_BYTES);
        let interrupt_state = Arc::new(ScriptInterruptState::new());
        let handler_state = Arc::clone(&interrupt_state);
        runtime.set_interrupt_handler(Some(Box::new(move || handler_state.should_interrupt())));
        let context = QuickJsContext::full(&runtime)?;
        let host_ops = Arc::new(Mutex::new(Vec::new()));
        let host_call_counter = Arc::new(AtomicU32::new(0));
        let tree_bridge_state = Arc::new(Mutex::new(QuickJsTreeBridgeState::default()));

        let runtime = Self {
            runtime,
            context,
            budgets,
            entrypoints: QuickJsEntrypoints::default(),
            manifest: None,
            host_ops,
            host_call_counter,
            tree_bridge_state,
            interrupt_state,
        };
        runtime.load_timed("host API bootstrap", || runtime.install_host_api())?;
        Ok(runtime)
    }

    /// Returns a handle that can cancel an entry from a watchdog or owning runtime.
    pub fn cancellation_handle(&self) -> ScriptCancellationHandle {
        ScriptCancellationHandle {
            state: Arc::clone(&self.interrupt_state),
        }
    }

    pub(super) fn validate_host_input_size(value: &str, label: &str, max_bytes: usize) -> Result<(), QuickJsError> {
        if value.len() <= max_bytes {
            return Ok(());
        }
        Err(QuickJsError::new_from_js_message(
            "script",
            "host",
            format!("script host {label} exceeds {max_bytes} bytes"),
        ))
    }
}
