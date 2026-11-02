use crate::preview::{layout::PreviewLayout, protocol::PreviewSource};
use crate::screens::protocol::ScreenId;
use golden_core::model::NodeUuid;
use golden_io::RevisionSource;
use golden_video::plan::{PlanLimits, RenderOperation, RenderPass, RenderPlan, UnitRect};
use golden_video::preview::{PreviewSnapshot, ResolvedView};
use golden_video::runtime::PlanSubmission;
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
};
use std::collections::HashMap;

#[test]
fn native_active_areas_are_black_even_without_sources_while_the_workbench_keeps_its_theme() {
    let screens = [
        ScreenId(NodeUuid(uuid::Uuid::new_v4())),
        ScreenId(NodeUuid(uuid::Uuid::new_v4())),
    ];
    let destinations = [
        UnitRect {
            x: 0.,
            y: 0.25,
            width: 0.4,
            height: 0.5,
        },
        UnitRect {
            x: 0.6,
            y: 0.25,
            width: 0.4,
            height: 0.5,
        },
    ];
    let revision = RevisionSource::default();
    let reader = revision.reader();
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(128, 64).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let preview = PreviewLayout {
        output: OutputId::new(1).unwrap(),
        descriptor,
        layout: PreviewSnapshot::new(
            reader.current(),
            screens
                .into_iter()
                .zip(destinations)
                .map(|(screen, destination)| ResolvedView {
                    id: uuid::Uuid::new_v4(),
                    source: serde_json::to_string(&PreviewSource::Screen { screen }).unwrap(),
                    destination,
                    backdrop: destination,
                    source_rect: UnitRect::FULL,
                })
                .collect(),
            reader.guard_at(reader.current()),
        ),
    };
    let source = TextureId::new(PreviewLayout::RESERVED_TEXTURES + 1).unwrap();
    let mut submission = PlanSubmission::new(RenderPlan::default());
    submission.plan.passes.push(RenderPass {
        id: PassId::new(source.get()).unwrap(),
        output: source,
        descriptor,
        operation: RenderOperation::Clear(LinearColor::from_straight(1., 0., 0., 0.5).unwrap()),
    });
    // The second screen has no available texture. It still owns an active black rectangle.
    preview
        .append(&mut submission, &HashMap::new(), &HashMap::from([(screens[0], source)]))
        .unwrap();
    let pass = |id| {
        submission
            .plan
            .passes
            .iter()
            .find(|pass| pass.output == TextureId::new(id).unwrap())
            .unwrap()
    };
    let RenderOperation::Clear(theme) = pass(2).operation else {
        panic!("theme base missing")
    };
    assert_eq!(theme.components(), [0.0075, 0.0075, 0.0075, 1.]);
    let RenderOperation::Clear(black) = pass(4).operation else {
        panic!("black fill missing")
    };
    assert_eq!(black.components(), [0., 0., 0., 1.]);
    let RenderOperation::Regions {
        regions: black_regions,
        visibility,
    } = &pass(5).operation
    else {
        panic!("active regions missing")
    };
    assert!(visibility.is_none());
    assert_eq!(black_regions.len(), destinations.len() * 2);
    assert_eq!(
        black_regions
            .iter()
            .map(|region| region.destination)
            .collect::<Vec<_>>(),
        destinations.repeat(2)
    );
    assert!(
        black_regions
            .iter()
            .all(|region| region.source == TextureId::new(4).unwrap())
    );
    let RenderOperation::Regions { regions: content, .. } = &pass(1).operation else {
        panic!("source regions missing")
    };
    assert_eq!(content.len(), 1);
    assert_eq!(content[0].source, source);
    let RenderOperation::Composite { layers } = &pass(3).operation else {
        panic!("compositor missing")
    };
    assert_eq!(
        layers
            .iter()
            .map(|layer| layer.source.texture().unwrap().get())
            .collect::<Vec<_>>(),
        [2, 5, 1]
    );
    assert_eq!(submission.plan.passes.len(), PreviewLayout::PASS_COUNT + 1);
    submission
        .plan
        .compile(&PlanLimits {
            max_passes: 16,
            max_external_textures: 16,
            max_outputs: 1,
            max_dependencies: 64,
            max_texture_dimension: 1024,
            max_texture_bytes: 16 * 1024 * 1024,
        })
        .unwrap();
}
