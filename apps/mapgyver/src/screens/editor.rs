//! Per-panel ownership and bounded geometry. Presentation never chooses a domain default.
use super::{authoring, protocol::*};
use crate::app::{AppEngine, AppNode};
use golden_core::node::{Node, NodeUuid};

fn owner(engine: &AppEngine, selected: Option<NodeUuid>) -> Option<(ScreenId, Option<SurfaceReference>)> {
    let mut next = selected.and_then(|uuid| engine.node_id_by_uuid(uuid));
    for _ in 0..64 {
        let id = next?;
        let node = engine.nodes.get(id)?;
        match node {
            AppNode::Screen(_) => return Some((ScreenId(node.node_data().meta.uuid), None)),
            AppNode::Surface(_) => {
                let parent = engine.nodes.get(node.node_data().parent?)?;
                if !matches!(parent, AppNode::Screen(_)) {
                    return None;
                }
                let screen = ScreenId(parent.node_data().meta.uuid);
                return Some((
                    screen,
                    Some(SurfaceReference {
                        screen,
                        surface: SurfaceId(node.node_data().meta.uuid),
                    }),
                ));
            }
            _ => next = node.node_data().parent,
        }
    }
    None
}

fn surface(engine: &AppEngine, reference: SurfaceReference) -> Option<SurfaceSnapshot> {
    let id = authoring::surface_id(engine, reference).ok()?;
    let AppNode::Surface(node) = engine.nodes.get(id)? else {
        return None;
    };
    Some(SurfaceSnapshot::new(
        reference,
        node.node_data().meta.label.clone(),
        node.definition(),
    ))
}

pub(super) fn inspect(engine: &AppEngine, query: ScreenEditorQuery) -> Result<ScreenEditorPage, String> {
    for target in [query.locked, query.retained].into_iter().flatten() {
        if !matches!(
            (query.editor, target),
            (ScreenEditorKind::Screen, ScreenEditorTarget::Screen { .. })
                | (ScreenEditorKind::Surface, ScreenEditorTarget::Surface { .. })
        ) {
            return Err("editor binding has the wrong target kind".into());
        }
        if query.document.is_none() {
            return Err("editor bindings require a document identity".into());
        }
    }
    let document = engine
        .nodes
        .get(engine.root)
        .ok_or("project root is missing")?
        .node_data()
        .meta
        .uuid;
    let followed = owner(engine, query.selected);
    let eligible = followed.and_then(|(screen, reference)| match query.editor {
        ScreenEditorKind::Screen => Some(ScreenEditorTarget::Screen { screen }),
        ScreenEditorKind::Surface => reference.map(|reference| ScreenEditorTarget::Surface { reference }),
    });
    let matches = query.document.is_none_or(|expected| expected == document);
    let mut page = ScreenEditorPage {
        at: engine.time,
        parameters: Vec::new(),
        document,
        document_matches: matches || (query.locked.is_none() && eligible.is_some()),
        target: None,
        screen: None,
        has_more_surfaces: false,
        selected_surface: None,
        surface: None,
    };
    if !page.document_matches {
        return Ok(page);
    }
    let Some(target) = query.locked.or(eligible).or(query.retained.filter(|_| matches)) else {
        return Ok(page);
    };
    let (screen, selected) = match target {
        ScreenEditorTarget::Screen { screen } => (
            screen,
            followed
                .filter(|(id, _)| *id == screen)
                .map(|(_, reference)| reference.map(|reference| reference.surface))
                .unwrap_or(query.surface),
        ),
        ScreenEditorTarget::Surface { reference } => (reference.screen, Some(reference.surface)),
    };
    let Ok(id) = authoring::screen_id(engine, screen) else {
        return Ok(page);
    };
    let Some(AppNode::Screen(node)) = engine.nodes.get(id) else {
        return Ok(page);
    };
    page.selected_surface = selected;
    page.surface = selected.and_then(|surface_id| {
        surface(
            engine,
            SurfaceReference {
                screen,
                surface: surface_id,
            },
        )
    });
    if query.editor == ScreenEditorKind::Surface && page.surface.is_none() {
        return Ok(page);
    }
    let mut snapshot = ScreenSnapshot {
        id: screen,
        label: node.node_data().meta.label.clone(),
        definition: node.definition(),
        surfaces: Vec::new(),
    };
    let mut next = node.node_data().first_child;
    while let Some(id) = next.filter(|_| query.editor == ScreenEditorKind::Screen) {
        let child = engine.nodes.get(id).ok_or("Screen child is missing")?;
        next = child.node_data().next_sibling;
        if let AppNode::Surface(node) = child {
            if snapshot.surfaces.len() == 128 {
                page.has_more_surfaces = true;
                break;
            }
            page.parameters
                .push(node.parameter_bindings(engine, SurfaceId(node.node_data().meta.uuid)));
            snapshot.surfaces.push(SurfaceSnapshot::new(
                SurfaceReference {
                    screen,
                    surface: SurfaceId(node.node_data().meta.uuid),
                },
                node.node_data().meta.label.clone(),
                node.definition(),
            ));
        }
    }
    if let Some(selected) = &page.surface
        && !page
            .parameters
            .iter()
            .any(|binding| binding.surface == selected.reference.surface)
    {
        let id = authoring::surface_id(engine, selected.reference)?;
        let Some(AppNode::Surface(node)) = engine.nodes.get(id) else {
            return Err("Surface changed".into());
        };
        page.parameters
            .push(node.parameter_bindings(engine, selected.reference.surface));
    }
    page.target = Some(target);
    page.screen = Some(snapshot);
    Ok(page)
}

pub(super) fn hit_test(engine: &AppEngine, screen: ScreenId, point: Point) -> Result<Option<SurfaceId>, String> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err("drop coordinates must be finite".into());
    }
    if !(0.0..=1.0).contains(&point.x) || !(0.0..=1.0).contains(&point.y) {
        return Ok(None);
    }
    let id = authoring::screen_id(engine, screen)?;
    let mut next = engine.nodes.get(id).and_then(|node| node.node_data().last_child);
    while let Some(id) = next {
        let node = engine.nodes.get(id).ok_or("Screen child is missing")?;
        next = node.node_data().prev_sibling;
        if let AppNode::Surface(node) = node {
            let definition = node.definition();
            if golden_video::plan::ProjectiveMapping::new(definition.crop, definition.quad)
                .is_ok_and(|mapping| mapping.source_coordinate(point).is_some())
            {
                return Ok(Some(SurfaceId(node.node_data().meta.uuid)));
            }
        }
    }
    Ok(None)
}
