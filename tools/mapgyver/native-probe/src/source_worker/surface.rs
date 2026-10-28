//! Only the NT handle and immutable dimensions cross threads; COM stays device-local.

use crate::{ProbeResult, d3d::Gpu, texture::Texture};
use mapgyver_native_probe::allocation::Owner;
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::sync::Arc;
use windows::Win32::Graphics::{
    Direct3D11::*,
    Dxgi::{Common::*, *},
};
use windows::core::{Interface, PCWSTR};

pub struct Surface {
    pub(super) handle: OwnedHandle,
    pub extent: [u32; 2],
}

/// The batch belongs to a publication, not the reusable allocation's identity.
#[derive(Clone)]
pub(super) struct Publication {
    pub surface: Arc<Surface>,
    pub batch: u64,
}

pub struct Slot {
    pub surface: Arc<Surface>,
    pub texture: Texture,
    pub mutex: IDXGIKeyedMutex,
}

impl Slot {
    pub fn new(gpu: &Gpu, extent: [u32; 2]) -> ProbeResult<Self> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: extent[0],
            Height: extent[1],
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED_NTHANDLE | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX).0 as u32,
            ..Default::default()
        };
        let mut texture = None;
        // SAFETY: one-level BGRA resource with the NT-handle/keyed-mutex sharing contract.
        let (texture, handle) = unsafe {
            gpu.device.CreateTexture2D(&desc, None, Some(&mut texture))?;
            let texture = texture.ok_or("missing source bridge texture")?;
            let handle = texture.cast::<IDXGIResource1>()?.CreateSharedHandle(
                None,
                (DXGI_SHARED_RESOURCE_READ | DXGI_SHARED_RESOURCE_WRITE).0,
                PCWSTR::null(),
            )?;
            (texture, OwnedHandle::from_raw_handle(handle.0))
        };
        Ok(Self {
            mutex: texture.cast()?,
            texture: Texture::new(texture, &gpu.allocations, Owner::Application)?,
            surface: Arc::new(Surface { handle, extent }),
        })
    }

    pub fn acquire_free(&self) -> ProbeResult<bool> {
        if Arc::strong_count(&self.surface) != 1 {
            return Ok(false);
        }
        // No publication/reader retains this slot. A discarded publication may leave key 1;
        // a completed reader leaves key 0. Both probes are explicitly nonblocking.
        Ok(acquire(&self.mutex, 0)? || acquire(&self.mutex, 1)?)
    }
}

pub fn acquire(mutex: &IDXGIKeyedMutex, key: u64) -> ProbeResult<bool> {
    // SAFETY: zero timeout on a live local interface. windows-rs Result<()> erases positive
    // WAIT_TIMEOUT/WAIT_ABANDONED HRESULTs, so inspect the exact ABI return instead.
    let result = unsafe { (Interface::vtable(mutex).AcquireSync)(Interface::as_raw(mutex), key, 0) };
    acquired(result.0)
}

fn acquired(result: i32) -> ProbeResult<bool> {
    match result {
        0 => Ok(true),
        258 => Ok(false), // WAIT_TIMEOUT
        128 => Err("source bridge keyed mutex abandoned; resource must be recreated".into()),
        code => Err(windows::core::Error::from_hresult(windows::core::HRESULT(code)).into()),
    }
}

#[cfg(test)]
mod tests;
