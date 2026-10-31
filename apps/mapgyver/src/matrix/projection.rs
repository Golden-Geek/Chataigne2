//! Immutable sparse lookup index, rebuilt on edits rather than on launches or frames.
use super::{document::Document, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::ChannelReference,
};
use golden_core::node::Node;
use std::collections::{BTreeMap, HashMap};
pub(crate) struct RenderItem {
    pub channel: ChannelReference,
    pub parent: Option<GroupId>,
    pub settings: CompositeSettings,
    pub enabled: bool,
    pub order: usize,
    pub end: usize,
}
pub(crate) struct RenderIndex {
    pub columns: Vec<ColumnSnapshot>,
    pub canvas: golden_video::Extent2D,
    pub items: HashMap<MatrixItemId, RenderItem>,
    pub cells: HashMap<ColumnId, BTreeMap<usize, CellSource>>,
}
pub(crate) struct CellSource {
    pub layer: LayerId,
    pub source: ChannelReference,
    pub version: u64,
}
impl RenderIndex {
    pub(super) fn build(engine: &AppEngine, document: &Document) -> Self {
        let mut items = HashMap::with_capacity(document.snapshot.items.len());
        let mut groups = Vec::<MatrixItemId>::new();
        for (order, item) in document.snapshot.items.iter().enumerate() {
            while groups
                .last()
                .is_some_and(|id| Some(*id) != item.parent.map(|id| MatrixItemId::Group { id }))
            {
                let entry: &mut RenderItem = items.get_mut(&groups.pop().unwrap()).unwrap();
                entry.end = order;
            }
            let Some(AppNode::MatrixElement(node)) = engine.nodes.get(document.items[&item.id]) else {
                unreachable!()
            };
            items.insert(
                item.id,
                RenderItem {
                    channel: item.channel,
                    parent: item.parent,
                    settings: item.settings.clone(),
                    enabled: node.node_data().effective_enabled,
                    order,
                    end: order + 1,
                },
            );
            if matches!(item.id, MatrixItemId::Group { .. }) {
                groups.push(item.id);
            }
        }
        for id in groups {
            items.get_mut(&id).unwrap().end = document.snapshot.items.len();
        }
        let mut cells = HashMap::<ColumnId, BTreeMap<usize, CellSource>>::new();
        for cell in &document.snapshot.cells {
            if let Some(source) = cell.definition.source {
                let AppNode::MatrixElement(node) = engine
                    .nodes
                    .get(document.cells[&(cell.layer, cell.definition.column)])
                    .unwrap()
                else {
                    unreachable!()
                };
                let entry = &items[&MatrixItemId::Layer { id: cell.layer }];
                if !entry.enabled || !entry.settings.enabled || entry.settings.opacity == 0.0 {
                    continue;
                }
                cells.entry(cell.definition.column).or_default().insert(
                    items[&MatrixItemId::Layer { id: cell.layer }].order,
                    CellSource {
                        layer: cell.layer,
                        source,
                        version: node.source_version(),
                    },
                );
            }
        }
        Self {
            columns: document.snapshot.columns.clone(),
            canvas: document.snapshot.definition.canvas,
            items,
            cells,
        }
    }
}
