//! Reference Cells share producer application identity; cache invalidation follows definition edits, not launches.
use super::protocol::CellId;
use crate::{
    app::{AppEngine, AppNode},
    applications::protocol::ApplicationId,
    media::protocol::{MediaId, MediaKind},
};
use golden_core::node::{Node, NodeId};
use golden_io::RevisionGuard;
use std::collections::HashSet;

pub(super) fn snapshot_has_application(ctx: &golden_core::process_ctx::ProcessCtx, cell: NodeId) -> bool {
    let Some(tree) = ctx.tree_snapshot() else { return false };
    let Some(node) = tree.node(cell) else { return false };
    if node.node_type != super::cell_node::CELL_TYPE {
        return false;
    }
    let parameter = |root, name: &str| {
        tree.find_child_by_decl_id(root, "parameters")
            .and_then(|folder| tree.find_child_by_decl_id(folder, name))
            .and_then(|id| tree.node(id))
            .and_then(|node| node.param_value.as_ref())
    };
    if matches!(parameter(cell, "parameters/shared_app_control"), Some(golden_core::parameter::ParamValue::Reference(value)) if !value.uuid.0.is_nil())
    {
        return true;
    }
    tree.find_child_by_decl_id(cell, "app_control").is_some_and(|control| matches!(parameter(control, "parameters/app_file"), Some(golden_core::parameter::ParamValue::File(value)) if !value.is_empty()))
}

pub(super) fn matrix(engine: &AppEngine, cell: NodeId) -> Result<MediaId, String> {
    let mut next = Some(cell);
    for _ in 0..64 {
        let node = engine
            .nodes
            .get(next.ok_or("Cell has no Matrix owner")?)
            .ok_or("Cell owner is missing")?;
        if let AppNode::Media(media) = node
            && media.kind() == MediaKind::Matrix
        {
            return Ok(MediaId(node.node_data().meta.uuid));
        }
        next = node.node_data().parent;
    }
    Err("Cell ownership exceeds depth64".into())
}

pub(super) fn owner(engine: &AppEngine, mut cell: CellId) -> Result<ApplicationId, String> {
    let mut seen = HashSet::new();
    let mut configured = None;
    for _ in 0..32 {
        if !seen.insert(cell) {
            return Err("Cell app-control reference cycle".into());
        }
        let Some(id) = engine.node_id_by_uuid(cell.0) else {
            return Ok(configured.unwrap_or(ApplicationId(cell.0)));
        };
        let Some(AppNode::MatrixCell(node)) = engine.nodes.get(id) else {
            return Err("Shared app control must reference a Cell".into());
        };
        if configured.is_none()
            && crate::applications::control_node(engine, ApplicationId(cell.0)).is_some_and(|node| node.configured())
        {
            configured = Some(ApplicationId(cell.0));
        }
        match node.application_source() {
            Some(source) => cell = source,
            None => return Ok(configured.unwrap_or(ApplicationId(cell.0))),
        }
    }
    Err("Cell app-control reference chain exceeds32".into())
}

pub(super) fn dependencies(
    engine: &AppEngine,
    document: &super::document::Document,
) -> Result<Option<RevisionGuard>, String> {
    let manager = crate::media::authoring::manager(engine)?;
    let Some(AppNode::MediaManager(manager)) = engine.nodes.get(manager) else {
        unreachable!()
    };
    let mut linked = false;
    let mut seen = HashSet::new();
    for cell in &document.snapshot.cells {
        let Some(AppNode::MatrixCell(origin)) = engine.nodes.get(document.cells[&(cell.layer, cell.definition.column)])
        else {
            unreachable!()
        };
        let Some(mut next) = origin.application_source() else {
            continue;
        };
        linked = true;
        for _ in 0..32 {
            if !seen.insert(next) {
                break;
            }
            let Some(id) = engine.node_id_by_uuid(next.0) else {
                break;
            };
            let Some(AppNode::MatrixCell(node)) = engine.nodes.get(id) else {
                return Err("Shared app control must reference a Cell".into());
            };
            let source_media = matrix(engine, id)?;
            let root = super::playback::root(engine, source_media)?;
            let Some(AppNode::MediaChannel(root)) = engine.nodes.get(root) else {
                unreachable!()
            };
            let control = root.matrix_playback().ok_or("Matrix playback is missing")?;
            control.bind_application_parent(&manager.application_revision, &manager.application_definition_revision);
            node.bind_runtime(control.binding());
            if let Some(settings) = node.application_node().and_then(|id| engine.nodes.get(id))
                && let AppNode::Application(settings) = settings
            {
                super::cell_applications::bind(settings, control);
            }
            let Some(source) = node.application_source() else {
                break;
            };
            linked = true;
            next = source;
        }
    }
    Ok(linked.then(|| {
        let reader = manager.application_definition_revision.reader();
        reader.guard_at(reader.current())
    }))
}
