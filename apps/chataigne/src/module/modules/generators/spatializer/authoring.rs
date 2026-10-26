//! Endpoint mutation policy shared by native, transport and embedded-script app-operation callers.
use super::*;
use crate::app::{AppEngine, AppNode};
use chataigne_spatializer_protocol::*;
use golden_core::{edit::Edit, parameter::ParameterControlMode};

#[cfg(test)]
mod tests;

fn child(engine: &AppEngine, owner: NodeId, declaration: &str) -> Result<NodeId, String> {
    let mut next = engine.nodes.get(owner).and_then(|node| node.node_data().first_child);
    // Spatializer declarations are bounded; malformed user structure must not turn an intent into a full-tree scan.
    for _ in 0..32 {
        let Some(id) = next else { break };
        let node = engine.nodes.get(id).ok_or("Spatializer field was retired")?;
        let key = &node.node_data().meta.decl_id.0;
        if key == declaration || key.rsplit('/').next() == Some(declaration) {
            return Ok(id);
        }
        next = node.node_data().next_sibling;
    }
    Err(format!(
        "Spatializer field '{declaration}' is unavailable or structure exceeds its bound"
    ))
}

fn parameter(engine: &AppEngine, node: NodeId) -> Result<golden_core::parameter::ParameterSnapshot, String> {
    let parameter = engine
        .nodes
        .get(node)
        .and_then(Node::engine_param_snapshot)
        .ok_or("Spatializer field is not a parameter")?;
    if parameter.read_only || parameter.control.mode != ParameterControlMode::Manual {
        return Err("Spatializer field is read-only or externally controlled".into());
    }
    Ok(parameter)
}

fn endpoint(engine: &AppEngine, parameters: NodeId, identity: SpatializerIdentity) -> Result<(), String> {
    let node = engine
        .nodes
        .get(identity.node)
        .filter(|node| node.node_data().meta.uuid == identity.uuid)
        .ok_or("Spatializer endpoint identity changed")?;
    let list = match node.get_type() {
        SpatializerSourceItem::NODE_TYPE => "sources",
        SpatializerTargetItem::NODE_TYPE => "targets",
        _ => return Err("Spatializer target is not an endpoint".into()),
    };
    if node.node_data().parent != Some(child(engine, parameters, list)?) {
        return Err("Spatializer endpoint belongs to another module".into());
    }
    Ok(())
}

fn enumeration(engine: &AppEngine, parameters: NodeId, key: &str) -> Result<String, String> {
    match engine
        .nodes
        .get(child(engine, parameters, key)?)
        .and_then(Node::engine_param_snapshot)
        .map(|p| p.value)
    {
        Some(ParamValue::Enum(value)) => Ok(value),
        _ => Err(format!("Spatializer configuration '{key}' is unavailable")),
    }
}

fn dimension(engine: &AppEngine, parameters: NodeId) -> Result<SpatializerDimension, String> {
    match enumeration(engine, parameters, "dimensions")?.as_str() {
        SPATIALIZER_DIMENSION_2D => Ok(SpatializerDimension::Two),
        SPATIALIZER_DIMENSION_3D => Ok(SpatializerDimension::Three),
        _ => Err("Spatializer dimension is invalid".into()),
    }
}

fn position_edit(
    engine: &AppEngine,
    parameters: NodeId,
    item: SpatializerIdentity,
    position: [f64; 2],
    relative: bool,
) -> Result<Edit, String> {
    endpoint(engine, parameters, item)?;
    if !position.into_iter().all(f64::is_finite) {
        return Err("Spatializer coordinates must be finite".into());
    }
    let dimension = dimension(engine, parameters)?;
    let node = child(engine, item.node, dimension.position_decl_id())?;
    let parameter = parameter(engine, node)?;
    let offset = |old: f64, requested: f64| if relative { old + requested } else { requested };
    let value = match (dimension, parameter.value) {
        (SpatializerDimension::Two, ParamValue::Vec2(x, y)) => {
            ParamValue::Vec2(offset(x, position[0]), offset(y, position[1]))
        }
        (SpatializerDimension::Three, ParamValue::Vec3(x, height, z)) => {
            ParamValue::Vec3(offset(x, position[0]), height, offset(z, position[1]))
        }
        _ => return Err("Spatializer position kind does not match its current dimension".into()),
    };
    if !match value {
        ParamValue::Vec2(x, y) => [x, y].into_iter().all(f64::is_finite),
        ParamValue::Vec3(x, y, z) => [x, y, z].into_iter().all(f64::is_finite),
        _ => false,
    } {
        return Err("Spatializer position would overflow".into());
    }
    Ok(Edit::SetParam {
        node,
        value,
        behaviour: parameter.event_behaviour,
    })
}

