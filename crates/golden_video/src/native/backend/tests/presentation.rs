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
use crate::native::outputs::OutputDemand;

pub(super) fn scene(placement: &RegionPlacement) -> CompiledPlan {
    compile_scene(placement, false)
}

fn compile_scene(placement: &RegionPlacement, shared: bool) -> CompiledPlan {
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
                source: TextureId::new(if shared { 2 } else { 4 }).unwrap(),
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
fn shared_geometry_stays_pending_for_each_output_until_its_delivery() {
    use OutputDemand::{Draw, Hidden, None as NotDue};

    let placement = RegionPlacement::new(UnitRect::FULL, UnitRect::FULL).unwrap();
    let plan = compile_scene(&placement, true);
    let mut presentation = Presentation::new(&plan, None);
    let slow = OutputId::new(1).unwrap();
    let fast = OutputId::new(2).unwrap();

    presentation.begin(false, |_| Draw);
    presentation.accepted(0);
    presentation.accepted(1);
    assert!(placement.update(
        UnitRect {
            width: 0.5,
            ..UnitRect::FULL
        },
        UnitRect::FULL
    ));

    presentation.begin(true, |id| if id == slow { Hidden } else { Draw });
    assert_eq!(presentation.output(0), Hidden);
    assert_eq!(presentation.output(1), Draw);
    assert!(!presentation.pass(0));
    assert!(presentation.pass(1));
    assert!(!presentation.pass(2));
    presentation.accepted(1);
    assert_eq!(presentation.pending_outputs().collect::<Vec<_>>(), [slow]);

    presentation.begin(true, |id| {
        assert_ne!(id, fast, "an unrelated placement retry must not consume pacing");
        Draw
    });
    assert_eq!(presentation.output(0), Draw);
    assert_eq!(presentation.output(1), NotDue);
    assert!(!presentation.pass(0));
    assert!(presentation.pass(1));
    assert!(presentation.pass(2));
    presentation.accepted(0);
    assert!(!presentation.pending());
}

#[test]
fn a_skipped_dependency_is_refreshed_on_the_next_placement_delivery() {
    use OutputDemand::{Draw, None as NotDue};

    let placement = RegionPlacement::new(UnitRect::FULL, UnitRect::FULL).unwrap();
    let plan = scene(&placement);
    let mut presentation = Presentation::new(&plan, None);
    let slow = OutputId::new(1).unwrap();

    presentation.begin(false, |_| Draw);
    presentation.accepted(0);
    presentation.accepted(1);
    presentation.begin(false, |id| if id == slow { NotDue } else { Draw });
    assert!((0..3).all(|index| !presentation.pass(index)));
    assert!(presentation.pass(3));
    assert!(placement.update(
        UnitRect {
            width: 0.5,
            ..UnitRect::FULL
        },
        UnitRect::FULL
    ));
    presentation.begin(true, |_| Draw);
    assert!((0..3).all(|index| presentation.pass(index)));
    assert!(!presentation.pass(3));
}

#[test]
fn live_geometry_redraws_its_dependent_passes_and_output_without_redoing_sources_or_peers() {
    use OutputDemand::{Draw, None as NotDue};

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
    presentation.begin(false, |_| Draw);
    presentation.accepted(0);
    presentation.accepted(1);
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
    presentation.begin(true, |_| Draw);
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
    assert_eq!(presentation.output(0), Draw);
    assert_eq!(presentation.output(1), NotDue);
    presentation.accepted(0);
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
    presentation.begin(false, |_| Draw);
    presentation.accepted(0);
    assert!(!presentation.pending());
    assert!((0..plan.passes().len()).all(|i| presentation.pass(i)));
    assert_eq!(presentation.output(0), Draw);
    assert_eq!(presentation.output(1), Draw);
}
