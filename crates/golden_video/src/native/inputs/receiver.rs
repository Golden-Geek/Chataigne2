use std::ffi::CString;
use std::ptr::NonNull;

use spout2_sys as sdk;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Device, ID3D11Texture2D};
use windows::core::{Error, HRESULT, Interface, Result};

use super::super::texture::{Texture, format};
use crate::{Extent2D, TextureDescriptor};

pub(crate) enum Received {
    Absent,
    Waiting,
    Unchanged,
    Mismatch,
    Frame,
}

/// The receive-into overload never creates the SDK's unbounded private receiving copy.
/// All endpoint operations run on a dedicated source device/context.
pub(crate) struct Receiver {
    raw: NonNull<sdk::spout_dx_t>,
    device: ID3D11Device,
    name: CString,
    descriptor: TextureDescriptor,
    handle: usize,
    alias: Option<ID3D11Texture2D>,
    last_frame: Option<i32>,
    clear_capture: bool,
    actual_descriptor: Option<TextureDescriptor>,
}

impl Receiver {
    pub fn new(device: &ID3D11Device, name: &str, descriptor: TextureDescriptor) -> Result<Self> {
        let name = CString::new(name).map_err(|_| invalid())?;
        // SAFETY: uniquely owned SDK instance, retained device AddRef outlives its borrowed pointer.
        let raw = NonNull::new(unsafe { sdk::spout_dx_create() }).ok_or_else(invalid)?;
        let receiver = Self {
            raw,
            device: device.clone(),
            name,
            descriptor,
            handle: 0,
            alias: None,
            last_frame: None,
            clear_capture: true,
            actual_descriptor: None,
        };
        unsafe {
            if sdk::spout_dx_open_directx11(raw.as_ptr(), receiver.device.as_raw()) == 0
                || sdk::spout_dx_get_device(raw.as_ptr()) != receiver.device.as_raw()
            {
                return Err(invalid());
            }
            sdk::spout_dx_set_receiver_name(raw.as_ptr(), receiver.name.as_ptr());
        }
        Ok(receiver)
    }

    /// The caller fences previous source commands before invoking this method again or
    /// disconnecting, so SDK replacement cannot abandon an in-flight copy's source alias.
    pub fn preflight(&mut self) -> Result<Option<Received>> {
        let (mut width, mut height, mut storage) = (0, 0, 0);
        let mut handle = std::ptr::null_mut();
        // SAFETY: fixed selector and initialized metadata outputs; no image is allocated/copied.
        let found = unsafe {
            sdk::spout_dx_get_sender_info(
                self.raw.as_ptr(),
                self.name.as_ptr(),
                &mut width,
                &mut height,
                &mut handle,
                &mut storage,
            ) != 0
        };
        if !found || handle.is_null() {
            self.disconnect();
            return Ok(Some(Received::Absent));
        }
        if width == 0
            || height == 0
            || width > self.descriptor.extent.width()
            || height > self.descriptor.extent.height()
            || storage != format(self.descriptor.format).0 as u32
        {
            self.disconnect();
            return Ok(Some(Received::Mismatch));
        }
        let descriptor = TextureDescriptor {
            extent: Extent2D::new(width, height).map_err(|_| invalid())?,
            ..self.descriptor
        };
        if handle as usize != self.handle || self.actual_descriptor != Some(descriptor) {
            self.disconnect();
            if !self.validate_alias(handle, descriptor)? {
                return Ok(Some(Received::Mismatch));
            }
        }
        Ok(None)
    }

