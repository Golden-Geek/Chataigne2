use std::marker::PhantomData;
use std::rc::Rc;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::{Error, HRESULT, Result, s};

use crate::plan::{BlendMode, BlitRegion, SampleFilter, UnitRect};
use crate::{AlphaMode, ColorEncoding, LinearColor};

use super::texture::Texture;

const SHADER: &str = include_str!("shader.hlsl");

/// A prepared destination view; its owner retains storage through actual GPU completion.
#[derive(Clone, Copy)]
pub(super) struct RenderTarget<'a> {
    pub view: &'a ID3D11RenderTargetView,
    pub descriptor: crate::TextureDescriptor,
    /// Encoding expected from the shader, before the view's optional fixed-function transfer.
    pub shader_encoding: ColorEncoding,
}

impl<'a> From<&'a Texture> for RenderTarget<'a> {
    fn from(texture: &'a Texture) -> Self {
        Self {
            view: &texture.rtv,
            descriptor: texture.descriptor,
            shader_encoding: texture.descriptor.color_encoding,
        }
    }
}

#[repr(C)]
struct Parameters {
    source_extent: [u32; 2],
    source_encoding: u32,
    source_alpha: u32,
    target_encoding: u32,
    target_alpha: u32,
    filter: u32,
    blend: u32,
    opacity: f32,
    has_previous: u32,
    reserved: [u32; 2],
    source_rect: [f32; 4],
}

const _: () = assert!(size_of::<Parameters>() == 64);
pub(super) const CONSTANT_BYTES: u64 = size_of::<Parameters>() as u64;

struct DrawSettings {
    opacity: f32,
    blend: BlendMode,
    filter: SampleFilter,
    destination: UnitRect,
    source_rect: UnitRect,
}

/// All fallible shader/state allocation precedes frame submission.
pub(super) struct Pipeline {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    constants: ID3D11Buffer,
    rasterizer: ID3D11RasterizerState,
    _local: PhantomData<Rc<()>>,
}

