//! D3D11 staging stays on the renderer. Only owned CPU rows reach the publisher.
use std::{collections::VecDeque, time::Instant};

use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D};
use windows::core::{HRESULT, Result};

use super::readback::{copy_completed_rows, staging_texture};
use super::{Delivery, INVALID_ARGUMENT};
use crate::native::{OutputTextureAllocation, texture::Texture};
use crate::ndi::{NdiOutputAudio, NdiVideoFormat, output_timecode};

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
    pub fn new(
        device: &ID3D11Device,
        name: String,
        format: NdiVideoFormat,
        audio: Option<NdiOutputAudio>,
    ) -> Result<Self> {
        let descriptor = format.descriptor();
        let staging = [
            staging_texture(device, descriptor)?,
            staging_texture(device, descriptor)?,
        ];
        let publisher = Publisher::start(name, format, audio)?;
        Ok(Self {
            format,
            staging,
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

    pub fn deliver(&mut self, context: &ID3D11DeviceContext, texture: &Texture) -> Delivery {
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
            match copy_completed_rows(context, &self.staging[slot], self.format.descriptor(), &mut pixels) {
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
        self.pending.push_back((self.next, output_timecode(Instant::now())));
        self.next = (self.next + 1) % STAGING_BUFFERS;
        Delivery::Accepted
    }
}
