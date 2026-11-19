//! D3D11 staging stays on the renderer. Only owned CPU rows reach the publisher.
use std::collections::VecDeque;

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
use crate::native::{OutputTextureAllocation, texture::Texture};
use crate::ndi::NdiVideoFormat;

mod worker;
use worker::Publisher;

const STAGING_BUFFERS: usize = 2;
const FAILURE: HRESULT = HRESULT(0x80004005_u32 as i32);
pub(in crate::native) const PAYLOAD_COPIES: u64 = (STAGING_BUFFERS + worker::CPU_BUFFERS) as u64;

pub(super) struct NdiOutput {
    format: NdiVideoFormat,
    staging: [ID3D11Texture2D; STAGING_BUFFERS],
    pending: VecDeque<(usize, i64)>,
    next: usize,
    publisher: Publisher,
}

impl NdiOutput {
    pub fn new(device: &ID3D11Device, name: String, format: NdiVideoFormat) -> Result<Self> {
        let descriptor = format.descriptor();
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
        let mut staging = [const { None }; STAGING_BUFFERS];
        for slot in &mut staging {
            // SAFETY: complete single-sample staging layout and initialized output.
            unsafe { device.CreateTexture2D(&layout, None, Some(slot))? };
            if slot.is_none() {
                return Err(Error::from_hresult(FAILURE));
            }
        }
        let publisher = Publisher::start(name, format)?;
        Ok(Self {
            format,
            staging: staging.map(Option::unwrap),
            pending: VecDeque::with_capacity(STAGING_BUFFERS),
            next: 0,
            publisher,
        })
    }

    pub fn allocation(&self) -> OutputTextureAllocation {
        OutputTextureAllocation::Ndi {
            descriptor: self.format.descriptor(),
            staging_buffers: STAGING_BUFFERS as u32,
        }
    }

    pub fn stop(&self) {
        self.publisher.stop();
    }

    pub fn deliver(&mut self, context: &ID3D11DeviceContext, texture: &Texture, timecode: i64) -> Delivery {
        if !self.publisher.is_open() {
            return Delivery::Failed(FAILURE);
        }
        if !self.publisher.ready() {
            return Delivery::Unavailable;
        }
        if texture.descriptor != self.format.descriptor() {
            return Delivery::Failed(INVALID_ARGUMENT);
        }
        if let Some(&(slot, timestamp)) = self.pending.front()
            && let Some(mut pixels) = self.publisher.take_buffer()
        {
            match copy_completed_rows(context, &self.staging[slot], self.format, &mut pixels) {
                Ok(true) => {
                    if self.publisher.send(pixels, timestamp) {
                        self.pending.pop_front();
                    }
                }
                Ok(false) => self.publisher.recycle(pixels),
                Err(error) => return Delivery::Failed(error.code()),
            }
        }
        if self.pending.len() == STAGING_BUFFERS {
            return Delivery::Busy;
        }
        // SAFETY: matching same-device layouts, with no mapped or outstanding use of this
        // ring slot. Endpoint retirement retains all staging and conversion storage by token.
        unsafe { context.CopyResource(&self.staging[self.next], &texture.texture) };
        self.pending.push_back((self.next, timecode));
        self.next = (self.next + 1) % STAGING_BUFFERS;
        Delivery::Accepted
    }
}

fn copy_completed_rows(
    context: &ID3D11DeviceContext,
    staging: &ID3D11Texture2D,
    format: NdiVideoFormat,
    pixels: &mut [u8],
) -> Result<bool> {
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: oldest pending copy on the owned immediate context. GPU contention never waits.
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
    let row_bytes = format.descriptor().extent.width() as usize * 4;
    let pitch = mapped.RowPitch as usize;
    let valid = !mapped.pData.is_null() && pitch >= row_bytes && pixels.len() == format.bytes();
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
