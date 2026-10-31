//! Bounded actor capture over the cached authored index; no device, file or frame polling.
use super::{document::Document, projection::RenderIndex, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{ChannelReference, MediaId},
};
use golden_core::node::Node;
use golden_io::RevisionGuard;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
#[derive(Clone)]
pub(crate) struct MatrixInput {
    pub source: ChannelReference,
    pub blend: golden_video::plan::BlendMode,
    pub weight: Option<golden_video::plan::OpacityEnvelope>,
}
#[derive(Clone)]
pub(crate) struct CapturedMatrix {
    pub observation: MatrixPlanStamp,
    pub canvas: golden_video::Extent2D,
    pub opacity: f32,
    pub inputs: Vec<MatrixInput>,
    pub until: Option<golden_video::plan::OpacityEnvelope>,
    pub control: super::playback::PlaybackControl,
}
impl CapturedMatrix {
    pub fn needs_scratch(&self) -> bool {
        self.opacity < 1.0 && self.inputs.len() > 1
    }
}

pub(crate) fn capture(
    engine: &AppEngine,
    reference: ChannelReference,
    output: &MatrixOutput,
    budget: &mut usize,
    guarded: &mut HashSet<MediaId>,
    guards: &mut Vec<RevisionGuard>,
    time: Option<golden_time::TimeSnapshot>,
) -> Result<Option<CapturedMatrix>, String> {
    let root_id = super::playback::root(engine, reference.media)?;
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(root_id) else {
        return Err("Matrix owner is not a channel".into());
    };
    let Some(AppNode::Media(media)) = root.node_data().parent.and_then(|id| engine.nodes.get(id)) else {
        return Err("Matrix media owner is missing".into());
    };
    if media.node_data().meta.uuid != reference.media.0 {
        return Err("Matrix export belongs to another media".into());
    }
    let control = root.matrix_playback().ok_or("Matrix runtime owner is missing")?;
    let index = match control.cached() {
        Some(index) => index,
        None => {
            let document = Document::load(engine, reference.media)?;
            for id in &document.elements {
                let Some(AppNode::MatrixElement(node)) = engine.nodes.get(*id) else {
                    unreachable!()
                };
                node.bind_runtime(control.binding());
            }
            let index = Arc::new(RenderIndex::build(engine, &document));
            control.cache(index.clone());
            index
        }
    };
    let captured = control
        .with_frame(time, |timeline, frame, playback| {
            let mut columns = Vec::new();
            for contribution in &frame.clips {
                if !columns.contains(&contribution.clip.column) {
                    columns.push(contribution.clip.column);
                }
            }
            let mut captured = CapturedMatrix {
                observation: MatrixPlanStamp {
                    playback: playback.clone(),
                    revision: control.authoring_revision(),
                    columns,
                    transitioning: frame.interval.is_some(),
                },
                canvas: index.canvas,
                opacity: 1.0,
                inputs: vec![],
                until: frame.interval.map(|interval| interval.envelope(0.0, 1.0)),
                control: control.clone(),
            };
            let item = match output {
                MatrixOutput::Final { .. } => {
                    if root.node_data().meta.uuid != reference.channel.0 {
                        return Err("Matrix Final identity mismatch".into());
                    }
                    None
                }
                MatrixOutput::Layer { layer } => Some(MatrixItemId::Layer { id: *layer }),
                MatrixOutput::Group { group } => Some(MatrixItemId::Group { id: *group }),
            };
            if let Some(item) = item {
                let entry = index.items.get(&item).ok_or("Matrix export owner is missing")?;
                if entry.channel != reference {
                    return Err("Matrix export identity mismatch".into());
                }
                captured.opacity = entry.settings.opacity;
                if !entry.enabled || !entry.settings.enabled || entry.settings.opacity == 0.0 {
                    return Ok(Some(captured));
                }
            }
            if let Some(MatrixItemId::Layer { .. }) = item {
                let mut sources = Vec::<(ChannelReference, f32, f32)>::new();
                let MatrixItemId::Layer { id: layer } = item.unwrap() else {
                    unreachable!()
                };
                for entry in &frame.clips {
                    let candidate = index
                        .cells
                        .get(&entry.clip.column)
                        .and_then(|cells| cells.get(&index.items[&item.unwrap()].order));
                    let source = match timeline.bindings.resolve(entry.clip, layer, reference, candidate) {
                        Ok(Some(source)) => source,
                        Ok(None) => continue,
                        Err(_) => return Ok(None),
                    };
                    if let Some((_, current_from, current_to)) = sources.iter_mut().find(|(id, _, _)| *id == source) {
                        *current_from += entry.from;
                        *current_to += entry.to;
                    } else {
                        sources.push((source, entry.from, entry.to));
                    }
                }
                for (source, from, to) in sources {
                    captured.inputs.push(MatrixInput {
                        source,
                        blend: golden_video::plan::BlendMode::Add,
                        weight: frame.interval.map(|interval| interval.envelope(from, to)),
                    });
                }
            } else {
                let scope = item.and_then(|id| {
                    if let MatrixItemId::Group { id } = id {
                        Some(id)
                    } else {
                        None
                    }
                });
                let range = item
                    .map(|id| {
                        let entry = &index.items[&id];
                        entry.order..entry.end
                    })
                    .unwrap_or(0..index.items.len());
                let mut children = HashMap::<MatrixItemId, usize>::new();
                // Previously rendered rows survive authored column/cell changes. This scans only the
                // bounded binding cache and never retains an authored index per clip.
                for layer in timeline.bindings.layers() {
                    if *budget == 0 {
                        return Ok(None);
                    }
                    *budget -= 1;
                    let current = MatrixItemId::Layer { id: layer };
                    if let Some(entry) = index.items.get(&current)
                        && range.contains(&entry.order)
                    {
                        immediate_child(&index, current, scope, &mut children);
                    }
                }
                for entry in &frame.clips {
                    let Some(cells) = index.cells.get(&entry.clip.column) else {
                        continue;
                    };
                    let mut next = range.start;
                    while let Some((&order, cell)) = cells.range(next..range.end).next() {
                        if *budget == 0 {
                            return Ok(None);
                        }
                        *budget -= 1;
                        next = order + 1;
                        let current = MatrixItemId::Layer { id: cell.layer };
                        let owner = index.items[&current].channel;
                        match timeline.bindings.resolve(entry.clip, cell.layer, owner, Some(cell)) {
                            Ok(Some(_)) => {
                                if let Some(child) = immediate_child(&index, current, scope, &mut children) {
                                    next = index.items[&child].end;
                                }
                            }
                            Ok(None) => {}
                            Err(_) => return Ok(None),
                        }
                    }
                }
                let mut children = children.into_iter().collect::<Vec<_>>();
                children.sort_by_key(|(_, order)| *order);
                for (item, _) in children {
                    let entry = &index.items[&item];
                    if entry.enabled && entry.settings.enabled && entry.settings.opacity > 0.0 {
                        captured.inputs.push(MatrixInput {
                            source: entry.channel,
                            blend: entry.settings.blend,
                            weight: None,
                        });
                    }
                }
            }
            if captured.inputs.len() > *budget {
                return Ok(None);
            }
            *budget -= captured.inputs.len();
            Ok(Some(captured))
        })?
        .flatten();
    if guarded.insert(reference.media) {
        guards.push(root.guard());
        guards.push(control.guard());
    }
    Ok(captured)
}

fn immediate_child(
    index: &RenderIndex,
    mut current: MatrixItemId,
    scope: Option<GroupId>,
    children: &mut HashMap<MatrixItemId, usize>,
) -> Option<MatrixItemId> {
    loop {
        let entry = &index.items[&current];
        if entry.parent == scope {
            children.insert(current, entry.order);
            return Some(current);
        }
        current = MatrixItemId::Group { id: entry.parent? };
    }
}
