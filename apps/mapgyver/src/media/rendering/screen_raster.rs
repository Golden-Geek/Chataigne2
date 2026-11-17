//! Screen quality policy over reusable, pixel-aligned native raster tiles.
use super::{ScreenEntry, plan::allocate};
use crate::{
    media::protocol::ChannelReference,
    screens::{CalibrationAssets, rendering::CapturedScreen},
};
use golden_video::plan::{BlitRegion, RasterTile, RenderOperation, RenderPass, RenderPlan, SampleFilter, UnitRect};
use golden_video::{AlphaMode, ColorEncoding, Extent2D, PassId, PixelFormat, TextureDescriptor, TextureId};
use std::collections::HashMap;

pub(super) struct Inputs<'a> {
    pub screen: &'a CapturedScreen,
    pub extent: Extent2D,
    pub tiles: &'a [RasterTile],
    pub channels: &'a HashMap<ChannelReference, TextureId>,
    pub assets: &'a mut CalibrationAssets,
}

pub(super) fn append(
    plan: &mut RenderPlan,
    entry: &mut ScreenEntry,
    next: &mut u64,
    inputs: Inputs<'_>,
) -> Result<(), String> {
    let Inputs {
        screen,
        extent,
        tiles,
        channels,
        assets,
    } = inputs;
    let layers = screen.layers(channels, extent, assets)?;
    while entry.smooth4.len() < tiles.len() {
        entry.smooth4.push([allocate(next)?, allocate(next)?, allocate(next)?]);
    }
    let mut regions = Vec::new();
    for (index, tile) in tiles.iter().enumerate() {
        let viewport = tile.viewport(extent);
        let mut layers = layers.clone();
        for layer in &mut layers {
            layer.mapping = layer
                .mapping
                .map(|mapping| mapping.in_viewport(viewport))
                .transpose()
                .map_err(|error| error.to_string())?;
        }
        let [raster, half, resolved] = entry.smooth4[index];
        push(
            plan,
            raster,
            tile.raster_extent(4).ok_or("4× Screen raster overflows")?,
            RenderOperation::Composite { layers },
        );
        push(
            plan,
            half,
            tile.raster_extent(2).ok_or("2× Screen resolve overflows")?,
            RenderOperation::Sample {
                source: raster,
                filter: SampleFilter::Linear,
            },
        );
        // A 4:1 bilinear sample would omit twelve subpixels. Two 2:1 stages average all sixteen.
        let output = if tiles.len() == 1 { entry.target } else { resolved };
        push(
            plan,
            output,
            tile.extent,
            RenderOperation::Sample {
                source: half,
                filter: SampleFilter::Linear,
            },
        );
        if tiles.len() > 1 {
            regions.push(BlitRegion {
                source: output,
                destination: viewport,
                source_rect: UnitRect::FULL,
                filter: SampleFilter::Nearest,
                visibility: None,
                placement: None,
            });
        }
    }
    if !regions.is_empty() {
        push(
            plan,
            entry.target,
            extent,
            RenderOperation::Regions {
                regions,
                visibility: None,
            },
        );
    }
    Ok(())
}

fn push(plan: &mut RenderPlan, output: TextureId, extent: Extent2D, operation: RenderOperation) {
    plan.passes.push(RenderPass {
        id: PassId::new(output.get()).unwrap(),
        output,
        descriptor: TextureDescriptor {
            extent,
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
        operation,
    });
}
