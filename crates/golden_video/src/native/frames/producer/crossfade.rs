//! Blend two worker-local decode results before the shared publication boundary.

use super::{
    Conversion, Device, GpuFrameBudget, GpuFrameError, GpuFramePublisher, Input, copy_visible_planes,
    software::upload_image,
};
use crate::decode::{D3d11Frame, HapFrame};
use crate::image::ImageData;
use crate::native::{AllocationKind, SoftwareUploadTrace, pipeline::shaders, texture::Texture};
use crate::{AlphaMode, ColorEncoding, TextureDescriptor};
use std::sync::Arc;
use windows::Win32::Graphics::{Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, Direct3D11::*};
use windows::core::{Error, HRESULT, Result};

/// Decoded input retained through completion of its worker-owned conversion.
pub enum GpuFrameInput<'a> {
    /// Tight CPU pixels with the publisher's immutable descriptor.
    Software(Arc<ImageData>),
    /// The publisher's admitted HAP compression format and extent.
    Hap(&'a HapFrame),
    /// A frame on the publisher's exact D3D11 device, including the decoder pool lease.
    Decoded(&'a D3d11Frame),
}

impl GpuFrameInput<'_> {
    fn retain(&self) -> std::result::Result<Input, GpuFrameError> {
        Ok(match self {
            Self::Software(image) => Input::Software {
                _image: Arc::clone(image),
            },
            Self::Hap(frame) => Input::Hap {
                _frame: (*frame).clone(),
            },
            Self::Decoded(frame) => Input::Decoded {
                _frame: frame.retain()?,
            },
        })
    }
}

#[repr(C)]
struct Parameters {
    weight: f32,
    encoding: u32,
    alpha: u32,
    reserved: u32,
}
const _: () = assert!(size_of::<Parameters>() == 16);

pub(super) struct Crossfade {
    scratch: [Texture; 2],
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    rasterizer: ID3D11RasterizerState,
    constants: ID3D11Buffer,
    // Drop this after the scratch textures are retired.
    _lease: Arc<dyn Send + Sync>,
}

impl Crossfade {
    fn new(device: &Device, descriptor: TextureDescriptor, lease: Arc<dyn Send + Sync>) -> Result<Self> {
        let scratch = [
            Texture::new(&device.device, descriptor)?,
            Texture::new(&device.device, descriptor)?,
        ];
        let (mut vertex, mut pixel, mut rasterizer, mut constants) = (None, None, None, None);
        // SAFETY: fixed compiled shader, valid same-device state, and initialized outputs.
        unsafe {
            device
                .device
                .CreateVertexShader(shaders::VERTEX, None, Some(&mut vertex))?;
            device
                .device
                .CreatePixelShader(shaders::CROSSFADE_PIXEL, None, Some(&mut pixel))?;
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
                    ByteWidth: size_of::<Parameters>() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut constants),
            )?;
        }
        let missing = || Error::from_hresult(HRESULT(0x8000_4005_u32 as i32));
        Ok(Self {
            scratch,
            vertex: vertex.ok_or_else(missing)?,
            pixel: pixel.ok_or_else(missing)?,
            rasterizer: rasterizer.ok_or_else(missing)?,
            constants: constants.ok_or_else(missing)?,
            _lease: lease,
        })
    }

    fn draw(&self, context: &ID3D11DeviceContext, target: &Texture, weight: f32) {
        let descriptor = target.descriptor;
        let parameters = Parameters {
            weight,
            encoding: u32::from(descriptor.color_encoding == ColorEncoding::Srgb),
            alpha: match descriptor.alpha_mode {
                AlphaMode::Straight => 0,
                AlphaMode::Premultiplied => 1,
                AlphaMode::Opaque => 2,
            },
            reserved: 0,
        };
        // SAFETY: resource creation and the exclusive output slot precede the draw.
        // Inputs and constants remain alive through the same fence as the output.
        unsafe {
            context.UpdateSubresource(&self.constants, 0, None, std::ptr::from_ref(&parameters).cast(), 0, 0);
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex, None);
            context.GSSetShader(None, None);
            context.HSSetShader(None, None);
            context.DSSetShader(None, None);
            context.PSSetShader(&self.pixel, None);
            context.PSSetShaderResources(
                0,
                Some(&self.scratch.each_ref().map(|texture| Some(texture.srv.clone()))),
            );
            context.PSSetConstantBuffers(4, Some(&[Some(self.constants.clone())]));
            context.OMSetDepthStencilState(None, 0);
            context.OMSetBlendState(None, None, u32::MAX);
            context.OMSetRenderTargets(Some(&[Some(target.rtv.clone())]), None);
            context.RSSetState(&self.rasterizer);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: descriptor.extent.width() as f32,
                Height: descriptor.extent.height() as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
                ..Default::default()
            }]));
            context.Draw(3, 0);
            context.PSSetShaderResources(0, Some(&[None, None]));
            context.PSSetConstantBuffers(4, Some(&[None]));
            context.OMSetRenderTargets(None, None);
        }
    }
}

