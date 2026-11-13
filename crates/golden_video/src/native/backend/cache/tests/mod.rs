use super::*;
use crate::plan::{
    BlendMode, CanvasPaint, CanvasPattern, CanvasPrimitive, ExternalSource, ExternalTexture, Layer, PlanLimits,
    PlanOutput, RenderPass, RenderPlan, SampleFilter, UnitRect,
};
use crate::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, Opacity, OutputId, PassId, PixelFormat, TextureDescriptor,
};
use std::collections::BTreeMap;

fn id(value: u64) -> TextureId {
    TextureId::new(value).unwrap()
}
fn token(serial: u64) -> CompletionToken {
    CompletionToken {
        device: crate::runtime::DeviceGeneration::new(uuid::Uuid::nil(), 1),
        serial,
    }
}
fn layer() -> Layer {
    Layer {
        source: id(2).into(),
        opacity: Opacity::OPAQUE,
        opacity_envelope: None,
        blend: BlendMode::Normal,
        filter: SampleFilter::Nearest,
        mapping: None,
    }
}
fn plan(operation: RenderOperation, tracked: bool) -> CompiledPlan {
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(2, 1).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    RenderPlan {
        external_textures: vec![ExternalTexture {
            id: id(1),
            descriptor,
            source: tracked.then(|| ExternalSource::Frames(crate::image::ImageStream::new(descriptor))),
        }],
        passes: [
            RenderOperation::Sample {
                source: id(1),
                filter: SampleFilter::Nearest,
            },
            operation,
            RenderOperation::Clear(LinearColor::TRANSPARENT),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, operation)| RenderPass {
            id: PassId::new(index as u64 + 1).unwrap(),
            output: id(index as u64 + 2),
            descriptor,
            operation,
        })
        .collect(),
        outputs: [3, 3, 4]
            .into_iter()
            .enumerate()
            .map(|(index, source)| PlanOutput {
                id: OutputId::new(index as u64 + 1).unwrap(),
                source: id(source),
            })
            .collect(),
    }
    .compile(&PlanLimits {
        max_passes: 16,
        max_external_textures: 4,
        max_outputs: 4,
        max_dependencies: 64,
        max_texture_dimension: 8,
        max_texture_bytes: 4096,
    })
    .unwrap()
}
fn execute(
    cache: &mut Cache,
    plan: &CompiledPlan,
    contents: &mut BTreeMap<TextureId, Option<CompletionToken>>,
    frame: u64,
) -> (usize, usize) {
    let mut rendered = 0;
    for (index, pass) in plan.passes().iter().enumerate() {
        if cache.needs_render(index, contents[&pass.output], |source| contents[&source]) {
            contents.insert(pass.output, Some(token(frame)));
            cache.rendered(index, token(frame), |source| contents[&source]);
            rendered += 1;
        }
    }
    (rendered, plan.passes().len() - rendered)
}
fn contents() -> BTreeMap<TextureId, Option<CompletionToken>> {
    [(id(1), Some(token(1))), (id(2), None), (id(3), None), (id(4), None)].into()
}

#[test]
fn held_inputs_reuse_targets_and_actual_changes_propagate_only_to_dependents() {
    let plan = plan(RenderOperation::Composite { layers: vec![layer()] }, true);
    let mut cache = Cache::new(&plan);
    let mut contents = contents();
    assert_eq!(execute(&mut cache, &plan, &mut contents, 2), (3, 0));
    assert_eq!(execute(&mut cache, &plan, &mut contents, 3), (0, 3));
    contents.insert(id(1), Some(token(4)));
    assert_eq!(execute(&mut cache, &plan, &mut contents, 4), (2, 1));
    assert_eq!(execute(&mut cache, &plan, &mut contents, 100), (0, 3));
    // A compatible generation overwrote the same output allocation.
    contents.insert(id(3), Some(token(101)));
    assert_eq!(execute(&mut cache, &plan, &mut contents, 102), (1, 2));
    // Reprepared/device-rebuilt resources must initialize every target again.
    let mut replacement = Cache::new(&plan);
    assert_eq!(execute(&mut replacement, &plan, &mut contents, 103), (3, 0));
}

#[test]
fn untracked_external_storage_cannot_supply_a_false_cache_hit() {
    let plan = plan(RenderOperation::Composite { layers: vec![layer()] }, false);
    let mut cache = Cache::new(&plan);
    let mut contents = contents();
    assert_eq!(execute(&mut cache, &plan, &mut contents, 2), (3, 0));
    assert_eq!(execute(&mut cache, &plan, &mut contents, 3), (2, 1));
}

#[test]
fn envelopes_canvas_and_live_regions_continue_to_evaluate_without_new_source_pixels() {
    let anchor = golden_time::TimeAuthority::new(
        golden_time::SystemClock::new(),
        golden_time::ShowPosition::from_nanos(0),
        golden_time::PlaybackRate::NORMAL,
    )
    .unwrap()
    .sample()
    .unwrap();
    let mut fading = layer();
    fading.opacity_envelope = Some(
        crate::plan::OpacityEnvelope::new(
            anchor,
            std::time::Duration::from_secs(1),
            Opacity::TRANSPARENT,
            Opacity::OPAQUE,
            crate::plan::InterpolationCurve::Linear,
        )
        .unwrap(),
    );
    let mut canvas = layer();
    canvas.source = LayerSource::Pattern(Pattern::Canvas(
        CanvasPattern::new(vec![
            CanvasPrimitive::rectangle(UnitRect::FULL, CanvasPaint::solid(LinearColor::TRANSPARENT)).unwrap(),
        ])
        .unwrap(),
    ));
    for operation in [
        RenderOperation::Composite { layers: vec![fading] },
        RenderOperation::Composite { layers: vec![canvas] },
        RenderOperation::Regions {
            regions: vec![],
            visibility: None,
        },
    ] {
        let plan = plan(operation, true);
        let mut cache = Cache::new(&plan);
        let mut contents = contents();
        assert_eq!(execute(&mut cache, &plan, &mut contents, 2).0, plan.passes().len());
        assert_eq!(execute(&mut cache, &plan, &mut contents, 3).0, 1);
    }
}
