use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::s;

use crate::{ProbeResult, d3d::Gpu, presentation::PhysicalRect};

const SHADER: &str = r#"
Texture2D image : register(t0);
SamplerState image_sampler : register(s0);
struct Vertex { float4 position : SV_Position; float2 uv : TEXCOORD; };
Vertex vs_main(uint id : SV_VertexID) {
    float2 positions[6] = {
        float2(-1,-1), float2(-1,1), float2(1,1),
        float2(-1,-1), float2(1,1), float2(1,-1)
    };
    Vertex v;
    v.position = float4(positions[id], 0, 1);
    v.uv = positions[id] * float2(0.5,-0.5) + 0.5;
    return v;
}
float4 ps_main(Vertex v) : SV_Target {
    return image.Sample(image_sampler, v.uv);
}
"#;

pub struct Quad {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    sampler: ID3D11SamplerState,
    rasterizer: ID3D11RasterizerState,
}

impl Quad {
    pub fn new(gpu: &Gpu) -> ProbeResult<Self> {
        let mut vertex_code = None;
        let mut pixel_code = None;
        let mut vertex = None;
        let mut pixel = None;
        let mut sampler = None;
        let mut rasterizer = None;
        // SAFETY: in-memory owned shader bytes; no includes, macros or external compiler inputs.
        unsafe {
            D3DCompile(
                SHADER.as_ptr().cast(),
                SHADER.len(),
                s!("probe"),
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
                SHADER.as_ptr().cast(),
                SHADER.len(),
                s!("probe"),
                None,
                None,
                s!("ps_main"),
                s!("ps_5_0"),
                0,
                0,
                &mut pixel_code,
                None,
            )?;
            gpu.device.CreateVertexShader(
                blob(vertex_code.as_ref().ok_or("no vertex code")?),
                None,
                Some(&mut vertex),
            )?;
            gpu.device.CreatePixelShader(
                blob(pixel_code.as_ref().ok_or("no pixel code")?),
                None,
                Some(&mut pixel),
            )?;
            gpu.device.CreateSamplerState(
                &D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    MaxLOD: f32::MAX,
                    ..Default::default()
                },
                Some(&mut sampler),
            )?;
            gpu.device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    ScissorEnable: true.into(),
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut rasterizer),
            )?;
        }
        Ok(Self {
            vertex: vertex.ok_or("no vertex shader")?,
            pixel: pixel.ok_or("no pixel shader")?,
            sampler: sampler.ok_or("no sampler")?,
            rasterizer: rasterizer.ok_or("no rasterizer")?,
        })
    }

    pub fn draw(
        &self,
        gpu: &Gpu,
        target: &ID3D11Texture2D,
        source: &ID3D11Texture2D,
        rect: PhysicalRect,
    ) -> ProbeResult<()> {
        self.draw_clipped(gpu, target, source, rect, rect)
    }

    pub fn draw_clipped(
        &self,
        gpu: &Gpu,
        target: &ID3D11Texture2D,
        source: &ID3D11Texture2D,
        rect: PhysicalRect,
        clip: PhysicalRect,
    ) -> ProbeResult<()> {
        if rect.width == 0 || rect.height == 0 {
            return Ok(());
        }
        gpu.allocations.work(mapgyver_native_probe::allocation::Work::Draw);
        let mut render_view = None;
        let mut source_view = None;
        // SAFETY: all resources belong to gpu, target is renderable and source sampleable.
        // Binding is cleared before return, preventing resize hazards and retained frame views.
        unsafe {
            gpu.device
                .CreateRenderTargetView(target, None, Some(&mut render_view))?;
            gpu.device
                .CreateShaderResourceView(source, None, Some(&mut source_view))?;
            gpu.context.OMSetRenderTargets(Some(&[render_view]), None);
            gpu.context.VSSetShader(&self.vertex, None);
            gpu.context.PSSetShader(&self.pixel, None);
            gpu.context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            gpu.context.PSSetShaderResources(0, Some(&[source_view]));
            gpu.context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            gpu.context.RSSetState(&self.rasterizer);
            gpu.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: rect.left as f32,
                TopLeftY: rect.top as f32,
                Width: rect.width as f32,
                Height: rect.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            gpu.context.RSSetScissorRects(Some(&[windows::Win32::Foundation::RECT {
                left: clip.left.max(0),
                top: clip.top.max(0),
                right: clip.left.saturating_add_unsigned(clip.width).max(0),
                bottom: clip.top.saturating_add_unsigned(clip.height).max(0),
            }]));
            gpu.context.Draw(6, 0);
            gpu.context.PSSetShaderResources(0, Some(&[None]));
            gpu.context.OMSetRenderTargets(None, None);
        }
        Ok(())
    }
}

unsafe fn blob(value: &ID3DBlob) -> &[u8] {
    // SAFETY: caller keeps the compiled blob alive for the returned borrow.
    unsafe { std::slice::from_raw_parts(value.GetBufferPointer().cast(), value.GetBufferSize()) }
}
