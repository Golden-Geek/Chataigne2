//! Runtime-only demanded producer graphs, independent of authored nodes and native leases.
use super::{
    capture::CapturedChannel,
    protocol::{ChannelReference, MediaId},
};
use crate::matrix::playback::RuntimeBinding;
use golden_io::{RevisionGuard, RevisionSource};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

// Matches the capture/clip metadata allowance; pixels and GPU resources have separate owners.
const MAX_CHANNELS: usize = 4096;

#[derive(Default)]
pub(crate) struct RuntimeSources {
    state: Mutex<State>,
    revision: RevisionSource,
}
#[derive(Default)]
struct State {
    channels: HashMap<ChannelReference, Arc<CapturedChannel>>,
    owners: HashMap<MediaId, RuntimeBinding>,
}
impl RuntimeSources {
    pub fn register(&self, media: MediaId, owner: RuntimeBinding) -> Result<(), String> {
        let mut state = self.state.lock().expect("source retention lock poisoned");
        if owner.sources().is_none_or(|sources| sources.is_empty()) {
            return Ok(());
        }
        if !state.owners.contains_key(&media) && state.owners.len() == MAX_CHANNELS {
            Self::roots(&mut state.owners);
            if state.owners.len() == MAX_CHANNELS {
                return Err("retained producer registry exceeds 4096 consumers".into());
            }
        }
        state.owners.insert(media, owner);
        Ok(())
    }
    pub fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(super) fn snapshot(&self) -> HashMap<ChannelReference, Arc<CapturedChannel>> {
        self.state
            .lock()
            .expect("source retention lock poisoned")
            .channels
            .clone()
    }
    pub(super) fn update(&self, channels: &[CapturedChannel]) -> Result<(), String> {
        let mut state = self.state.lock().expect("source retention lock poisoned");
        let mut next = state.channels.clone();
        for channel in channels {
            if channel.definition.is_none() {
                continue;
            }
            let mut channel = channel.clone();
            // Authored-node guards expire on removal. The owning clip/project fences this graph.
            channel.instance_guards.clear();
            if let Some(matrix) = &mut channel.matrix {
                matrix.control = None;
            }
            next.insert(channel.reference, Arc::new(channel));
        }
        let roots = Self::roots(&mut state.owners);
        let keep = reachable(&next, roots);
        next.retain(|key, _| keep.contains(key));
        if next.len() > MAX_CHANNELS {
            return Err("retained producer graph exceeds 4096 channels".into());
        }
        let elements = next
            .values()
            .map(|channel| {
                channel
                    .instances
                    .as_ref()
                    .map_or(0, Vec::len)
                    .saturating_add(channel.matrix.as_ref().map_or(0, |matrix| matrix.inputs.len()))
            })
            .fold(0usize, usize::saturating_add);
        if elements > MAX_CHANNELS {
            return Err("retained producer graph exceeds 4096 placement/input elements".into());
        }
        if state.channels.keys().any(|key| !next.contains_key(key)) {
            self.revision.advance();
        }
        state.channels = next;
        Ok(())
    }
    pub fn prune(&self) {
        // Pruning adds no records, so it cannot exceed the admitted metadata capacity.
        self.update(&[]).expect("source pruning cannot grow the graph");
    }
    fn roots(owners: &mut HashMap<MediaId, RuntimeBinding>) -> Vec<ChannelReference> {
        let mut roots = Vec::new();
        owners.retain(|_, owner| {
            if let Some(sources) = owner.sources()
                && !sources.is_empty()
            {
                roots.extend(sources);
                true
            } else {
                false
            }
        });
        roots
    }
    pub fn edges(
        &self,
        bindings: Vec<(ChannelReference, ChannelReference)>,
    ) -> Vec<(ChannelReference, ChannelReference)> {
        let state = self.state.lock().expect("source retention lock poisoned");
        let mut result = Vec::new();
        for (owner, source) in bindings {
            result.extend(
                frontier(&state.channels, source)
                    .into_iter()
                    .map(|source| (owner, source)),
            );
        }
        result
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.state.lock().unwrap().channels.len()
    }
}
fn reachable(
    channels: &HashMap<ChannelReference, Arc<CapturedChannel>>,
    roots: impl IntoIterator<Item = ChannelReference>,
) -> HashSet<ChannelReference> {
    let mut keep = HashSet::new();
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    while let Some(key) = pending.pop() {
        if keep.insert(key)
            && let Some(channel) = channels.get(&key)
        {
            pending.extend(channel.dependencies());
        }
    }
    keep
}

// External producers remain authored/live; only removed owned subgraphs need expansion.
fn frontier(
    channels: &HashMap<ChannelReference, Arc<CapturedChannel>>,
    root: ChannelReference,
) -> HashSet<ChannelReference> {
    let mut result = HashSet::from([root]);
    let mut seen = HashSet::new();
    let mut pending = vec![root];
    while let Some(key) = pending.pop() {
        if !seen.insert(key) {
            continue;
        }
        let Some(channel) = channels.get(&key).filter(|channel| channel.owned) else {
            result.insert(key);
            continue;
        };
        let dependencies = channel.dependencies().collect::<Vec<_>>();
        if dependencies.is_empty() {
            result.insert(key);
        } else {
            pending.extend(dependencies);
        }
    }
    result
}
