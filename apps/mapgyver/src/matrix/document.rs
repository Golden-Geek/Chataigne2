use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{ChannelId, ChannelReference, ChannelSource, MediaId, MediaKind},
};
use golden_core::node::{Node, NodeId};
use std::collections::{HashMap, HashSet};
pub(super) const MAX_ELEMENTS: usize = 100_000;
// Keep every admitted hierarchy roundtrippable through Golden's bounded JSON decoder.
pub(super) const MAX_DEPTH: usize = 32;

pub(super) struct Document {
    pub media: NodeId,
    pub root: NodeId,
    pub snapshot: MatrixSnapshot,
    pub items: HashMap<MatrixItemId, NodeId>,
    pub columns: HashMap<ColumnId, NodeId>,
    pub cells: HashMap<(LayerId, ColumnId), NodeId>,
    pub elements: HashSet<NodeId>,
    pub exports: HashMap<MatrixItemId, ChannelReference>,
}
pub(super) fn children(engine: &AppEngine, parent: NodeId) -> Result<Vec<NodeId>, String> {
    let mut next = engine
        .nodes
        .get(parent)
        .ok_or("missing Matrix parent")?
        .node_data()
        .first_child;
    let mut result = Vec::new();
    while let Some(id) = next {
        let node = engine.nodes.get(id).ok_or("missing Matrix child")?;
        result.push(id);
        next = node.node_data().next_sibling;
    }
    Ok(result)
}
impl Document {
    pub fn load(engine: &AppEngine, media: MediaId) -> Result<Self, String> {
        let (media_node, kind) = crate::media::authoring::media(engine, media)?;
        if kind != MediaKind::Matrix {
            return Err("media is not a Matrix".into());
        }
        let mut root = None;
        let mut exports = HashMap::new();
        for id in children(engine, media_node)? {
            let Some(AppNode::MediaChannel(node)) = engine.nodes.get(id) else {
                return Err("Matrix children must be channels".into());
            };
            let ChannelSource::Matrix { output } = &node.definition().source else {
                return Err("Matrix channel kind mismatch".into());
            };
            let reference = ChannelReference {
                media,
                channel: ChannelId(node.node_data().meta.uuid),
            };
            let key = match output {
                MatrixOutput::Final { definition } => {
                    definition.validate()?;
                    if root.replace((id, reference, definition.clone())).is_some() {
                        return Err("Matrix must have exactly one final output".into());
                    }
                    continue;
                }
                MatrixOutput::Layer { layer } => MatrixItemId::Layer { id: *layer },
                MatrixOutput::Group { group } => MatrixItemId::Group { id: *group },
            };
            if exports.insert(key, reference).is_some() {
                return Err("duplicate Matrix output export".into());
            }
        }
        let (root, final_channel, definition) = root.ok_or("Matrix final output is missing")?;
        let roots = children(engine, root)?;
        let mut document = Self {
            media: media_node,
            root,
            snapshot: MatrixSnapshot {
                media,
                definition,
                final_channel,
                items: vec![],
                columns: vec![],
                cells: vec![],
            },
            items: HashMap::new(),
            columns: HashMap::new(),
            cells: HashMap::new(),
            elements: HashSet::new(),
            exports: HashMap::new(),
        };
        for id in &roots {
            if let Some(AppNode::MatrixElement(node)) = engine.nodes.get(*id)
                && let ElementDefinition::Column { definition } = node.definition()
            {
                if node.node_data().first_child.is_some() {
                    return Err("Matrix column cannot own children".into());
                }
                let column = ColumnId(node.node_data().meta.uuid);
                document.columns.insert(column, *id);
                document.elements.insert(*id);
                document.snapshot.columns.push(ColumnSnapshot {
                    id: column,
                    label: node.node_data().meta.label.clone(),
                    definition: definition.clone(),
                });
            }
        }
        let mut pending = roots.into_iter().rev().map(|id| (id, None, 0usize)).collect::<Vec<_>>();
        while let Some((id, parent, depth)) = pending.pop() {
            if depth >= MAX_DEPTH {
                return Err("Matrix groups exceed depth 32".into());
            }
            let Some(AppNode::MatrixElement(node)) = engine.nodes.get(id) else {
                return Err("unexpected Matrix document child".into());
            };
            let uuid = node.node_data().meta.uuid;
            let (item, settings) = match node.definition() {
                ElementDefinition::Column { .. } if parent.is_none() => continue,
                ElementDefinition::Layer { settings } => (MatrixItemId::Layer { id: LayerId(uuid) }, settings),
                ElementDefinition::Group { settings } => (MatrixItemId::Group { id: GroupId(uuid) }, settings),
                _ => return Err("only groups and layers belong in Matrix groups".into()),
            };
            settings.validate()?;
            document.elements.insert(id);
            document.items.insert(item, id);
            let channel = exports.remove(&item).ok_or("Matrix item export is missing")?;
            document.exports.insert(item, channel);
            document.snapshot.items.push(MatrixItemSnapshot {
                id: item,
                parent,
                label: node.node_data().meta.label.clone(),
                settings: settings.clone(),
                channel,
            });
            match item {
                MatrixItemId::Group { id: group } => pending.extend(
                    children(engine, id)?
                        .into_iter()
                        .rev()
                        .map(|id| (id, Some(group), depth + 1)),
                ),
                MatrixItemId::Layer { id: layer } => {
                    for cell in children(engine, id)? {
                        let Some(AppNode::MatrixElement(node)) = engine.nodes.get(cell) else {
                            return Err("Matrix layer children must be cells".into());
                        };
                        let ElementDefinition::Cell { definition } = node.definition() else {
                            return Err("Matrix layer children must be cells".into());
                        };
                        if node.node_data().first_child.is_some() || !document.columns.contains_key(&definition.column)
                        {
                            return Err("Matrix cell has invalid column or children".into());
                        }
                        if document.cells.insert((layer, definition.column), cell).is_some() {
                            return Err("duplicate Matrix cell address".into());
                        }
                        document.elements.insert(cell);
                        document.snapshot.cells.push(CellSnapshot {
                            id: CellId(node.node_data().meta.uuid),
                            layer,
                            definition: definition.clone(),
                        });
                    }
                }
            }
        }
        if !exports.is_empty() {
            return Err("Matrix export has no owning item".into());
        }
        if document.elements.len() > MAX_ELEMENTS {
            return Err("Matrix exceeds 100000 document elements".into());
        }
        Ok(document)
    }
    pub fn item(&self, item: MatrixItemId) -> Result<NodeId, String> {
        self.items
            .get(&item)
            .copied()
            .ok_or("Matrix item is missing or belongs to another Matrix".into())
    }
    pub fn column(&self, column: ColumnId) -> Result<NodeId, String> {
        self.columns
            .get(&column)
            .copied()
            .ok_or("column is missing or belongs to another Matrix".into())
    }
    pub fn parent(&self, parent: Option<GroupId>) -> Result<NodeId, String> {
        parent
            .map(|id| self.item(MatrixItemId::Group { id }))
            .unwrap_or(Ok(self.root))
    }
    pub fn channel(&self, item: MatrixItemId) -> ChannelReference {
        self.exports[&item]
    }
    pub fn admit(&self, additions: usize) -> Result<(), String> {
        if self.elements.len().saturating_add(additions) > MAX_ELEMENTS {
            Err("Matrix exceeds 100000 document elements".into())
        } else {
            Ok(())
        }
    }
}
pub(crate) fn inspect(engine: &AppEngine, media: MediaId) -> Result<MatrixSnapshot, String> {
    Ok(Document::load(engine, media)?.snapshot)
}
pub(crate) fn validate_project(engine: &AppEngine) -> Result<(), String> {
    let mut owned = HashSet::new();
    for (_, node) in engine.nodes.iter() {
        if let AppNode::Media(node) = node
            && node.kind == MediaKind::Matrix
        {
            owned.extend(Document::load(engine, MediaId(node.node_data().meta.uuid))?.elements);
        }
    }
    for (id, node) in engine.nodes.iter() {
        if matches!(node, AppNode::MatrixElement(_)) && !owned.contains(&id) {
            return Err("Matrix element is outside a Matrix document".into());
        }
    }
    Ok(())
}

/// Conservative document DAG: every authored cell, including unloaded columns, participates.
pub(crate) fn dependencies(snapshot: &MatrixSnapshot) -> HashMap<ChannelReference, Vec<ChannelReference>> {
    let mut sources = HashMap::<LayerId, Vec<ChannelReference>>::new();
    for cell in &snapshot.cells {
        if let Some(source) = cell.definition.source {
            sources.entry(cell.layer).or_default().push(source);
        }
    }
    let mut groups = HashMap::<Option<GroupId>, Vec<ChannelReference>>::new();
    for item in &snapshot.items {
        groups.entry(item.parent).or_default().push(item.channel);
    }
    let mut result = HashMap::new();
    result.insert(snapshot.final_channel, groups.remove(&None).unwrap_or_default());
    for item in &snapshot.items {
        result.insert(
            item.channel,
            match item.id {
                MatrixItemId::Layer { id } => sources.remove(&id).unwrap_or_default(),
                MatrixItemId::Group { id } => groups.remove(&Some(id)).unwrap_or_default(),
            },
        );
    }
    result
}
