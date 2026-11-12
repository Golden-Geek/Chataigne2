//! Receipt-prepared Video policy, independent of UI panels and native renderer incarnations.
use super::{configuration::Configuration, protocol::*, timeline::Frame};
use crate::{
    app::{AppEngine, AppNode},
    media::{
        protocol::MediaId,
        video::{MatrixVideoUse, VideoControl},
    },
};
use golden_core::node::Node;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub(super) struct Prepared {
    rows: HashMap<LayerId, Row>,
}
type VideoOwner = (MediaId, Arc<VideoControl>);
struct Row {
    identity: Option<uuid::Uuid>,
    key: Option<Key>,
    cut: bool,
    source: Option<VideoOwner>,
}
#[derive(Clone, Copy)]
struct Key {
    column: ColumnId,
    version: u64,
    individual: bool,
}
struct Active {
    identity: uuid::Uuid,
    key: Key,
    source: VideoOwner,
    lease: MatrixVideoUse,
}
struct Outgoing {
    layer: LayerId,
    active: Active,
}
#[derive(Default)]
pub(super) struct Playback {
    active: HashMap<LayerId, Active>,
    outgoing: Vec<Outgoing>,
}
impl Prepared {
    pub fn empty() -> Arc<Self> {
        Arc::new(Self { rows: HashMap::new() })
    }
    pub fn capture(
        engine: &AppEngine,
        desired: &MatrixPlaybackSnapshot,
        index: &Configuration,
        version: u64,
    ) -> Result<Arc<Self>, String> {
        let overrides = desired
            .layers
            .iter()
            .map(|row| (row.layer, row))
            .collect::<HashMap<_, _>>();
        let mut sources: HashMap<_, Option<VideoOwner>> = HashMap::new();
        let mut rows = HashMap::new();
        for &layer in &index.layers {
            let row = overrides.get(&layer);
            let (column, identity, transition, individual) = row.map_or(
                (desired.desired_column, desired.launch_id, desired.transition, false),
                |row| (row.column, row.launch_id, row.transition, row.individual),
            );
            let source = match column
                .and_then(|column| index.cell_nodes.get(&(layer, column)))
                .and_then(|id| engine.nodes.get(*id))
            {
                Some(AppNode::MatrixCell(cell))
                    if cell.node_data().effective_enabled && cell.settings(engine).enabled =>
                {
                    cell.selection(engine)?
                }
                _ => None,
            };
            let source = if let Some(source) = source {
                let key = (source.media, source.channel);
                match sources.entry(key) {
                    std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(source.video_owner(engine)?).clone()
                    }
                }
            } else {
                None
            };
            rows.insert(
                layer,
                Row {
                    identity,
                    key: column.map(|column| Key {
                        column,
                        version,
                        individual,
                    }),
                    cut: transition.duration_ms == 0,
                    source,
                },
            );
        }
        Ok(Arc::new(Self { rows }))
    }
}
impl Playback {
    /// Cut acquires every incoming owner before any outgoing release. Nonzero transitions
    /// retain existing Cut consumers through the real frame; their activation is the next slice.
    pub fn commit(&mut self, prepared: &Prepared) -> Result<(), String> {
        let mut incoming = HashMap::new();
        let mut failure = None;
        for (&layer, row) in &prepared.rows {
            if let Some(existing) = self.active.get(&layer)
                && row.identity == Some(existing.identity)
                && row
                    .source
                    .as_ref()
                    .is_some_and(|source| Arc::ptr_eq(&source.1, &existing.source.1))
            {
                incoming.insert(layer, self.active.remove(&layer).unwrap());
                continue;
            }
            if row.cut
                && let (Some(identity), Some(key), Some(source)) = (row.identity, row.key, row.source.as_ref())
            {
                match source.1.activate_for_matrix(identity) {
                    Ok(lease) => {
                        incoming.insert(
                            layer,
                            Active {
                                identity,
                                key,
                                source: source.clone(),
                                lease,
                            },
                        );
                    }
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }
        }
        let owners = incoming
            .values()
            .map(|active| Arc::as_ptr(&active.source.1))
            .collect::<HashSet<_>>();
        self.outgoing.retain(|outgoing| {
            prepared.rows.get(&outgoing.layer).is_some_and(|row| !row.cut)
                && !owners.contains(&Arc::as_ptr(&outgoing.active.source.1))
        });
        for (layer, mut active) in self.active.drain() {
            if prepared.rows.get(&layer).is_some_and(|row| !row.cut) && !owners.contains(&Arc::as_ptr(&active.source.1))
            {
                if let Err(error) = active.lease.deactivate() {
                    failure.get_or_insert(error);
                }
                self.outgoing.push(Outgoing { layer, active });
            }
        }
        self.active = incoming;
        failure.map_or(Ok(()), Err)
    }
    pub fn retain_frame(&mut self, frame: &Frame) {
        self.outgoing.retain(|outgoing| {
            frame.layer(outgoing.layer).clips.iter().any(|entry| {
                let key = outgoing.active.key;
                entry.clip.column == key.column
                    && entry.clip.version == key.version
                    && entry.clip.individual == key.individual
            })
        });
    }
    pub fn retain_timeline(&mut self, timeline: &super::timeline::Timeline) {
        self.outgoing.retain(|outgoing| {
            timeline.contains_video_clip(
                outgoing.layer,
                outgoing.active.key.column,
                outgoing.active.key.version,
                outgoing.active.key.individual,
            )
        });
    }
    pub fn clear(&mut self) {
        self.active.clear();
        self.outgoing.clear();
    }
}
