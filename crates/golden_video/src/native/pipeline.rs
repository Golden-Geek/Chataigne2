use std::marker::PhantomData;
use std::rc::Rc;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
#[cfg(test)]
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
#[cfg(test)]
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::Win32::Graphics::Direct3D11::*;
#[cfg(test)]
use windows::core::s;
use windows::core::{Error, HRESULT, Result};

use crate::plan::{BlendMode, BlitRegion, Layer, LayerSource, Pattern, ProjectiveMapping, SampleFilter, UnitRect};
mod overlay;
mod pattern;
use crate::{AlphaMode, ColorEncoding, LinearColor};
use pattern::PatternParameters;
use std::cell::Cell;

use super::texture::Texture;

#[cfg(test)]
const SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/native_shader.hlsl"));

pub(super) mod shaders {
    include!(concat!(env!("OUT_DIR"), "/native_shaders.rs"));
}

#[cfg(test)]
mod tests;

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
    source_storage: u32,
    reserved: u32,
    source_rect: [f32; 4],
    inverse: [[f32; 4]; 3],
}

const _: () = assert!(size_of::<Parameters>() == 112);
pub(super) const FIXED_BUFFER_BYTES: u64 =
    (size_of::<Parameters>() + size_of::<PatternParameters>() + overlay::CONSTANT_BYTES) as u64;

struct DrawSettings {
    opacity: f32,
    blend: BlendMode,
    filter: SampleFilter,
    destination: UnitRect,
    source_rect: UnitRect,
    mapping: Option<ProjectiveMapping>,
    pattern: Option<Pattern>,
}

/// All fallible shader/state allocation precedes frame submission.
pub(super) struct Pipeline {
    frame_sampler: Cell<pattern::frame::FrameSampler>,
    frame_text: Cell<pattern::frame::FrameText>,
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    mapped_pixel: ID3D11PixelShader,
    pattern_pixel: ID3D11PixelShader,
    overlay_pixel: ID3D11PixelShader,
    overlay_constants: ID3D11Buffer,
    overlay_blend: ID3D11BlendState,
    constants: ID3D11Buffer,
    pattern_data: pattern::PatternBuffer,
    rasterizer: ID3D11RasterizerState,
    _local: PhantomData<Rc<()>>,
}

