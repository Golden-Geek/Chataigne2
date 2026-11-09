//! Worker-local BC texture upload/sample. No CPU BC/YCoCg pixel expansion.
use super::producer::GpuFrameError;
use crate::{
    decode::{HapFormat, HapFrame},
    native::{device::Device, pipeline::shaders, texture::Texture},
};
use windows::{
    Win32::Graphics::{Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, Direct3D11::*, Dxgi::Common::*},
    core::Result,
};

pub(super) struct HapImage {
    pub format: HapFormat,
    texture: ID3D11Texture2D,
    view: ID3D11ShaderResourceView,
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    rasterizer: ID3D11RasterizerState,
    constants: ID3D11Buffer,
    pitch: u32,
}
impl HapImage {
    pub fn new(device: &Device, frame: &HapFrame) -> Result<Self> {
        let extent = frame.extent();
        let format = frame.format();
        let (storage, mode, block_bytes) = match format {
            HapFormat::Bc1Rgb => (DXGI_FORMAT_BC1_UNORM, 0u32, 8),
            HapFormat::Bc3Rgba => (DXGI_FORMAT_BC3_UNORM, 1, 16),
            HapFormat::Bc3YCoCg => (DXGI_FORMAT_BC3_UNORM, 2, 16),
        };
        let mut texture = None;
        let mut view = None;
        let mut vertex = None;
        let mut pixel = None;
        let mut rasterizer = None;
        let mut constants = None;
        let parameters = [mode, 0, 0, 0];
        unsafe {
            device.device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: extent.width().div_ceil(4) * 4,
                    Height: extent.height().div_ceil(4) * 4,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: storage,
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut texture),
            )?;
            device
                .device
                .CreateShaderResourceView(texture.as_ref().unwrap(), None, Some(&mut view))?;
            device
                .device
                .CreateVertexShader(shaders::VERTEX, None, Some(&mut vertex))?;
            device
                .device
                .CreatePixelShader(shaders::HAP_PIXEL, None, Some(&mut pixel))?;
            device.device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut rasterizer),
            )?;
            device.device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: 16,
                    Usage: D3D11_USAGE_IMMUTABLE,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                Some(&D3D11_SUBRESOURCE_DATA {
                    pSysMem: parameters.as_ptr().cast(),
                    ..Default::default()
                }),
                Some(&mut constants),
            )?;
        }
        Ok(Self {
            format,
            texture: texture.unwrap(),
            view: view.unwrap(),
            vertex: vertex.unwrap(),
            pixel: pixel.unwrap(),
            rasterizer: rasterizer.unwrap(),
            constants: constants.unwrap(),
            pitch: extent.width().div_ceil(4) * block_bytes,
        })
    }
    pub fn upload_draw(
        &self,
        device: &Device,
        frame: &HapFrame,
        target: &Texture,
    ) -> std::result::Result<(), GpuFrameError> {
        if frame.format() != self.format || frame.extent() != target.descriptor.extent {
            return Err(GpuFrameError::Layout);
        }
        unsafe {
            device
                .context
                .UpdateSubresource(&self.texture, 0, None, frame.blocks().as_ptr().cast(), self.pitch, 0);
            device.context.IASetInputLayout(None);
            device
                .context
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            device.context.VSSetShader(&self.vertex, None);
            device.context.GSSetShader(None, None);
            device.context.HSSetShader(None, None);
            device.context.DSSetShader(None, None);
            device.context.PSSetShader(&self.pixel, None);
            device.context.PSSetShaderResources(0, Some(&[Some(self.view.clone())]));
            device
                .context
                .PSSetConstantBuffers(3, Some(&[Some(self.constants.clone())]));
            device.context.OMSetDepthStencilState(None, 0);
            device.context.OMSetBlendState(None, None, u32::MAX);
            device
                .context
                .OMSetRenderTargets(Some(&[Some(target.rtv.clone())]), None);
            device.context.RSSetState(&self.rasterizer);
            device.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: target.descriptor.extent.width() as f32,
                Height: target.descriptor.extent.height() as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
                ..Default::default()
            }]));
            device.context.Draw(3, 0);
            device.context.PSSetShaderResources(0, Some(&[None]));
            device.context.PSSetConstantBuffers(3, Some(&[None]));
            device.context.OMSetRenderTargets(None, None);
        }
        Ok(())
    }
}
