//! Lossless component transport for layouts that cannot be shared through NT handles.
//! No image readback, transfer conversion, filtering, or per-frame buffer upload.
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::{Error, HRESULT, Result};

use super::super::{pipeline::shaders, texture::Texture};
use crate::{PixelFormat, TextureDescriptor};

pub(in crate::native) fn bridge_descriptor(source: TextureDescriptor) -> TextureDescriptor {
    if matches!(
        source.format,
        PixelFormat::Rg32Float | PixelFormat::Bgr565Unorm | PixelFormat::Bgra5551Unorm | PixelFormat::Bgra4444Unorm
    ) {
        TextureDescriptor {
            format: PixelFormat::Rgba32Float,
            ..source
        }
    } else {
        source
    }
}

/// Fixed shader/state objects only; all image storage is separately charged before creation.
/// A dedicated device/context and its existing completion authority retain each instance.
pub(super) struct Converter {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    rasterizer: ID3D11RasterizerState,
}

impl Converter {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let mut vertex = None;
        let mut pixel = None;
        let mut rasterizer = None;
        // SAFETY: fixed build-compiled bytecode and complete immutable rasterizer state.
        unsafe {
            device.CreateVertexShader(shaders::VERTEX, None, Some(&mut vertex))?;
            device.CreatePixelShader(shaders::IMPORT_PIXEL, None, Some(&mut pixel))?;
            device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut rasterizer),
            )?;
        }
        let missing = || Error::from_hresult(HRESULT(0x8000_4005_u32 as i32));
        Ok(Self {
            vertex: vertex.ok_or_else(missing)?,
            pixel: pixel.ok_or_else(missing)?,
            rasterizer: rasterizer.ok_or_else(missing)?,
        })
    }

    /// Raw SRV to raw RTV: declared encoding and alpha are retained, never applied twice.
    /// Caller acquires image ownership and opens a completion submission before this call.
    pub fn copy(&self, context: &ID3D11DeviceContext, source: &Texture, target: &Texture) {
        debug_assert_eq!(source.descriptor.extent, target.descriptor.extent);
        // SAFETY: prepared resources on this context's device, matching extents, immutable
        // source throughout this GPU use. Remove view bindings before ownership can retire.
        unsafe {
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex, None);
            context.GSSetShader(None, None);
            context.HSSetShader(None, None);
            context.DSSetShader(None, None);
            context.PSSetShader(&self.pixel, None);
            context.PSSetShaderResources(0, Some(&[Some(source.srv.clone())]));
            context.OMSetDepthStencilState(None, 0);
            context.OMSetBlendState(None, None, u32::MAX);
            context.OMSetRenderTargets(Some(&[Some(target.rtv.clone())]), None);
            context.RSSetState(&self.rasterizer);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: target.descriptor.extent.width() as f32,
                Height: target.descriptor.extent.height() as f32,
                MinDepth: 0.,
                MaxDepth: 1.,
                ..Default::default()
            }]));
            context.Draw(3, 0);
            context.PSSetShaderResources(0, Some(&[None]));
            context.OMSetRenderTargets(None, None);
        }
    }
}
