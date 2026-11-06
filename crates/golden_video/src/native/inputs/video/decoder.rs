use super::{Format, Image, invalid};
use crate::native::{pipeline::shaders, texture::Texture};
use crate::{AlphaMode, ColorEncoding, TextureDescriptor};
use windows::Win32::Graphics::{Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, Direct3D11::*};
use windows::core::Result;

#[repr(C)]
struct Parameters {
    format: u32,
    encoding: u32,
    alpha: u32,
    reserved: u32,
}
const _: () = assert!(size_of::<Parameters>() == super::UNIFORM_BYTES as usize);

pub(in crate::native::inputs) struct Decoder {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    rasterizer: ID3D11RasterizerState,
    constants: ID3D11Buffer,
}
impl Decoder {
    pub fn new(device: &ID3D11Device, format: Format, descriptor: TextureDescriptor) -> Result<Self> {
        let parameters = Parameters {
            format: format.native(),
            encoding: u32::from(descriptor.color_encoding == ColorEncoding::Srgb),
            alpha: match descriptor.alpha_mode {
                AlphaMode::Straight => 0,
                AlphaMode::Premultiplied => 1,
                AlphaMode::Opaque => 2,
            },
            reserved: 0,
        };
        let (mut vertex, mut pixel, mut rasterizer, mut constants) = (None, None, None, None);
        unsafe {
            device.CreateVertexShader(shaders::VERTEX, None, Some(&mut vertex))?;
            device.CreatePixelShader(shaders::VIDEO_PIXEL, None, Some(&mut pixel))?;
            device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut rasterizer),
            )?;
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: size_of::<Parameters>() as u32,
                    Usage: D3D11_USAGE_IMMUTABLE,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                Some(&D3D11_SUBRESOURCE_DATA {
                    pSysMem: std::ptr::from_ref(&parameters).cast(),
                    ..Default::default()
                }),
                Some(&mut constants),
            )?;
        }
        Ok(Self {
            vertex: vertex.ok_or_else(invalid)?,
            pixel: pixel.ok_or_else(invalid)?,
            rasterizer: rasterizer.ok_or_else(invalid)?,
            constants: constants.ok_or_else(invalid)?,
        })
    }
    pub fn draw(&self, context: &ID3D11DeviceContext, source: &Image, target: &Texture) {
        debug_assert_eq!(source.descriptor, target.descriptor);
        unsafe {
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex, None);
            context.GSSetShader(None, None);
            context.HSSetShader(None, None);
            context.DSSetShader(None, None);
            context.PSSetShader(&self.pixel, None);
            context.PSSetShaderResources(3, Some(&[Some(source.view.clone())]));
            context.PSSetShaderResources(5, Some(std::slice::from_ref(&source.chroma)));
            context.PSSetConstantBuffers(3, Some(&[Some(self.constants.clone())]));
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
            context.PSSetShaderResources(3, Some(&[None]));
            context.PSSetShaderResources(5, Some(&[None]));
            context.PSSetConstantBuffers(3, Some(&[None]));
            context.OMSetRenderTargets(None, None);
        }
    }
}
