use std::ffi::CString;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

use spout2_sys as sdk;
use windows::Win32::Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Device};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM};
use windows::core::{Error, HRESULT, Interface, Result};

use crate::{PixelFormat, TextureDescriptor};

use super::super::texture::Texture;
use super::Delivery;

const INVALID_ARGUMENT: HRESULT = HRESULT(0x8007_0057_u32 as i32);
const FAILURE: HRESULT = HRESULT(0x8000_4005_u32 as i32);
const NAME_COLLISION: HRESULT = HRESULT(0x8007_00b7_u32 as i32);

/// Worker-local sender; its one lazily allocated SDK shared texture is budgeted by the pool.
pub(super) struct SpoutOutput {
    raw: NonNull<sdk::spout_dx_t>,
    // The SDK borrows this pointer without AddRef. Drop destroys the SDK before fields drop.
    device: ID3D11Device,
    name: CString,
    descriptor: TextureDescriptor,
    failure: Option<HRESULT>,
    _local: PhantomData<Rc<()>>,
}

impl SpoutOutput {
    pub(super) fn new(device: &ID3D11Device, name: String, descriptor: TextureDescriptor) -> Result<Self> {
        if name.is_empty() || name.len() > 255 {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Spout sender name must contain 1 to 255 bytes",
            ));
        }
        let name =
            CString::new(name).map_err(|_| Error::new(INVALID_ARGUMENT, "Spout sender name contains a NUL byte"))?;
        // SAFETY: the caller's retained device is live on the worker that owns this output.
        unsafe { device.GetDeviceRemovedReason()? };
        // SAFETY: the pinned SDK constructor returns a uniquely owned instance or null.
        let raw = NonNull::new(unsafe { sdk::spout_dx_create() })
            .ok_or_else(|| Error::new(HRESULT(0x8007_000e_u32 as i32), "Spout SDK allocation failed"))?;
        let output = Self {
            raw,
            device: device.clone(),
            name,
            descriptor,
            failure: None,
            _local: PhantomData,
        };
        // SAFETY: output owns the instance and an AddRef of the exact borrowed device.
        // Any early return drops output, destroying the SDK before releasing that AddRef.
        unsafe {
            if sdk::spout_dx_open_directx11(output.raw.as_ptr(), output.device.as_raw()) == 0
                || sdk::spout_dx_get_device(output.raw.as_ptr()) != output.device.as_raw()
                || sdk::spout_dx_get_context(output.raw.as_ptr()).is_null()
            {
                return Err(Error::new(
                    FAILURE,
                    "Spout did not retain the caller-selected device/context",
                ));
            }
            if sdk::spout_dx_set_sender_name(output.raw.as_ptr(), output.name.as_ptr()) == 0 {
                return Err(Error::new(FAILURE, "Spout rejected the sender name"));
            }
            sdk::spout_dx_set_sender_format(output.raw.as_ptr(), format(descriptor.format).0 as u32);
        }
        if !output.has_exact_name() {
            return Err(Error::new(NAME_COLLISION, "Spout changed the requested sender name"));
        }
        Ok(output)
    }

    /// Accepted means SDK submission, not receiver delivery. SDK 2.007.017 may wait up to
    /// 67 ms for texture access and still return success after a timeout without copying.
    /// Its published shim does not expose that timeout, so it cannot be reported as Busy.
    pub(super) fn deliver(&mut self, texture: &Texture) -> Delivery {
        if let Some(failure) = self.failure {
            return Delivery::Failed(failure);
        }
        match self.try_deliver(texture) {
            Ok(()) => Delivery::Accepted,
            Err(error) => Delivery::Failed(error.code()),
        }
    }

    fn try_deliver(&mut self, texture: &Texture) -> Result<()> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: this worker retains both the texture and caller-selected device.
        unsafe {
            self.device.GetDeviceRemovedReason()?;
            texture.texture.GetDesc(&mut desc);
            if texture.descriptor != self.descriptor
                || texture.texture.GetDevice()?.as_raw() != self.device.as_raw()
                || sdk::spout_dx_get_device(self.raw.as_ptr()) != self.device.as_raw()
                || desc.Width != self.descriptor.extent.width()
                || desc.Height != self.descriptor.extent.height()
                || desc.Format != format(self.descriptor.format)
                || desc.MipLevels != 1
                || desc.ArraySize != 1
                || desc.SampleDesc.Count != 1
                || desc.SampleDesc.Quality != 0
            {
                return Err(Error::new(
                    INVALID_ARGUMENT,
                    "Spout texture does not match its admitted descriptor/device",
                ));
            }
        }
        if !self.has_exact_name() {
            return self.reject_renamed_sender();
        }
        // SAFETY: exact device, storage and subresource layout were checked above. The pool
        // reserved this sender's single shared texture before its lazy SDK allocation here.
        let accepted = unsafe { sdk::spout_dx_send_texture(self.raw.as_ptr(), texture.texture.as_raw()) != 0 };
        // Registration during SendTexture can rename a sender even after SetSenderName passed.
        if !self.has_exact_name() {
            return self.reject_renamed_sender();
        }
        // SAFETY: this is the retained caller-selected device; prefer its actual failure code.
        unsafe { self.device.GetDeviceRemovedReason()? };
        if !accepted {
            return Err(Error::new(FAILURE, "Spout SendTexture failed"));
        }
        // SAFETY: the successful send must have initialized this immutable sender layout.
        let initialized = unsafe {
            sdk::spout_dx_is_initialized(self.raw.as_ptr()) != 0
                && sdk::spout_dx_get_width(self.raw.as_ptr()) == self.descriptor.extent.width()
                && sdk::spout_dx_get_height(self.raw.as_ptr()) == self.descriptor.extent.height()
        };
        if !initialized {
            return Err(Error::new(FAILURE, "Spout sender did not retain its admitted extent"));
        }
        Ok(())
    }

    fn has_exact_name(&self) -> bool {
        let mut actual = [0_i8; 256];
        // SAFETY: the shim bounds and terminates its copy into this initialized output buffer.
        let length = unsafe { sdk::spout_dx_get_name(self.raw.as_ptr(), actual.as_mut_ptr(), actual.len() as i32) };
        let expected = self.name.as_bytes();
        length == expected.len() as i32
            && actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| *actual as u8 == *expected)
    }

    fn reject_renamed_sender(&mut self) -> Result<()> {
        // Do not release SDK storage here: SendTexture may already have queued a copy. The
        // pool latches this failure and destroys the instance only after its last-use fence.
        self.failure = Some(NAME_COLLISION);
        Err(Error::new(NAME_COLLISION, "Spout changed the requested sender name"))
    }
}

impl Drop for SpoutOutput {
    fn drop(&mut self) {
        // SAFETY: the uniquely owned SDK instance is destroyed once, on its owning worker,
        // while device still retains the borrowed COM pointer. The SDK releases its sender
        // texture and immediate context before Rust drops our device AddRef.
        unsafe { sdk::spout_dx_destroy(self.raw.as_ptr()) };
    }
}

fn format(format: PixelFormat) -> DXGI_FORMAT {
    match format {
        PixelFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        PixelFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
    }
}
