use super::*;

impl QuickJsRuntime {
    pub(super) fn install_app_operations(&self, table: &QuickJsObject<'_>) -> Result<(), QuickJsError> {
        let host_ops = self.host_ops.clone();
        let counter = self.host_call_counter.clone();
        let maximum_calls = self.budgets.max_host_calls_per_callback.max(1);
        let invoke = QuickJsFunc::from(QuickJsMutFn::from(
            move |request_id: String,
                  namespace: String,
                  operation: String,
                  payload: String|
                  -> Result<bool, QuickJsError> {
                if counter.fetch_add(1, Ordering::Relaxed) >= maximum_calls {
                    return Err(QuickJsError::new_from_js_message(
                        "script",
                        "host",
                        SCRIPT_HOST_CALL_BUDGET_MESSAGE,
                    ));
                }
                QuickJsRuntime::validate_host_input_size(&payload, "app-operation JSON", SCRIPT_APP_OPERATION_BYTES)?;
                let error = |message: String| QuickJsError::new_from_js_message("script", "appOperation", message);
                let reservation =
                    ScriptAppOperationRequest::reserve(&request_id, &namespace, &operation).map_err(error)?;
                let payload = super::bounded_json::parse_with_limits(
                    &payload,
                    SCRIPT_APP_OPERATION_VALUES,
                    SCRIPT_APP_OPERATION_DEPTH,
                )
                .map_err(|failure| error(failure.to_string()))?;
                let request = reservation
                    .finish(payload)
                    .map_err(|failure| error(failure.to_string()))?;
                QuickJsRuntime::enqueue_host_op(&host_ops, ScriptHostOp::InvokeAppOperation { request })?;
                Ok(true)
            },
        ));
        table.set("__app_operation_raw", invoke)
    }
}
