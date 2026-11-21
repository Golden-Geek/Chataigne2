//! A bounded, nonblocking handoff from the renderer's texture to a CPU consumer.
use std::sync::mpsc::TrySendError;

use windows::{
    Win32::Graphics::{
        Direct3D11::{
            D3D11_CPU_ACCESS_READ, D3D11_MAP_FLAG_DO_NOT_WAIT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
            D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
        },
        Dxgi::{Common::DXGI_SAMPLE_DESC, DXGI_ERROR_WAS_STILL_DRAWING},
    },
    core::{Error, HRESULT, Result},
};

use super::{Delivery, INVALID_ARGUMENT};
use crate::readback::{ReadbackFrame, ReadbackSink};
use crate::{TextureDescriptor, native::OutputTextureAllocation, native::texture::Texture, runtime::PlanIdentity};

const FAILURE: HRESULT = HRESULT(0x80004005_u32 as i32);

pub(super) struct ReadbackOutput {
    descriptor: TextureDescriptor,
    staging: ID3D11Texture2D,
    pending: Option<PlanIdentity>,
    pixels: Vec<u8>,
    frames: ReadbackSink,
}

impl ReadbackOutput {
    pub fn new(device: &ID3D11Device, descriptor: TextureDescriptor, frames: ReadbackSink) -> Result<Self> {
        let bytes = descriptor
            .byte_len()
            .map_err(|_| Error::from_hresult(INVALID_ARGUMENT))? as usize;
        Ok(Self {
            descriptor,
            staging: staging_texture(device, descriptor)?,
            pending: None,
            pixels: vec![0; bytes],
            frames,
        })
    }

    pub fn allocation(&self) -> OutputTextureAllocation {
        OutputTextureAllocation::Readback {
            descriptor: self.descriptor,
            staging_buffers: 1,
        }
    }

    pub fn deliver(&mut self, context: &ID3D11DeviceContext, texture: &Texture, plan: PlanIdentity) -> Delivery {
        if texture.descriptor != self.descriptor {
            return Delivery::Failed(INVALID_ARGUMENT);
        }
        if let Some(pending) = self.pending {
            match copy_completed_rows(context, &self.staging, self.descriptor, &mut self.pixels) {
                Ok(true) => {}
                Ok(false) => return Delivery::Busy,
                Err(error) => return Delivery::Failed(error.code()),
            }
            let bytes = self.pixels.len();
            let frame = ReadbackFrame {
                plan: pending,
                pixels: std::mem::take(&mut self.pixels),
            };
            match self.frames.try_send(frame) {
                Ok(()) => self.pixels.resize(bytes, 0),
                Err(TrySendError::Full(frame)) => self.pixels = frame.pixels,
                Err(TrySendError::Disconnected(frame)) => {
                    self.pixels = frame.pixels;
                    return Delivery::Unavailable;
                }
            }
        }
        // SAFETY: both textures belong to this immediate context's device and have the same
        // layout. The previous map completed before the staging texture is reused.
        unsafe { context.CopyResource(&self.staging, &texture.texture) };
        self.pending = Some(plan);
        Delivery::Accepted
    }
}

pub(super) fn staging_texture(device: &ID3D11Device, descriptor: TextureDescriptor) -> Result<ID3D11Texture2D> {
    let layout = D3D11_TEXTURE2D_DESC {
        Width: descriptor.extent.width(),
        Height: descriptor.extent.height(),
        MipLevels: 1,
        ArraySize: 1,
        Format: crate::native::texture::format(descriptor.format),
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_STAGING,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        ..Default::default()
    };
    let mut staging = None;
    // SAFETY: complete single-sample staging layout and initialized output.
    unsafe { device.CreateTexture2D(&layout, None, Some(&mut staging))? };
    staging.ok_or_else(|| Error::from_hresult(FAILURE))
}

pub(super) fn copy_completed_rows(
    context: &ID3D11DeviceContext,
    staging: &ID3D11Texture2D,
    descriptor: TextureDescriptor,
    pixels: &mut [u8],
) -> Result<bool> {
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: only the renderer maps this staging texture. GPU contention never waits.
    match unsafe {
        context.Map(
            staging,
            0,
            D3D11_MAP_READ,
            D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
            Some(&mut mapped),
        )
    } {
        Err(error) if error.code() == DXGI_ERROR_WAS_STILL_DRAWING => return Ok(false),
        other => other?,
    }
    let row_bytes = descriptor.extent.width() as usize * 4;
    let pitch = mapped.RowPitch as usize;
    let valid = !mapped.pData.is_null()
        && pitch >= row_bytes
        && pixels.len() == row_bytes * descriptor.extent.height() as usize;
    if valid {
        for (index, row) in pixels.chunks_exact_mut(row_bytes).enumerate() {
            // SAFETY: each indicated row is inside the matched staging extent, excluding padding.
            let source = unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>().add(index * pitch), row_bytes) };
            row.copy_from_slice(source);
        }
    }
    // SAFETY: every successful map is paired with this unmap.
    unsafe { context.Unmap(staging, 0) };
    if valid {
        Ok(true)
    } else {
        Err(Error::from_hresult(INVALID_ARGUMENT))
    }
}
