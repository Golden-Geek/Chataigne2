use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::plan::{
    BlitRegion, CompiledPlan, PlanLimits, PlanOutput, RegionPlacement, RenderOperation, RenderPass, RenderPlan,
    SampleFilter, UnitRect,
};
use crate::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
};

use super::super::presentation::Presentation;

pub(super) fn scene(placement: &RegionPlacement) -> CompiledPlan {
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(4, 4).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let region = BlitRegion {
        source: TextureId::new(1).unwrap(),
        destination: UnitRect::FULL,
        source_rect: UnitRect::FULL,
        filter: SampleFilter::Nearest,
        visibility: None,
        placement: Some(placement.clone()),
    };
    let operations = [
        RenderOperation::Clear(LinearColor::from_straight(1.0, 0.0, 0.0, 1.0).unwrap()),
        RenderOperation::Regions {
            regions: vec![region],
            visibility: None,
        },
        RenderOperation::Sample {
            source: TextureId::new(2).unwrap(),
            filter: SampleFilter::Nearest,
        },
        RenderOperation::Clear(LinearColor::from_straight(0.0, 1.0, 0.0, 1.0).unwrap()),
    ];
    RenderPlan {
        passes: operations
            .into_iter()
            .enumerate()
            .map(|(i, operation)| RenderPass {
                id: PassId::new(i as u64 + 1).unwrap(),
                output: TextureId::new(i as u64 + 1).unwrap(),
                descriptor,
                operation,
            })
            .collect(),
        outputs: vec![
            PlanOutput {
                id: OutputId::new(1).unwrap(),
                source: TextureId::new(3).unwrap(),
            },
            PlanOutput {
                id: OutputId::new(2).unwrap(),
                source: TextureId::new(4).unwrap(),
            },
        ],
        ..Default::default()
    }
    .compile(&PlanLimits {
        max_passes: 4,
        max_external_textures: 0,
        max_outputs: 2,
        max_dependencies: 8,
        max_texture_dimension: 16,
        max_texture_bytes: 4096,
    })
    .unwrap()
}

#[test]
fn live_geometry_redraws_its_dependent_passes_and_output_without_redoing_sources_or_peers() {
    let placement = RegionPlacement::new(UnitRect::FULL, UnitRect::FULL).unwrap();
    let plan = scene(&placement);
    let wakes = Arc::new(AtomicUsize::new(0));
    let observed = wakes.clone();
    let mut presentation = Presentation::new(
        &plan,
        Some(golden_io::WorkerNotifier::new(move || {
            observed.fetch_add(1, Ordering::SeqCst);
        })),
    );
    presentation.begin(false);
    presentation.accepted(OutputId::new(1).unwrap());
    presentation.accepted(OutputId::new(2).unwrap());
    assert!(!presentation.pending());
    assert!(placement.update(
        UnitRect {
            width: 0.5,
            ..UnitRect::FULL
        },
        UnitRect::FULL
    ));
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert!(presentation.pending());
    presentation.begin(true);
    assert!(
        presentation.pending(),
        "a sampled batch does not establish endpoint acceptance"
    );
    let passes: Vec<_> = plan
        .passes()
        .iter()
        .enumerate()
        .filter(|(i, _)| presentation.pass(*i))
        .map(|(_, pass)| pass.id.get())
        .collect();
    assert_eq!(passes, [2, 3]);
    assert!(presentation.output(0));
    assert!(!presentation.output(1));
    presentation.accepted(OutputId::new(1).unwrap());
    assert!(!presentation.pending());
    assert!(placement.update(
        UnitRect {
            width: 0.5,
            ..UnitRect::FULL
        },
        UnitRect::FULL
    ));
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        1,
        "unchanged geometry does not wake or redraw"
    );
    assert!(placement.update(UnitRect::FULL, UnitRect::FULL));
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        2,
        "the next changed cohort rearms its one-shot wake"
    );
    assert!(presentation.pending());
    presentation.begin(false);
    presentation.accepted(OutputId::new(1).unwrap());
    assert!(!presentation.pending());
    assert!((0..plan.passes().len()).all(|i| presentation.pass(i)));
    assert!(presentation.output(0) && presentation.output(1));
}
