//! Builds an editable, independently identified project Formula from an authored recipe.

use super::*;

impl FormulaCatalog {
    pub(crate) fn project_formula_copy_tree(
        snapshot: &ProcessTreeSnapshot,
        source: NodeId,
        label: &str,
    ) -> Result<NodeTree, BuiltinFormulaLoadError> {
        let exported = Self::export_formula_json(snapshot, source).ok_or_else(|| {
            BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: "source Formula could not be serialized".to_owned(),
            }
        })?;
        let mut document = decode_exported_formula_tree(Path::new("<converted Mapping>"), &exported)?;
        let mut remapped = HashMap::new();
        for node in &document.nodes {
            collect_fresh_uuids(node, &mut remapped);
        }
        let root = document.nodes.first_mut().ok_or_else(|| {
            BuiltinFormulaLoadError::InvalidExportedFormula {
                reason: "source Formula has no root".to_owned(),
            }
        })?;
        root.label = label.to_owned();
        root.meta.label = Some(label.to_owned());
        remap_editable_tree(root, &remapped)?;
        let mut tree = document.into_formula_node_tree(None, false, None, None)?;
        mark_authored_graph_items(&mut tree);
        Ok(tree)
    }
}

fn collect_fresh_uuids(node: &ExportedNode, remapped: &mut HashMap<Uuid, Uuid>) {
    remapped.insert(node.source_uuid, Uuid::new_v4());
    for child in &node.children {
        collect_fresh_uuids(child, remapped);
    }
}

fn remap_editable_tree(
    node: &mut ExportedNode,
    remapped: &HashMap<Uuid, Uuid>,
) -> Result<(), BuiltinFormulaLoadError> {
    let source_uuid = node.source_uuid;
    node.source_uuid = remapped[&node.source_uuid];
    node.meta.tags.retain(|tag| {
        tag != FORMULA_EXTERNAL_READ_ONLY_TAG
            && tag != FORMULA_EXTERNAL_FILE_TAG
            && tag != PREFERENCES_APP_DATA_TAG
            && !tag.starts_with(FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX)
            && !tag.starts_with(BUILTIN_FORMULA_CONTENT_TAG_PREFIX)
    });
    if matches!(
        node.node_type.as_str(),
        super::super::PROPERTY_NODE_TYPE
            | super::super::PROPERTY_MANAGER_NODE_TYPE
            | super::super::PROPERTY_FOLDER_NODE_TYPE
    ) {
        node.meta.tags.push(format!(
            "{}{source_uuid}",
            super::super::PROCESSOR_SURFACE_IDENTITY_TAG_PREFIX
        ));
    }
    if node.node_type == "alchemist_anode" {
        node.meta.can_be_disabled = true;
    }
    if let Some(param) = node.data.get_mut("param") {
        for field in ["value", "default_value"] {
            if let Some(value) = param.get_mut(field) {
                remap_param_reference(value, remapped);
            }
        }
        if node.decl_id == "managed_regions_json" {
            let Some(value) = param.get_mut("value").and_then(|value| value.get_mut("value")) else {
                return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
                    reason: "Mapping region metadata is missing".to_owned(),
                });
            };
            let Some(metadata) = value.as_str() else {
                return Err(BuiltinFormulaLoadError::InvalidExportedFormula {
                    reason: "Mapping region metadata is not a string".to_owned(),
                });
            };
            let mut decoded: JsonValue = serde_json::from_str(metadata).map_err(BuiltinFormulaLoadError::Decode)?;
            remap_json_uuids(&mut decoded, remapped);
            *value = JsonValue::String(serde_json::to_string(&decoded).map_err(BuiltinFormulaLoadError::Decode)?);
        }
        let intrinsic_read_only = matches!(
            node.decl_id.as_str(),
            "is_valid" | "diagnostics_json" | "managed_regions_json" | "anode_type"
        );
        if let Some(fields) = param.as_object_mut() {
            fields.insert("read_only".to_owned(), JsonValue::Bool(intrinsic_read_only));
            if node.decl_id == "managed_regions_json" {
                fields.insert("persist_read_only_value".to_owned(), JsonValue::Bool(true));
            }
        }
    }
    for child in &mut node.children {
        remap_editable_tree(child, remapped)?;
    }
    Ok(())
}

fn remap_param_reference(value: &mut JsonValue, remapped: &HashMap<Uuid, Uuid>) {
    if value.get("kind").and_then(JsonValue::as_str) != Some("reference") {
        return;
    }
    let Some(uuid) = value
        .get("uuid")
        .and_then(JsonValue::as_str)
        .and_then(|text| Uuid::parse_str(text).ok())
    else {
        return;
    };
    if let Some(replacement) = remapped.get(&uuid) {
        value["uuid"] = JsonValue::String(replacement.to_string());
    }
}

fn remap_json_uuids(value: &mut JsonValue, remapped: &HashMap<Uuid, Uuid>) {
    match value {
        JsonValue::String(text) => {
            if let Ok(uuid) = Uuid::parse_str(text) {
                if let Some(replacement) = remapped.get(&uuid) {
                    *text = replacement.to_string();
                }
            }
        }
        JsonValue::Array(items) => {
            for item in items {
                remap_json_uuids(item, remapped);
            }
        }
        JsonValue::Object(fields) => {
            if fields
                .get("uuid")
                .and_then(JsonValue::as_str)
                .and_then(|text| Uuid::parse_str(text).ok())
                .is_some_and(|uuid| remapped.contains_key(&uuid))
            {
                fields.remove("cached_id");
            }
            for item in fields.values_mut() {
                remap_json_uuids(item, remapped);
            }
        }
        _ => {}
    }
}

fn mark_authored_graph_items(tree: &mut NodeTree) {
    for child in &mut tree.children {
        if matches!(
            child.node.get_type(),
            "alchemist_anode"
                | "alchemist_connection"
                | "alchemist_property_manager"
                | "alchemist_property_folder"
                | "alchemist_property"
        ) {
            child.user_role = golden_core::node::UserNodeRole::ItemRoot;
        }
        mark_authored_graph_items(child);
    }
}
