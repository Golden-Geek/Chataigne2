//! Live NDI sources share the project receiver with the renderer and mix off the realtime worker.
use super::{Layout, frame_nanos};
use crate::{
    app::AppNode,
    audio::voice::Binding,
    media::{
        authoring::effective_definition,
        protocol::{ChannelId, ChannelReference, ChannelSource, MediaId},
        rendering::ndi::{AudioInput, Inputs},
    },
};
use golden_audio::GainSmoother;
use golden_core::{
    application::ProductionRuntime,
    node::{Node, NodeUuid},
    parameter::ParameterEnumOptions,
};
use golden_io::RevisionGuard;
use golden_video::ndi::NdiAudioFrame;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

const MAX_SOURCES: usize = 8;
const BUFFERED_PACKETS: usize = 12;
// NDI ±1 is -20 dBFS at SMPTE reference level.
const SMPTE_FULL_SCALE: f32 = 0.1;

#[derive(Default)]
pub(super) struct Mix {
    voices: HashMap<ChannelReference, Voice>,
}

struct Selected {
    reference: ChannelReference,
    sender: String,
    binding: Arc<Binding>,
    channel: RevisionGuard,
    media: RevisionGuard,
}

struct Voice {
    selected: Selected,
    input: AudioInput,
    epoch: u64,
    packets: VecDeque<(Instant, NdiAudioFrame)>,
    lanes: [Option<usize>; 2],
    gain: GainSmoother,
}

impl Mix {
    pub fn clear(&mut self) {
        self.voices.clear();
    }

    pub fn observe(
        &mut self,
        runtime: &ProductionRuntime<AppNode>,
        inputs: &Inputs,
        layout: &Layout,
        options: &ParameterEnumOptions,
        refresh: bool,
        now: Instant,
    ) -> (bool, bool) {
        let mut changed = false;
        self.voices.retain(|_, voice| {
            let usable = voice.selected.channel.is_current()
                && voice.selected.media.is_current()
                && voice.selected.binding.routing().is_some_and(|route| {
                    route.enabled
                        && route
                            .destinations
                            .iter()
                            .flatten()
                            .any(|id| layout.virtual_lanes.contains_key(id))
                });
            changed |= !usable;
            usable
        });
        if refresh {
            changed |= self.reconcile(runtime, inputs, layout, options);
        }
        for voice in self.voices.values_mut() {
            let Some(routing) = voice.selected.binding.routing().filter(|route| route.enabled) else {
                changed |= voice.lanes != [None, None];
                voice.lanes = [None, None];
                continue;
            };
            let lanes = routing
                .destinations
                .map(|id| id.and_then(|id| layout.virtual_lanes.get(&id).copied()));
            changed |= voice.lanes != lanes;
            voice.lanes = lanes;
            let gain = routing.gain.to_linear() * SMPTE_FULL_SCALE;
            if voice.gain.target() != gain {
                voice.gain.set_target(gain, 64);
            }
            let mut arrived = VecDeque::new();
            let epoch = voice.input.drain(&mut arrived);
            if epoch != voice.epoch {
                voice.epoch = epoch;
                voice.packets.clear();
                changed = true;
            }
            voice.packets.append(&mut arrived);
            while voice.packets.len() > BUFFERED_PACKETS
                || voice
                    .packets
                    .front()
                    .is_some_and(|(at, packet)| packet_end(*at, packet) <= now)
            {
                voice.packets.pop_front();
            }
        }
        (self.voices.values().any(|voice| voice.lanes != [None, None]), changed)
    }

