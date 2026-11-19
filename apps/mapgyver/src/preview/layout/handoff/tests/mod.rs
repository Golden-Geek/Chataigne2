use super::*;
use crate::media::protocol::{ChannelId, MediaId};
use golden_core::model::NodeUuid;
use golden_video::preview::ResolvedView;
use golden_video::{AlphaMode, ColorEncoding, Extent2D, LinearColor, PixelFormat, plan::UnitRect};

#[test]
fn pending_replacement_retains_each_views_own_image_and_installs_when_ready() {
    let reference = ChannelReference {
        media: MediaId(NodeUuid(Uuid::new_v4())),
        channel: ChannelId(NodeUuid(Uuid::new_v4())),
    };
    let requested = PreviewSource::Channel { reference };
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(64, 64).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let old_source = PreviewSource::Calibration {
        mode: crate::screens::protocol::Calibration::ColorBars,
        extent: descriptor.extent,
    };
    let views = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let old_textures = [TextureId::new(10).unwrap(), TextureId::new(11).unwrap()];
    let previous = views
        .into_iter()
        .zip(old_textures)
        .map(|(id, texture)| {
            (
                id,
                PreviewImage {
                    source: old_source,
                    texture,
                    descriptor,
                },
            )
        })
        .collect();
    let revision = golden_io::RevisionSource::default();
    let reader = revision.reader();
    let layout = PreviewSnapshot::new(
        reader.current(),
        views
            .into_iter()
            .map(|id| ResolvedView {
                id,
                source: serde_json::to_string(&requested).unwrap(),
                destination: UnitRect::FULL,
                source_rect: UnitRect::FULL,
                backdrop: UnitRect::FULL,
                overlays: Default::default(),
                overlay_scale: 1.,
                overlay_capacity: 0,
                tile_groups: None,
            })
            .collect(),
        reader.guard_at(reader.current()),
        LinearColor::TRANSPARENT,
    );
    let texture = TextureId::new(12).unwrap();
    let mut plan = RenderPlan {
        passes: vec![RenderPass {
            id: PassId::new(texture.get()).unwrap(),
            output: texture,
            descriptor,
            operation: RenderOperation::Clear(LinearColor::TRANSPARENT),
        }],
        ..Default::default()
    };
    let channels = HashMap::from([(reference, texture)]);
    let make = |availability, plan: &mut RenderPlan| {
        Handoff {
            previous: &previous,
            availability: &HashMap::from([(reference, availability)]),
            max_passes: 16,
        }
        .images(&layout, plan, &channels, &HashMap::new())
    };
    let mut pending = plan.clone();
    let waiting = make(ChannelAvailability::Unavailable, &mut pending);
    assert_eq!(waiting[&views[0]].texture, old_textures[0]);
    assert_eq!(waiting[&views[1]].texture, old_textures[1]);
    assert!(!waiting.contains_key(&views[2]));
    assert!(
        pending.passes[1..]
            .iter()
            .all(|pass| matches!(pass.operation, RenderOperation::Retain))
    );
    assert!(pending.external_textures.is_empty());
    let ready = make(ChannelAvailability::Ready, &mut plan);
    assert!(views.iter().all(|id| ready[id].texture == texture));
    assert_eq!(plan.passes.len(), 1, "the prior targets are no longer required");
    assert!(make(ChannelAvailability::Unconfigured, &mut plan).is_empty());
}
