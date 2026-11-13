//! Receipt-prepared Video policy, independent of UI panels and native renderer incarnations.
use super::{configuration::Configuration, protocol::*, timeline::Frame};
use crate::{
    app::{AppEngine, AppNode},
    media::{
        MediaSource,
        protocol::MediaId,
        video::{MatrixVideoUse, VideoControl},
    },
};
use golden_core::node::Node;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
    time::{Duration, Instant},
};

pub(super) struct Prepared {
    rows: HashMap<LayerId, Row>,
    routes: HashMap<MediaSource, Route>,
}
struct Route {
    owner: Option<(MediaId, Weak<VideoControl>)>,
    missing: bool,
    guards: Vec<golden_io::RevisionGuard>,
}
impl Route {
    fn owner(&self) -> Option<VideoOwner> {
        let (media, control) = self.owner.as_ref()?;
        Some((*media, control.upgrade()?))
    }
}
type VideoOwner = (MediaId, Arc<VideoControl>);
#[derive(Clone)]
struct Row {
    identity: Option<uuid::Uuid>,
    key: Option<Key>,
    duration: Duration,
    source: Option<VideoOwner>,
    selection: Option<MediaSource>,
    available: bool,
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
    selection: MediaSource,
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
    fn retains(&self, row: &Row, active: &Active) -> bool {
        if !row.available || row.identity != Some(active.identity) || row.key != Some(active.key) {
            return false;
        }
        if let Some(source) = &row.source {
            return Arc::ptr_eq(&source.1, &active.source.1) && active.lease.is_current();
        }
        // A frozen producer can still feed retained pixels after its authored node disappears.
        row.selection
            .and_then(|selection| self.routes.get(&selection))
            .is_some_and(|route| route.missing)
    }
    pub fn is_current(&self) -> bool {
        self.routes
            .values()
            .flat_map(|route| &route.guards)
            .all(golden_io::RevisionGuard::is_current)
    }
    pub fn empty() -> Arc<Self> {
        Arc::new(Self {
            rows: HashMap::new(),
            routes: HashMap::new(),
        })
    }
    pub fn capture(
        engine: &AppEngine,
        desired: &MatrixPlaybackSnapshot,
        index: &Configuration,
        version: u64,
        retained: Vec<MediaSource>,
        epochs: &HashMap<uuid::Uuid, u64>,
        previous: Option<&Self>,
    ) -> Result<Arc<Self>, String> {
        let overrides = desired
            .layers
            .iter()
            .map(|row| (row.layer, row))
            .collect::<HashMap<_, _>>();
        let mut routes = HashMap::new();
        let mut rows = HashMap::new();
        for &layer in &index.layers {
            let row = overrides.get(&layer);
            let (column, identity, transition, individual) = row.map_or(
                (desired.desired_column, desired.launch_id, desired.transition, false),
                |row| (row.column, row.launch_id, row.transition, row.individual),
            );
            let frozen = previous.and_then(|plan| plan.rows.get(&layer)).filter(|row| {
                row.identity == identity
                    && row
                        .key
                        .is_some_and(|key| Some(key.column) == column && key.individual == individual)
            });
            let source = if let Some(frozen) = frozen {
                frozen.selection
            } else {
                match column
                    .and_then(|column| index.cell_nodes.get(&(layer, column)))
                    .and_then(|id| engine.nodes.get(*id))
                {
                    Some(AppNode::MatrixCell(cell))
                        if cell.node_data().effective_enabled && cell.settings(engine).enabled =>
                    {
                        cell.selection(engine)?
                    }
                    _ => None,
                }
            };
            let selection = source;
            let source = selection
                .map(|selection| resolve(engine, &mut routes, selection))
                .transpose()?
                .flatten();
            rows.insert(
                layer,
                Row {
                    identity,
                    key: column.map(|column| Key {
                        column,
                        version: identity.and_then(|id| epochs.get(&id)).copied().unwrap_or(version),
                        individual,
                    }),
                    duration: Duration::from_millis(u64::from(transition.duration_ms)),
                    source,
                    selection,
                    available: true,
                },
            );
        }
        for selection in retained {
            resolve(engine, &mut routes, selection)?;
        }
        Ok(Arc::new(Self { rows, routes }))
    }
    pub fn refresh(
        &self,
        engine: &AppEngine,
        layers: &HashSet<LayerId>,
        retained: Vec<MediaSource>,
    ) -> Result<Arc<Self>, String> {
        let mut rows = self.rows.clone();
        let mut routes = HashMap::new();
        for (layer, row) in &mut rows {
            row.available = layers.contains(layer);
            row.source = if row.available {
                row.selection
                    .map(|selection| resolve(engine, &mut routes, selection))
                    .transpose()?
                    .flatten()
            } else {
                None
            };
        }
        for selection in retained {
            resolve(engine, &mut routes, selection)?;
        }
        Ok(Arc::new(Self { rows, routes }))
    }
    pub fn subscribe(&self, notifier: golden_io::WorkerNotifier) -> Vec<golden_io::RevisionSubscription> {
        self.routes
            .values()
            .flat_map(|route| &route.guards)
            .map(|guard| guard.subscribe(notifier.clone()))
            .collect()
    }
}
fn resolve(
    engine: &AppEngine,
    routes: &mut HashMap<MediaSource, Route>,
    selection: MediaSource,
) -> Result<Option<VideoOwner>, String> {
    if let std::collections::hash_map::Entry::Vacant(entry) = routes.entry(selection) {
        let mut guards = Vec::new();
        let resolved = selection.video_owner(engine, &mut guards)?;
        let missing = matches!(resolved, crate::media::VideoRoute::Missing);
        let owner = match resolved {
            crate::media::VideoRoute::Video(media, control) => Some((media, control)),
            _ => None,
        };
        let owner = owner.map(|(media, control)| (media, Arc::downgrade(&control)));
        entry.insert(Route { owner, missing, guards });
    }
    Ok(routes[&selection].owner())
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
                && prepared.retains(row, existing)
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
                                selection: row.selection.expect("resolved Video selection"),
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
    pub fn selections(&self) -> Vec<MediaSource> {
        self.active
            .values()
            .chain(self.outgoing.iter().map(|outgoing| &outgoing.active))
            .map(|active| active.selection)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn reconcile(&mut self, plan: &Prepared, orders: &HashMap<uuid::Uuid, u64>) -> Result<(), String> {
        let mut failure = None;
        let mut replacements = Vec::new();
        for (&layer, row) in &plan.rows {
            if self.active.get(&layer).is_some_and(|active| plan.retains(row, active)) {
                continue;
            }
            let next = match (row.identity, row.key, row.source.as_ref(), row.selection) {
                (Some(identity), Some(key), Some(source), Some(selection)) => {
                    match source.1.activate_for_matrix(identity) {
                        Ok(lease) => Some(Active {
                            identity,
                            key,
                            source: source.clone(),
                            lease,
                            order: orders.get(&identity).copied().unwrap_or(0),
                            selection,
                        }),
                        Err(error) => {
                            failure.get_or_insert(error);
                            None
                        }
                    }
                }
                _ => None,
            };
            replacements.push((layer, next));
        }
        let mut outgoing_replacements = Vec::new();
        for (index, outgoing) in self.outgoing.iter().enumerate() {
            if plan.rows.get(&outgoing.layer).is_some_and(|row| row.available)
                && plan
                    .routes
                    .get(&outgoing.active.selection)
                    .is_some_and(|route| route.missing)
            {
                continue;
            }
            let source = plan
                .routes
                .get(&outgoing.active.selection)
                .filter(|_| plan.rows.get(&outgoing.layer).is_some_and(|row| row.available))
                .and_then(Route::owner);
            if source
                .as_ref()
                .is_some_and(|source| Arc::ptr_eq(&source.1, &outgoing.active.source.1))
                && outgoing.active.lease.is_current()
            {
                continue;
            }
            let next =
                source
                    .as_ref()
                    .and_then(|source| match source.1.activate_for_matrix(outgoing.active.identity) {
                        Ok(lease) => Some((source.clone(), lease)),
                        Err(error) => {
                            failure.get_or_insert(error);
                            None
                        }
                    });
            outgoing_replacements.push((index, next));
        }
        // All incoming uses exist before any old owner can receive a final deactivation.
        for (layer, next) in replacements {
            self.active.remove(&layer);
            if let Some(next) = next {
                self.active.insert(layer, next);
            }
        }
        for (index, next) in outgoing_replacements.into_iter().rev() {
            if let Some((source, mut lease)) = next {
                if let Err(error) = lease.deactivate() {
                    failure.get_or_insert(error);
                }
                self.outgoing[index].active.source = source;
                self.outgoing[index].active.lease = lease;
            } else {
                self.outgoing.remove(index);
            }
        }
        failure.map_or(Ok(()), Err)
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