impl Pipeline {
    pub(super) fn begin_frame(&self, time: golden_time::TimeSnapshot) {
        let mut sampler = self.frame_sampler.get();
        self.frame_text.set(sampler.begin(time));
        self.frame_sampler.set(sampler);
    }
    pub(super) fn submitted(&self, time: golden_time::TimeSnapshot) {
        let mut sampler = self.frame_sampler.get();
        sampler.submitted(time);
        self.frame_sampler.set(sampler);
    }
    pub(super) fn new(device: &ID3D11Device) -> Result<Self> {
        let mut vertex = None;
        let mut pixel = None;
        let mut mapped_pixel = None;
        let mut pattern_pixel = None;
        let mut constants = None;
        let mut rasterizer = None;
        // SAFETY: compiler-validated immutable programs are embedded in this crate. Each
        // native object still belongs exclusively to this device/worker and its retirement.
        unsafe {
            device.CreateVertexShader(shaders::VERTEX, None, Some(&mut vertex))?;
            device.CreatePixelShader(shaders::PIXEL, None, Some(&mut pixel))?;
            device.CreatePixelShader(shaders::MAPPED_PIXEL, None, Some(&mut mapped_pixel))?;
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
            device.CreatePixelShader(shaders::PATTERN_PIXEL, None, Some(&mut pattern_pixel))?;
        }
        let (overlay_pixel, overlay_constants, overlay_blend) = overlay::prepare(device)?;
        Ok(Self {
            frame_sampler: Cell::default(),
            frame_text: Cell::default(),
            overlay_pixel,
            overlay_constants,
            overlay_blend,
            vertex: vertex.ok_or_else(missing_output)?,
            pixel: pixel.ok_or_else(missing_output)?,
            mapped_pixel: mapped_pixel.ok_or_else(missing_output)?,
            pattern_pixel: pattern_pixel.ok_or_else(missing_output)?,
            constants: constants.ok_or_else(missing_output)?,
            pattern_data: pattern::PatternBuffer::new(device)?,
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
            Some(source),
            previous,
            target,
            DrawSettings {
                opacity,
                blend,
                filter,
                destination: UnitRect::FULL,
                source_rect: UnitRect::FULL,
                mapping: None,
                pattern: None,
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
            Some(source),
            None,
            target.into(),
            DrawSettings {
                opacity: 1.0,
                blend: BlendMode::Normal,
                filter: region.filter,
                destination: region.destination,
                source_rect: region.source_rect,
                mapping: None,
                pattern: None,
            },
        );
    }

    pub(super) fn draw_layer(
        &self,
        context: &ID3D11DeviceContext,
        source: Option<&Texture>,
        previous: Option<&Texture>,
        target: RenderTarget<'_>,
        layer: &Layer,
    ) {
        self.draw_view(
            context,
            source,
            previous,
            target,
            DrawSettings {
                opacity: layer.opacity.get(),
                blend: layer.blend,
                filter: layer.filter,
                destination: UnitRect::FULL,
                source_rect: layer.mapping.map_or(UnitRect::FULL, ProjectiveMapping::crop),
                mapping: layer.mapping,
                pattern: match &layer.source {
                    LayerSource::Pattern(pattern) => Some(pattern.clone()),
                    LayerSource::Texture(_) => None,
                },
            },
        );
    }

    fn draw_view(
        &self,
        context: &ID3D11DeviceContext,
        source: Option<&Texture>,
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
            mapping,
            pattern,
        } = settings;
        debug_assert!(opacity.is_finite() && (0.0..=1.0).contains(&opacity));
        if let Some(previous) = previous {
            debug_assert!(previous.descriptor.is_compositing_target());
            debug_assert_eq!(previous.descriptor.extent, target.descriptor.extent);
        }
        let (source_view, source_encoding) = source
            .map(|source| {
                let (view, encoding) = source.sampling_view();
                (Some(view), encoding)
            })
            .unwrap_or((None, ColorEncoding::Linear));
        let parameters = Parameters {
            source_extent: source.map_or([1, 1], |source| {
                [source.descriptor.extent.width(), source.descriptor.extent.height()]
            }),
            source_encoding: encoding(source_encoding),
            source_alpha: alpha(source.map_or(AlphaMode::Premultiplied, |source| source.descriptor.alpha_mode)),
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
            source_storage: match source.map(|texture| texture.descriptor.format) {
                Some(crate::PixelFormat::Bgr565Unorm) => 1,
                Some(crate::PixelFormat::Bgra5551Unorm) => 2,
                Some(crate::PixelFormat::Rgb10XrA2Unorm) => 3,
                _ => 0,
            },
            reserved: 0,
            source_rect: [source_rect.x, source_rect.y, source_rect.width, source_rect.height],
            inverse: mapping.map_or([[0.0; 4]; 3], ProjectiveMapping::inverse_rows),
        };
        let width = target.descriptor.extent.width() as f32;
        let height = target.descriptor.extent.height() as f32;
        // SAFETY: all resources and views were prepared on this device. Only uniform metadata
        // is uploaded; no fallible API operation occurs after acquiring GPU use.
        // Every resource view is unbound before returning, avoiding read/write/retirement hazards.
        unsafe {
            if let Some(pattern) = &pattern {
                let data = PatternParameters::with_frame(pattern, mapping.is_some(), self.frame_text.get());
                self.pattern_data.bind(context, &data);
            }
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
            // Existing full-target/region draws retain their original pixel entry point.
            context.PSSetShader(
                if pattern.is_some() {
                    &self.pattern_pixel
                } else if mapping.is_some() {
                    &self.mapped_pixel
                } else {
                    &self.pixel
                },
                None,
            );
            context.PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            context.PSSetShaderResources(
                0,
                Some(&[source_view.cloned(), previous.map(|texture| texture.srv.clone())]),
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
            context.PSSetShaderResources(0, Some(&[None, None, None]));
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

#[cfg(test)]
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
