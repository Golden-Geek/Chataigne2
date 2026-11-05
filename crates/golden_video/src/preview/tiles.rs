//! Per-source live placement inside one logical passive preview view.
use super::{ResolvedTileGroup, ResolvedView};
use crate::{
    OutputId, TextureId,
    plan::{BlitRegion, RegionPlacement, SampleFilter},
};
use golden_io::RevisionGuard;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub(super) type Placements = HashMap<(Uuid, String), RegionPlacement>;

pub(super) fn same_routing(old: &ResolvedView, new: &ResolvedView) -> bool {
    match (&old.tile_groups, &new.tile_groups) {
        (None, None) => true,
        (Some(old), Some(new)) => {
            old.len() == new.len()
                && old
                    .iter()
                    .zip(new.iter())
                    .all(|(old, new)| old.source == new.source && old.capacity == new.capacity && old.fit == new.fit)
        }
        _ => false,
    }
}

pub(super) fn update(placements: &mut Placements, views: &[ResolvedView]) {
    let mut wanted = HashSet::new();
    for view in views {
        for group in view.tile_groups.iter().flat_map(|groups| groups.iter()) {
            let key = (view.id, group.source.clone());
            wanted.insert(key.clone());
            if placements.get(&key).is_some_and(|placement| {
                placement.tile_capacity() != group.capacity || placement.tile_fit() != group.fit
            }) && let Some(old) = placements.remove(&key)
            {
                assert!(old.update_tiles(&[]));
            }
            let placement = placements.entry(key).or_insert_with(|| {
                RegionPlacement::with_tile_capacity(view.destination, view.source_rect, group.capacity, group.fit)
                    .expect("validated tile group capacity")
            });
            assert!(placement.update_tiles(&group.tiles));
        }
    }
    placements.retain(|key, placement| {
        if wanted.contains(key) {
            true
        } else {
            assert!(placement.update_tiles(&[]));
            false
        }
    });
}

pub(super) fn regions(
    view: Uuid,
    groups: &[ResolvedTileGroup],
    placements: &Placements,
    output: OutputId,
    visibility: &RevisionGuard,
    resolve: &mut impl FnMut(&str) -> Option<TextureId>,
) -> Vec<BlitRegion> {
    groups
        .iter()
        .filter_map(|group| {
            let placement = placements.get(&(view, group.source.clone()))?;
            if !placement.bind_presentation_output(output) {
                return None;
            }
            Some(BlitRegion {
                source: resolve(&group.source)?,
                destination: crate::plan::UnitRect::FULL,
                source_rect: crate::plan::UnitRect::FULL,
                filter: SampleFilter::Linear,
                visibility: Some(visibility.clone()),
                placement: Some(placement.clone()),
            })
        })
        .collect()
}
