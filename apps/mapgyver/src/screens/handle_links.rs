//! Screen corners share their real Vec2 binding controls, with no parallel link state.
use super::{
    authoring,
    protocol::{ScreenCorner, ScreenId, SurfaceId, SurfaceReference},
};
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    node::{Node, NodeId},
    parameter::{ParameterControlMode, ParameterControlSpec, ParameterControlState},
};
use std::collections::HashSet;

// Linking is an explicit user action, never a per-pointer full-document scan.
const MAX_LINKED_CORNERS: usize = 256;

pub(super) fn parameter(engine: &AppEngine, corner: ScreenCorner) -> Result<NodeId, String> {
    let id = authoring::surface_id(engine, corner.reference)?;
    let Some(AppNode::Surface(node)) = engine.nodes.get(id) else {
        unreachable!()
    };
    node.corner_parameter(corner.corner)
}

pub(super) fn corner(engine: &AppEngine, param: NodeId) -> Result<ScreenCorner, String> {
    let parent = engine
        .nodes
        .get(param)
        .and_then(|node| node.node_data().parent)
        .and_then(|folder| engine.nodes.get(folder)?.node_data().parent)
        .ok_or("linked Parameter has no Surface")?;
    let Some(AppNode::Surface(surface)) = engine.nodes.get(parent) else {
        return Err("existing binding includes a non-Surface Parameter".into());
    };
    let screen = surface
        .node_data()
        .parent
        .and_then(|id| engine.nodes.get(id))
        .ok_or("linked Surface has no Screen")?;
    let reference = SurfaceReference {
        screen: ScreenId(screen.node_data().meta.uuid),
        surface: SurfaceId(surface.node_data().meta.uuid),
    };
    authoring::surface_id(engine, reference)?;
    let corner = (0..4)
        .find(|&index| surface.corner_parameter(index) == Ok(param))
        .ok_or("existing binding includes a non-corner Parameter")?;
    Ok(ScreenCorner { reference, corner })
}

pub(super) fn validate_members(engine: &AppEngine, members: &[NodeId], linking: bool) -> Result<(), String> {
    if members.len() > MAX_LINKED_CORNERS {
        return Err("a Screen-handle group supports at most 256 corners".into());
    }
    let mut surfaces = HashSet::new();
    let mut screen = None;
    for &member in members {
        let member = corner(engine, member)?;
        if linking {
            if screen.is_some_and(|screen| screen != member.reference.screen) {
                return Err("linked corners must belong to the same Screen".into());
            }
            screen = Some(member.reference.screen);
            if !surfaces.insert(member.reference.surface) {
                return Err("linked corners must belong to different Surfaces".into());
            }
        }
    }
    Ok(())
}

pub(super) fn directly_linkable(engine: &AppEngine, members: &[NodeId]) -> bool {
    members.len() <= MAX_LINKED_CORNERS
        && members.iter().all(|&id| {
            let Some(state) = engine.nodes.get(id).and_then(|node| node.engine_param_control_state()) else {
                return false;
            };
            match state.mode {
                ParameterControlMode::Manual => true,
                ParameterControlMode::Binding => engine
                    .parameter_binding_target(id)
                    .is_some_and(|(_, projection)| projection.is_none()),
                _ => false,
            }
        })
}

pub(super) fn link(engine: &mut AppEngine, source: ScreenCorner, target: ScreenCorner) -> Result<(), String> {
    let source = parameter(engine, source)?;
    let target = parameter(engine, target)?;
    let mut members = engine.parameter_binding_group(source);
    let target_members = engine.parameter_binding_group(target);
    if members.contains(&target) {
        return Ok(());
    }
    members.extend(target_members);
    members.sort_by_key(|node| node.0);
    members.dedup();
    validate_members(engine, &members, true)?;
    // Generic controls reject other control modes, projected/dangling bindings and
    // incompatible constraints rather than silently replacing the user's controls.
    engine.queue_parameter_binding_group(&members, target)
}

pub(super) fn unlink(engine: &mut AppEngine, member: ScreenCorner) -> Result<(), String> {
    let param = parameter(engine, member)?;
    let members = engine.parameter_binding_group(param);
    // Unlink also permits cleanup of existing generic or formerly authored groups.
    validate_members(engine, &members, false)?;
    for param in members {
        engine.edits.push(Edit::SetParamControlState {
            node: param,
            state: ParameterControlState::new(ParameterControlMode::Manual, ParameterControlSpec::Manual),
        });
    }
    Ok(())
}
