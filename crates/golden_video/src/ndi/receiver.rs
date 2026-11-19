use std::ffi::CString;
use std::mem::MaybeUninit;
use std::ptr::{NonNull, null, null_mut};
use std::time::Duration;

use ndi_sdk::sys;

use crate::image::ImageData;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

use super::{NdiError, NdiSource, Runtime};

/// A connection owned and used by one capture worker.
pub struct NdiReceiver {
    runtime: &'static Runtime,
    instance: NonNull<sys::NDIlib_recv_instance_type>,
}

impl NdiReceiver {
    /// Connect to the selected discovery entry. The SDK reconnects if this sender goes offline.
    pub fn connect(source: &NdiSource) -> Result<Self, NdiError> {
        let runtime = Runtime::get()?;
        let name = CString::new(source.name.as_str()).map_err(|_| NdiError::Endpoint("a receiver for this name"))?;
        let settings = sys::NDIlib_recv_create_v3_t {
            source_to_connect_to: sys::NDIlib_source_t {
                p_ndi_name: name.as_ptr(),
                __bindgen_anon_1: sys::NDIlib_source_t__bindgen_ty_1 {
                    p_url_address: source.address.as_ref().map_or(null(), |address| address.as_ptr()),
                },
            },
            color_format: sys::NDIlib_recv_color_format_e::NDIlib_recv_color_format_BGRX_BGRA,
            bandwidth: sys::NDIlib_recv_bandwidth_e::NDIlib_recv_bandwidth_highest,
            allow_video_fields: false,
            p_ndi_recv_name: null(),
        };
        // NDI copies the source name and address during creation.
        let instance = unsafe { runtime.api.recv_create_v3.unwrap()(&settings) };
        let instance = NonNull::new(instance).ok_or(NdiError::Endpoint("a receiver"))?;
        Ok(Self { runtime, instance })
    }

    /// Number of active connections. Zero while the exact sender is unavailable.
    pub fn connection_count(&self) -> u32 {
        let connections = unsafe { self.runtime.api.recv_get_no_connections.unwrap()(self.instance.as_ptr()) };
        connections.max(0) as u32
    }

    /// Capture one progressive frame with a bounded wait. No SDK buffer crosses the worker boundary.
    pub fn capture_video(&mut self, timeout: Duration) -> Result<Option<ImageData>, NdiError> {
        let mut frame = MaybeUninit::<sys::NDIlib_video_frame_v2_t>::zeroed();
        let timeout = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        // Only video is requested here. The SDK initializes the frame only for a video result.
        let kind = unsafe {
            self.runtime.api.recv_capture_v3.unwrap()(
                self.instance.as_ptr(),
                frame.as_mut_ptr(),
                null_mut(),
                null_mut(),
                timeout,
            )
        };
        match kind {
            sys::NDIlib_frame_type_e::NDIlib_frame_type_video => {
                let frame = CapturedVideo {
                    receiver: self,
                    frame: unsafe { frame.assume_init() },
                };
                frame.copy_image()
            }
            sys::NDIlib_frame_type_e::NDIlib_frame_type_error => Err(NdiError::Disconnected),
            _ => Ok(None),
        }
    }
}

impl Drop for NdiReceiver {
    fn drop(&mut self) {
        // Capture guards borrow this receiver, so all frames were freed before destruction.
        unsafe { self.runtime.api.recv_destroy.unwrap()(self.instance.as_ptr()) };
    }
}

struct CapturedVideo<'a> {
    receiver: &'a NdiReceiver,
    frame: sys::NDIlib_video_frame_v2_t,
}

impl CapturedVideo<'_> {
    fn copy_image(&self) -> Result<Option<ImageData>, NdiError> {
        use sys::NDIlib_FourCC_video_type_e as FourCc;

        let frame = &self.frame;
        let (format, alpha_mode) = match frame.FourCC {
            FourCc::NDIlib_FourCC_video_type_BGRA => (PixelFormat::Bgra8Unorm, AlphaMode::Straight),
            FourCc::NDIlib_FourCC_video_type_BGRX => (PixelFormat::Bgrx8Unorm, AlphaMode::Opaque),
            _ => return Err(NdiError::Frame("the SDK returned an unexpected pixel format")),
        };
        let extent = Extent2D::new(frame.xres.max(0) as u32, frame.yres.max(0) as u32)
            .map_err(|_| NdiError::Frame("empty extent"))?;
        if extent.width() > 16_384 || extent.height() > 16_384 || frame.p_data.is_null() {
            return Err(NdiError::Frame("invalid image bounds"));
        }
        let row_bytes = extent.width() as usize * 4;
        let rows = extent.height() as usize;
        let size = row_bytes * rows;
        if size > 128 * 1024 * 1024 {
            return Err(NdiError::Frame("image exceeds the 128 MiB frame limit"));
        }
        let sdk_stride = unsafe { frame.__bindgen_anon_1.line_stride_in_bytes };
        let stride = if sdk_stride == 0 {
            row_bytes
        } else {
            usize::try_from(sdk_stride).map_err(|_| NdiError::Frame("negative line stride"))?
        };
        if stride < row_bytes || stride * rows > isize::MAX as usize {
            return Err(NdiError::Frame("invalid line stride"));
        }
        let Some(storage) = self.receiver.runtime.images.reserve(size) else {
            // Other producers and renderer generations may still hold images. Drop this frame.
            return Ok(None);
        };
        let mut pixels = Vec::with_capacity(size);
        // The SDK owns `rows` valid lines at the reported stride for the lifetime of this guard.
        for row in 0..rows {
            let row_start = unsafe { frame.p_data.add(row * stride) };
            pixels.extend_from_slice(unsafe { std::slice::from_raw_parts(row_start, row_bytes) });
        }
        let image = ImageData::new(
            TextureDescriptor {
                extent,
                format,
                color_encoding: ColorEncoding::Srgb,
                alpha_mode,
            },
            pixels,
        )?
        .with_storage(storage)?;
        Ok(Some(image))
    }
}

impl Drop for CapturedVideo<'_> {
    fn drop(&mut self) {
        unsafe { self.receiver.runtime.api.recv_free_video_v2.unwrap()(self.receiver.instance.as_ptr(), &self.frame) };
    }
}
