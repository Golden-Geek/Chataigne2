use super::*;
use crate::runtime::{CompletionToken, DeviceGeneration};

fn tiled() -> PreviewView {
    let mut definition = view();
    definition.rect = CssRect {
        x: 0.,
        y: 0.,
        width: 20.,
        height: 10.,
    };
    definition.clip = definition.rect;
    definition.tile_capacity = 2;
    definition.tiles = Some(vec![
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
    ]);
    definition
}

#[test]
fn tiled_sources_clip_independently_share_one_binding_and_update_without_routing_replacement() {
    let (control, reader, session) = fixture();
    let mut definition = tiled();
    assert!(matches!(
        layout(&control, session, 1, vec![definition.clone()]),
        PreviewResponse::Applied { visible_views: 1, .. }
    ));
    let first = reader.snapshot();
    let tiles = first.views[0].tiles.as_ref().unwrap();
    close(tiles[0].destination.x, 0.1);
    close(tiles[0].destination.width, 0.3);
    close(tiles[0].source_rect.x, 0.25);
    close(tiles[0].source_rect.width, 0.75);
    close(tiles[1].destination.x, 0.4);
    close(tiles[1].destination.width, 0.1);
    close(tiles[1].source_rect.width, 0.5);
    let output = crate::OutputId::new(1).unwrap();
    let regions = first.regions(output, |_| crate::TextureId::new(1));
    assert_eq!(regions.len(), 1);
    assert_eq!(first.backdrops(crate::TextureId::new(2).unwrap()).len(), 1);
    let placement = regions[0].placement.as_ref().unwrap();
    assert_eq!(placement.tile_capacity(), 2);
    let device = DeviceGeneration::new(Uuid::new_v4(), 1);
    let token = CompletionToken { device, serial: 1 };
    assert_eq!(placement.sample(token).tiles.as_ref(), tiles.as_ref());
    definition.tiles = Some(Vec::new());
    assert!(matches!(
        layout(&control, session, 2, vec![definition]),
        PreviewResponse::Applied { visible_views: 1, .. }
    ));
    assert_eq!(reader.snapshot().routing_generation, first.routing_generation);
    assert_eq!(placement.sample(token).tiles.as_ref(), tiles.as_ref());
    assert!(placement.sample(CompletionToken { device, serial: 2 }).tiles.is_empty());
    assert!(regions[0].visibility.as_ref().unwrap().is_current());
    layout(&control, session, 3, Vec::new());
    assert!(!regions[0].visibility.as_ref().unwrap().is_current());
}

#[test]
fn invalid_or_excess_tile_metadata_is_rejected_before_publication() {
    let (control, reader, session) = fixture();
    for invalid in [
        PreviewView {
            tile_capacity: 0,
            ..tiled()
        },
        PreviewView { tiles: None, ..tiled() },
        PreviewView {
            overlay_capacity: 1,
            ..tiled()
        },
        PreviewView {
            tiles: Some(vec![CssRect {
                x: f64::NAN,
                ..tiled().rect
            }]),
            ..tiled()
        },
    ] {
        assert!(matches!(
            layout(&control, session, 1, vec![invalid]),
            PreviewResponse::Rejected {
                code: PreviewRejection::Geometry
            }
        ));
        assert!(reader.snapshot().views.is_empty());
    }
    let mut oversized = tiled();
    oversized.tile_capacity = crate::plan::MAX_REGION_TILES as u32;
    oversized.tiles = Some(vec![oversized.rect; crate::plan::MAX_REGION_TILES + 1]);
    assert!(matches!(
        layout(&control, session, 1, vec![oversized]),
        PreviewResponse::Rejected {
            code: PreviewRejection::Capacity
        }
    ));
    let reserved = (0..3)
        .map(|_| PreviewView {
            tile_capacity: crate::plan::MAX_REGION_TILES as u32,
            tiles: Some(Vec::new()),
            ..tiled()
        })
        .collect();
    assert!(matches!(
        layout(&control, session, 1, reserved),
        PreviewResponse::Rejected {
            code: PreviewRejection::Capacity
        }
    ));
}
