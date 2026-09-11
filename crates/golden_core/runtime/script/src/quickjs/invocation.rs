use super::*;

impl QuickJsRuntime {
    pub(super) fn reset_host_callback_state(&self) -> Result<(), ScriptRuntimeError> {
        self.host_call_counter.store(0, Ordering::Relaxed);
        let mut guard = self
            .host_ops
            .lock()
            .map_err(|_| ScriptRuntimeError::Host("script host-op queue lock poisoned".to_string()))?;
        guard.clear();
        let mut tree_guard = self
            .tree_bridge_state
            .lock()
            .map_err(|_| ScriptRuntimeError::Host("script tree bridge lock poisoned".to_string()))?;
        tree_guard.snapshot = None;
        tree_guard.host = None;
        tree_guard.script = None;
        tree_guard.time_seconds = 0.0;
        tree_guard.delta_seconds = 0.0;
        Ok(())
    }

    pub(super) fn sync_tree_bridge_state(&self, host: &dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let mut tree_guard = self
            .tree_bridge_state
            .lock()
            .map_err(|_| ScriptRuntimeError::Host("script tree bridge lock poisoned".to_string()))?;
        tree_guard.snapshot = host.tree_snapshot();
        tree_guard.host = host.owner_node();
        tree_guard.script = host.script_node();
        tree_guard.time_seconds = host.time_seconds();
        tree_guard.delta_seconds = host.delta_seconds();
        Ok(())
    }

