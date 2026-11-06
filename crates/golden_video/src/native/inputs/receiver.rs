use std::ffi::CString;
use std::ptr::NonNull;

use spout2_sys as sdk;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_TEXTURE2D_DESC, ID3D11Device, ID3D11RenderTargetView, ID3D11Texture2D,
};
use windows::core::{Error, HRESULT, Interface, Result};

use super::super::spout_layout;
use super::super::texture::Texture;
use crate::plan::{SpoutFrameCount, SpoutSource};
use crate::{Extent2D, TextureDescriptor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Received {
    Absent,
    Waiting,
    Unchanged,
    Busy,
    Mismatch,
    Frame,
}

/// A canonical image descriptor does not identify the exact sender storage (UNORM versus
/// SRGB). Authenticate and retain both for the entire alias/counter lifetime.
struct Endpoint {
    handle: usize,
    registered_storage: u32,
    native_storage: u32,
    descriptor: TextureDescriptor,
    _alias: ID3D11Texture2D,
}

/// The receive-into overload never creates the SDK's unbounded private receiving copy.
/// All endpoint operations run on a dedicated source device/context.
pub(crate) struct Receiver {
    raw: NonNull<sdk::spout_dx_t>,
    device: ID3D11Device,
    name: CString,
    binding: SpoutSource,
    endpoint: Option<Endpoint>,
    freshness: Freshness,
    clear_capture: bool,
}

impl Receiver {
    pub fn new(device: &ID3D11Device, binding: &SpoutSource) -> Result<Self> {
        let name = &binding.sender;
        let frame_count = binding.frame_count;
        if !frame_count.accepts_sender_name(name) {
            return Err(invalid());
        }
        let name = CString::new(name.as_str()).map_err(|_| invalid())?;
        // SAFETY: uniquely owned SDK instance, retained device AddRef outlives its borrowed pointer.
        let raw = NonNull::new(unsafe { sdk::spout_dx_create() }).ok_or_else(invalid)?;
        let receiver = Self {
            raw,
            device: device.clone(),
            name,
            binding: binding.clone(),
            endpoint: None,
            freshness: Freshness::new(frame_count),
            clear_capture: true,
        };
        unsafe {
            let mode = match frame_count {
                SpoutFrameCount::Unversioned => sdk::SPOUT_DX_FRAME_COUNT_UNVERSIONED,
                SpoutFrameCount::VersionedV1 => sdk::SPOUT_DX_FRAME_COUNT_RECEIVER_VERSIONED,
            };
            if sdk::spout_dx_set_frame_count_mode(raw.as_ptr(), mode) == 0 {
                return Err(invalid());
            }
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
        let Some(extent) = Extent2D::new(width, height).ok().filter(|extent| {
            extent.width() <= self.binding.descriptor.extent.width()
                && extent.height() <= self.binding.descriptor.extent.height()
        }) else {
            self.disconnect();
            return Ok(Some(Received::Mismatch));
        };
        if storage != 0 && layout(&self.binding, width, height, storage).is_none() {
            self.disconnect();
            return Ok(Some(Received::Mismatch));
        }
        if self.endpoint.as_ref().is_none_or(|endpoint| {
            endpoint.handle != handle as usize
                || endpoint.registered_storage != storage
                || endpoint.descriptor.extent != extent
        }) {
            self.disconnect();
            if !self.validate_alias(handle, storage, extent)? {
                return Ok(Some(Received::Mismatch));
            }
            // Withdraw the old endpoint's publication before a replacement can be Busy.
            return Ok(Some(Received::Waiting));
        }
        if self.freshness.policy == SpoutFrameCount::VersionedV1 {
            let mut frame = 0;
            // SAFETY: explicit caller-owned counter contract; this CPU peek does not consume
            // receiver state or queue GPU work. Missing/zero/reset counts proceed to receive.
            let available = unsafe { sdk::spout_dx_peek_frame_count(self.raw.as_ptr(), &mut frame) != 0 };
            if self.freshness.unchanged(available.then_some(frame)) {
                return Ok(Some(Received::Unchanged));
            }
        }
        Ok(None)
    }

    pub fn receive(&mut self, capture: &Texture) -> Result<Received> {
        self.receive_resource(&capture.texture, capture.descriptor, Some(&capture.rtv))
    }

    /// Non-renderable video captures are initialized only by a successful full SDK copy.
    /// Validate their real storage independently of the canonical sampled RGB contract.
    pub(in crate::native) fn receive_resource(
        &mut self,
        capture: &ID3D11Texture2D,
        descriptor: TextureDescriptor,
        clear: Option<&ID3D11RenderTargetView>,
    ) -> Result<Received> {
        let endpoint = self.endpoint.as_ref().ok_or_else(invalid)?;
        let (expected, expected_storage, expected_handle) =
            (endpoint.descriptor, endpoint.native_storage, endpoint.handle);
        let mut storage = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            capture.GetDesc(&mut storage);
        }
        let format_matches = if super::video::Format::from_native(expected_storage).is_some() {
            storage.Format.0 as u32 == expected_storage
        } else {
            storage.Format == super::super::texture::format(expected.format)
                || super::super::texture::typeless_accepts(storage.Format, expected.format)
        };
        if descriptor != expected
            || !format_matches
            || storage.Width != expected.extent.width()
            || storage.Height != expected.extent.height()
            || storage.MipLevels != 1
            || storage.ArraySize != 1
            || storage.SampleDesc.Count != 1
            || storage.SampleDesc.Quality != 0
            || unsafe { capture.GetDevice()? }.as_raw() != self.device.as_raw()
        {
            return Err(invalid());
        }
        if self.clear_capture {
            // SAFETY: no source bytes become observable before a successful detailed copy.
            if let Some(view) = clear {
                unsafe {
                    self.device
                        .GetImmediateContext()?
                        .ClearRenderTargetView(view, &[0.0; 4]);
                }
            }
            self.clear_capture = false;
        }
        let mut texture = capture.as_raw();
        let mut frame = 0;
        let mut counter_available = 0;
        // SAFETY: a fixed canonical receiver texture in the authenticated native format's
        // copy-compatible family, on the exact caller device. Unlike the
        // no-argument overload this public SDK API does not allocate a private receiving image.
        let status = unsafe {
            sdk::spout_dx_receive_texture_into_status(
                self.raw.as_ptr(),
                &mut texture,
                &mut frame,
                &mut counter_available,
            )
        };
        // SAFETY: read-only device status; preserve its actual removal HRESULT on failure.
        unsafe {
            self.device.GetDeviceRemovedReason()?;
        }
        let received = classify(status)?;
        if texture != capture.as_raw() || unsafe { sdk::spout_dx_get_device(self.raw.as_ptr()) } != self.device.as_raw()
        {
            return Err(invalid());
        }
        if matches!(received, Received::Waiting | Received::Absent) {
            self.freshness.transferred(received, None);
            self.clear_capture = true;
            return Ok(received);
        }
        let actual = unsafe { sdk::spout_dx_get_sender_handle(self.raw.as_ptr()) };
        if !actual.is_null() && actual as usize != expected_handle {
            // A metadata-to-receive race sets IsUpdated before CopyResource in the pinned SDK.
            // Withdraw this attempt. After the caller fences it, common preflight validates
            // the fresh registration/alias pair; SDK native format is not registered metadata.
            self.freshness.reset();
            self.clear_capture = true;
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
            || actual_format != expected_storage
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
        self.freshness
            .transferred(received, (counter_available != 0).then_some(frame));
        Ok(received)
    }

    fn validate_alias(&mut self, handle: *mut std::ffi::c_void, storage: u32, extent: Extent2D) -> Result<bool> {
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
        let Some(descriptor) = spout_layout::observed_alias(&self.binding, extent, storage, desc.Format) else {
            return Ok(false);
        };
        if desc.Width != extent.width()
            || desc.Height != extent.height()
            || desc.MipLevels != 1
            || desc.ArraySize != 1
            || desc.SampleDesc.Count != 1
            || desc.SampleDesc.Quality != 0
        {
            return Ok(false);
        }
        self.endpoint = Some(Endpoint {
            handle: handle as usize,
            registered_storage: storage,
            native_storage: desc.Format.0 as u32,
            descriptor,
            _alias: alias,
        });
        Ok(true)
    }

    pub fn disconnect(&mut self) {
        if self.endpoint.is_some() {
            // SAFETY: caller has drained previous source-device uses before SDK/alias replacement.
            unsafe {
                sdk::spout_dx_release_receiver(self.raw.as_ptr());
                sdk::spout_dx_set_receiver_name(self.raw.as_ptr(), self.name.as_ptr());
            }
        }
        self.endpoint = None;
        self.freshness.reset();
        self.clear_capture = true;
    }

    pub fn descriptor(&self) -> Option<TextureDescriptor> {
        self.endpoint.as_ref().map(|endpoint| endpoint.descriptor)
    }

    pub fn native_storage(&self) -> Option<u32> {
        self.endpoint.as_ref().map(|endpoint| endpoint.native_storage)
    }

    pub fn last_copied_frame(&self) -> Option<i32> {
        self.freshness.last_copied()
    }

    pub fn copied_image_version(&self) -> Option<super::super::timing::SpoutImageVersion> {
        let endpoint = self.endpoint.as_ref()?;
        self.last_copied_frame()
            .and_then(|frame| super::super::timing::SpoutImageVersion::copied_legacy(endpoint.handle, frame))
    }
}

pub(super) fn layout(binding: &SpoutSource, width: u32, height: u32, storage: u32) -> Option<TextureDescriptor> {
    spout_layout::observed(binding, Extent2D::new(width, height).ok()?, storage)
}

/// Counter history belongs to one validated sender-handle/descriptor lifetime. Only detailed
/// successful copies advance it; a semaphore's mere presence never enables the optimization.
pub(super) struct Freshness {
    policy: SpoutFrameCount,
    last_copied: Option<i32>,
}
impl Freshness {
    pub(super) fn new(policy: SpoutFrameCount) -> Self {
        Self {
            policy,
            last_copied: None,
        }
    }
    pub(super) fn transferred(&mut self, outcome: Received, frame: Option<i32>) {
        match outcome {
            Received::Frame => {
                self.last_copied = if self.policy == SpoutFrameCount::VersionedV1 {
                    frame
                } else {
                    None
                };
            }
            Received::Absent | Received::Waiting | Received::Mismatch => self.reset(),
            Received::Busy | Received::Unchanged => {}
        }
    }
    pub(super) fn unchanged(&self, frame: Option<i32>) -> bool {
        self.policy == SpoutFrameCount::VersionedV1
            && frame.is_some_and(|frame| frame > 0 && self.last_copied == Some(frame))
    }
    pub(super) fn reset(&mut self) {
        self.last_copied = None;
    }
    pub(super) fn last_copied(&self) -> Option<i32> {
        self.last_copied
    }
}

pub(super) fn classify(status: i32) -> Result<Received> {
    match status {
        sdk::SPOUT_DX_TRANSFER_COPIED => Ok(Received::Frame),
        sdk::SPOUT_DX_TRANSFER_UNCHANGED => Ok(Received::Unchanged),
        sdk::SPOUT_DX_TRANSFER_BUSY => Ok(Received::Busy),
        sdk::SPOUT_DX_TRANSFER_UPDATED => Ok(Received::Waiting),
        sdk::SPOUT_DX_TRANSFER_DISCONNECTED => Ok(Received::Absent),
        _ => Err(Error::from_hresult(HRESULT(0x8000_4005_u32 as i32))),
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
