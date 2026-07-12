use std::path::PathBuf;

use golden_core::{
    events::CustomEvent,
    node::{NodeId, NodeScriptDescriptor},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
    script::ScriptNodeConfig,
};
use serde_json::Value as JsonValue;

pub(crate) const MODULE_SCRIPT_CALLBACK_TOPIC: &str = "chataigne.module.script.callback";

pub(crate) const MODULE_CONNECTION_CHANGED_CALLBACK: &str = "moduleConnectionChanged";
pub(crate) const MODULE_PARAMETER_CHANGED_CALLBACK: &str = "moduleModuleParameterChanged";
pub(crate) const MODULE_VALUE_CHANGED_CALLBACK: &str = "moduleModuleValueChanged";

const MODULE_SCRIPT_TEMPLATE_DIR: &str = "src/module/script_templates";

pub(crate) fn descriptor_for_node(
    node_data: &golden_core::node::NodeData,
    node_type: &str,
    methods: &[&str],
) -> NodeScriptDescriptor {
    let mut descriptor = NodeScriptDescriptor::for_node(node_data, node_type);
    descriptor.add_methods(methods.iter().copied());
    descriptor
}

pub(crate) fn module_script_config(host_node_type: &str) -> ScriptNodeConfig {
    module_script_config_from_candidates(module_script_template_host_candidates(host_node_type), host_node_type)
}

pub(crate) fn module_script_config_for_node(
    node_data: &golden_core::node::NodeData,
    host_node_type: &str,
) -> ScriptNodeConfig {
    if node_data.meta.decl_id.0.trim().is_empty() {
        return module_script_config(host_node_type);
    }

    let candidates = module_script_template_candidates(node_data, host_node_type);

    module_script_config_from_candidates(candidates, host_node_type)
}

fn module_script_config_from_candidates(candidates: Vec<&str>, host_node_type: &str) -> ScriptNodeConfig {
    let template_dir = module_script_template_dir();

    for candidate in &candidates {
        if let Some(config) = ScriptNodeConfig::try_for_host_node_type_in_template_dir(candidate, &template_dir) {
            return config;
        }
    }

    candidates
        .first()
        .map(|candidate| ScriptNodeConfig::for_host_node_type(candidate))
        .unwrap_or_else(|| ScriptNodeConfig::for_host_node_type(host_node_type))
}

fn module_script_template_host_candidates(host_node_type: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    push_template_candidate(&mut candidates, host_node_type.trim());
    push_template_candidate_aliases(&mut candidates, host_node_type.trim());
    candidates
}

fn module_script_template_candidates<'a>(
    node_data: &'a golden_core::node::NodeData,
    host_node_type: &'a str,
) -> Vec<&'a str> {
    let mut candidates = module_script_template_host_candidates(host_node_type);

    let decl_id = node_data.meta.decl_id.0.trim();
    if !decl_id.is_empty() {
        let decl_tail = decl_id.rsplit('/').next().unwrap_or(decl_id);
        push_template_candidate(&mut candidates, decl_tail);
        push_template_candidate_aliases(&mut candidates, decl_tail);
    }

    candidates
}

fn push_template_candidate<'a>(candidates: &mut Vec<&'a str>, candidate: &'a str) {
    let candidate = candidate.trim();
    if candidate.is_empty() || candidates.contains(&candidate) {
        return;
    }
    candidates.push(candidate);
}

fn push_template_candidate_aliases<'a>(candidates: &mut Vec<&'a str>, candidate: &'a str) {
    if let Some(module_type) = candidate.strip_suffix("_module_base") {
        push_template_candidate(candidates, module_type);
    }
    if let Some(module_type) = candidate.strip_suffix("_module") {
        push_template_candidate(candidates, module_type);
    }
    if let Some(base_type) = candidate.strip_suffix("_base") {
        push_template_candidate(candidates, base_type);
    }
}

