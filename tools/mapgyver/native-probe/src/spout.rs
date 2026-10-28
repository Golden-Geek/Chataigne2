use std::ffi::{CStr, CString};
use std::ptr::NonNull;

use spout2_sys as sdk;
use windows::Win32::Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D};
use windows::core::Interface;

use crate::ProbeResult;
use crate::d3d::Gpu;
use crate::texture::Texture;
use mapgyver_native_probe::allocation::{Lease, Owner, Work, bgra_bytes};

pub fn sdk_version() -> ProbeResult<String> {
    let mut version = [0_i8; 64];
    // SAFETY: SDK copies the linked version into a writable, bounded NUL-terminated buffer.
    let length = unsafe { sdk::spout_get_sdk_version(version.as_mut_ptr(), version.len() as i32) };
    if length <= 0 {
        return Err("linked SDK did not report its version".into());
    }
    // SAFETY: the successful copy includes a terminator inside the initialized buffer.
    Ok(unsafe { CStr::from_ptr(version.as_ptr()) }.to_str()?.to_owned())
}

/// The SDK borrows the device without AddRef. The Rust borrow fences teardown.
/// NonNull also keeps the SDK instance off Send/Sync; construct/use/drop on one worker.
pub struct Endpoint<'gpu> {
    raw: NonNull<sdk::spout_dx_t>,
    gpu: &'gpu Gpu,
    received: Option<Texture>,
    sender_allocation: Option<Lease>,
    sender_extent: [u32; 2],
}

impl<'gpu> Endpoint<'gpu> {
    pub fn new(gpu: &'gpu Gpu) -> ProbeResult<Self> {
        // SAFETY: the published SDK constructor returns a uniquely owned instance or null.
        let raw = NonNull::new(unsafe { sdk::spout_dx_create() }).ok_or("Spout allocation failed")?;
        let endpoint = Self {
            raw,
            gpu,
            received: None,
            sender_allocation: None,
            sender_extent: [0; 2],
        };
        // SAFETY: live SDK instance, device is retained by the lifetime borrow above.
        let ready = unsafe {
            sdk::spout_dx_open_directx11(endpoint.raw.as_ptr(), gpu.device.as_raw()) != 0
                && sdk::spout_dx_get_device(endpoint.raw.as_ptr()) == gpu.device.as_raw()
        };
        if !ready {
            return Err("Spout did not retain the explicitly selected device".into());
        }
        Ok(endpoint)
    }

    pub fn sender(gpu: &'gpu Gpu, name: &str) -> ProbeResult<Self> {
        let endpoint = Self::new(gpu)?;
        let name = CString::new(name)?;
        // SAFETY: SDK copies this NUL-terminated name; raw is live and uniquely owned.
        unsafe {
            if sdk::spout_dx_set_sender_name(endpoint.raw.as_ptr(), name.as_ptr()) == 0 {
                return Err("Spout rejected sender name".into());
            }
            sdk::spout_dx_set_sender_format(endpoint.raw.as_ptr(), 87);
        }
        Ok(endpoint)
    }

    pub fn receiver(gpu: &'gpu Gpu, name: &str) -> ProbeResult<Self> {
        let endpoint = Self::new(gpu)?;
        let name = CString::new(name)?;
        // SAFETY: SDK copies the explicit sender selector; never follow active-sender policy.
        unsafe { sdk::spout_dx_set_receiver_name(endpoint.raw.as_ptr(), name.as_ptr()) };
        Ok(endpoint)
    }

    pub fn send(&mut self, texture: &ID3D11Texture2D) -> ProbeResult<()> {
        self.gpu.allocations.work(Work::SdkSend);
        // SAFETY: all harness source textures are created on the borrowed GPU's device.
        if unsafe { sdk::spout_dx_send_texture(self.raw.as_ptr(), texture.as_raw()) } == 0 {
            return Err("Spout SendTexture failed".into());
        }
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: the successful SDK call created/updated its single shared sender texture.
        unsafe {
            texture.GetDesc(&mut desc);
        }
        let extent = [desc.Width, desc.Height];
        if extent != self.sender_extent {
            self.sender_allocation = Some(self.gpu.allocations.allocate(
                Owner::SdkSenderShared,
                bgra_bytes(desc.Width, desc.Height, 1)?,
                1,
            )?);
            self.sender_extent = extent;
        }
        Ok(())
    }

