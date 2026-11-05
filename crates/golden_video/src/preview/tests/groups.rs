use super::*;
use crate::runtime::{CompletionToken, DeviceGeneration};
use protocol::PreviewTileGroup;

fn tiled() -> PreviewView {
    let mut definition = view();
    definition.rect = CssRect {
        x: 0.,
        y: 0.,
        width: 20.,
        height: 10.,
    };
    definition.clip = definition.rect;
    definition.tile_groups = Some(vec![PreviewTileGroup {
        source: "first-source".into(),
        fit: crate::plan::RegionFit::Stretch,
        capacity: 2,
        tiles: vec![
            CssRect {
                x: -5.,
                y: 0.,
                width: 20.,
                height: 10.,
            },
            CssRect {
                x: 15.,
                y: 0.,
                width: 10.,
                height: 10.,
            },
        ],
    }]);
    definition
}

#[test]
fn changing_tile_fit_replaces_routing_and_withdraws_old_geometry() {
    let (control, reader, session) = fixture_with_limit(1);
    let mut definition = tiled();
    definition.tile_groups.as_mut().unwrap()[0].fit = crate::plan::RegionFit::Contain;
    layout(&control, session, 1, vec![definition.clone()]);
    let first = reader.snapshot();
    let regions = first.regions(crate::OutputId::new(1).unwrap(), |_| crate::TextureId::new(1));
    let old = regions[0].placement.as_ref().unwrap();
    assert_eq!(old.tile_fit(), crate::plan::RegionFit::Contain);
    definition.tile_groups.as_mut().unwrap()[0].fit = crate::plan::RegionFit::Stretch;
    layout(&control, session, 2, vec![definition]);
    let next = reader.snapshot();
    assert_ne!(next.routing_generation, first.routing_generation);
    let token = CompletionToken {
        device: DeviceGeneration::new(Uuid::new_v4(), 1),
        serial: 1,
    };
    assert!(old.sample(token).tiles.is_empty());
    assert_eq!(
        next.regions(crate::OutputId::new(1).unwrap(), |_| crate::TextureId::new(1))[0]
            .placement
            .as_ref()
            .unwrap()
            .tile_fit(),
        crate::plan::RegionFit::Stretch
    );
}

#[test]
fn groups_share_one_registration_clip_independently_and_sample_all_before_installation() {
    let (control, reader, session) = fixture_with_limit(1);
    let mut definition = tiled();
    definition.tile_groups.as_mut().unwrap().push(PreviewTileGroup {
        source: "second-source".into(),
        fit: crate::plan::RegionFit::Stretch,
        capacity: 1,
        tiles: vec![CssRect {
            x: 0.,
            y: 0.,
            width: 5.,
            height: 5.,
        }],
    });
    assert!(matches!(
        layout(&control, session, 1, vec![definition.clone()]),
        PreviewResponse::Applied { visible_views: 1, .. }
    ));
    let first = reader.snapshot();
    assert_eq!(
        first.source_keys().collect::<Vec<_>>(),
        ["first-source", "second-source"]
    );
    let tiles = &first.views[0].tile_groups.as_ref().unwrap()[0].tiles;
    close(tiles[0].destination.x, 0.1);
    close(tiles[0].destination.width, 0.3);
    close(tiles[0].source_rect.x, 0.25);
    close(tiles[0].source_rect.width, 0.75);
    close(tiles[1].destination.x, 0.4);
    close(tiles[1].source_rect.width, 0.5);
    let output = crate::OutputId::new(1).unwrap();
    let regions = first.regions(output, |_| crate::TextureId::new(1));
    assert_eq!(regions.len(), 2);
    let device = DeviceGeneration::new(Uuid::new_v4(), 1);
    let token = CompletionToken { device, serial: 1 };
    for backing in first.backdrops(crate::TextureId::new(2).unwrap()) {
        backing.placement.as_ref().unwrap().sample(token);
    }
    regions[0].placement.as_ref().unwrap().sample(token);
    assert!(reader.mark_frame_installed(first.routing_generation, token).is_none());
    regions[1].placement.as_ref().unwrap().sample(token);
    assert_eq!(
        reader.mark_frame_installed(first.routing_generation, token),
        Some(first.generation)
    );
    definition.tile_groups.as_mut().unwrap()[0].tiles.clear();
    layout(&control, session, 2, vec![definition.clone()]);
    assert_eq!(reader.snapshot().routing_generation, first.routing_generation);
    assert_eq!(
        regions[0].placement.as_ref().unwrap().sample(token).tiles.as_ref(),
        tiles.as_ref()
    );
    assert!(reader.mark_frame_installed(first.routing_generation, token).is_none());
    let next = CompletionToken { device, serial: 2 };
    assert!(regions[0].placement.as_ref().unwrap().sample(next).tiles.is_empty());
    assert!(reader.mark_frame_installed(first.routing_generation, next).is_some());
    definition.tile_groups.as_mut().unwrap().remove(1);
    layout(&control, session, 3, vec![definition]);
    assert!(
        regions[1]
            .placement
            .as_ref()
            .unwrap()
            .sample(CompletionToken { device, serial: 3 })
            .tiles
            .is_empty()
    );
    layout(&control, session, 4, Vec::new());
    assert!(!regions[0].visibility.as_ref().unwrap().is_current());
    assert!(matches!(
        layout(&control, session, 5, vec![view(), view()]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Capacity
        }
    ));
}

#[test]
fn group_metadata_is_bounded_and_rejects_duplicates_and_invalid_geometry() {
    let (control, reader, session) = fixture();
    let mut invalid = tiled();
    invalid.tile_groups.as_mut().unwrap()[0].capacity = 0;
    assert!(matches!(
        layout(&control, session, 1, vec![invalid]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Geometry
        }
    ));
    let mut invalid = tiled();
    invalid.tile_groups.as_mut().unwrap()[0].tiles[0].x = f64::NAN;
    assert!(matches!(
        layout(&control, session, 1, vec![invalid]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Geometry
        }
    ));
    let mut invalid = tiled();
    let duplicate = invalid.tile_groups.as_ref().unwrap()[0].clone();
    invalid.tile_groups.as_mut().unwrap().push(duplicate);
    assert!(matches!(
        layout(&control, session, 1, vec![invalid]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Duplicate
        }
    ));
    let mut invalid = tiled();
    invalid.tile_groups.as_mut().unwrap()[0].source = "x".repeat(256);
    assert!(matches!(
        layout(&control, session, 1, vec![invalid]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Source
        }
    ));
    let mut invalid = tiled();
    invalid.tile_groups.as_mut().unwrap()[0].capacity = crate::plan::MAX_REGION_TILES as u32;
    invalid.tile_groups.as_mut().unwrap()[0].tiles = vec![invalid.rect; crate::plan::MAX_REGION_TILES + 1];
    assert!(matches!(
        layout(&control, session, 1, vec![invalid]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Capacity
        }
    ));
    assert!(reader.snapshot().views.is_empty());
}
