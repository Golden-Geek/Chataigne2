use super::*;

impl QuickJsRuntime {
    pub(super) fn install_host_api(&self) -> Result<(), ScriptRuntimeError> {
        let max_host_calls = self.budgets.max_host_calls_per_callback.max(1);
        let shared_host_ops = Arc::clone(&self.host_ops);
        let shared_host_call_counter = Arc::clone(&self.host_call_counter);
        let shared_tree_bridge_state = Arc::clone(&self.tree_bridge_state);
        self.context.with(|ctx| -> Result<(), QuickJsError> {
            let gc_table = QuickJsObject::new(ctx.clone())?;

            let log_host_ops = Arc::clone(&shared_host_ops);
            let log_host_call_counter = Arc::clone(&shared_host_call_counter);
            let log_fn = QuickJsFunc::from(QuickJsMutFn::from(move |level_label: String, message: String| -> Result<(), QuickJsError> {
                let call_count = log_host_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }
                QuickJsRuntime::validate_host_input_size(
                    message.as_str(),
                    "log message",
                    SCRIPT_MAX_HOST_MESSAGE_BYTES,
                )?;

                let level = ScriptLogLevel::from_manifest_label(&level_label).ok_or_else(|| QuickJsError::new_from_js_message("string", "scriptLogLevel", format!("invalid log level '{level_label}'")))?;
                let mut guard = log_host_ops.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script host-op queue lock poisoned"))?;
                guard.push(ScriptHostOp::Log { level, message });
                Ok(())
            }));
            gc_table.set("log", log_fn)?;

            let emit_host_ops = Arc::clone(&shared_host_ops);
            let emit_host_call_counter = Arc::clone(&shared_host_call_counter);
            let emit_raw_fn = QuickJsFunc::from(QuickJsMutFn::from(move |topic: String, payload_json: Option<String>| -> Result<(), QuickJsError> {
                let call_count = emit_host_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }

                QuickJsRuntime::validate_host_input_size(
                    topic.as_str(),
                    "event topic",
                    SCRIPT_MAX_HOST_LABEL_BYTES,
                )?;
                if let Some(payload_json) = payload_json.as_deref() {
                    QuickJsRuntime::validate_host_input_size(
                        payload_json,
                        "event payload",
                        SCRIPT_MAX_HOST_JSON_BYTES,
                    )?;
                }

                let payload_json = serde_json::from_str::<JsonValue>(payload_json.as_deref().unwrap_or("null")).unwrap_or(JsonValue::Null);

                let mut guard = emit_host_ops.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script host-op queue lock poisoned"))?;
                guard.push(ScriptHostOp::EmitCustom { topic, payload: payload_json });
                Ok(())
            }));
            gc_table.set("__emit_raw", emit_raw_fn)?;

            let time_state = Arc::clone(&shared_tree_bridge_state);
            let time_call_counter = Arc::clone(&shared_host_call_counter);
            let time_fn = QuickJsFunc::from(QuickJsMutFn::from(move || -> Result<f64, QuickJsError> {
                let call_count = time_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }
                let state = time_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                Ok(state.time_seconds)
            }));
            gc_table.set("__time_seconds_raw", time_fn)?;

