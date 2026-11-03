//! Demand-admitted source references, independent of authored indices and native resources.
use super::{projection::CellSource, protocol::LayerId, timeline::Clip};
use crate::media::{MediaSource, protocol::ChannelReference};
use std::collections::{HashMap, HashSet};

pub(super) const MAX_BINDINGS: usize = 4096;

#[derive(Default)]
pub(crate) struct Bindings {
    entries: HashMap<(uuid::Uuid, LayerId), Binding>,
}
struct Binding {
    owner: ChannelReference,
    source: Option<MediaSource>,
    output: Option<ChannelReference>,
}
pub(crate) struct BindingCapacity;
impl Bindings {
    pub fn retain_frame(&mut self, frame: &super::timeline::Frame) {
        self.entries
            .retain(|(clip, layer), _| frame.layer(*layer).clips.iter().any(|entry| entry.clip.id == *clip));
    }
    pub fn resolve(
        &mut self,
        clip: Clip,
        layer: LayerId,
        owner: ChannelReference,
        candidate: Option<&CellSource>,
    ) -> Result<Option<MediaSource>, BindingCapacity> {
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
        self.entries.insert(
            key,
            Binding {
                owner,
                source,
                output: source.and_then(|source| {
                    source.channel.map(|channel| ChannelReference {
                        media: source.media,
                        channel,
                    })
                }),
            },
        );
        Ok(source)
    }
    pub fn output(&mut self, clip: Clip, layer: LayerId, output: Option<ChannelReference>) -> Option<ChannelReference> {
        let binding = self.entries.get_mut(&(clip.id, layer)).expect("resolved clip binding");
        if output.is_some() {
            binding.output = output;
        }
        binding.output
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
            .filter_map(|binding| binding.output.map(|source| (binding.owner, source)))
            .collect()
    }
}
