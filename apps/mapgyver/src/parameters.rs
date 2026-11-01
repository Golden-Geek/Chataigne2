//! Shared app parameter presentation; authored values remain Golden Parameter nodes.
use golden_core::parameter::{ParamValue, ParameterEnumOption};

pub(crate) mod reference_cycle;

pub(crate) fn reject_opaque(data: &serde_json::Value) -> Result<(), String> {
    if data.is_null() {
        Ok(())
    } else {
        Err("Authored settings use Parameter children; opaque definition records are unsupported".into())
    }
}

pub(crate) fn is_parameter_folder(node: &dyn golden_core::node::Node) -> bool {
    node.get_type() == "folder" && node.node_data().meta.decl_id.0 == "parameters"
}

pub(crate) fn has_domain_children(engine: &crate::app::AppEngine, parent: golden_core::node::NodeId) -> bool {
    use golden_core::node::Node;
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(id) = child {
        let Some(node) = engine.nodes.get(id) else { return true };
        if !is_parameter_folder(node) {
            return true;
        }
        child = node.node_data().next_sibling;
    }
    false
}

pub(crate) fn domain_children(
    engine: &crate::app::AppEngine,
    parent: golden_core::node::NodeId,
) -> Result<Vec<golden_core::node::NodeId>, String> {
    use golden_core::node::Node;
    let mut child = engine
        .nodes
        .get(parent)
        .ok_or("missing parent")?
        .node_data()
        .first_child;
    let mut result = Vec::new();
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("missing child")?;
        if !is_parameter_folder(node) {
            result.push(id);
        }
        child = node.node_data().next_sibling;
    }
    Ok(result)
}

pub(crate) fn enum_options(values: &[&str]) -> Vec<ParameterEnumOption> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| ParameterEnumOption {
            variant_id: (*value).into(),
            value: ParamValue::Enum((*value).into()),
            label: (*value).into(),
            tags: vec![],
            ordering: Some(index as i32),
        })
        .collect()
}