            let delta_state = Arc::clone(&shared_tree_bridge_state);
            let delta_call_counter = Arc::clone(&shared_host_call_counter);
            let delta_fn = QuickJsFunc::from(QuickJsMutFn::from(move || -> Result<f64, QuickJsError> {
                let call_count = delta_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }
                let state = delta_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                Ok(state.delta_seconds)
            }));
            gc_table.set("__delta_seconds_raw", delta_fn)?;

            let tree_root_state = Arc::clone(&shared_tree_bridge_state);
            let tree_root_call_counter = Arc::clone(&shared_host_call_counter);
            let tree_root_fn = QuickJsFunc::from(QuickJsMutFn::from(move || -> Result<Option<u64>, QuickJsError> {
                let call_count = tree_root_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }
                let state = tree_root_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                Ok(state.snapshot.as_ref().map(|snapshot| snapshot.root().0))
            }));
            gc_table.set("__tree_root_id", tree_root_fn)?;

            let tree_host_state = Arc::clone(&shared_tree_bridge_state);
            let tree_host_call_counter = Arc::clone(&shared_host_call_counter);
            let tree_host_fn = QuickJsFunc::from(QuickJsMutFn::from(move || -> Result<Option<u64>, QuickJsError> {
                let call_count = tree_host_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }
                let state = tree_host_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                Ok(state.host.map(|node| node.0))
            }));
            gc_table.set("__tree_host_id", tree_host_fn)?;

            let tree_script_state = Arc::clone(&shared_tree_bridge_state);
            let tree_script_call_counter = Arc::clone(&shared_host_call_counter);
            let tree_script_fn = QuickJsFunc::from(QuickJsMutFn::from(move || -> Result<Option<u64>, QuickJsError> {
                let call_count = tree_script_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }
                let state = tree_script_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                Ok(state.script.map(|node| node.0))
            }));
            gc_table.set("__tree_script_id", tree_script_fn)?;

            let tree_get_state = Arc::clone(&shared_tree_bridge_state);
            let tree_get_call_counter = Arc::clone(&shared_host_call_counter);
            let tree_get_fn = QuickJsFunc::from(QuickJsMutFn::from(move |node_id_raw: i64, key: String| -> Result<Option<String>, QuickJsError> {
                let call_count = tree_get_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }
                QuickJsRuntime::validate_host_input_size(
                    key.as_str(),
                    "tree key",
                    SCRIPT_MAX_HOST_LABEL_BYTES,
                )?;

                let node_id = u64::try_from(node_id_raw).map_err(|_| QuickJsError::new_from_js_message("number", "nodeId", "node id must be a non-negative integer"))?;
                let state = tree_get_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                let Some(snapshot) = state.snapshot.as_ref() else {
                    return Ok(None);
                };

                let node_id = NodeId(node_id);
                let Some(node) = snapshot.node(node_id) else {
                    return Ok(None);
                };
                let trimmed_key = key.trim();
                if trimmed_key.is_empty() {
                    return Ok(None);
                }

                let metadata = match trimmed_key {
                    "$id" => Some(serde_json::json!({ "kind": "value", "value": node.id.0 })),
                    "$type" => Some(serde_json::json!({ "kind": "value", "value": node.node_type.clone() })),
                    "$name" => Some(serde_json::json!({ "kind": "value", "value": node.label.clone() })),
                    "$declId" => Some(serde_json::json!({ "kind": "value", "value": node.decl_id.clone() })),
                    "$shortName" => Some(serde_json::json!({ "kind": "value", "value": node.short_name.clone() })),
                    "$enabled" => Some(serde_json::json!({ "kind": "value", "value": node.enabled })),
                    "$isParameter" => Some(serde_json::json!({ "kind": "value", "value": node.is_parameter() })),
                    _ => None,
                };
                if let Some(metadata) = metadata {
                    return Ok(Some(metadata.to_string()));
                }

                if let Some(child_id) = snapshot.find_child(node_id, trimmed_key)
                    && let Some(child) = snapshot.node(child_id) {
                        if let Some(value) = child.param_value.as_ref() {
                            let encoded = QuickJsRuntime::param_value_to_tree_json(value);
                            return Ok(Some(serde_json::json!({ "kind": "value", "value": encoded }).to_string()));
                        }

                        return Ok(Some(
                            serde_json::json!({
                                "kind": "node",
                                "id": child_id.0
                            })
                            .to_string(),
                        ));
                    }

                if let Some(value) = snapshot.script_property(node_id, trimmed_key) {
                    return Ok(Some(
                        serde_json::json!({ "kind": "value", "value": QuickJsRuntime::param_value_to_tree_json(&value) })
                            .to_string(),
                    ));
                }

                if snapshot.has_script_method(node_id, trimmed_key) {
                    return Ok(Some(serde_json::json!({ "kind": "method" }).to_string()));
                }

                Ok(None)
            }));
            gc_table.set("__tree_get_raw", tree_get_fn)?;

            let tree_set_property_ops = Arc::clone(&shared_host_ops);
            let tree_set_property_state = Arc::clone(&shared_tree_bridge_state);
            let tree_set_property_call_counter = Arc::clone(&shared_host_call_counter);
            let tree_set_property_fn = QuickJsFunc::from(QuickJsMutFn::from(move |node_id_raw: i64, property: String, value_json: Option<String>| -> Result<bool, QuickJsError> {
                let call_count = tree_set_property_call_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }
                QuickJsRuntime::validate_host_input_size(
                    property.as_str(),
                    "property name",
                    SCRIPT_MAX_HOST_LABEL_BYTES,
                )?;
                if let Some(value_json) = value_json.as_deref() {
                    QuickJsRuntime::validate_host_input_size(
                        value_json,
                        "property value",
                        SCRIPT_MAX_HOST_JSON_BYTES,
                    )?;
                }
                let node_id = u64::try_from(node_id_raw).map_err(|_| QuickJsError::new_from_js_message("number", "nodeId", "node id must be a non-negative integer"))?;
                let property = property.trim();
                if property.is_empty() {
                    return Ok(false);
                }
                let value_payload = serde_json::from_str::<JsonValue>(value_json.as_deref().unwrap_or("null")).unwrap_or(JsonValue::Null);
                let value = QuickJsRuntime::param_value_from_json(&value_payload).map_err(|message| QuickJsError::new_from_js_message("script", "paramValue", message))?;

                let mut target_node = NodeId(node_id);
                let mut target_property = property.to_string();
                let state = tree_set_property_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                if let Some(snapshot) = state.snapshot.as_ref()
                    && snapshot.node(NodeId(node_id)).is_some()
                        && snapshot.script_property(NodeId(node_id), property).is_none()
                            && let Some(child_id) = snapshot.find_child(NodeId(node_id), property)
                                && snapshot.node(child_id).is_some_and(|child| child.is_parameter()) {
                                    target_node = child_id;
                                    target_property = "value".to_string();
                                }
                drop(state);

                let mut guard = tree_set_property_ops.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script host-op queue lock poisoned"))?;
                guard.push(ScriptHostOp::SetNodeScriptProperty { node: target_node, property: target_property, value });
                Ok(true)
            }));
            gc_table.set("__tree_set_property_raw", tree_set_property_fn)?;

            let tree_call_method_ops = Arc::clone(&shared_host_ops);
            let tree_call_method_state = Arc::clone(&shared_tree_bridge_state);
            let tree_call_method_counter = Arc::clone(&shared_host_call_counter);
            let tree_call_method_fn = QuickJsFunc::from(QuickJsMutFn::from(move |node_id_raw: i64, method: String, args_json: Option<String>| -> Result<Option<String>, QuickJsError> {
                let call_count = tree_call_method_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }
                QuickJsRuntime::validate_host_input_size(
                    method.as_str(),
                    "method name",
                    SCRIPT_MAX_HOST_LABEL_BYTES,
                )?;
                if let Some(args_json) = args_json.as_deref() {
                    QuickJsRuntime::validate_host_input_size(
                        args_json,
                        "method arguments",
                        SCRIPT_MAX_HOST_JSON_BYTES,
                    )?;
                }
                let node_id = u64::try_from(node_id_raw).map_err(|_| QuickJsError::new_from_js_message("number", "nodeId", "node id must be a non-negative integer"))?;
                let method = method.trim();
                if method.is_empty() {
                    return Ok(None);
                }

                let args_payload = serde_json::from_str::<JsonValue>(args_json.as_deref().unwrap_or("[]")).unwrap_or(JsonValue::Null);
                let args_values = match args_payload {
                    JsonValue::Null => Vec::new(),
                    JsonValue::Array(values) => values,
                    _ => return Err(QuickJsError::new_from_js_message("script", "args", "script node method arguments must be a JSON array")),
                };

                if method == "getProperties" {
                    let state = tree_call_method_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                    let Some(snapshot) = state.snapshot.as_ref() else {
                        return Ok(None);
                    };
                    let Some(node) = snapshot.node(NodeId(node_id)) else {
                        return Ok(None);
                    };

                    let mut output = serde_json::Map::new();
                    output.insert("id".to_string(), serde_json::json!(node.id.0));
                    output.insert("type".to_string(), serde_json::json!(node.node_type.clone()));
                    output.insert("name".to_string(), serde_json::json!(node.label.clone()));
                    output.insert("label".to_string(), serde_json::json!(node.label.clone()));
                    output.insert("declId".to_string(), serde_json::json!(node.decl_id.clone()));
                    output.insert("shortName".to_string(), serde_json::json!(node.short_name.clone()));
                    output.insert("enabled".to_string(), serde_json::json!(node.enabled));
                    output.insert("isParameter".to_string(), serde_json::json!(node.is_parameter()));
                    output.insert("childCount".to_string(), serde_json::json!(node.child_count));
                    if let Some(value) = node.param_value.as_ref() {
                        output.insert("value".to_string(), QuickJsRuntime::param_value_to_tree_json(value));
                    }
                    if let Some(constraints) = node.param_constraints.as_ref() {
                        output.insert("constraints".to_string(), serde_json::to_value(constraints).unwrap_or(JsonValue::Null));
                    }
                    for (key, value) in snapshot.script_properties(NodeId(node_id)) {
                        output.insert(key, QuickJsRuntime::param_value_to_tree_json(&value));
                    }

                    return Ok(Some(serde_json::json!({ "kind": "value", "value": JsonValue::Object(output) }).to_string()));
                }

                if method == "getChildren" {
                    let state = tree_call_method_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                    let Some(snapshot) = state.snapshot.as_ref() else {
                        return Ok(None);
                    };
                    let child_ids = snapshot.child_ids(NodeId(node_id));
                    let ids = child_ids.iter().map(|id| id.0).collect::<Vec<_>>();
                    return Ok(Some(serde_json::json!({ "kind": "nodes", "ids": ids }).to_string()));
                }

                if method == "getChild" {
                    let state = tree_call_method_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                    let Some(snapshot) = state.snapshot.as_ref() else {
                        return Ok(None);
                    };
                    let Some(selector) = args_values.first() else {
                        return Ok(Some(serde_json::json!({ "kind": "void" }).to_string()));
                    };

                    let resolved = if let Some(index) = selector.as_i64() {
                        usize::try_from(index).ok().and_then(|index| snapshot.child_at(NodeId(node_id), index))
                    } else if let Some(key) = selector.as_str() {
                        let key = key.trim();
                        if key.is_empty() { None } else { snapshot.find_child(NodeId(node_id), key) }
                    } else {
                        None
                    };

                    if let Some(child) = resolved {
                        return Ok(Some(serde_json::json!({ "kind": "node", "id": child.0 }).to_string()));
                    }
                    return Ok(Some(serde_json::json!({ "kind": "void" }).to_string()));
                }

                if method == "toString" {
                    let state = tree_call_method_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                    let Some(snapshot) = state.snapshot.as_ref() else {
                        return Ok(None);
                    };
                    let Some(_) = snapshot.node(NodeId(node_id)) else {
                        return Ok(None);
                    };
                    return Ok(Some(serde_json::json!({ "kind": "value", "value": snapshot.describe_node(NodeId(node_id)).unwrap_or_default() }).to_string()));
                }

                if method == "listen" {
                    let level = if let Some(config) = args_values.first() {
                        if let Some(level) = config.as_u64() {
                            u32::try_from(level).map_err(|_| QuickJsError::new_from_js_message("number", "level", "listener level is too large"))?
                        } else if let Some(level) = config.as_i64() {
                            if level < 0 {
                                return Err(QuickJsError::new_from_js_message("number", "level", "listener level must be >= 0"));
                            }
                            u32::try_from(level).map_err(|_| QuickJsError::new_from_js_message("number", "level", "listener level is too large"))?
                        } else if let Some(object) = config.as_object() {
                            if let Some(level) = object.get("level").and_then(JsonValue::as_u64) {
                                u32::try_from(level).map_err(|_| QuickJsError::new_from_js_message("number", "level", "listener level is too large"))?
                            } else if let Some(level) = object.get("maxDepth").and_then(JsonValue::as_u64) {
                                u32::try_from(level).map_err(|_| QuickJsError::new_from_js_message("number", "maxDepth", "listener level is too large"))?
                            } else {
                                1
                            }
                        } else {
                            1
                        }
                    } else {
                        1
                    };

                    let mut guard = tree_call_method_ops.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script host-op queue lock poisoned"))?;
                    guard.push(ScriptHostOp::SetEventListener { target: NodeId(node_id), level });
                    return Ok(Some(serde_json::json!({ "kind": "value", "value": true }).to_string()));
                }

                if method == "unlisten" {
                    let mut guard = tree_call_method_ops.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script host-op queue lock poisoned"))?;
                    guard.push(ScriptHostOp::RemoveEventListener { target: NodeId(node_id) });
                    return Ok(Some(serde_json::json!({ "kind": "value", "value": true }).to_string()));
                }

                let mut predicted_result = None;
                if method == "addParameter" {
                    if let Some(key) = args_values.first().and_then(JsonValue::as_str) {
                        let key = key.trim();
                        if !key.is_empty() {
                            let state = tree_call_method_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                            if let Some(snapshot) = state.snapshot.as_ref() {
                                if let Some(existing) = snapshot.find_child(NodeId(node_id), key) {
                                    predicted_result = Some(serde_json::json!({ "kind": "node", "id": existing.0 }));
                                } else {
                                    predicted_result = Some(serde_json::json!({ "kind": "selector", "parent": node_id, "key": key }));
                                }
                            }
                        }
                    }
                } else if method == "addFolder" {
                    let key = args_values.first().and_then(JsonValue::as_str).map(str::trim).filter(|value| !value.is_empty()).unwrap_or("Folder");
                    let state = tree_call_method_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                    if let Some(snapshot) = state.snapshot.as_ref() {
                        if let Some(existing) = snapshot.find_child(NodeId(node_id), key) {
                            predicted_result = Some(serde_json::json!({ "kind": "node", "id": existing.0 }));
                        } else {
                            predicted_result = Some(serde_json::json!({ "kind": "selector", "parent": node_id, "key": key }));
                        }
                    }
                } else if method == "addNode" {
                    let node_type = args_values.first().and_then(JsonValue::as_str).map(str::trim).filter(|value| !value.is_empty()).unwrap_or("folder");
                    let normalized_node_type = node_type.to_ascii_lowercase();
                    let default_label = match normalized_node_type.as_str() {
                        "parameter" | "param" => "parameter".to_string(),
                        "folder" | "" => "Folder".to_string(),
                        _ => node_type.to_string(),
                    };
                    let key = args_values.get(1).and_then(JsonValue::as_str).map(str::trim).filter(|value| !value.is_empty()).map(ToString::to_string).unwrap_or(default_label);
                    let state = tree_call_method_state.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script tree bridge lock poisoned"))?;
                    if let Some(snapshot) = state.snapshot.as_ref() {
                        if let Some(existing) = snapshot.find_child(NodeId(node_id), key.as_str()) {
                            predicted_result = Some(serde_json::json!({ "kind": "node", "id": existing.0 }));
                        } else {
                            predicted_result = Some(serde_json::json!({ "kind": "selector", "parent": node_id, "key": key }));
                        }
                    }
                }

                let args = QuickJsRuntime::method_args_from_json(method, args_values).map_err(|message| QuickJsError::new_from_js_message("script", "paramValue", message))?;

                let mut guard = tree_call_method_ops.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script host-op queue lock poisoned"))?;
                guard.push(ScriptHostOp::CallNodeScriptMethod {
                    node: NodeId(node_id),
                    method: method.to_string(),
                    args,
                });
                if let Some(predicted_result) = predicted_result {
                    return Ok(Some(predicted_result.to_string()));
                }
                Ok(Some(serde_json::json!({ "kind": "value", "value": true }).to_string()))
            }));
            gc_table.set("__tree_call_method_raw", tree_call_method_fn)?;

            let clear_listeners_ops = Arc::clone(&shared_host_ops);
            let clear_listeners_counter = Arc::clone(&shared_host_call_counter);
            let clear_listeners_fn = QuickJsFunc::from(QuickJsMutFn::from(move || -> Result<bool, QuickJsError> {
                let call_count = clear_listeners_counter.fetch_add(1, Ordering::Relaxed) + 1;
                if call_count > max_host_calls {
                    return Err(QuickJsError::new_from_js_message("script", "host", "script host-call budget exceeded in current callback"));
                }

                let mut guard = clear_listeners_ops.lock().map_err(|_| QuickJsError::new_from_js_message("script", "host", "script host-op queue lock poisoned"))?;
                guard.push(ScriptHostOp::ClearEventListeners);
                Ok(true)
            }));
            gc_table.set("__listeners_clear_raw", clear_listeners_fn)?;

            ctx.globals().set("gc", gc_table)?;
            ctx.eval::<(), _>(
                r#"
globalThis.gc.emit = (topic, payload) => globalThis.gc.__emit_raw(
  topic,
  JSON.stringify(payload === undefined ? null : payload)
);

const __gcParseTreeMethodResult = (raw) => {
  if (!raw || typeof raw !== "string") {
    return undefined;
  }

  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : undefined;
  } catch {
    return undefined;
  }
};

