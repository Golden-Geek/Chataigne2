use super::{ChannelRoute, Entry, PreparedChannel, SourceObservations};
use crate::media::capture::CapturedChannels;
use crate::media::protocol::{ChannelReference, ChannelSource};
use crate::media::resolution::{self, ChannelAvailability, ChannelResolution, SourceObservation};
use golden_video::plan::{
    ExternalTexture, PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan, SpoutSource,
};
use golden_video::runtime::PlanSubmission;
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, PassId, PixelFormat, TextureDescriptor, TextureId,
};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

pub(super) struct Prepared {
    pub submission: PlanSubmission,
    pub entries: HashMap<ChannelReference, Entry>,
    pub channels: Vec<PreparedChannel>,
}

fn allocate(next: &mut u64) -> Result<TextureId, String> {
    *next = next
        .checked_add(1)
        .ok_or("channel runtime texture identity exhausted")?;
    Ok(TextureId::new(*next).expect("checked nonzero texture identity"))
}

pub(super) fn build(
    capture: &CapturedChannels,
    routes: &[ChannelRoute],
    previous: &HashMap<ChannelReference, Entry>,
    next: &mut u64,
    limits: PlanLimits,
    observations: &SourceObservations,
    pictures: Option<&crate::media::picture::Pictures>,
) -> Result<Prepared, String> {
    if routes.len() > limits.max_outputs || capture.channels.len() > limits.max_passes {
        return Err("channel scene exceeds admitted route/channel counts".into());
    }
    let captured = capture
        .channels
        .iter()
        .map(|channel| (channel.reference, channel))
        .collect::<HashMap<_, _>>();
    let mut seen_outputs = HashSet::new();
    let mut entries = HashMap::new();
    let mut channels = Vec::new();
    let mut submission = PlanSubmission::new(RenderPlan::default());
    submission.guards = capture.guards.clone();
    let mut guards = capture.guards.to_vec();
    let mut image_sources = HashMap::new();
    for route in routes {
        if !seen_outputs.insert(route.output) {
            return Err("output routes must be unique".into());
        }
        if let std::collections::hash_map::Entry::Vacant(slot) = entries.entry(route.channel) {
            let channel = captured
                .get(&route.channel)
                .ok_or("route was not captured from the backend")?;
            let mut entry = match previous.get(&route.channel) {
                Some(entry) => entry.clone(),
                None => Entry {
                    target: allocate(next)?,
                    source: None,
                },
            };
            let mut resolution = ChannelResolution {
                extent: None,
                availability: ChannelAvailability::Missing,
            };
            let mut operation = RenderOperation::Clear(LinearColor::TRANSPARENT);
            let source = channel
                .definition
                .as_ref()
                .filter(|_| channel.enabled)
                .map(|definition| &definition.source);
            match source {
                Some(source @ ChannelSource::Picture { asset }) => {
                    let value = asset.and_then(|id| pictures.and_then(|pictures| pictures.get(id)));
                    resolution = resolution::resolve(
                        channel.definition.as_ref().unwrap(),
                        value.map_or(SourceObservation::Unavailable, |value| {
                            SourceObservation::Available(value.value().descriptor())
                        }),
                    )?;
                    entry.source = None;
                    if let Some(value) = value {
                        let asset = value.id();
                        let id = if let Some(id) = image_sources.get(&asset) {
                            *id
                        } else {
                            let id = match previous.get(&route.channel).and_then(|entry| entry.source.as_ref()) {
                                Some((id, configured)) if configured == source => *id,
                                _ => allocate(next)?,
                            };
                            submission.plan.external_textures.push(ExternalTexture {
                                id,
                                descriptor: value.value().descriptor(),
                                source: Some(golden_video::plan::ExternalSource::Image(value.clone())),
                            });
                            image_sources.insert(asset, id);
                            id
                        };
                        entry.source = Some((id, source.clone()));
                        operation = RenderOperation::Sample {
                            source: id,
                            filter: channel.definition.as_ref().unwrap().filter,
                        };
                    } else if asset
                        .and_then(|id| pictures.and_then(|pictures| pictures.failure(id)))
                        .is_some_and(|error| {
                            matches!(
                                error,
                                golden_assets::PreparationFailure::Decode
                                    | golden_assets::PreparationFailure::SourceTooLarge
                            )
                        })
                    {
                        resolution.availability = ChannelAvailability::Incompatible;
                    }
                }
                Some(ChannelSource::SolidColor { rgba }) => {
                    entry.source = None;
                    resolution =
                        resolution::resolve(channel.definition.as_ref().unwrap(), SourceObservation::Unavailable)?;
                    operation = RenderOperation::Clear(
                        LinearColor::from_straight(rgba[0], rgba[1], rgba[2], rgba[3])
                            .map_err(|error| error.to_string())?,
                    );
                }
                Some(
                    source @ ChannelSource::Spout {
                        sender: Some(sender),
                        maximum,
                    },
                ) => {
                    let id = match &entry.source {
                        Some((id, configured)) if configured == source => *id,
                        _ => allocate(next)?,
                    };
                    entry.source = Some((id, source.clone()));
                    let observed = observations.sources.iter().find(|source| source.texture == id);
                    if let Some(observed) = observed {
                        guards.push(observed.guard.clone());
                    }
                    let actual = observed.and_then(|source| source.descriptor);
                    let definition = channel.definition.as_ref().unwrap();
                    resolution = resolution::resolve(
                        definition,
                        actual.map_or(SourceObservation::Unavailable, SourceObservation::Available),
                    )?;
                    if observed.is_some_and(|source| source.incompatible) {
                        resolution.availability = ChannelAvailability::Incompatible;
                    }
                    submission.plan.external_textures.push(ExternalTexture {
                        id,
                        descriptor: actual
                            .filter(|_| resolution.availability == ChannelAvailability::Ready)
                            .unwrap_or(*maximum),
                        source: Some(
                            SpoutSource {
                                sender: sender.clone(),
                                descriptor: *maximum,
                                poll_interval: Duration::from_millis(16),
                                frame_count: golden_video::plan::SpoutFrameCount::Unversioned,
                            }
                            .into(),
                        ),
                    });
                    submission.source_demands.push(id);
                    if resolution.availability == ChannelAvailability::Ready {
                        operation = RenderOperation::Sample {
                            source: id,
                            filter: definition.filter,
                        };
                    }
                }
                Some(ChannelSource::Spout { sender: None, .. }) => {
                    entry.source = None;
                    resolution =
                        resolution::resolve(channel.definition.as_ref().unwrap(), SourceObservation::Unavailable)?;
                }
                None => {
                    entry.source = None;
                    if let Some(definition) = &channel.definition {
                        resolution = resolution::resolve(definition, SourceObservation::Unavailable)?;
                        resolution.availability = ChannelAvailability::Disabled;
                    }
                }
            }
            let extent = resolution.extent.unwrap_or(Extent2D::new(1, 1).unwrap());
            submission.plan.passes.push(RenderPass {
                id: PassId::new(entry.target.get()).expect("nonzero target identity"),
                output: entry.target,
                descriptor: TextureDescriptor {
                    extent,
                    format: PixelFormat::Rgba8Unorm,
                    color_encoding: ColorEncoding::Linear,
                    alpha_mode: AlphaMode::Premultiplied,
                },
                operation,
            });
            channels.push(PreparedChannel {
                reference: route.channel,
                resolution,
                target: entry.target,
                source: entry.source.as_ref().map(|(id, _)| *id),
            });
            slot.insert(entry);
        }
        submission.plan.outputs.push(PlanOutput {
            id: route.output,
            source: entries[&route.channel].target,
        });
    }
    // Pure preparation validation, before publication can change a live runtime's desired plan.
    submission
        .plan
        .clone()
        .compile_with_source_demands(&limits, &submission.source_demands)
        .map_err(|error| error.to_string())?;
    submission.guards = std::sync::Arc::from(guards);
    Ok(Prepared {
        submission,
        entries,
        channels,
    })
}
