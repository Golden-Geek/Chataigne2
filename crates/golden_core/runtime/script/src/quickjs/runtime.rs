use super::*;

impl ScriptRuntime for QuickJsRuntime {
    fn load(
        &mut self,
        source: &str,
        source_name: &str,
        host: Option<&mut dyn ScriptHostBridge>,
    ) -> Result<ScriptManifest, ScriptRuntimeError> {
        self.entrypoints = QuickJsEntrypoints::default();
        self.manifest = None;
        self.reset_host_callback_state()?;
        if let Some(host_ref) = host.as_deref() {
            self.sync_tree_bridge_state(host_ref)?;
        }

        let bootstrap = r#"
globalThis.__gc_script_exports = {};
globalThis.__gc_script_manifest = {
  apiVersion: 1,
  updateRateHz: null,
  parameters: {},
  subscriptions: [],
  exports: globalThis.__gc_script_exports,
};
const __gcResolveParameterDefault = (spec) => {
  const normalized = spec && typeof spec === "object" && !Array.isArray(spec) ? spec : {};
  if (Object.prototype.hasOwnProperty.call(normalized, "default")) {
    return normalized.default;
  }
  const typeLabel = String(normalized.type ?? "float").trim().toLowerCase();
  switch (typeLabel) {
    case "trigger":
      return null;
    case "int":
      return 0;
    case "float":
      return 0.0;
    case "str":
    case "string":
    case "file":
    case "path":
    case "enum":
      return "";
    case "bool":
    case "boolean":
      return false;
    case "vec2":
      return [0.0, 0.0];
    case "vec3":
      return [0.0, 0.0, 0.0];
    case "color":
      return [0.0, 0.0, 0.0, 1.0];
    default:
      return 0.0;
  }
};
const __gcInvokeScriptMethod = (method, args) => {
  const gc = globalThis.gc;
  if (!gc || typeof gc !== "object") {
    return undefined;
  }
  if (typeof gc.__tree_script_id !== "function") {
    return undefined;
  }
  const scriptId = gc.__tree_script_id();
  if (scriptId === null || scriptId === undefined) {
    return undefined;
  }

  if (typeof globalThis.__gcInvokeTreeMethod === "function") {
    return globalThis.__gcInvokeTreeMethod(scriptId, method, args);
  }

  if (typeof gc.__tree_call_method_raw !== "function") {
    return undefined;
  }
  const raw = gc.__tree_call_method_raw(
    Number(scriptId),
    String(method ?? ""),
    JSON.stringify(Array.isArray(args) ? args : [])
  );
  if (typeof raw !== "string") {
    return undefined;
  }
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : undefined;
  } catch {
    return undefined;
  }
};
const __gcScriptMethodResultToJsValue = (parsed) => {
  if (!parsed || typeof parsed !== "object") {
    return undefined;
  }
  if (typeof globalThis.__gcTreeResultToJsValue === "function") {
    return globalThis.__gcTreeResultToJsValue(parsed);
  }
  if (parsed.kind === "value") {
    return parsed.value;
  }
  return undefined;
};
const __gcInvokeScriptMethodAsJsValue = (method, args) => __gcScriptMethodResultToJsValue(__gcInvokeScriptMethod(method, args));
const __gcScriptMethodSucceeded = (parsed) => {
  if (!parsed || typeof parsed !== "object") {
    return false;
  }
  if (parsed.kind === "value") {
    return Boolean(parsed.value);
  }
  return true;
};
globalThis.script = {
  setApiVersion(value) {
    const numeric = Number(value);
    if (Number.isFinite(numeric) && numeric >= 1) {
      globalThis.__gc_script_manifest.apiVersion = Math.floor(numeric);
    }
    return globalThis.__gc_script_manifest.apiVersion;
  },
  setUpdateRateHz(value) {
    if (value === null || value === undefined) {
      globalThis.__gc_script_manifest.updateRateHz = null;
      return null;
    }
    const numeric = Number(value);
    if (!Number.isFinite(numeric) || numeric <= 0) {
      globalThis.__gc_script_manifest.updateRateHz = null;
      return null;
    }
    const rounded = Math.floor(numeric);
    globalThis.__gc_script_manifest.updateRateHz = rounded > 0 ? rounded : null;
    return globalThis.__gc_script_manifest.updateRateHz;
  },
  time() {
    return Number(globalThis.gc.__time_seconds_raw());
  },
  listen(node, config = {}) {
    return globalThis.listen(node, config);
  },
  unlisten(node) {
    return globalThis.unlisten(node);
  },
  clearListeners() {
    return globalThis.clearListeners();
  },
  addParameter(name, spec = {}) {
    const key = String(name ?? "").trim();
    if (key.length === 0) {
      return undefined;
    }
    const normalizedSpec = spec && typeof spec === "object" && !Array.isArray(spec) ? spec : {};
    globalThis.__gc_script_manifest.parameters[key] = normalizedSpec;
    const defaultValue = __gcResolveParameterDefault(normalizedSpec);
    return __gcInvokeScriptMethodAsJsValue("addParameter", [key, defaultValue]);
  },
  addNode(nodeType = "folder", name, spec) {
    const typeLabel = String(nodeType ?? "").trim();
    const args = [typeLabel.length > 0 ? typeLabel : "folder"];
    if (name !== undefined) {
      args.push(name);
    }
    if (spec !== undefined) {
      if (args.length === 1) {
        args.push("");
      }
      args.push(spec);
    }
    return __gcInvokeScriptMethodAsJsValue("addNode", args);
  },
  addFolder(name = "Folder") {
    const key = String(name ?? "").trim();
    return __gcInvokeScriptMethodAsJsValue("addFolder", [key.length > 0 ? key : "Folder"]);
  },
  removeParameter(name) {
    const key = String(name ?? "").trim();
    if (key.length === 0) {
      return false;
    }
    const removed = delete globalThis.__gc_script_manifest.parameters[key];
    const applied = __gcInvokeScriptMethod("removeParameter", [key]);
    if (applied !== undefined) {
      return __gcScriptMethodSucceeded(applied);
    }
    return removed;
  },
};
if (globalThis.gc && typeof globalThis.gc === "object") {
  globalThis.gc.script = globalThis.script;
}
"#;

