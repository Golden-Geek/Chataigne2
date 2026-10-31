//! Bounded graph ordering and affine mapping run on the preparation worker.
use super::PreparedChannel;
use crate::media::resolution::ChannelAvailability;
use crate::media::{
    capture::{CapturedChannel, CapturedChannels},
    protocol::{ChannelReference, ChannelSource},
};
use golden_video::plan::{Layer, Point, ProjectiveMapping, Quad, SampleFilter, UnitRect};
use golden_video::{Extent2D, Opacity};
use std::collections::{HashMap, HashSet};

pub(super) fn ordered(
    capture: &CapturedChannels,
    roots: &[ChannelReference],
    mut budget: usize,
    mut guard_budget: usize,
) -> Result<(Vec<ChannelReference>, HashSet<ChannelReference>), String> {
    let channels = capture
        .channels
        .iter()
        .map(|channel| (channel.reference, channel))
        .collect::<HashMap<_, _>>();
    let mut states = HashMap::new();
    let mut rejected = HashSet::new();
    let mut result = Vec::new();
    for root in roots {
        let mut pending = vec![(*root, false)];
        while let Some((reference, exiting)) = pending.pop() {
            if exiting {
                states.insert(reference, 2u8);
                result.push(reference);
                continue;
            }
            match states.get(&reference) {
                Some(2) => continue,
                Some(1) => return Err("captured Composition dependency cycle".into()),
                _ => {}
            }
            let channel = channels
                .get(&reference)
                .ok_or("dependency was not captured from the backend")?;
            states.insert(reference, 1);
            pending.push((reference, true));
            if channel.enabled
                && channel
                    .definition
                    .as_ref()
                    .is_some_and(|definition| matches!(definition.source, ChannelSource::Composition { .. }))
            {
                match &channel.instances {
                    Some(instances) if instances.len() <= budget && channel.instance_guards.len() <= guard_budget => {
                        budget -= instances.len();
                        guard_budget -= channel.instance_guards.len();
                        pending.extend(
                            instances
                                .iter()
                                .rev()
                                .filter_map(|instance| instance.source)
                                .map(|source| (source, false)),
                        );
                    }
                    _ => {
                        rejected.insert(reference);
                    }
                }
            }
        }
    }
    Ok((result, rejected))
}

pub(super) fn layers(
    channel: &CapturedChannel,
    canvas: Extent2D,
    filter: SampleFilter,
    prepared: &HashMap<ChannelReference, PreparedChannel>,
) -> Result<Vec<Layer>, String> {
    let mut result = Vec::new();
    for instance in channel
        .instances
        .as_ref()
        .ok_or("Composition instance admission failed")?
    {
        let Some(source) = instance.source.and_then(|reference| prepared.get(&reference)) else {
            continue;
        };
        if source.resolution.availability != ChannelAvailability::Ready {
            continue;
        }
        let extent = source.resolution.extent.ok_or("ready source has no extent")?;
        let transform = instance.transform;
        let (sin, cos) = f64::from(transform.rotation).sin_cos();
        let size = [f64::from(extent.width()), f64::from(extent.height())];
        let dimensions = [f64::from(canvas.width()), f64::from(canvas.height())];
        let corners = [[0.0, 0.0], [size[0], 0.0], size, [0.0, size[1]]].map(|point| {
            let x = (point[0] - f64::from(transform.pivot[0]) * size[0]) * f64::from(transform.scale[0]);
            let y = (point[1] - f64::from(transform.pivot[1]) * size[1]) * f64::from(transform.scale[1]);
            Point {
                x: ((f64::from(transform.position[0]) + cos * x - sin * y) / dimensions[0]) as f32,
                y: ((f64::from(transform.position[1]) + sin * x + cos * y) / dimensions[1]) as f32,
            }
        });
        let mapping = ProjectiveMapping::new(UnitRect::FULL, Quad { corners }).map_err(|error| error.to_string())?;
        result.push(Layer {
            source: source.target.into(),
            opacity: Opacity::new(instance.opacity).map_err(|error| error.to_string())?,
            blend: instance.blend,
            filter,
            mapping: Some(mapping),
        });
    }
    Ok(result)
}

pub(super) fn reachable(
    capture: &CapturedChannels,
    roots: &[ChannelReference],
    rejected: &HashSet<ChannelReference>,
) -> HashSet<ChannelReference> {
    let channels = capture
        .channels
        .iter()
        .map(|channel| (channel.reference, channel))
        .collect::<HashMap<_, _>>();
    let mut live = HashSet::new();
    let mut pending = roots.to_vec();
    while let Some(reference) = pending.pop() {
        if !live.insert(reference) || rejected.contains(&reference) {
            continue;
        }
        if let Some(instances) = channels.get(&reference).and_then(|channel| channel.instances.as_ref()) {
            pending.extend(instances.iter().filter_map(|instance| instance.source));
        }
    }
    live
}
