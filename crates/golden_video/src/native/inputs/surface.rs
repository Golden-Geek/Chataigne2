use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::sync::Arc;

use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::{Common::DXGI_SAMPLE_DESC, *};
use windows::core::{Error, HRESULT, Interface, PCWSTR, Result};

use super::super::device::Device;
use super::super::texture::{Texture, format};
use crate::TextureDescriptor;

/// Only immutable identity and a retained NT handle cross workers, never COM resources.
pub(super) struct Surface {
    pub handle: OwnedHandle,
    pub index: usize,
    pub descriptor: TextureDescriptor,
}

pub(super) struct Slot {
    pub surface: Arc<Surface>,
    pub texture: Texture,
    pub mutex: IDXGIKeyedMutex,
}

impl Slot {
    pub fn new(device: &Device, descriptor: TextureDescriptor, index: usize) -> Result<Self> {
        let mut texture = None;
        // SAFETY: fixed admitted storage, one level/sample, explicit NT/keyed-mutex sharing.
        let (texture, handle) = unsafe {
            device.device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: descriptor.extent.width(),
                    Height: descriptor.extent.height(),
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: format(descriptor.format),
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
                    MiscFlags: (D3D11_RESOURCE_MISC_SHARED_NTHANDLE | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX).0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut texture),
            )?;
            let texture = texture.ok_or_else(|| Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?;
            let handle = texture.cast::<IDXGIResource1>()?.CreateSharedHandle(
                None,
                (DXGI_SHARED_RESOURCE_READ | DXGI_SHARED_RESOURCE_WRITE).0,
                PCWSTR::null(),
            )?;
            (texture, OwnedHandle::from_raw_handle(handle.0))
        };
        Ok(Self {
            mutex: texture.cast()?,
            texture: Texture::from_native(&device.device, texture, descriptor)?,
            surface: Arc::new(Surface {
                handle,
                index,
                descriptor,
            }),
        })
    }

    pub fn acquire_free(&self) -> Result<bool> {
        if Arc::strong_count(&self.surface) != 1 {
            return Ok(false);
        }
        Ok(acquire(&self.mutex, 0)? || acquire(&self.mutex, 1)?)
    }
}

pub(super) fn acquire(mutex: &IDXGIKeyedMutex, key: u64) -> Result<bool> {
    // SAFETY: zero timeout on a retained interface. Inspect exact HRESULT because positive
    // WAIT_TIMEOUT/WAIT_ABANDONED are erased by windows-rs Result<()>.
    let result = unsafe { (Interface::vtable(mutex).AcquireSync)(Interface::as_raw(mutex), key, 0) };
    acquired(result)
}

pub(super) fn acquired(result: HRESULT) -> Result<bool> {
    match result.0 {
        0 => Ok(true),
        258 => Ok(false),
        _ => Err(Error::from_hresult(result)),
    }
}