fn module_script_template_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(MODULE_SCRIPT_TEMPLATE_DIR)
}

pub(crate) fn emit_script_callback(
    ctx: &mut ProcessCtx,
    module_id: NodeId,
    callback: &str,
    args: Vec<JsonValue>,
) {
    ctx.emit_custom_event(CustomEvent::new(
        MODULE_SCRIPT_CALLBACK_TOPIC,
        Some(module_id),
        serde_json::json!({
            "callback": callback,
            "args": args,
        }),
    ));
}

pub(crate) struct ModuleParamCallbackRoots {
    connection: Option<NodeId>,
    parameters: Option<NodeId>,
    values: Option<NodeId>,
}

impl ModuleParamCallbackRoots {
    pub(crate) const fn new(
        connection: Option<NodeId>,
        parameters: Option<NodeId>,
        values: Option<NodeId>,
    ) -> Self {
        Self {
            connection,
            parameters,
            values,
        }
    }
}

pub(crate) fn emit_standard_module_param_callback(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module_id: NodeId,
    roots: ModuleParamCallbackRoots,
    param: NodeId,
    old_value: &ParamValue,
) {
    let callback = if roots
        .connection
        .is_some_and(|root| is_descendant_or_self(snapshot, param, root))
    {
        MODULE_CONNECTION_CHANGED_CALLBACK
    } else if roots
        .parameters
        .is_some_and(|root| is_descendant_or_self(snapshot, param, root))
    {
        MODULE_PARAMETER_CHANGED_CALLBACK
    } else if roots
        .values
        .is_some_and(|root| is_descendant_or_self(snapshot, param, root))
    {
        MODULE_VALUE_CHANGED_CALLBACK
    } else {
        return;
    };

    let Some(new_value) = snapshot
        .node(param)
        .and_then(|node| node.param_value.as_ref())
        .map(ParamValue::to_script_json)
    else {
        return;
    };

    let path = relative_path(snapshot, module_id, param).unwrap_or_default();
    emit_script_callback(
        ctx,
        module_id,
        callback,
        vec![
            node_arg(param),
            new_value.clone(),
            old_value.to_script_json(),
            serde_json::json!({
                "path": path,
                "newValue": new_value,
                "oldValue": old_value.to_script_json(),
            }),
        ],
    );
}

pub(crate) fn node_arg(node: NodeId) -> JsonValue {
    serde_json::json!({ "kind": "node", "id": node.0 })
}

pub(crate) fn bytes_arg(bytes: &[u8]) -> JsonValue {
    JsonValue::Array(
        bytes
            .iter()
            .map(|byte| JsonValue::Number(serde_json::Number::from(*byte)))
            .collect(),
    )
}

pub(crate) fn param_values_arg(values: &[ParamValue]) -> JsonValue {
    JsonValue::Array(values.iter().map(ParamValue::to_script_json).collect())
}

fn is_descendant_or_self(snapshot: &ProcessTreeSnapshot, node: NodeId, ancestor: NodeId) -> bool {
    let mut current = Some(node);
    while let Some(current_id) = current {
        if current_id == ancestor {
            return true;
        }
        current = snapshot.node(current_id).and_then(|node| node.parent);
    }
    false
}

fn relative_path(snapshot: &ProcessTreeSnapshot, root: NodeId, node: NodeId) -> Option<String> {
    let mut parts = Vec::new();
    let mut current = node;
    while current != root {
        let current_node = snapshot.node(current)?;
        parts.push(path_segment(current_node.decl_id.as_str(), current_node.label.as_str()));
        current = current_node.parent?;
    }

    parts.reverse();
    Some(parts.join("/"))
}

fn path_segment(decl_id: &str, label: &str) -> String {
    decl_id
        .rsplit('/')
        .next()
        .filter(|segment| !segment.trim().is_empty())
        .unwrap_or(label)
        .to_string()
}

#[cfg(test)]
mod tests;
