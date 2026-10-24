//! Formula-owned admission and complete authored fragment operations.

use super::*;
use chataigne_alchemist::authoring::*;
use golden_core::{app::ProjectNode, engine::ProjectImportSpec, node::UserNodeRole};

const MAX_ITEMS: usize = 16_384;
const MAX_RECORDS: usize = 100_000;
const MAX_DEPTH: usize = 32;

mod connection;
#[cfg(test)]
mod tests;

pub(crate) fn apply_operation(
    engine: &mut crate::app::AppEngine,
    operation: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    crate::app::module::common::json::retained_weight(&payload, 32 * 1024 * 1024, 2_000_000, 128)?;
    let result = match operation {
        FORMULA_COPY_OPERATION => serde_json::to_value(copy_fragment(
            engine,
            serde_json::from_value(payload).map_err(|error| error.to_string())?,
        )?)
        .map_err(|error| error.to_string()),
        FORMULA_PASTE_OPERATION => serde_json::to_value(paste_fragment(
            engine,
            serde_json::from_value(payload).map_err(|error| error.to_string())?,
        )?)
        .map_err(|error| error.to_string()),
        FORMULA_CONNECT_OPERATION => serde_json::to_value(connection::connect(
            engine,
            serde_json::from_value(payload).map_err(|error| error.to_string())?,
        )?)
        .map_err(|error| error.to_string()),
        _ => Err(format!("unsupported Formula operation '{operation}'")),
    }?;
    crate::app::module::common::json::retained_weight(&result, 32 * 1024 * 1024, 2_000_000, 128)?;
    Ok(result)
}

fn owner(engine: &crate::app::AppEngine, expected: &FormulaOwnerDto, writing: bool) -> Result<(), String> {
    let formula = engine
        .nodes
        .get(expected.node)
        .filter(|node| {
            node.get_type() == AlchemistFormulaDefinition::NODE_TYPE && node.node_data().meta.uuid == expected.uuid
        })
        .ok_or("Formula identity changed or is unavailable")?;
    if writing
        && formula
            .node_data()
            .meta
            .tags
            .iter()
            .any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG)
    {
        return Err("Formula is read-only".into());
    }
    Ok(())
}

fn validate_records(items: &[FormulaFragmentItem]) -> Result<(), String> {
    if items.is_empty() || items.len() > MAX_ITEMS {
        return Err("Formula fragment requires 1..16384 roots".into());
    }
    let mut pending = items.iter().map(|item| (&item.document.root, 0)).collect::<Vec<_>>();
    let mut identities = HashSet::new();
    while let Some((record, depth)) = pending.pop() {
        if depth > MAX_DEPTH || identities.len() >= MAX_RECORDS {
            return Err("Formula fragment exceeds its 100000-record / 32-level limit".into());
        }
        if record.uuid.0.is_nil() || !identities.insert(record.uuid) {
            return Err("Formula fragment contains a missing or duplicate identity".into());
        }
        pending.extend(record.children.iter().map(|child| (child, depth + 1)));
    }
    for item in items {
        match item.document.root.node_type.as_str() {
            ANODE_NODE_TYPE
                if item
                    .position
                    .is_some_and(|position| position.into_iter().all(f64::is_finite)) => {}
            CONNECTION_NODE_TYPE if item.position.is_none() => {}
            _ => return Err("Formula fragment roots must be ANodes with finite placement or connections".into()),
        }
    }
    Ok(())
}