impl Pipeline {
    pub(super) fn new(device: &ID3D11Device) -> Result<Self> {
        let mut vertex_code = None;
        let mut pixel_code = None;
        let mut vertex = None;
        let mut pixel = None;
        let mut constants = None;
        let mut rasterizer = None;
        // SAFETY: fixed in-memory source, no includes/macros or external compiler inputs.
        unsafe {
            D3DCompile(
                SHADER.as_ptr().cast(),
                SHADER.len(),
                s!("golden-video"),
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
                s!("golden-video"),
                None,
                None,
                s!("ps_main"),
                s!("ps_5_0"),
                0,
                0,
                &mut pixel_code,
                None,
            )?;
            device.CreateVertexShader(
                blob(vertex_code.as_ref().ok_or_else(missing_output)?),
                None,
                Some(&mut vertex),
            )?;
            device.CreatePixelShader(
                blob(pixel_code.as_ref().ok_or_else(missing_output)?),
                None,
                Some(&mut pixel),
            )?;
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: size_of::<Parameters>() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut constants),
            )?;
            device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    ScissorEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut rasterizer),
            )?;
        }
        Ok(Self {
            vertex: vertex.ok_or_else(missing_output)?,
            pixel: pixel.ok_or_else(missing_output)?,
            constants: constants.ok_or_else(missing_output)?,
            rasterizer: rasterizer.ok_or_else(missing_output)?,
            _local: PhantomData,
        })
    }

    pub(super) fn clear(&self, context: &ID3D11DeviceContext, target: &Texture, color: LinearColor) {
        debug_assert!(target.descriptor.is_compositing_target());
        // SAFETY: prepared RTV belongs to this worker's context and linear target.
        unsafe { context.ClearRenderTargetView(&target.rtv, &color.components()) };
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &self,
        context: &ID3D11DeviceContext,
        source: &Texture,
        previous: Option<&Texture>,
        target: RenderTarget<'_>,
        opacity: f32,
        blend: BlendMode,
        filter: SampleFilter,
    ) {
        self.draw_view(
            context,
            source,
            previous,
            target,
            DrawSettings {
                opacity,
                blend,
                filter,
                destination: UnitRect::FULL,
                source_rect: UnitRect::FULL,
            },
        );
    }

    pub(super) fn draw_region(
        &self,
        context: &ID3D11DeviceContext,
        source: &Texture,
        target: &Texture,
        region: &BlitRegion,
    ) {
        debug_assert!(region.destination.is_valid() && region.source_rect.is_valid());
        self.draw_view(
            context,
            source,
            None,
            target.into(),
            DrawSettings {
                opacity: 1.0,
                blend: BlendMode::Normal,
                filter: region.filter,
                destination: region.destination,
                source_rect: region.source_rect,
            },
        );
    }

    fn draw_view(
        &self,
        context: &ID3D11DeviceContext,
        source: &Texture,
        previous: Option<&Texture>,
        target: RenderTarget<'_>,
        settings: DrawSettings,
    ) {
        let DrawSettings {
            opacity,
            blend,
            filter,
            destination,
            source_rect,
        } = settings;
        debug_assert!(opacity.is_finite() && (0.0..=1.0).contains(&opacity));
        if let Some(previous) = previous {
            debug_assert!(previous.descriptor.is_compositing_target());
            debug_assert_eq!(previous.descriptor.extent, target.descriptor.extent);
        }
        let (source_view, source_encoding) = source.sampling_view();
        let parameters = Parameters {
            source_extent: [source.descriptor.extent.width(), source.descriptor.extent.height()],
            source_encoding: encoding(source_encoding),
            source_alpha: alpha(source.descriptor.alpha_mode),
            target_encoding: encoding(target.shader_encoding),
            target_alpha: alpha(target.descriptor.alpha_mode),
            filter: u32::from(filter == SampleFilter::Linear),
            blend: match blend {
                BlendMode::Normal => 0,
                BlendMode::Add => 1,
                BlendMode::Multiply => 2,
                BlendMode::Screen => 3,
            },
            opacity,
            has_previous: u32::from(previous.is_some()),
            reserved: [0; 2],
            source_rect: [source_rect.x, source_rect.y, source_rect.width, source_rect.height],
        };
        let width = target.descriptor.extent.width() as f32;
        let height = target.descriptor.extent.height() as f32;
        // SAFETY: all resources and views were prepared on this device. Only uniform metadata
        // is uploaded; no fallible API operation occurs after acquiring GPU use.
        // Every resource view is unbound before returning, avoiding read/write/retirement hazards.
        unsafe {
            context.UpdateSubresource(
                &self.constants,
                0,
                None,
                (&parameters as *const Parameters).cast(),
                0,
                0,
            );
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex, None);
            context.GSSetShader(None, None);
            context.HSSetShader(None, None);
            context.DSSetShader(None, None);
            context.PSSetShader(&self.pixel, None);
            context.PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            context.PSSetShaderResources(
                0,
                Some(&[Some(source_view.clone()), previous.map(|texture| texture.srv.clone())]),
            );
            context.OMSetDepthStencilState(None, 0);
            context.OMSetBlendState(None, None, u32::MAX);
            context.OMSetRenderTargets(Some(&[Some(target.view.clone())]), None);
            context.RSSetState(&self.rasterizer);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: destination.x * width,
                TopLeftY: destination.y * height,
                Width: destination.width * width,
                Height: destination.height * height,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            context.RSSetScissorRects(Some(&[RECT {
                left: (destination.x * width).floor() as i32,
                top: (destination.y * height).floor() as i32,
                right: ((destination.x + destination.width) * width).ceil().min(width) as i32,
                bottom: ((destination.y + destination.height) * height).ceil().min(height) as i32,
            }]));
            context.Draw(3, 0);
            context.PSSetShaderResources(0, Some(&[None, None]));
            context.OMSetRenderTargets(None, None);
        }
    }
}

fn encoding(value: ColorEncoding) -> u32 {
    match value {
        ColorEncoding::Linear => 0,
        ColorEncoding::Srgb => 1,
    }
}

fn alpha(value: AlphaMode) -> u32 {
    match value {
        AlphaMode::Straight => 0,
        AlphaMode::Premultiplied => 1,
        AlphaMode::Opaque => 2,
    }
}

unsafe fn blob(value: &ID3DBlob) -> &[u8] {
    // SAFETY: caller retains the compiled blob for the returned borrow.
    unsafe { std::slice::from_raw_parts(value.GetBufferPointer().cast(), value.GetBufferSize()) }
}

fn missing_output() -> Error {
    Error::new(
        HRESULT(0x8000_4005_u32 as i32),
        "D3D11 returned no shader or pipeline state",
    )
}