fn radius_edit(
    engine: &AppEngine,
    parameters: NodeId,
    item: SpatializerIdentity,
    field: SpatializerRadius,
    value: f64,
    relative: bool,
) -> Result<Edit, String> {
    endpoint(engine, parameters, item)?;
    if !value.is_finite() {
        return Err("Spatializer radius must be finite".into());
    }
    let key = match field {
        SpatializerRadius::Radius => RADIUS_DECL_ID,
        SpatializerRadius::FreezeRadius => FREEZE_RADIUS_DECL_ID,
    };
    let node = child(engine, item.node, key)?;
    let parameter = parameter(engine, node)?;
    let ParamValue::Float(previous) = parameter.value else {
        return Err("Spatializer radius is not numeric".into());
    };
    let value = if relative { (previous + value).max(0.0) } else { value };
    if !value.is_finite() || value < 0.0 {
        return Err("Spatializer radius must be finite and nonnegative".into());
    }
    Ok(Edit::SetParam {
        node,
        value: ParamValue::Float(value),
        behaviour: parameter.event_behaviour,
    })
}

pub(super) fn apply(
    engine: &mut AppEngine,
    operation: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    if operation != SPATIALIZER_EDIT_OPERATION {
        return Err("Unsupported Spatializer operation".into());
    }
    crate::app::module::common::json::retained_weight(&payload, 2 * 1024 * 1024, 150_000, 12)?;
    let request: SpatializerEditRequest = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let module = engine
        .nodes
        .get(request.module.node)
        .filter(|node| node.node_data().meta.uuid == request.module.uuid)
        .ok_or("Spatializer module identity changed")?;
    let AppNode::SpatializerModule(module) = module else {
        return Err("Owner is not a Spatializer module".into());
    };
    let parameters = module
        .base
        .parameters_id()
        .ok_or("Spatializer parameters are unavailable")?;
    if engine
        .nodes
        .get(parameters)
        .is_none_or(|node| node.node_data().parent != Some(request.module.node))
    {
        return Err("Spatializer parameters moved away".into());
    }
    let expected_space = match dimension(engine, parameters)? {
        SpatializerDimension::Two => SpatializerSpace::Xy,
        SpatializerDimension::Three => SpatializerSpace::Xz,
    };
    if request.space != expected_space {
        return Err("Spatializer coordinate space changed before dispatch".into());
    }
    let mut edits = Vec::new();
    let mut result = SpatializerEditResult {
        endpoints: Vec::new(),
        created: None,
    };
    match request.edit {
        SpatializerEdit::Move { moves } => {
            if moves.is_empty() || moves.len() > 4096 {
                return Err("Move requires 1..4096 Spatializer endpoints".into());
            }
            let mut seen = HashSet::new();
            for item in moves {
                if !seen.insert(item.endpoint.node) {
                    return Err("Spatializer move endpoints must be unique".into());
                }
                edits.push(position_edit(engine, parameters, item.endpoint, item.position, false)?);
                result.endpoints.push(item.endpoint);
            }
        }
        SpatializerEdit::Nudge { endpoint, delta } => {
            edits.push(position_edit(engine, parameters, endpoint, delta, true)?);
            result.endpoints.push(endpoint);
        }
        SpatializerEdit::SetRadius { endpoint, field, value } => {
            edits.push(radius_edit(engine, parameters, endpoint, field, value, false)?);
            result.endpoints.push(endpoint);
        }
        SpatializerEdit::AdjustRadius { endpoint, field, delta } => {
            edits.push(radius_edit(engine, parameters, endpoint, field, delta, true)?);
            result.endpoints.push(endpoint);
        }
        SpatializerEdit::CreateTarget { position } => {
            if !position.into_iter().all(f64::is_finite) {
                return Err("Spatializer coordinates must be finite".into());
            }
            let parent = child(engine, parameters, "targets")?;
            let dimension = dimension(engine, parameters)?;
            let mode = match enumeration(engine, parameters, "mode")?.as_str() {
                SPATIALIZER_MODE_VORONOI => SpatializerMode::Voronoi,
                SPATIALIZER_MODE_TARGET_RADIUS => SpatializerMode::TargetRadius,
                SPATIALIZER_MODE_SOURCE_RADIUS => SpatializerMode::SourceRadius,
                SPATIALIZER_MODE_OVERLAP => SpatializerMode::Overlap,
                _ => return Err("Spatializer mode is invalid".into()),
            };
            let target = SpatializerTargetItem::new();
            let uuid = target.node_data().meta.uuid;
            let point = match dimension {
                SpatializerDimension::Two => SpatialPoint::new(position[0], position[1], 0.0),
                SpatializerDimension::Three => SpatialPoint::new(position[0], 0.0, position[1]),
            };
            let mut tree = NodeTree::new(target).as_user_item();
            tree.push_child(NodeTree::new(position_param(dimension, point)));
            if mode.target_radius_required() {
                tree.push_child(NodeTree::new(radius_param(1.0)));
            }
            if mode.target_freeze_radius_required() {
                tree.push_child(NodeTree::new(freeze_radius_param(0.0)));
            }
            engine.add_user_item_tree(tree, Some(parent));
            engine.apply_edits().map_err(|error| error.to_string())?;
            let identity = SpatializerIdentity {
                node: engine
                    .node_id_by_uuid(uuid)
                    .ok_or("Created Spatializer target was retired")?,
                uuid,
            };
            result.endpoints.push(identity);
            result.created = Some(identity);
        }
    }
    for edit in edits {
        engine.edits.push(edit);
    }
    engine.apply_edits().map_err(|error| error.to_string())?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}