const __gcInvokeTreeMethod = (nodeId, method, args) => {
  const raw = globalThis.gc.__tree_call_method_raw(
    Number(nodeId),
    String(method ?? ""),
    JSON.stringify(Array.isArray(args) ? args : [])
  );
  return __gcParseTreeMethodResult(raw);
};

const __gcResolveNodeId = (value) => {
  if (value === null || value === undefined) {
    return undefined;
  }

  if (typeof value === "number" && Number.isFinite(value)) {
    return Math.floor(value);
  }

  if (typeof value === "object") {
    const rawNodeId = Number(value.__nodeId);
    if (Number.isFinite(rawNodeId)) {
      return Math.floor(rawNodeId);
    }

    if (typeof value.id === "function") {
      const byFunction = Number(value.id());
      if (Number.isFinite(byFunction)) {
        return Math.floor(byFunction);
      }
    }
  }

  return undefined;
};

const __gcScriptNodeProxyCache = new Map();
const __gcScriptNodeSelectorCache = new Map();
const __gcTreeResultToJsValue = (parsed) => {
  if (!parsed || typeof parsed !== "object") {
    return undefined;
  }

  if (parsed.kind === "node") {
    return __gcScriptNodeHandle(parsed.id);
  }

  if (parsed.kind === "selector") {
    return __gcScriptNodeSelector(parsed.parent, parsed.key);
  }

  if (parsed.kind === "nodes" && Array.isArray(parsed.ids)) {
    return parsed.ids
      .map((id) => __gcScriptNodeHandle(id))
      .filter((entry) => entry !== undefined);
  }

  if (parsed.kind === "value") {
    return parsed.value;
  }

  return undefined;
};