        let source_name = source_name.to_string();
        let preprocessed_source = Self::preprocess_source_for_exported_functions(source);
        let load_result = self.load_timed("script load", || {
            self.context
                .with(|ctx| -> Result<(QuickJsEntrypoints, String), ScriptRuntimeError> {
                    let result = (|| -> Result<(QuickJsEntrypoints, String), QuickJsError> {
                        let mut bootstrap_options = QuickJsEvalOptions::default();
                        bootstrap_options.filename = Some(format!("{source_name}#bootstrap"));
                        ctx.eval_with_options::<(), _>(bootstrap, bootstrap_options)?;

                        let mut eval_options = QuickJsEvalOptions::default();
                        eval_options.filename = Some(source_name.clone());
                        ctx.eval_with_options::<(), _>(preprocessed_source.as_str(), eval_options)?;

                        let globals = ctx.globals();
                        let root_value: QuickJsValue = globals.get("__gc_script_manifest")?;
                        if root_value.is_null() || root_value.is_undefined() || !root_value.is_object() {
                            return Err(QuickJsError::new_from_js_message("value", "object", "script manifest state must be an object"));
                        }
                        let _root = root_value.into_object().ok_or_else(|| QuickJsError::new_from_js_message("value", "object", "script manifest state must be an object"))?;

                        let init = Self::first_function_name(&globals, &["init"])?;
                        let update = Self::first_function_name(&globals, &["update"])?;
                        let event = Self::first_function_name(&globals, &["event"])?;
                        let param_changed = Self::first_function_name(&globals, &["paramChanged"])?;
                        let destroy = Self::first_function_name(&globals, &["destroy"])?;

                        let mut exports = Vec::new();
                        if let Some(export_table) = globals.get::<_, Option<QuickJsObject>>("__gc_script_exports")? {
                            exports.extend(Self::collect_export_names(&export_table)?);
                        }
                        exports.sort();
                        exports.dedup();

                        let manifest_json = ctx
                            .eval::<Option<String>, _>("JSON.stringify(globalThis.__gc_script_manifest ?? {}, (key, value) => typeof value === 'function' ? undefined : value)")?
                            .ok_or_else(|| QuickJsError::new_from_js_message("object", "string", "failed to stringify script manifest"))?;

                        Ok((QuickJsEntrypoints { init, update, event, param_changed, destroy, exports }, manifest_json))
                    })();
                    result.map_err(|error| Self::quickjs_error_with_context(&ctx, "script load", error))
                })
        });
        let (entrypoints, manifest_json) = match load_result {
            Ok(value) => value,
            Err(error) => {
                self.discard_host_ops()?;
                return Err(error);
            }
        };

