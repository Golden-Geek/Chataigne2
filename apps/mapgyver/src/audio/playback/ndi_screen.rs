//! Route Golden's rendered virtual buses into active Screen publishers.
use super::Layout;
use crate::screens::{NdiAudioRegistry, ScreenAudio};
use golden_audio::{AudioChannelId, VirtualOutputTapReader};
use golden_core::parameter::ParameterEnumOptions;
use std::{collections::HashMap, sync::Arc};

const TAP_SLOTS: usize = 8;
const SMPTE_REFERENCE: f32 = 10.0;

pub(in crate::audio) fn tap(
    config: &golden_audio::AudioEngineConfig,
    limits: &golden_audio::EngineLimits,
) -> Result<(golden_audio::VirtualOutputTapWriter, VirtualOutputTapReader), golden_audio::AudioError> {
    golden_audio::virtual_output_tap(config, limits, TAP_SLOTS)
}

struct Export {
    screen: Arc<ScreenAudio>,
    channels: usize,
    lanes: Vec<(usize, usize)>,
}

#[derive(Default)]
pub(super) struct Mix {
    exports: Vec<Export>,
    lanes: HashMap<AudioChannelId, usize>,
}

impl Mix {
    pub fn refresh(&mut self, registry: Option<&NdiAudioRegistry>, options: &ParameterEnumOptions) {
        self.exports.clear();
        for screen in registry.into_iter().flat_map(NdiAudioRegistry::owners) {
            screen.routing.update_options(options);
            self.exports.push(Export {
                screen,
                channels: 0,
                lanes: Vec::new(),
            });
        }
    }

    pub fn service(
        &mut self,
        layout: Option<&Layout>,
        reader: &mut VirtualOutputTapReader,
    ) -> &HashMap<AudioChannelId, usize> {
        self.lanes.clear();
        if let Some(layout) = layout {
            self.lanes.clone_from(&layout.lanes);
            for export in &mut self.exports {
                export.channels = 0;
                export.lanes.clear();
                if !export.screen.stream.is_connected() {
                    continue;
                }
                export.screen.routing.visit_ndi_routes(|output, source| {
                    export.channels = export.channels.max(output + 1);
                    if let Some(id) = source
                        && let Some(&lane) = layout.virtual_lanes.get(&id)
                    {
                        self.lanes.insert(id, lane);
                        export.lanes.push((output, lane));
                    }
                });
                export.lanes.sort_unstable();
                export.lanes.dedup();
            }
        }
        reader.set_enabled(layout.is_some() && self.exports.iter().any(|export| export.channels != 0));
        reader.drain(TAP_SLOTS, |block| {
            if !layout.is_some_and(|layout| block.generation == layout.generation.get()) {
                return;
            }
            for export in self.exports.iter().filter(|export| export.channels != 0) {
                let mut planes = vec![0.0; export.channels * block.frames];
                for &(output, lane) in &export.lanes {
                    let start = lane * block.frames;
                    let Some(source) = block.samples.get(start..start + block.frames) else {
                        continue;
                    };
                    let target = &mut planes[output * block.frames..(output + 1) * block.frames];
                    for (target, source) in target.iter_mut().zip(source) {
                        *target += source * SMPTE_REFERENCE;
                    }
                }
                export
                    .screen
                    .stream
                    .publish(block.sample_rate, export.channels, block.starts_at, planes);
            }
        });
        &self.lanes
    }
}