const __gcScriptNodeHandle = (nodeId) => {
  const numericId = Number(nodeId);
  if (!Number.isFinite(numericId)) {
    return undefined;
  }
  const cached = __gcScriptNodeProxyCache.get(numericId);
  if (cached) {
    return cached;
  }

  const target = {
    __nodeId: numericId,
    id() {
      return numericId;
    },
    is(other) {
      const otherId = __gcResolveNodeId(other);
      return Number.isFinite(otherId) && otherId === numericId;
    },
    [Symbol.toPrimitive](hint) {
      if (hint === "string") {
        const parsed = __gcInvokeTreeMethod(numericId, "toString", []);
        if (parsed && parsed.kind === "value") {
          return String(parsed.value ?? "");
        }
        return `[Node ${numericId}]`;
      }
      return numericId;
    },
  };

  const proxy = new Proxy(target, {
    get(innerTarget, prop) {
      if (typeof prop !== "string") {
        return innerTarget[prop];
      }
      if (prop in innerTarget) {
        const member = innerTarget[prop];
        return typeof member === "function" ? member.bind(innerTarget) : member;
      }

      const resolvedRaw = globalThis.gc.__tree_get_raw(numericId, prop);
      if (!resolvedRaw || typeof resolvedRaw !== "string") {
        return undefined;
      }

      let resolved = null;
      try {
        resolved = JSON.parse(resolvedRaw);
      } catch {
        return undefined;
      }

      if (!resolved || typeof resolved !== "object") {
        return undefined;
      }

      if (resolved.kind === "node") {
        return __gcScriptNodeHandle(resolved.id);
      }

      if (resolved.kind === "value") {
        return resolved.value;
      }

      if (resolved.kind === "method") {
        return (...args) => {
          const parsed = __gcInvokeTreeMethod(numericId, prop, args);
          if (!parsed) {
            return undefined;
          }
          return __gcTreeResultToJsValue(parsed);
        };
      }

      return undefined;
    },
    set(innerTarget, prop, value) {
      if (typeof prop !== "string") {
        innerTarget[prop] = value;
        return true;
      }
      if (prop in innerTarget) {
        innerTarget[prop] = value;
        return true;
      }
      return globalThis.gc.__tree_set_property_raw(
        numericId,
        prop,
        JSON.stringify(value === undefined ? null : value)
      );
    },
  });

  __gcScriptNodeProxyCache.set(numericId, proxy);
  return proxy;
};

