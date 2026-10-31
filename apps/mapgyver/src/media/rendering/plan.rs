use super::{ChannelRoute, Entry, PreparedChannel, ScreenRoute, SourceObservations};
use crate::media::capture::CapturedChannels;
use crate::media::protocol::{ChannelReference, ChannelSource};
use crate::media::resolution::{self, ChannelAvailability, ChannelResolution, SourceObservation};
use crate::screens::protocol::ScreenId;
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
    pub screens: HashMap<ScreenId, TextureId>,
}

pub(super) struct Previous<'a> {
    pub channels: &'a HashMap<ChannelReference, Entry>,
    pub screens: &'a HashMap<ScreenId, TextureId>,
}

fn allocate(next: &mut u64) -> Result<TextureId, String> {
    *next = next
        .checked_add(1)
        .ok_or("channel runtime texture identity exhausted")?;
    Ok(TextureId::new(*next).expect("checked nonzero texture identity"))
}

pub(super) struct Targets<'a> {
    pub routes: &'a [ChannelRoute],
    pub preview: Option<&'a super::PreviewLayout>,
    pub screens: &'a [ScreenRoute],
}

pub(super) fn build(
    capture: &CapturedChannels,
    targets: Targets<'_>,
    previous: Previous<'_>,
    next: &mut u64,
    limits: PlanLimits,
    observations: &SourceObservations,
    pictures: Option<&crate::media::picture::Pictures>,
) -> Result<Prepared, String> {
    let Targets {
        routes,
        preview,
        screens,
    } = targets;
    let screen_count = screens
        .iter()
        .map(|route| route.screen.id)
        .collect::<HashSet<_>>()
        .len();
    if routes
        .len()
        .saturating_add(screens.iter().filter(|route| route.output.is_some()).count())
        > limits.max_outputs
        || capture.channels.len().saturating_add(screen_count) > limits.max_passes
    {
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
    }
    let mut references = routes.iter().map(|route| route.channel).collect::<Vec<_>>();
    if let Some(preview) = preview {
        references.extend(preview.references()?);
    }
    for route in screens {
        if route.screen.project != capture.project || !route.screen.is_current() {
            return Err("Screen belongs to a stale/different project".into());
        }
        guards.extend(route.screen.guards.iter().cloned());
        references.extend(route.screen.references());
    }
    for reference in references {
        if let std::collections::hash_map::Entry::Vacant(slot) = entries.entry(reference) {
            let channel = captured
                .get(&reference)
                .ok_or("route was not captured from the backend")?;
            let mut entry = match previous.channels.get(&reference) {
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
                            let id = match previous
                                .channels
                                .get(&reference)
                                .and_then(|entry| entry.source.as_ref())
                            {
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
                Some(ChannelSource::Composition { .. }) => {
                    // Authoring is available before the demanded Composition producer is integrated.
                    // Retain the same truthful unavailable-channel presentation as missing media.
                    entry.source = None;
                    resolution =
                        resolution::resolve(channel.definition.as_ref().unwrap(), SourceObservation::Unavailable)?;
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
                reference,
                resolution,
                target: entry.target,
                source: entry.source.as_ref().map(|(id, _)| *id),
            });
            slot.insert(entry);
        }
    }
    for route in routes {
        submission.plan.outputs.push(PlanOutput {
            id: route.output,
            source: entries[&route.channel].target,
        });
    }
    let mut screen_entries = HashMap::new();
    let mut screen_extents = HashMap::new();
    let source_targets = entries
        .iter()
        .map(|(reference, entry)| (*reference, entry.target))
        .collect();
    for route in screens {
        if route.output.is_some_and(|output| !seen_outputs.insert(output)) {
            return Err("Screen output duplicates an output route".into());
        }
        if let std::collections::hash_map::Entry::Vacant(slot) = screen_entries.entry(route.screen.id) {
            let target = match previous.screens.get(&route.screen.id) {
                Some(target) => *target,
                None => allocate(next)?,
            };
            slot.insert(target);
            screen_extents.insert(route.screen.id, route.extent);
            submission.plan.passes.push(RenderPass {
                id: PassId::new(target.get()).unwrap(),
                output: target,
                descriptor: TextureDescriptor {
                    extent: route.extent,
                    format: PixelFormat::Rgba8Unorm,
                    color_encoding: ColorEncoding::Linear,
                    alpha_mode: AlphaMode::Premultiplied,
                },
                operation: RenderOperation::Composite {
                    layers: route.screen.layers(&source_targets)?,
                },
            });
        } else if screen_extents.get(&route.screen.id) != Some(&route.extent) {
            return Err("a Screen may have only one authored output extent per generation".into());
        }
        if let Some(output) = route.output {
            submission.plan.outputs.push(PlanOutput {
                id: output,
                source: screen_entries[&route.screen.id],
            });
        }
    }
    if let Some(preview) = preview {
        if !seen_outputs.insert(preview.output) {
            return Err("preview output duplicates an output route".into());
        }
        preview.append(&mut submission, &source_targets, &screen_entries)?;
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
        screens: screen_entries,
    })
}
