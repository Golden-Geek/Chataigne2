use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::{Error, HRESULT, Result, s};

const INVALID_ARGUMENT: HRESULT = HRESULT(0x8007_0057_u32 as i32);

/// Fixed shader/state allocation shared by the qualification producer and GPU log consumer.
pub(in crate::native) struct Pipeline {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    constants: Option<ID3D11Buffer>,
    constants_bytes: u32,
    raster: ID3D11RasterizerState,
    depth: ID3D11DepthStencilState,
}

impl Pipeline {
    pub(in crate::native) fn new(device: &ID3D11Device, source: &str, constants_bytes: u32) -> Result<Self> {
        if constants_bytes > 65536 || !constants_bytes.is_multiple_of(16) {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        let mut vertex_code = None;
        let mut pixel_code = None;
        let mut vertex = None;
        let mut pixel = None;
        let mut constants = None;
        let mut raster = None;
        let mut depth = None;
        // SAFETY: fixed in-memory source, initialized owned outputs and no external includes.
        unsafe {
            D3DCompile(
                source.as_ptr().cast(),
                source.len(),
                s!("golden-fixture"),
                None,
                None,
                s!("vs_main"),
                s!("vs_5_0"),
                0,
                0,
                &mut vertex_code,
                None,
            )?;
            D3DCompile(
                source.as_ptr().cast(),
                source.len(),
                s!("golden-fixture"),
                None,
                None,
                s!("ps_main"),
                s!("ps_5_0"),
                0,
                0,
                &mut pixel_code,
                None,
            )?;
            device.CreateVertexShader(blob(vertex_code.as_ref().ok_or_else(missing)?), None, Some(&mut vertex))?;
            device.CreatePixelShader(blob(pixel_code.as_ref().ok_or_else(missing)?), None, Some(&mut pixel))?;
            if constants_bytes != 0 {
                device.CreateBuffer(
                    &D3D11_BUFFER_DESC {
                        ByteWidth: constants_bytes,
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                        ..Default::default()
                    },
                    None,
                    Some(&mut constants),
                )?;
            }
            device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut raster),
            )?;
            device.CreateDepthStencilState(
                &D3D11_DEPTH_STENCIL_DESC {
                    DepthEnable: false.into(),
                    DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ZERO,
                    DepthFunc: D3D11_COMPARISON_ALWAYS,
                    FrontFace: D3D11_DEPTH_STENCILOP_DESC {
                        StencilFailOp: D3D11_STENCIL_OP_KEEP,
                        StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
                        StencilPassOp: D3D11_STENCIL_OP_KEEP,
                        StencilFunc: D3D11_COMPARISON_ALWAYS,
                    },
                    BackFace: D3D11_DEPTH_STENCILOP_DESC {
                        StencilFailOp: D3D11_STENCIL_OP_KEEP,
                        StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
                        StencilPassOp: D3D11_STENCIL_OP_KEEP,
                        StencilFunc: D3D11_COMPARISON_ALWAYS,
                    },
                    ..Default::default()
                },
                Some(&mut depth),
            )?;
        }
        Ok(Self {
            vertex: vertex.ok_or_else(missing)?,
            pixel: pixel.ok_or_else(missing)?,
            constants,
            constants_bytes,
            raster: raster.ok_or_else(missing)?,
            depth: depth.ok_or_else(missing)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::native) fn draw_region(
        &self,
        context: &ID3D11DeviceContext,
        target: &ID3D11RenderTargetView,
        origin: [u32; 2],
        extent: [u32; 2],
        resources: &[Option<ID3D11ShaderResourceView>],
        constants: &[u32],
    ) -> Result<()> {
        if extent.contains(&0)
            || origin[0].checked_add(extent[0]).is_none()
            || origin[1].checked_add(extent[1]).is_none()
            || resources.len() > 8
            || size_of_val(constants) != self.constants_bytes as usize
        {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        let clear: [Option<ID3D11ShaderResourceView>; 8] = std::array::from_fn(|_| None);
        // SAFETY: fixture callers retain same-device views and context. The complete constant
        // payload was validated before entering native commands and stays live through upload.
        unsafe {
            if let Some(buffer) = &self.constants {
                context.UpdateSubresource(buffer, 0, None, constants.as_ptr().cast(), 0, 0);
            }
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex, None);
            context.PSSetShader(&self.pixel, None);
            context.PSSetConstantBuffers(0, Some(std::slice::from_ref(&self.constants)));
            context.PSSetShaderResources(0, Some(resources));
            context.RSSetState(&self.raster);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: origin[0] as f32,
                TopLeftY: origin[1] as f32,
                Width: extent[0] as f32,
                Height: extent[1] as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            context.OMSetBlendState(None, None, u32::MAX);
            context.OMSetDepthStencilState(&self.depth, 0);
            context.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            context.Draw(3, 0);
            context.OMSetRenderTargets(Some(&[None]), None);
            context.PSSetShaderResources(0, Some(&clear[..resources.len()]));
        }
        Ok(())
    }
}

fn blob(value: &ID3DBlob) -> &[u8] {
    // SAFETY: this borrowed compiled blob retains the returned byte range for the call.
    unsafe { std::slice::from_raw_parts(value.GetBufferPointer().cast(), value.GetBufferSize()) }
}

fn missing() -> Error {
    Error::from_hresult(HRESULT(0x8000_4005_u32 as i32))
}
