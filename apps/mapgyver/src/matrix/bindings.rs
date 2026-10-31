//! Demand-admitted source references, independent of authored indices and native resources.
use super::{projection::CellSource, protocol::LayerId, timeline::Clip};
use crate::media::protocol::ChannelReference;
use std::collections::{HashMap, HashSet};

pub(super) const MAX_BINDINGS: usize = 4096;

#[derive(Default)]
pub(crate) struct Bindings {
    entries: HashMap<(uuid::Uuid, LayerId), Binding>,
}
struct Binding {
    owner: ChannelReference,
    source: Option<ChannelReference>,
}
pub(crate) struct BindingCapacity;
impl Bindings {
    pub fn resolve(
        &mut self,
        clip: Clip,
        layer: LayerId,
        owner: ChannelReference,
        candidate: Option<&CellSource>,
    ) -> Result<Option<ChannelReference>, BindingCapacity> {
        let key = (clip.id, layer);
        if let Some(binding) = self.entries.get(&key) {
            return Ok(binding.source);
        }
        if self.entries.len() == MAX_BINDINGS {
            return Err(BindingCapacity);
        }
        // A newly demanded row cannot display a source authored after this clip's epoch.
        // Remembering None also keeps a prepared empty cell empty until an explicit relaunch.
        let source = candidate
            .filter(|cell| cell.version <= clip.version)
            .map(|cell| cell.source);
        self.entries.insert(key, Binding { owner, source });
        Ok(source)
    }
    pub fn retain(&mut self, clips: impl Iterator<Item = uuid::Uuid>) {
        let clips = clips.collect::<HashSet<_>>();
        self.entries.retain(|(clip, _), _| clips.contains(clip));
    }
    pub fn layers(&self) -> HashSet<LayerId> {
        self.entries
            .iter()
            .filter_map(|((_, layer), binding)| binding.source.map(|_| *layer))
            .collect()
    }
    pub fn edges(&self) -> Vec<(ChannelReference, ChannelReference)> {
        self.entries
            .values()
            .filter_map(|binding| binding.source.map(|source| (binding.owner, source)))
            .collect()
    }
}
