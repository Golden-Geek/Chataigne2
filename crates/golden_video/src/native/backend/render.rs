//! Execute demanded passes, retaining unchanged targets without changing presentation cadence.
use std::rc::Rc;

use golden_time::TimeSnapshot;
use windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext;

use crate::LinearColor;
use crate::plan::{BlendMode, CompiledPlan, RenderOperation};
use crate::runtime::CompletionToken;

use super::D3d11Resources;
use super::presentation::Presentation;
use crate::native::pipeline::Pipeline;

#[derive(Default)]
pub(super) struct Work {
    pub rendered: u64,
    pub reused: u64,
}

pub(super) fn execute(
    resources: &D3d11Resources,
    plan: &CompiledPlan,
    token: CompletionToken,
    time: TimeSnapshot,
    context: &ID3D11DeviceContext,
    pipeline: &Pipeline,
    presentation: &Presentation,
) -> Work {
    let mut cache = resources.render_cache.borrow_mut();
    let mut work = Work::default();
    for (index, pass) in plan.passes().iter().enumerate() {
        if !presentation.pass(index) {
            continue;
        }
        let output = &resources.textures[&pass.output];
        let written = |source| resources.textures[&source].last_write.get();
        if !cache.needs_render(index, output.last_write.get(), written) {
            work.reused += 1;
            continue;
        }
        match &pass.operation {
            RenderOperation::Clear(color) => pipeline.clear(context, output, *color),
            RenderOperation::Sample { source, filter } => {
                pipeline.draw(
                    context,
                    &resources.textures[source],
                    None,
                    output.as_ref().into(),
                    1.0,
                    BlendMode::Normal,
                    *filter,
                );
            }
            RenderOperation::Composite { layers } => {
                if layers.is_empty() {
                    pipeline.clear(context, output, LinearColor::TRANSPARENT);
                }
                for (index, layer) in layers.iter().enumerate() {
                    let layer = layer.evaluated(time);
                    let to_output = (layers.len() - index) % 2 == 1;
                    let target = if to_output {
                        output
                    } else {
                        &resources.scratch[&pass.id]
                    };
                    let previous = (index != 0).then(|| {
                        if to_output {
                            &resources.scratch[&pass.id]
                        } else {
                            output
                        }
                    });
                    pipeline.draw_layer(
                        context,
                        match &layer.source {
                            crate::plan::LayerSource::Pattern(crate::plan::Pattern::Canvas(canvas)) => {
                                canvas.image().map(|image| {
                                    resources.canvas_images[&(std::sync::Arc::as_ptr(image) as usize)].as_ref()
                                })
                            }
                            source => source.texture().map(|id| resources.textures[&id].as_ref()),
                        },
                        previous.map(Rc::as_ref),
                        target.as_ref().into(),
                        &layer,
                    );
                }
            }
            RenderOperation::Regions { regions, visibility } => {
                // These live placements/visibility fences are frame-dependent, including
                // presentation-only work. Always reconstruct them to clear stale pixels.
                pipeline.clear(context, output, LinearColor::TRANSPARENT);
                if visibility.as_ref().is_none_or(|guard| guard.is_current()) {
                    for region in regions {
                        if region.visibility.as_ref().is_none_or(|guard| guard.is_current()) {
                            let region = region.for_frame(token);
                            if let Some(placement) = &region.placement
                                && placement.tile_capacity() > 0
                            {
                                let mut repeated = region.clone();
                                for tile in placement.sample(token).tiles.iter() {
                                    let Some(tile) = tile.fitted(
                                        placement.tile_fit(),
                                        resources.textures[&region.source].descriptor.extent,
                                        output.descriptor.extent,
                                    ) else {
                                        continue;
                                    };
                                    repeated.destination = tile.destination;
                                    repeated.source_rect = tile.source_rect;
                                    pipeline.draw_region(
                                        context,
                                        &resources.textures[&region.source],
                                        output,
                                        &repeated,
                                    );
                                }
                            } else {
                                pipeline.draw_region(context, &resources.textures[&region.source], output, &region);
                            }
                            if let Some(placement) = &region.placement {
                                pipeline.draw_overlays(context, output, &placement.sample(token));
                            }
                        }
                    }
                }
            }
        }
        // Same-context ordering protects these queued writes and all subsequent reads;
        // ordinary submission completion still retains the resources and input leases.
        output.last_write.set(Some(token));
        cache.rendered(index, token, written);
        work.rendered += 1;
    }
    work
}