    pub(super) fn flush_host_ops(&self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let mut drained = Vec::new();
        {
            let mut guard = self
                .host_ops
                .lock()
                .map_err(|_| ScriptRuntimeError::Host("script host-op queue lock poisoned".to_string()))?;
            std::mem::swap(&mut drained, &mut *guard);
        }

        for op in drained {
            match op {
                ScriptHostOp::Log { level, message } => {
                    host.log(level, &message);
                }
                ScriptHostOp::EmitCustom { topic, payload } => {
                    host.emit_custom(&topic, payload).map_err(ScriptRuntimeError::Host)?;
                }
                ScriptHostOp::SetNodeScriptProperty { node, property, value } => {
                    host.set_node_script_property(node, property, value)
                        .map_err(ScriptRuntimeError::Host)?;
                }
                ScriptHostOp::CallNodeScriptMethod { node, method, args } => {
                    host.call_node_script_method(node, method, args)
                        .map_err(ScriptRuntimeError::Host)?;
                }
                ScriptHostOp::SetEventListener { target, level } => {
                    host.set_event_listener(target, level)
                        .map_err(ScriptRuntimeError::Host)?;
                }
                ScriptHostOp::RemoveEventListener { target } => {
                    host.remove_event_listener(target).map_err(ScriptRuntimeError::Host)?;
                }
                ScriptHostOp::ClearEventListeners => {
                    host.clear_event_listeners().map_err(ScriptRuntimeError::Host)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn discard_host_ops(&self) -> Result<(), ScriptRuntimeError> {
        self.host_ops
            .lock()
            .map_err(|_| ScriptRuntimeError::Host("script host-op queue lock poisoned".to_string()))?
            .clear();
        Ok(())
    }

    pub(super) fn finish_host_invocation<T>(
        &self,
        result: Result<T, ScriptRuntimeError>,
        host: Option<&mut dyn ScriptHostBridge>,
    ) -> Result<T, ScriptRuntimeError> {
        let value = match result {
            Ok(value) => value,
            Err(error) => {
                self.discard_host_ops()?;
                self.interrupt_state.poisoned.store(true, Ordering::Release);
                return Err(error);
            }
        };

        let commit_result = match host {
            Some(host) => self.flush_host_ops(host),
            None => self.discard_host_ops(),
        };
        if commit_result.is_err() {
            self.interrupt_state.poisoned.store(true, Ordering::Release);
        }
        commit_result?;
        Ok(value)
    }

    pub(super) fn callback_timed<T, F>(&self, phase_label: &str, callback: F) -> Result<T, ScriptRuntimeError>
    where
        F: FnOnce() -> Result<T, ScriptRuntimeError>,
    {
        self.invocation_timed(
            phase_label,
            self.budgets.max_wall_time_us_per_callback,
            self.budgets.max_instructions_per_callback,
            callback,
        )
    }

    pub(super) fn load_timed<T, F>(&self, phase_label: &str, callback: F) -> Result<T, ScriptRuntimeError>
    where
        F: FnOnce() -> Result<T, ScriptRuntimeError>,
    {
        self.invocation_timed(
            phase_label,
            self.budgets.max_wall_time_us_per_load,
            self.budgets.max_instructions_per_load,
            callback,
        )
    }

    pub(super) fn invocation_timed<T, F>(
        &self,
        phase_label: &str,
        wall_time_us: u64,
        instruction_budget: u64,
        callback: F,
    ) -> Result<T, ScriptRuntimeError>
    where
        F: FnOnce() -> Result<T, ScriptRuntimeError>,
    {
        if self.interrupt_state.poisoned.load(Ordering::Acquire) {
            return Err(ScriptRuntimeError::QuickJs(
                "script runtime is quarantined after a failed invocation; reload is required".to_string(),
            ));
        }

        let elapsed_limit = Duration::from_micros(wall_time_us.max(1));
        let invocation_guard = self.interrupt_state.begin(elapsed_limit, instruction_budget);
        let started_at = Instant::now();
        let mut output = callback();
        if output.is_ok() && self.runtime.is_job_pending() {
            output = Err(ScriptRuntimeError::BudgetViolation(format!(
                "{phase_label} queued asynchronous jobs, which are unsupported by the synchronous script host"
            )));
        }
        let elapsed = started_at.elapsed();
        let interrupt_reason = self.interrupt_state.interrupt_reason();
        drop(invocation_guard);

        if interrupt_reason != ScriptInterruptReason::None {
            self.interrupt_state.poisoned.store(true, Ordering::Release);
            return Err(ScriptRuntimeError::BudgetViolation(format!(
                "{phase_label} {}",
                interrupt_reason.description()
            )));
        }
        if elapsed > elapsed_limit {
            self.interrupt_state.poisoned.store(true, Ordering::Release);
            return Err(ScriptRuntimeError::BudgetViolation(format!(
                "{phase_label} exceeded wall-time budget: {:?} > {:?}",
                elapsed, elapsed_limit
            )));
        }
        if output.is_err() {
            self.interrupt_state.poisoned.store(true, Ordering::Release);
        }
        output
    }

    pub(super) fn param_value_from_json(value: &JsonValue) -> Result<ParamValue, String> {
        ParamValue::from_script_json(value)
    }

    pub(super) fn param_value_from_parameter_spec_json(spec: &JsonValue) -> Result<ParamValue, String> {
        let Some(spec) = spec.as_object() else {
            return Self::param_value_from_json(spec);
        };

        let type_label = spec.get("type").and_then(JsonValue::as_str).unwrap_or("float");
        let value_type = ScriptValueType::from_manifest_label(type_label)
            .ok_or_else(|| format!("unsupported parameter type '{type_label}'"))?;
        match spec.get("default") {
            Some(raw_default) => {
                parameter_default_from_json_value(value_type, raw_default).map_err(|error| error.to_string())
            }
            None => Ok(default_param_value(value_type)),
        }
    }

    pub(super) fn method_args_from_json(method: &str, args_values: Vec<JsonValue>) -> Result<Vec<ParamValue>, String> {
        if method == "addParameter" {
            let mut args = Vec::with_capacity(args_values.len());
            if let Some(value) = args_values.first() {
                args.push(Self::param_value_from_json(value)?);
            }
            if let Some(value) = args_values.get(1) {
                let converted = if value.is_object() {
                    Self::param_value_from_parameter_spec_json(value)?
                } else {
                    Self::param_value_from_json(value)?
                };
                args.push(converted);
            }
            for value in args_values.iter().skip(2) {
                args.push(Self::param_value_from_json(value)?);
            }
            return Ok(args);
        }

        if method == "addNode" {
            let mut args = Vec::with_capacity(args_values.len());
            for (index, value) in args_values.iter().enumerate() {
                let converted = if index == 2 && value.is_object() {
                    Self::param_value_from_parameter_spec_json(value)?
                } else {
                    Self::param_value_from_json(value)?
                };
                args.push(converted);
            }
            return Ok(args);
        }

        let mut args = Vec::with_capacity(args_values.len());
        for value in args_values {
            args.push(Self::param_value_from_json(&value)?);
        }
        Ok(args)
    }

    pub(super) fn param_value_to_tree_json(value: &ParamValue) -> JsonValue {
        value.to_script_json()
    }

    pub(super) fn is_js_identifier_start(ch: char) -> bool {
        ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
    }

    pub(super) fn is_js_identifier_continue(ch: char) -> bool {
        Self::is_js_identifier_start(ch) || ch.is_ascii_digit()
    }

    pub(super) fn parse_exported_function_name(declaration: &str) -> Option<&str> {
        let trimmed = declaration.trim_start();
        let mut chars = trimmed.char_indices();
        let (_, first) = chars.next()?;
        if !Self::is_js_identifier_start(first) {
            return None;
        }

        let mut end = first.len_utf8();
        for (index, ch) in chars {
            if Self::is_js_identifier_continue(ch) {
                end = index + ch.len_utf8();
            } else {
                break;
            }
        }
        Some(&trimmed[..end])
    }

    pub(super) fn preprocess_source_for_exported_functions(source: &str) -> String {
        let mut transformed = String::new();
        let mut exported_names: Vec<String> = Vec::new();

        for segment in source.split_inclusive('\n') {
            let (line, line_break) = if let Some(stripped) = segment.strip_suffix('\n') {
                (stripped, "\n")
            } else {
                (segment, "")
            };

            let trimmed = line.trim_start();
            let indent_len = line.len().saturating_sub(trimmed.len());
            let indent = &line[..indent_len];

            if let Some(rest) = trimmed.strip_prefix("export function ")
                && let Some(name) = Self::parse_exported_function_name(rest)
            {
                if !exported_names.iter().any(|existing| existing == name) {
                    exported_names.push(name.to_string());
                }
                transformed.push_str(indent);
                transformed.push_str("function ");
                transformed.push_str(rest);
                transformed.push_str(line_break);
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("export async function ")
                && let Some(name) = Self::parse_exported_function_name(rest)
            {
                if !exported_names.iter().any(|existing| existing == name) {
                    exported_names.push(name.to_string());
                }
                transformed.push_str(indent);
                transformed.push_str("async function ");
                transformed.push_str(rest);
                transformed.push_str(line_break);
                continue;
            }

            transformed.push_str(segment);
        }

        if !exported_names.is_empty() {
            transformed.push('\n');
            transformed.push_str("// Auto-registered exported functions.\n");
            for name in exported_names {
                transformed.push_str("globalThis.__gc_script_exports[\"");
                transformed.push_str(&name);
                transformed.push_str("\"] = ");
                transformed.push_str(&name);
                transformed.push_str(";\n");
            }
        }

        transformed
    }

    pub(super) fn first_function_name<'js>(
        object: &QuickJsObject<'js>,
        candidates: &[&str],
    ) -> Result<Option<String>, QuickJsError> {
        for candidate in candidates {
            if object.get::<_, Option<QuickJsFunction>>(*candidate)?.is_some() {
                return Ok(Some((*candidate).to_string()));
            }
        }
        Ok(None)
    }

    pub(super) fn collect_export_names<'js>(object: &QuickJsObject<'js>) -> Result<Vec<String>, QuickJsError> {
        let mut exports = Vec::new();
        for key in object.keys::<String>() {
            let key = key?;
            if object.get::<_, Option<QuickJsFunction>>(key.as_str())?.is_some() {
                exports.push(key);
            }
        }
        exports.sort();
        exports.dedup();
        Ok(exports)
    }

    pub(super) fn lookup_export_callback<'js>(
        globals: &QuickJsObject<'js>,
        export_name: &str,
    ) -> Result<Option<QuickJsFunction<'js>>, QuickJsError> {
        if let Some(export_table) = globals.get::<_, Option<QuickJsObject>>("__gc_script_exports")?
            && let Some(callback) = export_table.get::<_, Option<QuickJsFunction>>(export_name)?
        {
            return Ok(Some(callback));
        }
        Ok(None)
    }

    pub(super) fn script_callback_arg_node_id(value: &JsonValue) -> Option<u64> {
        let object = value.as_object()?;
        let kind = object.get("kind").and_then(JsonValue::as_str);
        if kind != Some("node") {
            return None;
        }
        object.get("id").and_then(JsonValue::as_u64)
    }

    pub(super) fn callback_arg_to_quickjs_value<'js>(
        ctx: &QuickJsCtx<'js>,
        globals: &QuickJsObject<'js>,
        value: &JsonValue,
    ) -> Result<QuickJsValue<'js>, ScriptRuntimeError> {
        if let Some(node_id) = Self::script_callback_arg_node_id(value)
            && let Some(gc) = globals.get::<_, Option<QuickJsObject>>("gc")?
        {
            let factory = if let Some(factory) = gc.get::<_, Option<QuickJsFunction>>("__eventNodeHandle")? {
                Some(factory)
            } else {
                gc.get::<_, Option<QuickJsFunction>>("__nodeHandle")?
            };
            if let Some(factory) = factory {
                return Ok(factory.call::<_, QuickJsValue>((node_id as f64,))?);
            }
        }

