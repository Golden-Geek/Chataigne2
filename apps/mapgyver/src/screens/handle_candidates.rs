//! Read-only, bounded Screen-handle projection; no producer or preview is allocated.
use super::{authoring, handle_links, protocol::*};
use crate::app::{AppEngine, AppNode};
use golden_core::{
    node::{Node, NodeId},
    parameter::ParameterControlMode,
};
use std::collections::HashMap;

struct SelectedCorners {
    groups: Vec<ScreenHandleGroup>,
    bindings: Vec<Vec<NodeId>>,
}

fn selected_corners(engine: &AppEngine, reference: SurfaceReference) -> Result<SelectedCorners, String> {
    let mut groups = Vec::new();
    let mut bindings = Vec::with_capacity(4);
    for corner in 0..4 {
        let param = handle_links::parameter(engine, ScreenCorner { reference, corner })?;
        let group = engine.parameter_binding_group(param);
        let valid = handle_links::validate_members(engine, &group, false).is_ok();
        let missing = engine
            .nodes
            .get(param)
            .and_then(|node| node.engine_param_control_state())
            .is_some_and(|state| state.mode == ParameterControlMode::Binding)
            && engine.parameter_binding_target(param).is_none();
        if valid && (group.len() > 1 || missing) {
            groups.push(ScreenHandleGroup {
                corner,
                members: group
                    .iter()
                    .map(|&id| handle_links::corner(engine, id))
                    .collect::<Result<_, _>>()?,
                missing,
            });
        }
        bindings.push(group);
    }
    Ok(SelectedCorners { groups, bindings })
}

pub(super) fn linked_corners(
    engine: &AppEngine,
    reference: SurfaceReference,
) -> Result<Vec<ScreenHandleGroup>, String> {
    Ok(selected_corners(engine, reference)?.groups)
}

pub(super) fn inspect(engine: &AppEngine, query: ScreenHandleQuery) -> Result<ScreenHandlePage, String> {
    let document = engine
        .nodes
        .get(engine.root)
        .ok_or("project root is missing")?
        .node_data()
        .meta
        .uuid;
    let mut page = ScreenHandlePage {
        at: engine.time,
        document_matches: document == query.document,
        groups: Vec::new(),
        candidates: Vec::new(),
        next: None,
    };
    if !page.document_matches || authoring::surface_id(engine, query.reference).is_err() {
        return Ok(page);
    }
    let SelectedCorners { groups, bindings } = selected_corners(engine, query.reference)?;
    page.groups = groups;
    if !query.include_candidates {
        return Ok(page);
    }
    let mut group_cache = HashMap::<NodeId, Vec<NodeId>>::new();
    let screen = authoring::screen_id(engine, query.reference.screen)?;
    if let Some(cursor) = query.cursor.and_then(|uuid| engine.node_id_by_uuid(uuid))
        && engine
            .nodes
            .get(cursor)
            .is_none_or(|node| node.node_data().parent != Some(screen))
    {
        return Err("Screen-handle cursor is outside the selected Screen".into());
    }
    let cursor = query.cursor.filter(|uuid| engine.node_id_by_uuid(*uuid).is_some());
    let mut next = engine.nodes.get(screen).and_then(|node| node.node_data().first_child);
    let mut surfaced = 0;
    let mut started = cursor.is_none();
    let mut visited = 0;
    while let Some(id) = next {
        if surfaced == super::editor::MAX_EDITOR_SURFACES {
            break;
        }
        if visited == 512 || page.candidates.len() == 64 {
            page.next = engine.nodes.get(id).map(|node| node.node_data().meta.uuid);
            break;
        }
        visited += 1;
        let node = engine.nodes.get(id).ok_or("Screen handle changed")?;
        next = node.node_data().next_sibling;
        if !started {
            started = cursor == Some(node.node_data().meta.uuid);
        }
        let AppNode::Surface(surface) = node else { continue };
        surfaced += 1;
        if !started {
            continue;
        }
        let Some(AppNode::Screen(screen)) = node.node_data().parent.and_then(|parent| engine.nodes.get(parent)) else {
            continue;
        };
        let reference = SurfaceReference {
            screen: ScreenId(screen.node_data().meta.uuid),
            surface: SurfaceId(surface.node_data().meta.uuid),
        };
        if reference == query.reference {
            continue;
        }
        let definition = surface.definition();
        for corner in 0..4 {
            let param = surface.corner_parameter(corner)?;
            let target_group = if let Some(group) = group_cache.get(&param) {
                group.clone()
            } else {
                let group = engine.parameter_binding_group(param);
                for &member in &group {
                    group_cache.insert(member, group.clone());
                }
                group
            };
            let linkable = std::array::from_fn(|index| {
                let source = &bindings[index];
                if source.contains(&param)
                    || !handle_links::directly_linkable(engine, source)
                    || !handle_links::directly_linkable(engine, &target_group)
                {
                    return false;
                }
                let mut merged = source.clone();
                merged.extend_from_slice(&target_group);
                merged.sort_by_key(|node| node.0);
                merged.dedup();
                handle_links::validate_members(engine, &merged, true).is_ok()
            });
            page.candidates.push(ScreenHandleCandidate {
                member: ScreenCorner { reference, corner },
                parameter: engine.nodes.get(param).unwrap().node_data().meta.uuid,
                point: definition.quad.corners[usize::from(corner)],
                label: format!(
                    "{} / {} / {}",
                    screen.node_data().meta.label,
                    surface.node_data().meta.label,
                    corner + 1
                ),
                linkable,
            });
        }
    }
    Ok(page)
}
