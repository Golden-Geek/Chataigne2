//! Procedural diagnostic cards: only a small constant buffer is uploaded per source frame.

use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::s;

use crate::{ProbeResult, d3d::Gpu};

const SHADER: &str = r#"
cbuffer CardData : register(b0) { float4 color; uint4 meta; };
struct Vertex { float4 position : SV_Position; float2 uv : TEXCOORD; };
Vertex vs_main(uint id : SV_VertexID) {
    float2 p[3] = {float2(-1,-1), float2(-1,3), float2(3,-1)};
    Vertex v; v.position=float4(p[id],0,1); v.uv=p[id]*float2(0.5,-0.5)+0.5; return v;
}
bool segment(float2 p, float2 a, float2 b) {
    return all(p >= a) && all(p <= b);
}
float digit(float2 p, uint value) {
    uint masks[10]={63,6,91,79,102,109,125,7,127,111};
    uint mask=masks[value%10];
    bool bits[7]={segment(p,float2(.15,.02),float2(.85,.14)),
        segment(p,float2(.80,.10),float2(.94,.48)),segment(p,float2(.80,.52),float2(.94,.90)),
        segment(p,float2(.15,.86),float2(.85,.98)),segment(p,float2(.06,.52),float2(.20,.90)),
        segment(p,float2(.06,.10),float2(.20,.48)),segment(p,float2(.15,.44),float2(.85,.56))};
    for(uint i=0;i<7;i++) if(bits[i] && (mask&(1u<<i))) return 1;
    return 0;
}
float4 ps_main(Vertex v) : SV_Target {
    float2 uv=v.uv;
    // Wide constant-color cells survive the existing bilinear composite scaling. Values
    // travel through the image itself, including channel identity and a corruption check.
    if(uv.y>.25 && uv.y<.28 && uv.x>=.02 && uv.x<.12) {
        uint magic=0xa671c300u|meta.y;
        uint check=((meta.x<<7)|(meta.x>>25))^meta.z^((meta.w<<13)|(meta.w>>19))^magic^0x9e3779b9u;
        uint words[5]={meta.x,magic,meta.z,meta.w,check};
        uint word=words[min((uint)((uv.x-.02)/.02),4u)];
        return float4(word&255u,(word>>8)&255u,(word>>16)&255u,word>>24)/255.0;
    }
    if(uv.y>.76) return float4(color.rgb,(floor(min(uv.x,.999)*4)+1)*.25);
    if(uv.y>.35 && uv.y<.65) return color;
    if(uv.y>.04 && uv.y<.23 && uv.x>.05 && uv.x<.95) {
        float2 p=float2((uv.x-.05)/.90*7,(uv.y-.04)/.19);
        uint column=(uint)p.x;
        uint divisor=1;
        for(uint i=0;i<6-min(column,6u);i++) divisor*=10;
        uint number=column==0?meta.y:meta.x/divisor;
        return float4(lerp(float3(.02,.02,.02),float3(1,1,1),digit(frac(p),number)),1);
    }
    uint2 cell=(uint2)floor(uv*float2(16,9));
    float grid=(frac(uv.x*16)<.025 || frac(uv.y*9)<.025)?1:0;
    float checker=((cell.x+cell.y)&1)?0.22:0.05;
    return float4(lerp(lerp(float3(checker,checker,checker),color.rgb,.45),float3(1,1,1),grid),1);
}
"#;

#[repr(C)]
struct Constants {
    color: [f32; 4],
    meta: [u32; 4],
}

pub struct Card {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    constants: ID3D11Buffer,
    raster: ID3D11RasterizerState,
}

impl Card {
    pub fn new(gpu: &Gpu) -> ProbeResult<Self> {
        let (mut vs, mut ps, mut vertex, mut pixel, mut constants, mut raster) = (None, None, None, None, None, None);
        // SAFETY: owned shader source, checked output slots and exact constant-buffer layout.
        unsafe {
            D3DCompile(
                SHADER.as_ptr().cast(),
                SHADER.len(),
                s!("probe-card"),
                None,
                None,
                s!("vs_main"),
                s!("vs_5_0"),
                0,
                0,
                &mut vs,
                None,
            )?;
            D3DCompile(
                SHADER.as_ptr().cast(),
                SHADER.len(),
                s!("probe-card"),
                None,
                None,
                s!("ps_main"),
                s!("ps_5_0"),
                0,
                0,
                &mut ps,
                None,
            )?;
            gpu.device
                .CreateVertexShader(blob(vs.as_ref().ok_or("no card vertex code")?), None, Some(&mut vertex))?;
            gpu.device
                .CreatePixelShader(blob(ps.as_ref().ok_or("no card pixel code")?), None, Some(&mut pixel))?;
            gpu.device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: 32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut constants),
            )?;
            gpu.device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut raster),
            )?;
        }
        Ok(Self {
            vertex: vertex.ok_or("no card vertex shader")?,
            pixel: pixel.ok_or("no card pixel shader")?,
            constants: constants.ok_or("no card constants")?,
            raster: raster.ok_or("no card raster state")?,
        })
    }
    pub fn draw(
        &self,
        gpu: &Gpu,
        target: &ID3D11Texture2D,
        color: [f32; 4],
        frame: u32,
        channel: u32,
    ) -> ProbeResult<()> {
        self.draw_input(
            gpu,
            target,
            mapgyver_native_probe::color_reference::CardInput {
                color,
                frame,
                channel,
                stamp: crate::clock::now()?,
            },
        )
    }

    pub fn draw_input(
        &self,
        gpu: &Gpu,
        target: &ID3D11Texture2D,
        input: mapgyver_native_probe::color_reference::CardInput,
    ) -> ProbeResult<()> {
        gpu.allocations
            .work(mapgyver_native_probe::allocation::Work::UniformUpload(32));
        gpu.allocations.work(mapgyver_native_probe::allocation::Work::Draw);
        let data = Constants {
            color: input.color,
            meta: [
                input.frame,
                input.channel,
                input.stamp as u32,
                (input.stamp >> 32) as u32,
            ],
        };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        let mut view = None;
        // SAFETY: target belongs to this device; only 32 bytes of uniform metadata are uploaded.
        unsafe {
            target.GetDesc(&mut desc);
            gpu.device.CreateRenderTargetView(target, None, Some(&mut view))?;
            gpu.context
                .UpdateSubresource(&self.constants, 0, None, (&data as *const Constants).cast(), 0, 0);
            gpu.context.OMSetRenderTargets(Some(&[view]), None);
            gpu.context.VSSetShader(&self.vertex, None);
            gpu.context.PSSetShader(&self.pixel, None);
            gpu.context
                .PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            gpu.context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            gpu.context.RSSetState(&self.raster);
            gpu.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: desc.Width as f32,
                Height: desc.Height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
                ..Default::default()
            }]));
            gpu.context.Draw(3, 0);
            gpu.context.OMSetRenderTargets(None, None);
        }
        Ok(())
    }
}

unsafe fn blob(value: &ID3DBlob) -> &[u8] {
    // SAFETY: compiled blob remains alive for this borrow.
    unsafe { std::slice::from_raw_parts(value.GetBufferPointer().cast(), value.GetBufferSize()) }
}