        let json = serde_json::to_string(value).map_err(|err| {
            ScriptRuntimeError::InvalidManifest(format!("failed to serialize callback argument: {err}"))
        })?;
        Ok(ctx.json_parse(json.as_str())?)
    }

    pub(super) fn to_quickjs_value<'js>(
        &self,
        ctx: &QuickJsCtx<'js>,
        value: &ScriptValue,
    ) -> Result<QuickJsValue<'js>, ScriptRuntimeError> {
        let js_value = match value {
            ScriptValue::Nil => QuickJsValue::new_null(ctx.clone()),
            ScriptValue::Bool(value) => value.into_js(ctx)?,
            ScriptValue::Int(value) => {
                if let Ok(small) = i32::try_from(*value) {
                    small.into_js(ctx)?
                } else {
                    (*value as f64).into_js(ctx)?
                }
            }
            ScriptValue::Float(value) => value.into_js(ctx)?,
            ScriptValue::Str(value) => value.as_str().into_js(ctx)?,
            ScriptValue::Json(value) => {
                let json = serde_json::to_string(value).map_err(|err| {
                    ScriptRuntimeError::InvalidManifest(format!("failed to serialize JSON argument: {err}"))
                })?;
                ctx.json_parse(json)?
            }
        };
        Ok(js_value)
    }

    pub(super) fn quickjs_value_to_script<'js>(
        &self,
        ctx: &QuickJsCtx<'js>,
        value: QuickJsValue<'js>,
    ) -> Result<ScriptValue, ScriptRuntimeError> {
        if value.is_null() || value.is_undefined() {
            return Ok(ScriptValue::Nil);
        }
        if let Some(value) = value.as_bool() {
            return Ok(ScriptValue::Bool(value));
        }
        if let Some(value) = value.as_int() {
            return Ok(ScriptValue::Int(value as i64));
        }
        if let Some(value) = value.as_float() {
            return Ok(ScriptValue::Float(value));
        }
        if value.is_string() {
            let text: String = value.get()?;
            return Ok(ScriptValue::Str(text));
        }
        if value.is_big_int() {
            let int_value: i64 = value.get()?;
            return Ok(ScriptValue::Int(int_value));
        }

        let Some(payload_text) = ctx.json_stringify(&value)?.and_then(|raw| raw.to_string().ok()) else {
            return Ok(ScriptValue::Nil);
        };
        let payload = serde_json::from_str::<JsonValue>(&payload_text)
            .map_err(|err| ScriptRuntimeError::InvalidManifest(format!("failed to parse JSON return value: {err}")))?;
        Ok(ScriptValue::Json(payload))
    }

    pub(super) fn is_exception_placeholder(message: &str) -> bool {
        message.trim().eq_ignore_ascii_case("exception generated by quickjs")
    }

    pub(super) fn js_value_to_text<'js>(ctx: &QuickJsCtx<'js>, value: &QuickJsValue<'js>) -> Option<String> {
        if value.is_string()
            && let Ok(text) = value.get::<String>()
        {
            return Some(text);
        }

        if let Ok(Some(raw)) = ctx.json_stringify(value)
            && let Ok(text) = raw.to_string()
        {
            return Some(text);
        }

        None
    }

    pub(super) fn describe_quickjs_exception<'js>(ctx: &QuickJsCtx<'js>) -> String {
        let exception = ctx.catch();
        if exception.is_null() || exception.is_undefined() {
            return "exception generated by QuickJS with no error value".to_string();
        }

        if let Some(error_object) = exception.as_object() {
            let name = error_object.get::<_, Option<String>>("name").ok().flatten();
            let message = error_object.get::<_, Option<String>>("message").ok().flatten();
            let stack = error_object.get::<_, Option<String>>("stack").ok().flatten();
            let file_name = error_object.get::<_, Option<String>>("fileName").ok().flatten();
            let line_number = error_object.get::<_, Option<i32>>("lineNumber").ok().flatten();
            let column_number = error_object.get::<_, Option<i32>>("columnNumber").ok().flatten();

            let mut summary = match (name.as_deref(), message.as_deref()) {
                (Some(name), Some(message)) if !name.trim().is_empty() && !message.trim().is_empty() => {
                    format!("{name}: {message}")
                }
                (_, Some(message)) if !message.trim().is_empty() => message.to_string(),
                (Some(name), _) if !name.trim().is_empty() => name.to_string(),
                _ => "JavaScript exception".to_string(),
            };

            if let Some(stack) = stack.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                summary.push('\n');
                summary.push_str(stack);
                return summary;
            }

            if let Some(file_name) = file_name {
                let mut location = file_name;
                if let Some(line_number) = line_number {
                    location.push(':');
                    location.push_str(&line_number.to_string());
                    if let Some(column_number) = column_number {
                        location.push(':');
                        location.push_str(&column_number.to_string());
                    }
                }
                summary.push_str(" (");
                summary.push_str(&location);
                summary.push(')');
            }

            return summary;
        }

        if let Some(text) = Self::js_value_to_text(ctx, &exception) {
            return format!("JavaScript exception: {text}");
        }

        "JavaScript exception (unable to stringify thrown value)".to_string()
    }

    pub(super) fn quickjs_error_with_context<'js>(
        ctx: &QuickJsCtx<'js>,
        phase: &str,
        error: QuickJsError,
    ) -> ScriptRuntimeError {
        if error.is_exception() {
            return ScriptRuntimeError::QuickJs(format!("{phase}: {}", Self::describe_quickjs_exception(ctx)));
        }

        ScriptRuntimeError::QuickJs(format!("{phase}: {error}"))
    }

    pub(super) fn enrich_runtime_error_with_context<'js>(
        ctx: &QuickJsCtx<'js>,
        phase: &str,
        error: ScriptRuntimeError,
    ) -> ScriptRuntimeError {
        match error {
            ScriptRuntimeError::QuickJs(message) => {
                let message = if Self::is_exception_placeholder(&message) {
                    Self::describe_quickjs_exception(ctx)
                } else {
                    message
                };
                if message.contains(SCRIPT_HOST_CALL_BUDGET_MESSAGE) {
                    ScriptRuntimeError::BudgetViolation(format!("{phase}: {SCRIPT_HOST_CALL_BUDGET_MESSAGE}"))
                } else {
                    ScriptRuntimeError::QuickJs(format!("{phase}: {message}"))
                }
            }
            other => other,
        }
    }
}