const __gcScriptNodeSelector = (parentNodeId, childKey) => {
  const numericParentId = Number(parentNodeId);
  const selectorKey = String(childKey ?? "").trim();
  if (!Number.isFinite(numericParentId) || selectorKey.length === 0) {
    return undefined;
  }

  const cacheKey = `${Math.floor(numericParentId)}:${selectorKey}`;
  const cached = __gcScriptNodeSelectorCache.get(cacheKey);
  if (cached) {
    return cached;
  }

  const target = {
    __selectorParentId: Math.floor(numericParentId),
    __selectorKey: selectorKey,
    id() {
      const parsed = __gcInvokeTreeMethod(
        this.__selectorParentId,
        "getChild",
        [this.__selectorKey]
      );
      return parsed && parsed.kind === "node"
        ? Number(parsed.id)
        : undefined;
    },
    is(other) {
      const selfId = __gcResolveNodeId(this);
      const otherId = __gcResolveNodeId(other);
      return Number.isFinite(selfId) && Number.isFinite(otherId) && selfId === otherId;
    },
    [Symbol.toPrimitive](hint) {
      const resolvedId = __gcResolveNodeId(this);
      if (hint === "string") {
        if (Number.isFinite(resolvedId)) {
          const resolved = __gcScriptNodeHandle(resolvedId);
          if (resolved !== undefined) {
            return String(resolved);
          }
        }
        return `[${this.__selectorKey} (pending)]`;
      }
      return Number.isFinite(resolvedId) ? resolvedId : NaN;
    },
  };

  const proxy = new Proxy(target, {
    get(innerTarget, prop) {
      if (typeof prop !== "string") {
        return innerTarget[prop];
      }
      if (prop in innerTarget) {
        const member = innerTarget[prop];
        return typeof member === "function" ? member.bind(innerTarget) : member;
      }

      const resolvedId = innerTarget.id();
      if (!Number.isFinite(resolvedId)) {
        return undefined;
      }
      const resolved = __gcScriptNodeHandle(resolvedId);
      return resolved === undefined ? undefined : resolved[prop];
    },
    set(innerTarget, prop, value) {
      if (typeof prop !== "string") {
        innerTarget[prop] = value;
        return true;
      }
      if (prop in innerTarget) {
        innerTarget[prop] = value;
        return true;
      }

      const resolvedId = innerTarget.id();
      if (!Number.isFinite(resolvedId)) {
        return false;
      }
      return globalThis.gc.__tree_set_property_raw(
        resolvedId,
        prop,
        JSON.stringify(value === undefined ? null : value)
      );
    },
  });

  __gcScriptNodeSelectorCache.set(cacheKey, proxy);
  return proxy;
};