    pub fn receive(&mut self) -> ProbeResult<Option<ReceivedFrame>> {
        self.gpu.allocations.work(Work::SdkReceive);
        // SAFETY: each SDK call uses this worker's uniquely owned instance. IsUpdated must be
        // consumed on every receive or the SDK stops refreshing after a size/format change.
        unsafe {
            let connected = sdk::spout_dx_receive_texture(self.raw.as_ptr()) != 0;
            sdk::spout_dx_is_updated(self.raw.as_ptr());
            if !connected {
                self.received = None;
                return Ok(None);
            }
            let new_frame = sdk::spout_dx_is_frame_new(self.raw.as_ptr()) != 0;
            self.gpu.allocations.work(Work::SdkTextureQuery);
            let raw = sdk::spout_dx_get_sender_texture(self.raw.as_ptr());
            let texture = ID3D11Texture2D::from_raw_borrowed(&raw).ok_or("connected Spout receiver has no texture")?;
            if texture.GetDevice()?.as_raw() != self.gpu.device.as_raw() {
                return Err("Spout silently changed the receiver device".into());
            }
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            if self
                .received
                .as_ref()
                .is_none_or(|current| current.as_raw() != texture.as_raw())
            {
                self.received = Some(Texture::new(
                    texture.clone(),
                    &self.gpu.allocations,
                    Owner::SdkReceiverCopy,
                )?);
            }
            // The SDK can allocate its private texture during a metadata update without
            // publishing a new frame. Account that ownership independently of frame delivery.
            if !new_frame {
                return Ok(None);
            }
            self.gpu.allocations.work(Work::SdkObservation);
            // AddRef before the next SDK call; the SDK's borrowed pointer is not retained.
            Ok(Some(ReceivedFrame {
                texture: self.received.as_ref().ok_or("missing received allocation")?.clone(),
                width: desc.Width,
                height: desc.Height,
            }))
        }
    }

    pub fn name(&self) -> String {
        let mut name = [0_i8; 256];
        // SAFETY: correctly sized output buffer, SDK always NUL-terminates successful names.
        unsafe {
            sdk::spout_dx_get_name(self.raw.as_ptr(), name.as_mut_ptr(), name.len() as i32);
            CStr::from_ptr(name.as_ptr()).to_string_lossy().into_owned()
        }
    }

    pub fn connected(&self) -> bool {
        // SAFETY: uniquely owned SDK instance, queried on its owning worker.
        unsafe { sdk::spout_dx_is_connected(self.raw.as_ptr()) != 0 }
    }

    pub fn sender_exists(&self, name: &str) -> ProbeResult<bool> {
        let name = CString::new(name)?;
        let (mut width, mut height, mut format) = (0, 0, 0);
        let mut handle = std::ptr::null_mut();
        // SAFETY: all metadata outputs have their ABI-declared type and lifetime.
        Ok(unsafe {
            sdk::spout_dx_get_sender_info(
                self.raw.as_ptr(),
                name.as_ptr(),
                &mut width,
                &mut height,
                &mut handle,
                &mut format,
            ) != 0
        })
    }
}

pub struct ReceivedFrame {
    pub texture: Texture,
    pub width: u32,
    pub height: u32,
}

impl Drop for Endpoint<'_> {
    fn drop(&mut self) {
        // SAFETY: unique instance, device borrow remains live. Upstream destructor releases both
        // sender/receiver resources then its independently retained immediate context.
        unsafe { sdk::spout_dx_destroy(self.raw.as_ptr()) };
    }
}