pub(super) fn copy_fragment(
    engine: &crate::app::AppEngine,
    request: FormulaCopyRequest,
) -> Result<FormulaFragment, String> {
    owner(engine, &request.formula, false)?;
    if request.nodes.is_empty() || request.nodes.len() > MAX_ITEMS {
        return Err("Select 1..16384 ANodes".into());
    }
    let snapshot = engine.process_tree_snapshot();
    let mut selected = HashSet::new();
    let mut selected_uuids = HashSet::new();
    let mut roots = Vec::new();
    for id in request.nodes {
        let node = snapshot
            .node(id)
            .filter(|node| node.parent == Some(request.formula.node) && node.node_type == ANODE_NODE_TYPE)
            .ok_or("Selected node is not an ANode of this Formula")?;
        if !selected.insert(id) {
            return Err("Selected ANode identities must be unique".into());
        }
        selected_uuids.insert(node.uuid);
        roots.push((
            id,
            Some(child_vec2(&snapshot, id, ANODE_POSITION_DECL_ID).ok_or("ANode placement is unavailable")?),
        ));
    }
    for id in snapshot.child_ids(request.formula.node) {
        if snapshot
            .node(id)
            .is_some_and(|node| node.node_type == CONNECTION_NODE_TYPE)
            && child_reference_uuid(&snapshot, id, "source_node").is_some_and(|uuid| selected_uuids.contains(&uuid))
            && child_reference_uuid(&snapshot, id, "target_node").is_some_and(|uuid| selected_uuids.contains(&uuid))
        {
            roots.push((id, None));
        }
    }
    // Bound live traversal before recursive encoding, with no full-tree rebuild per root.
    let mut pending = roots.iter().map(|(id, _)| (*id, 0)).collect::<Vec<_>>();
    let mut records = 0;
    while let Some((id, depth)) = pending.pop() {
        records += 1;
        if depth > MAX_DEPTH || records > MAX_RECORDS {
            return Err("Selected Formula forest exceeds capture limits".into());
        }
        pending.extend(snapshot.child_ids(id).into_iter().map(|id| (id, depth + 1)));
    }
    let items = roots
        .into_iter()
        .map(|(id, position)| {
            Ok(FormulaFragmentItem {
                document: engine
                    .capture_project_subtree_with(id, Node::project_encode_data)
                    .map_err(|error| error.to_string())?,
                position,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    validate_records(&items)?;
    Ok(FormulaFragment {
        version: FORMULA_FRAGMENT_VERSION,
        source_formula_uuid: request.formula.uuid,
        items,
    })
}

fn admit_node(parent: &AppNode, node: &AppNode) -> Result<(), String> {
    let generic = matches!(
        node,
        AppNode::Folder(_)
            | AppNode::Parameter(_)
            | AppNode::Curve(_)
            | AppNode::CurveRange(_)
            | AppNode::CurveKey(_)
            | AppNode::CurveEasing(_)
            | AppNode::Gradient(_)
            | AppNode::GradientStop(_)
            | AppNode::UserContext(_)
            | AppNode::UserContextFolder(_)
            | AppNode::UserContextMultiplex(_)
            | AppNode::UserContextMultiplexList(_)
    );
    let kind = node.user_item_kind();
    let formula_owned = matches!(
        kind,
        "alchemist_anode"
            | "alchemist_connection"
            | "alchemist_property"
            | "alchemist_property_manager"
            | "alchemist_property_folder"
            | "state_processor"
            | "state_processor_folder"
            | "sm_input"
            | "sm_filter"
            | "sm_output"
            | "module_command"
            | "generic_command"
    );
    let declared_part = matches!(
        node,
        AppNode::AlchemistInputSocket(_) | AppNode::AlchemistOutputSocket(_)
    ) || (kind == node.get_type()
        && (node.get_type().starts_with("processor_") || node.get_type().starts_with("state_processor_")));
    if !generic && !formula_owned && !declared_part {
        return Err(format!("{} does not belong inside a Formula fragment", node.get_type()));
    }
    if node.node_data().user_role == UserNodeRole::ItemRoot
        && !parent.user_container_accepts_item(node.get_type(), kind)
    {
        return Err(format!("{} does not admit {}", parent.get_type(), node.get_type()));
    }
    if node.get_type() == ANODE_NODE_TYPE {
        let type_id = anode_type_from_tags(&node.node_data().meta.tags).ok_or("ANode type is missing")?;
        if registry().get(&ANodeTypeId::new(type_id)).is_none() {
            return Err("ANode type is unavailable".into());
        }
    }
    Ok(())
}

pub(super) fn paste_fragment(
    engine: &mut crate::app::AppEngine,
    request: FormulaPasteRequest,
) -> Result<FormulaPasteResult, String> {
    owner(engine, &request.formula, true)?;
    if request.fragment.version != FORMULA_FRAGMENT_VERSION {
        return Err("Unsupported Formula fragment version".into());
    }
    validate_records(&request.fragment.items)?;
    let offset = request.offset.unwrap_or([2.0, 2.0]);
    if !offset.into_iter().all(f64::is_finite) {
        return Err("Formula paste offset must be finite".into());
    }
    let mut anodes = HashSet::new();
    let imports = request
        .fragment
        .items
        .into_iter()
        .map(|item| {
            let mut project = item.document;
            project.root.user_role = UserNodeRole::ItemRoot;
            let initial_params = if let Some(position) = item.position {
                anodes.insert(project.root.uuid);
                let position = [position[0] + offset[0], position[1] + offset[1]];
                if !position.into_iter().all(f64::is_finite) {
                    return Err("Formula paste placement overflowed".into());
                }
                vec![(
                    DeclId(ANODE_POSITION_DECL_ID.into()),
                    ParamValue::Vec2(position[0], position[1]),
                )]
            } else {
                Vec::new()
            };
            Ok(ProjectImportSpec {
                project,
                initial_params,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let previous = engine.nodes.get(request.formula.node).unwrap().node_data().last_child;
    let result = engine
        .import_project_subtrees_with(
            imports,
            request.formula.node,
            previous,
            AppNode::project_decode_node,
            admit_node,
        )
        .map_err(|error| error.to_string())?;
    let selected_nodes = result
        .identities
        .iter()
        .filter(|identity| anodes.contains(&identity.source_uuid))
        .map(|identity| identity.node)
        .collect::<HashSet<_>>();
    let selected_nodes = result
        .roots
        .iter()
        .copied()
        .filter(|node| selected_nodes.contains(node))
        .collect();
    // Validate the resulting domain graph inside the operation's atomic journal.
    // Socket declarations may depend on restored configuration and lifecycle materialization.
    formula_from_snapshot(&engine.process_tree_snapshot(), request.formula.node)?;
    let identities = result
        .identities
        .into_iter()
        .map(|identity| FormulaImportedIdentity {
            source_uuid: identity.source_uuid,
            uuid: identity.uuid,
            node: identity.node,
        })
        .collect();
    Ok(FormulaPasteResult {
        selected_nodes,
        identities,
    })
}