const __gcScriptEventNodeHandle = (nodeId) => {
  const numericId = Number(nodeId);
  if (!Number.isFinite(numericId)) {
    return undefined;
  }
  const normalizedId = Math.floor(numericId);
  for (const selector of __gcScriptNodeSelectorCache.values()) {
    if (!selector || typeof selector.id !== "function") {
      continue;
    }
    try {
      const selectorId = Number(selector.id());
      if (Number.isFinite(selectorId) && Math.floor(selectorId) === normalizedId) {
        return selector;
      }
    } catch {}
  }
  return __gcScriptNodeHandle(normalizedId);
};

globalThis.gc.__nodeHandle = __gcScriptNodeHandle;
globalThis.gc.__eventNodeHandle = __gcScriptEventNodeHandle;
globalThis.__gcInvokeTreeMethod = __gcInvokeTreeMethod;
globalThis.__gcTreeResultToJsValue = __gcTreeResultToJsValue;
globalThis.__gcScriptNodeSelector = __gcScriptNodeSelector;
globalThis.__gcResolveNodeId = __gcResolveNodeId;

globalThis.gc.tree = {
  root() {
    const rootId = globalThis.gc.__tree_root_id();
    if (rootId === null || rootId === undefined) {
      return undefined;
    }
    return __gcScriptNodeHandle(rootId);
  },
  host() {
    const hostId = globalThis.gc.__tree_host_id();
    if (hostId === null || hostId === undefined) {
      return undefined;
    }
    return __gcScriptNodeHandle(hostId);
  },
};

