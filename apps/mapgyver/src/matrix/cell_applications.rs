//! Sparse Cell settings and projection into the reusable app-control demand input.
use super::{document::Document, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    applications::{demand::*, protocol::*},
};
use golden_core::{
    edit::Edit,
    node::{Node, NodeId},
    parameter::ParameterEventBehaviour,
};
use std::collections::HashMap;

pub(super) fn configure_column(
    engine: &mut AppEngine,
    document: &Document,
    column: ColumnId,
    definition: ApplicationDefinition,
    pre: u16,
    post: u16,
) -> Result<crate::media::protocol::MediaEditResult, String> {
    definition.validate()?;
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(document.root) else {
        unreachable!()
    };
    let control = root.matrix_playback().ok_or("Matrix playback is missing")?;
    let Some(AppNode::MatrixColumn(owner)) = engine.nodes.get(document.column(column)?) else {
        unreachable!()
    };
    owner.bind_runtime(control.binding());
    let settings = owner
        .application_node()
        .ok_or("Column app-control settings are missing")?;
    let Some(AppNode::Application(node)) = engine.nodes.get(settings) else {
        return Err("Column app-control type is invalid".into());
    };
    bind(node, control);
    let distances = node.distance_values(pre, post);
    crate::applications::configure_node(engine, settings, definition)?;
    engine.edits.push(Edit::SetParamsAtomic {
        values: distances.into(),
        behaviour: ParameterEventBehaviour::Append,
    });
    Ok(crate::media::protocol::MediaEditResult::Changed)
}

pub(super) fn configure(
    engine: &mut AppEngine,
    document: &Document,
    layer: LayerId,
    column: ColumnId,
    definition: ApplicationDefinition,
    pre: u16,
    post: u16,
) -> Result<crate::media::protocol::MediaEditResult, String> {
    definition.validate()?;
    let (cell, settings) = ensure(engine, document, layer, column)?;
    let Some(AppNode::MatrixCell(owner)) = engine.nodes.get(cell) else {
        unreachable!()
    };
    engine.edits.push(Edit::SetParam {
        node: owner.application_source_parameter(),
        value: golden_core::parameter::ParamValue::Reference(golden_core::node::NodeReference::empty()),
        behaviour: ParameterEventBehaviour::Append,
    });
    let Some(AppNode::Application(node)) = engine.nodes.get(settings) else {
        return Err("Cell app-control type is invalid".into());
    };
    let distances = node.distance_values(pre, post);
    crate::applications::configure_node(engine, settings, definition)?;
    engine.edits.push(Edit::SetParamsAtomic {
        values: distances.into(),
        behaviour: ParameterEventBehaviour::Append,
    });
    result(engine, cell)
}

pub(super) fn prepare(
    engine: &mut AppEngine,
    document: &Document,
    layer: LayerId,
    column: ColumnId,
) -> Result<crate::media::protocol::MediaEditResult, String> {
    let (cell, _) = ensure(engine, document, layer, column)?;
    result(engine, cell)
}
fn result(engine: &AppEngine, cell: NodeId) -> Result<crate::media::protocol::MediaEditResult, String> {
    Ok(crate::media::protocol::MediaEditResult::MatrixCellPrepared {
        cell: CellId(engine.nodes.get(cell).ok_or("Cell is missing")?.node_data().meta.uuid),
        application: ApplicationId(engine.nodes.get(cell).ok_or("Cell is missing")?.node_data().meta.uuid),
    })
}
fn ensure(
    engine: &mut AppEngine,
    document: &Document,
    layer: LayerId,
    column: ColumnId,
) -> Result<(NodeId, NodeId), String> {
    let cell = super::cells::ensure(engine, document, layer, column)?;
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(document.root) else {
        unreachable!()
    };
    let control = root.matrix_playback().ok_or("Matrix playback is missing")?;
    let Some(AppNode::MatrixCell(node)) = engine.nodes.get(cell) else {
        unreachable!()
    };
    node.bind_runtime(control.binding());
    let settings = node.application_node().ok_or("Cell app-control settings are missing")?;
    let Some(AppNode::Application(node)) = engine.nodes.get(settings) else {
        return Err("Cell app-control type is invalid".into());
    };
    bind(node, control);
    Ok((cell, settings))
}

pub(super) fn bind(node: &crate::applications::AppControlNode, control: &super::playback::PlaybackControl) {
    let binding = control.binding();
    node.bind_invalidation(move || {
        binding.invalidate();
        binding.invalidate_applications();
    });
}

/// Configuration does not require a media child or a visible/rendered layer.
pub(super) fn append(
    engine: &AppEngine,
    document: &Document,
    control: &super::playback::PlaybackControl,
    columns: &mut [DemandColumn],
) -> Result<usize, String> {
    let positions = columns
        .iter()
        .enumerate()
        .map(|(index, column)| (column.id, index))
        .collect::<HashMap<_, _>>();
    let mut count = 0;
    for column in &document.snapshot.columns {
        let Some(AppNode::MatrixColumn(owner)) = engine.nodes.get(document.columns[&column.id]) else {
            unreachable!()
        };
        owner.bind_runtime(control.binding());
        let Some(node) = crate::applications::control_node(engine, column.application) else {
            return Err("Column app control is missing".into());
        };
        bind(node, control);
        if !node.configured() || !node.node_data().effective_enabled {
            continue;
        }
        count += 1;
        if count > DemandLimits::default().applications {
            return Err("Matrix app-control binding capacity exceeded".into());
        }
        let (pre, post) = node.distances();
        columns[positions[&column.id]].bindings.push(DemandBinding {
            application: column.application,
            pre,
            post,
        });
    }
    for cell in &document.snapshot.cells {
        let Some(AppNode::MatrixCell(owner)) = engine.nodes.get(document.cells[&(cell.layer, cell.definition.column)])
        else {
            unreachable!()
        };
        owner.bind_runtime(control.binding());
        if let Some(AppNode::Application(own)) = owner.application_node().and_then(|id| engine.nodes.get(id)) {
            bind(own, control);
        }
        let Some(node) = crate::applications::control_node(engine, cell.definition.application) else {
            if cell.definition.application.0 == cell.id.0 {
                return Err("Cell app-control settings are missing".into());
            }
            continue;
        };
        if !node.configured() || !node.node_data().effective_enabled {
            continue;
        }
        count += 1;
        if count > DemandLimits::default().applications {
            return Err("Matrix exceeds admitted Cell app-control bindings".into());
        }
        let (pre, post) = node.distances();
        columns[*positions.get(&cell.definition.column).ok_or("Cell Column is missing")?]
            .bindings
            .push(DemandBinding {
                application: cell.definition.application,
                pre,
                post,
            });
    }
    Ok(count)
}
