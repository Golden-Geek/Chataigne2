//! Cursor-bounded related geometry, using canonical channel identity rather than UI guesses.
use super::{authoring, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    media::SourceIdentity,
};
use golden_core::node::Node;

pub(super) fn inspect(engine: &AppEngine, query: RelatedSurfaceQuery) -> Result<RelatedSurfacePage, String> {
    let document = engine
        .nodes
        .get(engine.root)
        .ok_or("project root is missing")?
        .node_data()
        .meta
        .uuid;
    let mut page = RelatedSurfacePage {
        at: engine.time,
        document_matches: query.document == document,
        source: None,
        surfaces: vec![],
        next: None,
    };
    if !page.document_matches {
        return Ok(page);
    }
    let Ok(selected) = authoring::surface_id(engine, query.reference) else {
        return Ok(page);
    };
    let Some(AppNode::Surface(selected)) = engine.nodes.get(selected) else {
        unreachable!()
    };
    let mut identities = SourceIdentity::new(engine);
    page.source = identities.resolve(selected.definition().source);
    if page.source.is_none() {
        return Ok(page);
    }
    let mut next = super::surface_pages::start(engine, query.cursor)?;
    let mut visited = 0;
    while let Some(id) = next {
        if visited == 512 || page.surfaces.len() == 64 {
            page.next = engine.nodes.get(id).map(|node| node.node_data().meta.uuid);
            break;
        }
        visited += 1;
        let node = engine.nodes.get(id).ok_or("related Surface changed")?;
        next = super::surface_pages::advance(engine, id);
        let AppNode::Surface(surface) = node else { continue };
        let Some(AppNode::Screen(screen)) = node.node_data().parent.and_then(|id| engine.nodes.get(id)) else {
            continue;
        };
        let reference = SurfaceReference {
            screen: ScreenId(screen.node_data().meta.uuid),
            surface: SurfaceId(node.node_data().meta.uuid),
        };
        if reference == query.reference || identities.resolve(surface.definition().source) != page.source {
            continue;
        }
        page.surfaces.push(RelatedSurface {
            screen_label: screen.node_data().meta.label.clone(),
            surface: SurfaceSnapshot::new(reference, node.node_data().meta.label.clone(), surface.definition()),
            parameters: surface.parameter_bindings(engine, reference.surface),
        });
    }
    Ok(page)
}