impl GpuFramePublisher {
    /// Admit one fixed two-image scratch set for this publisher on demand.
    pub fn prepare_crossfade(&mut self) -> std::result::Result<(), GpuFrameError> {
        self.healthy()?;
        if self.crossfade.is_none() {
            let descriptor = self.stream.descriptor();
            let kind = match self.conversion {
                Conversion::Software { .. } => AllocationKind::SoftwareProducer,
                Conversion::Hap(_) => AllocationKind::HapProducer,
                Conversion::Video { .. } => AllocationKind::HardwareProducer,
            };
            let bytes = descriptor
                .byte_len()
                .ok()
                .and_then(|bytes| bytes.checked_mul(2))
                .and_then(|bytes| bytes.checked_add(size_of::<Parameters>() as u64))
                .ok_or(GpuFrameError::Capacity)?;
            let lease = self.budget.reserve(bytes, kind).ok_or(GpuFrameError::Capacity)?;
            self.crossfade = Some(Crossfade::new(&self.device, descriptor, lease)?);
        }
        Ok(())
    }

    /// Release scratch once the matching write fence is observed. The published
    /// output has its own storage lease and can remain sampled after this succeeds.
    pub fn release_crossfade(&mut self) -> bool {
        if self.pending.as_ref().is_some_and(|pending| pending.head.is_some()) {
            return false;
        }
        self.crossfade = None;
        true
    }

    /// Submit one paired conversion to one output slot. Frames must have the same
    /// admitted storage class and interpretation. The caller supplies the evaluated
    /// head weight in [0, 1], and the shader uses its complement for the tail.
    pub fn submit_crossfade(
        &mut self,
        tail: GpuFrameInput<'_>,
        head: GpuFrameInput<'_>,
        weight: f32,
    ) -> std::result::Result<bool, GpuFrameError> {
        if !(0.0..=1.0).contains(&weight) {
            return Err(GpuFrameError::Layout);
        }
        if weight == 0.0 || weight == 1.0 {
            return match if weight == 0.0 { tail } else { head } {
                GpuFrameInput::Software(image) => self.submit_software(image),
                GpuFrameInput::Hap(frame) => self.submit_hap(frame),
                GpuFrameInput::Decoded(frame) => self.submit(frame),
            };
        }
        if self.crossfade.is_none() {
            return Err(GpuFrameError::Layout);
        }
        self.validate_blend_input(&tail)?;
        self.validate_blend_input(&head)?;
        if let Conversion::Software { sequence, .. } = self.conversion {
            sequence.checked_add(2).ok_or(GpuFrameError::Capacity)?;
        }
        let retained_head = head.retain()?;
        let Some(index) = self.begin(tail.retain()?)? else {
            return Ok(false);
        };
        let pending = self.pending.as_mut().unwrap();
        pending.head = Some(retained_head);
        let token = pending.token;
        let _cpu = self
            .diagnostics
            .as_ref()
            .map(|diagnostics| diagnostics.cpu_scope(token));
        let _mark = self.completion.begin(&self.device, token);
        let _gpu = self
            .diagnostics
            .as_mut()
            .map(|diagnostics| diagnostics.gpu.begin(&self.device.context, token));
        let crossfade = self.crossfade.as_ref().unwrap();
        for (side, (frame, scratch)) in [tail, head].into_iter().zip(&crossfade.scratch).enumerate() {
            pending.crossfade_uploads[side] = stage_frame(
                &mut self.conversion,
                &self.device,
                self.stream.id(),
                &frame,
                scratch,
                self.uploads.as_ref(),
            )?;
        }
        crossfade.draw(&self.device.context, &self.slots[index].texture, weight);
        Ok(true)
    }

    fn validate_blend_input(&self, frame: &GpuFrameInput<'_>) -> std::result::Result<(), GpuFrameError> {
        let descriptor = self.stream.descriptor();
        match (&self.conversion, frame) {
            (Conversion::Software { .. }, GpuFrameInput::Software(image)) if image.descriptor() == descriptor => Ok(()),
            (Conversion::Hap(image), GpuFrameInput::Hap(frame))
                if frame.extent() == descriptor.extent && frame.format() == image.format =>
            {
                Ok(())
            }
            (Conversion::Video { .. }, GpuFrameInput::Decoded(frame)) => self.validate_decoded(frame),
            _ => Err(GpuFrameError::Layout),
        }
    }
}

fn stage_frame(
    conversion: &mut Conversion,
    device: &Device,
    stream: uuid::Uuid,
    frame: &GpuFrameInput<'_>,
    target: &Texture,
    uploads: Option<&GpuFrameBudget>,
) -> std::result::Result<Option<SoftwareUploadTrace>, GpuFrameError> {
    use super::super::budget::UploadKind;
    match (conversion, frame) {
        (Conversion::Video { input, shader, .. }, GpuFrameInput::Decoded(frame)) => {
            // The conversion image is reused by the two ordered draws on one context.
            copy_visible_planes(&device.context, input, &frame.texture, frame.slice);
            shader.draw(&device.context, input, target);
        }
        (Conversion::Hap(input), GpuFrameInput::Hap(frame)) => {
            input.upload_draw(device, frame, target)?;
            if let Some(uploads) = uploads {
                uploads.record_upload(UploadKind::Hap, frame.blocks().len() as u64);
            }
        }
        (Conversion::Software { sequence, clock }, GpuFrameInput::Software(image)) => {
            *sequence += 1;
            let trace = upload_image(&device.context, target, image, stream, *sequence, *clock);
            if let Some(uploads) = uploads {
                uploads.record_upload(UploadKind::Software, trace.bytes);
            }
            return Ok(Some(trace));
        }
        _ => unreachable!("the input/storage combination was validated before submission"),
    }
    Ok(None)
}