globalThis.tree = globalThis.gc.tree;
Object.defineProperty(globalThis, "root", {
  configurable: true,
  enumerable: false,
  get() {
    return globalThis.gc.tree.root();
  },
});
Object.defineProperty(globalThis, "local", {
  configurable: true,
  enumerable: false,
  get() {
    return globalThis.gc.tree.host();
  },
});
globalThis.listen = (node, config = {}) => {
  const targetId = __gcResolveNodeId(node);
  if (!Number.isFinite(targetId)) {
    return false;
  }
  const parsed = __gcInvokeTreeMethod(targetId, "listen", [config]);
  return parsed && parsed.kind === "value" ? Boolean(parsed.value) : false;
};
globalThis.unlisten = (node) => {
  const targetId = __gcResolveNodeId(node);
  if (!Number.isFinite(targetId)) {
    return false;
  }
  const parsed = __gcInvokeTreeMethod(targetId, "unlisten", []);
  return parsed && parsed.kind === "value" ? Boolean(parsed.value) : false;
};
globalThis.clearListeners = () => globalThis.gc.__listeners_clear_raw() === true;
globalThis.time = () => Number(globalThis.gc.__time_seconds_raw());
Object.defineProperty(globalThis, "deltaTime", {
  configurable: true,
  enumerable: false,
  get() {
    return Number(globalThis.gc.__delta_seconds_raw());
  },
});
const __gcFormatLogArg = (value) => {
  if (typeof value === "string") {
    return value;
  }
  if (value === undefined) {
    return "undefined";
  }
  if (typeof value === "object" && value !== null) {
    try {
      const text = String(value);
      if (text !== "[object Object]") {
        return text;
      }
    } catch {}
    try {
      const encoded = JSON.stringify(value);
      if (typeof encoded === "string") {
        return encoded;
      }
    } catch {}
  }
  return String(value);
};
const __gcFormatLogArgs = (args) =>
  Array.isArray(args) && args.length > 0
    ? args.map((value) => __gcFormatLogArg(value)).join(" ")
    : "";
globalThis.log = (...args) => globalThis.gc.log("info", __gcFormatLogArgs(args));
globalThis.success = (...args) => globalThis.gc.log("success", __gcFormatLogArgs(args));
globalThis.warn = (...args) => globalThis.gc.log("warning", __gcFormatLogArgs(args));
globalThis.error = (...args) => globalThis.gc.log("error", __gcFormatLogArgs(args));
globalThis.emit = (topic, payload) => globalThis.gc.emit(topic, payload);
"#,
            )?;
            Ok(())
        })?;
        Ok(())
    }
}
