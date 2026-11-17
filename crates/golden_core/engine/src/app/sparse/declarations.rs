//! Pure constructor inputs for declaration-only sparse baselines.
use std::collections::BTreeMap;

use crate::{
    app::ProjectNode,
    engine::{ProjectNodeRecord, ProjectPersistenceError},
    node::UserNodeRole,
    parameter::ParamValue,
};

pub(super) type DeclarationInputs = BTreeMap<String, Option<ParamValue>>;

pub(super) fn prepare<T: ProjectNode>(
    node: &mut T,
    record: &ProjectNodeRecord,
) -> Result<DeclarationInputs, ProjectPersistenceError> {
    let mut inputs = BTreeMap::new();
    node.project_prepare_declarations(&mut |declaration| {
        let value = read::<T>(&record.children, declaration)?;
        inputs.insert(declaration.to_owned(), value.clone());
        Ok(value)
    })
    .map_err(|message| ProjectPersistenceError::Codec {
        node_type: record.node_type.clone(),
        message,
    })?;
    Ok(inputs)
}

fn read<T: ProjectNode>(records: &[ProjectNodeRecord], declaration: &str) -> Result<Option<ParamValue>, String> {
    // Stay inside this owner's declared folders, never traverse a user collection or
    // another declared node's independent Parameter namespace.
    let mut pending = vec![records];
    let mut matches = Vec::new();
    while let Some(records) = pending.pop() {
        for record in records {
            if record.user_role == UserNodeRole::ItemRoot {
                continue;
            }
            if record.meta.decl_id.as_ref().is_some_and(|id| id.0 == declaration) {
                matches.push(record);
            } else if record.node_type == "folder" {
                pending.push(&record.children);
            }
        }
    }
    let Some(record) = matches
        .iter()
        .copied()
        .find(|record| super::sparse_overlay_has_payload(record))
        .or_else(|| matches.first().copied())
    else {
        return Ok(None);
    };
    let mut parameter = T::project_create_node(&record.node_type)
        .filter(|node| node.engine_param_snapshot().is_some())
        .ok_or_else(|| format!("declaration input '{declaration}' must be a registered Parameter"))?;
    parameter.project_decode_data(record.data.as_ref().unwrap_or(&serde_json::Value::Null))?;
    Ok(parameter.engine_param_snapshot().map(|snapshot| snapshot.value))
}