    fn reconcile(
        &mut self,
        runtime: &ProductionRuntime<AppNode>,
        inputs: &Inputs,
        layout: &Layout,
        options: &ParameterEnumOptions,
    ) -> bool {
        let mut bindings = Vec::new();
        for (id, binding) in inputs.audio_bindings() {
            binding.update_options(options);
            if bindings.len() < MAX_SOURCES
                && binding.routing().is_some_and(|route| {
                    route.enabled
                        && route
                            .destinations
                            .iter()
                            .flatten()
                            .any(|id| layout.virtual_lanes.contains_key(id))
                })
            {
                bindings.push((id, binding));
            }
        }
        let desired = if bindings.is_empty() {
            Vec::new()
        } else {
            let Ok(desired) = runtime.capture_app(move |engine| {
                bindings
                    .into_iter()
                    .filter_map(|(uuid, binding)| {
                        let id = engine.node_id_by_uuid(NodeUuid(uuid))?;
                        let AppNode::MediaChannel(channel) = engine.nodes.get(id)? else {
                            return None;
                        };
                        let AppNode::Media(media) = engine.nodes.get(channel.node_data().parent?)? else {
                            return None;
                        };
                        let ChannelSource::Ndi { sender: Some(sender) } =
                            effective_definition(engine, channel.as_ref()).ok()?.source
                        else {
                            return None;
                        };
                        Some(Selected {
                            reference: ChannelReference {
                                media: MediaId(media.node_data().meta.uuid),
                                channel: ChannelId(NodeUuid(uuid)),
                            },
                            sender,
                            binding,
                            channel: channel.guard(),
                            media: media.guard(),
                        })
                    })
                    .collect::<Vec<_>>()
            }) else {
                return false;
            };
            desired
        };
        let before = self.voices.len();
        self.voices.retain(|reference, voice| {
            desired.iter().any(|wanted| {
                wanted.reference == *reference
                    && wanted.sender == voice.selected.sender
                    && Arc::ptr_eq(&wanted.binding, &voice.selected.binding)
            })
        });
        let mut changed = self.voices.len() != before;
        for selected in desired {
            if let Some(voice) = self.voices.get_mut(&selected.reference) {
                voice.selected = selected;
            } else if let Ok(input) = inputs.audio(selected.reference, &selected.sender) {
                self.voices.insert(
                    selected.reference,
                    Voice {
                        selected,
                        input,
                        epoch: 0,
                        packets: VecDeque::new(),
                        lanes: [None, None],
                        gain: GainSmoother::settled(0.0),
                    },
                );
                changed = true;
            }
        }
        changed
    }

    pub fn render(&mut self, output: &mut [f32], present: Instant, frames: usize, rate: u32) {
        for voice in self.voices.values_mut() {
            if voice.lanes == [None, None] || !voice.selected.channel.is_current() || !voice.selected.media.is_current()
            {
                continue;
            }
            voice.render(output, present, frames, rate);
        }
    }
}

fn packet_end(start: Instant, packet: &NdiAudioFrame) -> Instant {
    start + Duration::from_nanos(frame_nanos(packet.frames() as u64, packet.sample_rate().get()))
}

impl Voice {
    fn render(&mut self, output: &mut [f32], present: Instant, frames: usize, rate: u32) {
        if self.packets.is_empty() {
            return;
        }
        let mut packet_index = 0;
        for frame in 0..frames {
            let instant = present + Duration::from_nanos(frame_nanos(frame as u64, rate));
            while self
                .packets
                .get(packet_index)
                .is_some_and(|(at, packet)| packet_end(*at, packet) <= instant)
            {
                packet_index += 1;
            }
            let Some((at, packet)) = self.packets.get(packet_index) else {
                break;
            };
            let gain = self.gain.next_gain();
            let Some(offset) = instant.checked_duration_since(*at) else {
                continue;
            };
            let sample = offset.as_secs_f64() * f64::from(packet.sample_rate().get());
            let before = (sample as usize).min(packet.frames() - 1);
            let after = (before + 1).min(packet.frames() - 1);
            let fraction = (sample - before as f64) as f32;
            for (channel, lane) in self.lanes.iter().enumerate() {
                if let Some(lane) = lane
                    && let Some(plane) = packet.plane(channel)
                {
                    output[lane * frames + frame] += (plane[before] + (plane[after] - plane[before]) * fraction) * gain;
                }
            }
        }
    }
}