    pub fn receive(&mut self, capture: &Texture) -> Result<Received> {
        let expected = self.actual_descriptor.ok_or_else(invalid)?;
        if self.clear_capture {
            // SAFETY: first accepted session starts from deterministic zero storage. SDK may
            // report acceptance without copying after a mutex timeout; never expose garbage.
            unsafe {
                self.device
                    .GetImmediateContext()?
                    .ClearRenderTargetView(&capture.rtv, &[0.0; 4]);
            }
            self.clear_capture = false;
        }
        let mut texture = capture.texture.as_raw();
        // SAFETY: a fixed matching receiver texture on the exact caller device. Unlike the
        // no-argument overload this public SDK API does not allocate a private receiving image.
        let connected = unsafe { sdk::spout_dx_receive_texture_into(self.raw.as_ptr(), &mut texture) != 0 };
        let updated = unsafe { sdk::spout_dx_is_updated(self.raw.as_ptr()) != 0 };
        if texture != capture.texture.as_raw()
            || unsafe { sdk::spout_dx_get_device(self.raw.as_ptr()) } != self.device.as_raw()
        {
            return Err(invalid());
        }
        let actual = unsafe { sdk::spout_dx_get_sender_handle(self.raw.as_ptr()) };
        if !actual.is_null() && actual as usize != self.handle {
            // A metadata-to-receive race sets IsUpdated before CopyResource in the pinned SDK.
            // Validate the new foreign alias and skip this call; fixed owned pools never resize.
            self.alias = None;
            let width = unsafe { sdk::spout_dx_get_sender_width(self.raw.as_ptr()) };
            let height = unsafe { sdk::spout_dx_get_sender_height(self.raw.as_ptr()) };
            let storage = unsafe { sdk::spout_dx_get_sender_format(self.raw.as_ptr()) };
            if width == 0
                || height == 0
                || width > self.descriptor.extent.width()
                || height > self.descriptor.extent.height()
                || storage != format(self.descriptor.format).0 as u32
            {
                return Ok(Received::Mismatch);
            }
            let descriptor = TextureDescriptor {
                extent: Extent2D::new(width, height).map_err(|_| invalid())?,
                ..self.descriptor
            };
            if !self.validate_alias(actual, descriptor)? {
                return Ok(Received::Mismatch);
            }
            self.last_frame = None;
            self.clear_capture = true;
            return Ok(Received::Waiting);
        }
        if !connected || updated {
            return Ok(Received::Waiting);
        }
        let (actual_width, actual_height, actual_format) = unsafe {
            (
                sdk::spout_dx_get_sender_width(self.raw.as_ptr()),
                sdk::spout_dx_get_sender_height(self.raw.as_ptr()),
                sdk::spout_dx_get_sender_format(self.raw.as_ptr()),
            )
        };
        if actual_width != expected.extent.width()
            || actual_height != expected.extent.height()
            || actual_format != format(self.descriptor.format).0 as u32
        {
            return Ok(Received::Mismatch);
        }
        let mut actual_name = [0_i8; 256];
        let length = unsafe { sdk::spout_dx_get_sender_name(self.raw.as_ptr(), actual_name.as_mut_ptr(), 256) };
        if length != self.name.as_bytes().len() as i32
            || !actual_name
                .iter()
                .zip(self.name.as_bytes())
                .all(|(a, b)| *a as u8 == *b)
        {
            return Err(invalid());
        }
        let new_frame = unsafe { sdk::spout_dx_is_frame_new(self.raw.as_ptr()) != 0 };
        let frame = unsafe { sdk::spout_dx_get_sender_frame(self.raw.as_ptr()) };
        if !new_frame || (frame != 0 && self.last_frame == Some(frame)) {
            return Ok(Received::Unchanged);
        }
        self.last_frame = Some(frame);
        Ok(Received::Frame)
    }

    fn validate_alias(&mut self, handle: *mut std::ffi::c_void, descriptor: TextureDescriptor) -> Result<bool> {
        // SAFETY: legacy Spout share handle, opened only on the explicitly selected adapter.
        // This is an alias of sender-owned storage, not an allocation of receiving storage.
        let mut alias = None;
        unsafe {
            self.device
                .OpenSharedResource::<ID3D11Texture2D>(HANDLE(handle), &mut alias)?;
        }
        let alias = alias.ok_or_else(invalid)?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            alias.GetDesc(&mut desc);
        }
        if desc.Width != descriptor.extent.width()
            || desc.Height != descriptor.extent.height()
            || desc.Format != format(self.descriptor.format)
            || desc.MipLevels != 1
            || desc.ArraySize != 1
            || desc.SampleDesc.Count != 1
            || desc.SampleDesc.Quality != 0
        {
            return Ok(false);
        }
        self.handle = handle as usize;
        self.actual_descriptor = Some(descriptor);
        self.alias = Some(alias);
        Ok(true)
    }

    pub fn disconnect(&mut self) {
        if self.handle != 0 || self.alias.is_some() {
            // SAFETY: caller has drained previous source-device uses before SDK/alias replacement.
            unsafe {
                sdk::spout_dx_release_receiver(self.raw.as_ptr());
                sdk::spout_dx_set_receiver_name(self.raw.as_ptr(), self.name.as_ptr());
            }
        }
        self.alias = None;
        self.handle = 0;
        self.last_frame = None;
        self.clear_capture = true;
        self.actual_descriptor = None;
    }

    pub fn descriptor(&self) -> Option<TextureDescriptor> {
        self.actual_descriptor
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        // SAFETY: source Session drains its own context before destroying this owned SDK instance.
        unsafe {
            sdk::spout_dx_destroy(self.raw.as_ptr());
        }
    }
}

fn invalid() -> Error {
    Error::from_hresult(HRESULT(0x8007_0057_u32 as i32))
}
