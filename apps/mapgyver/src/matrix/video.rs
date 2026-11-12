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
    time::{Duration, Instant},
};

pub(super) struct Prepared {
    rows: HashMap<LayerId, Row>,
}
type VideoOwner = (MediaId, Arc<VideoControl>);
struct Row {
    identity: Option<uuid::Uuid>,
    key: Option<Key>,
    duration: Duration,
    source: Option<VideoOwner>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
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
    order: u64,
}
struct Outgoing {
    layer: LayerId,
    active: Active,
    deadline: Instant,
}
#[derive(Default)]
pub(super) struct Playback {
    active: HashMap<LayerId, Active>,
    outgoing: Vec<Outgoing>,
    targets: HashMap<LayerId, Option<uuid::Uuid>>,
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
                    duration: Duration::from_millis(u64::from(transition.duration_ms)),
                    source,
                },
            );
        }
        Ok(Arc::new(Self { rows }))
    }
}
impl Playback {
    pub fn accepts(&self, prepared: &Prepared) -> bool {
        let owners = prepared
            .rows
            .values()
            .filter_map(|row| row.source.as_ref())
            .map(|source| Arc::as_ptr(&source.1))
            .collect::<HashSet<_>>();
        let mut retained = HashMap::<LayerId, usize>::new();
        for (layer, active) in self
            .active
            .iter()
            .map(|(layer, active)| (*layer, active))
            .chain(self.outgoing.iter().map(|outgoing| (outgoing.layer, &outgoing.active)))
        {
            if !owners.contains(&Arc::as_ptr(&active.source.1)) {
                *retained.entry(layer).or_default() += 1;
            }
        }
        prepared.rows.iter().all(|(layer, row)| {
            if row.duration.is_zero() {
                return true;
            }
            retained.get(layer).copied().unwrap_or_default() + usize::from(row.source.is_some())
                <= super::timeline::MAX_MIX_COLUMNS
        })
    }
    /// Acquire incoming owners before releasing outgoing uses, including same-owner reentry.
    pub fn commit(&mut self, prepared: &Prepared, order: u64, now: Instant) -> Result<(), String> {
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
            if let (Some(identity), Some(key), Some(source)) = (row.identity, row.key, row.source.as_ref()) {
                match source.1.activate_for_matrix(identity) {
                    Ok(lease) => {
                        incoming.insert(
                            layer,
                            Active {
                                identity,
                                key,
                                source: source.clone(),
                                lease,
                                order,
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
        self.outgoing.retain_mut(|outgoing| {
            let Some(row) = prepared.rows.get(&outgoing.layer).filter(|row| !row.duration.is_zero()) else {
                return false;
            };
            // A new destination restarts the outgoing mix; an unrelated Layer launch does not.
            if self.targets.get(&outgoing.layer) != Some(&row.identity) {
                outgoing.deadline = now + row.duration;
            }
            !owners.contains(&Arc::as_ptr(&outgoing.active.source.1))
        });
        for (layer, mut active) in self.active.drain() {
            if let Some(row) = prepared.rows.get(&layer).filter(|row| !row.duration.is_zero())
                && !owners.contains(&Arc::as_ptr(&active.source.1))
            {
                if let Err(error) = active.lease.deactivate() {
                    failure.get_or_insert(error);
                }
                self.outgoing.push(Outgoing {
                    layer,
                    active,
                    deadline: now + row.duration,
                });
            }
        }
        self.active = incoming;
        self.targets = prepared
            .rows
            .iter()
            .map(|(layer, row)| (*layer, row.identity))
            .collect();
        failure.map_or(Ok(()), Err)
    }
    pub fn cut(&mut self, cuts: &super::stops::Cuts) {
        let removed = |layer, active: &Active| {
            cuts.removes(
                Some(layer),
                super::timeline::Clip {
                    id: active.identity,
                    column: active.key.column,
                    version: active.key.version,
                    individual: active.key.individual,
                    order: active.order,
                },
            )
        };
        self.active.retain(|layer, active| !removed(*layer, active));
        self.outgoing
            .retain(|outgoing| !removed(outgoing.layer, &outgoing.active));
    }
    pub fn next_deadline(&self) -> Option<Instant> {
        self.outgoing.iter().map(|outgoing| outgoing.deadline).min()
    }
    pub fn columns(&self) -> impl Iterator<Item = ColumnId> + '_ {
        self.active
            .values()
            .map(|active| active.key.column)
            .chain(self.outgoing.iter().map(|outgoing| outgoing.active.key.column))
    }
    pub fn layers(&self) -> impl Iterator<Item = LayerId> + '_ {
        self.active
            .keys()
            .copied()
            .chain(self.outgoing.iter().map(|outgoing| outgoing.layer))
    }
    pub fn finish_due(&mut self, now: Instant) {
        self.outgoing.retain(|outgoing| outgoing.deadline > now);
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
        self.targets.clear();
    }
}
