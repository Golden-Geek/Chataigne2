//! Decorations are drawn into the same linear target and GPU batch as their image.
use super::*;
use crate::plan::{OverlayPrimitive, OverlayShape, RegionGeometry};

#[repr(C)]
pub(super) struct OverlayParameters {
    points: [[f32; 4]; 4],
    fill: [f32; 4],
    stroke: [f32; 4],
    style: [f32; 4],
    kind: [u32; 4],
}

pub(super) const CONSTANT_BYTES: usize = size_of::<OverlayParameters>();
const _: () = assert!(CONSTANT_BYTES == 128);

pub(super) fn prepare(device: &ID3D11Device) -> Result<(ID3D11PixelShader, ID3D11Buffer, ID3D11BlendState)> {
    let (mut shader, mut constants, mut blend) = (None, None, None);
    // SAFETY: fixed compiler-validated shader and bounded immutable state descriptors.
    unsafe {
        device.CreatePixelShader(shaders::OVERLAY_PIXEL, None, Some(&mut shader))?;
        device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: CONSTANT_BYTES as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            },
            None,
            Some(&mut constants),
        )?;
        let mut description = D3D11_BLEND_DESC::default();
        description.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            SrcBlend: D3D11_BLEND_ONE,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_ONE,
            DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
        device.CreateBlendState(&description, Some(&mut blend))?;
    }
    Ok((
        shader.ok_or_else(missing_output)?,
        constants.ok_or_else(missing_output)?,
        blend.ok_or_else(missing_output)?,
    ))
}

fn linear(color: [f32; 4]) -> [f32; 4] {
    let decode = |value: f32| {
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    [
        decode(color[0]) * color[3],
        decode(color[1]) * color[3],
        decode(color[2]) * color[3],
        color[3],
    ]
}

impl Pipeline {
    pub(in crate::native) fn draw_overlays(
        &self,
        context: &ID3D11DeviceContext,
        target: &Texture,
        geometry: &RegionGeometry,
    ) {
        if geometry.overlays.is_empty() {
            return;
        }
        debug_assert!(target.descriptor.is_compositing_target());
        let width = target.descriptor.extent.width() as f32;
        let height = target.descriptor.extent.height() as f32;
        let clip = geometry.overlay_clip;
        let scissor = RECT {
            left: (clip.x * width).ceil() as i32,
            top: (clip.y * height).ceil() as i32,
            right: ((clip.x + clip.width) * width).floor() as i32,
            bottom: ((clip.y + clip.height) * height).floor() as i32,
        };
        for overlay in geometry.overlays.iter() {
            self.draw_overlay(context, target, geometry, overlay, scissor);
        }
    }

    fn draw_overlay(
        &self,
        context: &ID3D11DeviceContext,
        target: &Texture,
        geometry: &RegionGeometry,
        overlay: &OverlayPrimitive,
        clip: RECT,
    ) {
        let extent = target.descriptor.extent;
        let transform = |point: [f32; 2]| {
            [
                (geometry.destination.x
                    + (point[0] - geometry.source_rect.x) / geometry.source_rect.width * geometry.destination.width)
                    * extent.width() as f32,
                (geometry.destination.y
                    + (point[1] - geometry.source_rect.y) / geometry.source_rect.height * geometry.destination.height)
                    * extent.height() as f32,
            ]
        };
        let (points, radius, kind) = match overlay.shape {
            OverlayShape::Quad { corners } => (corners.map(transform), 0.0, 0),
            OverlayShape::Circle { center, radius } => ([transform(center); 4], radius * geometry.overlay_scale, 1),
        };
        let stroke = overlay.width * geometry.overlay_scale;
        let margin = radius + stroke * 0.5 + 1.0;
        let left = (points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min) - margin)
            .floor()
            .max(clip.left as f32);
        let top = (points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min) - margin)
            .floor()
            .max(clip.top as f32);
        let right = (points.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max) + margin)
            .ceil()
            .min(clip.right as f32);
        let bottom = (points.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max) + margin)
            .ceil()
            .min(clip.bottom as f32);
        if right <= left || bottom <= top {
            return;
        }
        let dash = overlay.dash.unwrap_or([0.0; 2]).map(|v| v * geometry.overlay_scale);
        let parameters = OverlayParameters {
            points: points.map(|p| [p[0], p[1], 0.0, 0.0]),
            fill: linear(overlay.fill),
            stroke: linear(overlay.stroke),
            style: [stroke, radius, dash[0], dash[1]],
            kind: [kind, 0, 0, 0],
        };
        // SAFETY: prepared device-local objects; no allocation or fallible operation after
        // frame admission. Each primitive has a bounded viewport and aperture scissor.
        unsafe {
            context.UpdateSubresource(
                &self.overlay_constants,
                0,
                None,
                (&parameters as *const OverlayParameters).cast(),
                0,
                0,
            );
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex, None);
            context.GSSetShader(None, None);
            context.HSSetShader(None, None);
            context.DSSetShader(None, None);
            context.PSSetShader(&self.overlay_pixel, None);
            context.PSSetConstantBuffers(2, Some(&[Some(self.overlay_constants.clone())]));
            context.PSSetShaderResources(0, Some(&[None, None]));
            context.OMSetDepthStencilState(None, 0);
            context.OMSetBlendState(&self.overlay_blend, None, u32::MAX);
            context.OMSetRenderTargets(Some(&[Some(target.rtv.clone())]), None);
            context.RSSetState(&self.rasterizer);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: left,
                TopLeftY: top,
                Width: right - left,
                Height: bottom - top,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            context.RSSetScissorRects(Some(&[clip]));
            context.Draw(3, 0);
            context.OMSetRenderTargets(None, None);
            context.OMSetBlendState(None, None, u32::MAX);
        }
    }
}