        let manifest_result = serde_json::from_str::<JsonValue>(&manifest_json)
            .map_err(|err| ScriptRuntimeError::InvalidManifest(format!("failed to parse manifest JSON: {err}")))
            .and_then(|manifest_payload| parse_manifest_from_json(&manifest_payload, entrypoints.exports.clone()));
        let manifest = self.finish_host_invocation(manifest_result, host)?;

        self.entrypoints = entrypoints;
        self.manifest = Some(manifest.clone());
        Ok(manifest)
    }

    fn reload(
        &mut self,
        source: &str,
        source_name: &str,
        host: Option<&mut dyn ScriptHostBridge>,
    ) -> Result<ScriptManifest, ScriptRuntimeError> {
        let budgets = self.budgets;
        *self = Self::new(budgets)?;
        self.load(source, source_name, host)
    }

    fn manifest(&self) -> Option<&ScriptManifest> {
        self.manifest.as_ref()
    }

    fn export_names(&self) -> Vec<String> {
        self.entrypoints.exports.clone()
    }

    fn call_export(
        &mut self,
        export_name: &str,
        args: &[ScriptValue],
        host: &mut dyn ScriptHostBridge,
    ) -> Result<ScriptValue, ScriptRuntimeError> {
        if !self.entrypoints.exports.iter().any(|name| name == export_name) {
            return Err(ScriptRuntimeError::MissingExport(export_name.to_string()));
        }

        self.reset_host_callback_state()?;
        self.sync_tree_bridge_state(host)?;
        let result = self.callback_timed("export", || {
            self.context.with(|ctx| -> Result<ScriptValue, ScriptRuntimeError> {
                let result = (|| -> Result<ScriptValue, ScriptRuntimeError> {
                    let globals = ctx.globals();
                    let callback = Self::lookup_export_callback(&globals, export_name)?
                        .ok_or_else(|| ScriptRuntimeError::MissingExport(export_name.to_string()))?;

                    let mut call_args = QuickJsArgs::new(ctx.clone(), args.len());
                    for argument in args {
                        call_args.push_arg(self.to_quickjs_value(&ctx, argument)?)?;
                    }
                    let return_value = callback.call_arg::<QuickJsValue>(call_args)?;
                    self.quickjs_value_to_script(&ctx, return_value)
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "export callback", error))
            })
        });
        self.finish_host_invocation(result, Some(host))
    }

    fn call_on_init(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(callback_name) = self.entrypoints.init.clone() else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        self.sync_tree_bridge_state(host)?;
        let result = self.callback_timed("on_init", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let result = (|| -> Result<(), ScriptRuntimeError> {
                    let globals = ctx.globals();
                    if let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name.as_str())? {
                        callback.call::<_, ()>(())?;
                    }
                    Ok(())
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "on_init callback", error))
            })
        });
        self.finish_host_invocation(result, Some(host))
    }

    fn call_on_update(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(callback_name) = self.entrypoints.update.clone() else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        self.sync_tree_bridge_state(host)?;
        let delta_seconds = host.delta_seconds();
        let result = self.callback_timed("on_update", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let result = (|| -> Result<(), ScriptRuntimeError> {
                    let globals = ctx.globals();
                    if let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name.as_str())? {
                        callback.call::<_, ()>((delta_seconds,))?;
                    }
                    Ok(())
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "on_update callback", error))
            })
        });
        self.finish_host_invocation(result, Some(host))
    }

    fn call_on_event(
        &mut self,
        event: &ScriptEvent,
        host: &mut dyn ScriptHostBridge,
    ) -> Result<(), ScriptRuntimeError> {
        let event_callback_name = self.entrypoints.event.clone();
        let param_changed_callback_name = if event.kind == "paramChanged" {
            self.entrypoints.param_changed.clone()
        } else {
            None
        };
        let custom_callback_invocation = event.custom_callback_invocation();
        if event_callback_name.is_none()
            && param_changed_callback_name.is_none()
            && custom_callback_invocation.is_none()
        {
            return Ok(());
        }

        self.reset_host_callback_state()?;
        self.sync_tree_bridge_state(host)?;
        let event_payload = serde_json::to_string(event)
            .map_err(|err| ScriptRuntimeError::InvalidManifest(format!("failed to encode event payload: {err}")))?;
        let result = self.callback_timed("on_event", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let result = (|| -> Result<(), ScriptRuntimeError> {
                    let globals = ctx.globals();

                    if let Some(invocation) = custom_callback_invocation.as_ref()
                        && let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(invocation.name.as_str())?
                    {
                        let mut call_args = QuickJsArgs::new(ctx.clone(), invocation.args.len());
                        for arg in &invocation.args {
                            call_args.push_arg(Self::callback_arg_to_quickjs_value(&ctx, &globals, arg)?)?;
                        }
                        callback.call_arg::<()>(call_args)?;
                    }

                    if let Some(callback_name) = param_changed_callback_name.as_deref()
                        && let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name)?
                    {
                        let param_value = if let Some(param_node) = event.origin {
                            if let Some(gc) = globals.get::<_, Option<QuickJsObject>>("gc")? {
                                let factory =
                                    if let Some(factory) = gc.get::<_, Option<QuickJsFunction>>("__eventNodeHandle")? {
                                        Some(factory)
                                    } else {
                                        gc.get::<_, Option<QuickJsFunction>>("__nodeHandle")?
                                    };
                                if let Some(factory) = factory {
                                    factory.call::<_, QuickJsValue>((param_node.0 as f64,))?
                                } else {
                                    ctx.json_parse("null")?
                                }
                            } else {
                                ctx.json_parse("null")?
                            }
                        } else {
                            ctx.json_parse("null")?
                        };
                        let old_value_payload = event
                            .old_value
                            .as_ref()
                            .map(Self::param_value_to_tree_json)
                            .unwrap_or(JsonValue::Null);
                        let old_value_json = serde_json::to_string(&old_value_payload).map_err(|err| {
                            ScriptRuntimeError::InvalidManifest(format!("failed to encode oldValue payload: {err}"))
                        })?;
                        let old_value_value = ctx.json_parse(old_value_json.as_str())?;
                        let event_value = ctx.json_parse(event_payload.as_str())?;
                        callback.call::<_, ()>((param_value, old_value_value, event_value))?;
                    }

                    if let Some(callback_name) = event_callback_name.as_deref()
                        && let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name)?
                    {
                        let event_value = ctx.json_parse(event_payload.as_str())?;
                        callback.call::<_, ()>((event_value,))?;
                    }
                    Ok(())
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "on_event callback", error))
            })
        });
        self.finish_host_invocation(result, Some(host))
    }

    fn call_on_destroy(&mut self, host: &mut dyn ScriptHostBridge) -> Result<(), ScriptRuntimeError> {
        let Some(callback_name) = self.entrypoints.destroy.clone() else {
            return Ok(());
        };

        self.reset_host_callback_state()?;
        self.sync_tree_bridge_state(host)?;
        let result = self.callback_timed("on_destroy", || {
            self.context.with(|ctx| -> Result<(), ScriptRuntimeError> {
                let result = (|| -> Result<(), ScriptRuntimeError> {
                    let globals = ctx.globals();
                    if let Some(callback) = globals.get::<_, Option<QuickJsFunction>>(callback_name.as_str())? {
                        callback.call::<_, ()>(())?;
                    }
                    Ok(())
                })();
                result.map_err(|error| Self::enrich_runtime_error_with_context(&ctx, "on_destroy callback", error))
            })
        });
        self.finish_host_invocation(result, Some(host))
    }

    fn has_on_update(&self) -> bool {
        self.entrypoints.update.is_some()
    }
}
