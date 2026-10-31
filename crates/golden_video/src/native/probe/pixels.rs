//! Explicit finite qualification readback; production readiness never maps or exports pixels.
use super::*;
use windows::Win32::Graphics::{
    Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_MAP_FLAG_DO_NOT_WAIT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
        D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11Texture2D,
    },
    Dxgi::DXGI_ERROR_WAS_STILL_DRAWING,
};

#[cfg(test)]
mod tests;

/// Raw received RGBA8 texels, with CPU storage charged to the probe's caller-owned payload pool.
/// Color/alpha interpretation follows the declared sender contract; no color conversion is applied.
pub struct ProbePixels {
    extent: Extent2D,
    pixels: Vec<[u8; 4]>,
    _reservation: PayloadReservation,
}

impl ProbePixels {
    /// Observed texel extent; Spout metadata does not independently tag color or alpha semantics.
    pub fn extent(&self) -> Extent2D {
        self.extent
    }
    /// Tightly packed row-major RGBA8 bytes, with BGRA native storage reordered to RGBA.
    pub fn pixels(&self) -> &[[u8; 4]] {
        &self.pixels
    }
}

pub(super) struct Readback {
    texture: ID3D11Texture2D,
    _reservation: PayloadReservation,
}

impl SpoutProbe {
    /// Observe this probe's single received image, returning None while GPU/SDK work is pending.
    /// Qualification only: call on an owned IO/test worker, never the actor or UI thread.
    /// Fresh observations require a fresh probe. Tight receiving/staging/CPU texel charges share
    /// the caller's payload cap (excluding driver padding). Staging retains actual GPU ownership.
    pub fn capture_pixels(&mut self) -> Result<Option<ProbePixels>> {
        if !self.poll(true)?.first_frame {
            return Ok(None);
        }
        self.session.as_mut().unwrap().capture_pixels(&self.budget)
    }
}

impl Session {
    fn capture_pixels(&mut self, budget: &PayloadPool) -> Result<Option<ProbePixels>> {
        let descriptor = self.texture.descriptor;
        let count = u64::from(descriptor.extent.width()) * u64::from(descriptor.extent.height());
        let bytes = usize::try_from(count.checked_mul(4).ok_or_else(invalid)?).map_err(|_| invalid())?;
        if self.readback.is_none() {
            let submitted = self.submitted.checked_add(1).ok_or_else(invalid)?;
            let reservation = budget.try_reserve(bytes).map_err(|_| invalid())?;
            let mut layout = D3D11_TEXTURE2D_DESC::default();
            unsafe { self.texture.texture.GetDesc(&mut layout) };
            layout.Usage = D3D11_USAGE_STAGING;
            layout.BindFlags = 0;
            layout.MiscFlags = 0;
            layout.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            let mut texture = None;
            // SAFETY: bounded same-device single-image staging layout and initialized output.
            unsafe { self.device.device.CreateTexture2D(&layout, None, Some(&mut texture))? };
            self.readback = Some(Readback {
                texture: texture.ok_or_else(invalid)?,
                _reservation: reservation,
            });
            self.submitted = submitted;
            self.pending = true;
            let _completion = self.completion.begin(
                &self.device,
                CompletionToken {
                    device: self.generation,
                    serial: self.submitted,
                },
            );
            // SAFETY: the session retains both textures and the completion guard through the copy.
            unsafe {
                self.device
                    .context
                    .CopyResource(&self.readback.as_ref().unwrap().texture, &self.texture.texture)
            };
            return Ok(None);
        }
        if !self.drained()? || self.device.removed() {
            return Ok(None);
        }
        let reservation = budget.try_reserve(bytes).map_err(|_| invalid())?;
        let staging = &self.readback.as_ref().unwrap().texture;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: completed caller-owned staging; DONOTWAIT keeps a driver's late Map readiness explicit.
        match unsafe {
            self.device.context.Map(
                staging,
                0,
                D3D11_MAP_READ,
                D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                Some(&mut mapped),
            )
        } {
            Err(error) if error.code() == DXGI_ERROR_WAS_STILL_DRAWING => return Ok(None),
            other => other?,
        }
        let result = (|| {
            let width = descriptor.extent.width() as usize;
            let height = descriptor.extent.height() as usize;
            if mapped.pData.is_null() || (mapped.RowPitch as usize) < width * 4 {
                return Err(invalid());
            }
            let mut pixels = Vec::new();
            pixels.try_reserve_exact(bytes / 4).map_err(|_| invalid())?;
            for y in 0..height {
                // SAFETY: matching mapped image rows; copy only admitted texels, excluding row padding.
                let row = unsafe {
                    std::slice::from_raw_parts(mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize), width * 4)
                };
                for pixel in row.chunks_exact(4) {
                    pixels.push(match descriptor.format {
                        PixelFormat::Bgra8Unorm => [pixel[2], pixel[1], pixel[0], pixel[3]],
                        PixelFormat::Rgba8Unorm => [pixel[0], pixel[1], pixel[2], pixel[3]],
                    });
                }
            }
            Ok(Some(ProbePixels {
                extent: descriptor.extent,
                pixels,
                _reservation: reservation,
            }))
        })();
        // SAFETY: every successful map is unmapped, including allocation/layout rejection.
        unsafe { self.device.context.Unmap(staging, 0) };
        result
    }
}
