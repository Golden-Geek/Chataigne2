//! FFmpeg-owned D3D11 devices and frame-pool leases. No hardware-to-CPU transfer.
use super::{DecodeError, DecodeLimits, Decoder, DecoderKind, ffmpeg};
use crate::native::GpuFrameBudget;
use crate::{Extent2D, native::AdapterId};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{ffi::CString, marker::PhantomData, path::Path, ptr, rc::Rc};
use windows::Win32::Graphics::{
    Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D},
    Dxgi::{Common::*, CreateDXGIFactory1, DXGI_ERROR_NOT_FOUND, IDXGIDevice, IDXGIFactory1},
};
use windows::core::Interface;

/// GPU storage layout, separate from sampled RGB/color interpretation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum D3d11Storage {
    /// Eight-bit planar 4:2:0.
    Nv12,
    /// Ten-bit planar 4:2:0 in sixteen-bit elements.
    P010,
}

/// Worker-local decoded surface. Its FFmpeg frame retains the pool slot, device and storage.
/// A COM texture reference alone would not prevent decoder reuse of that slot.
pub struct D3d11Frame {
    pub(crate) texture: ID3D11Texture2D,
    pub(crate) slice: u32,
    extent: Extent2D,
    storage: D3d11Storage,
    adapter: AdapterId,
    frame: ffmpeg::frame::Video,
    _local: PhantomData<Rc<()>>,
}
impl D3d11Frame {
    pub(crate) fn retain(&self) -> Result<Self, DecodeError> {
        // Video::clone copies planes. av_frame_ref instead retains the hardware pool lease.
        let mut frame = ffmpeg::frame::Video::empty();
        let result = unsafe { ffmpeg::ffi::av_frame_ref(frame.as_mut_ptr(), self.frame.as_ptr()) };
        if result < 0 {
            return Err(ffmpeg::Error::from(result).into());
        }
        Ok(Self {
            texture: self.texture.clone(),
            slice: self.slice,
            extent: self.extent,
            storage: self.storage,
            adapter: self.adapter,
            frame,
            _local: PhantomData,
        })
    }
    /// Exact observed adapter LUID, checked against the requested adapter on every frame.
    pub fn adapter(&self) -> AdapterId {
        self.adapter
    }
    /// Visible image size. The underlying decoder texture can include aligned padding.
    pub fn extent(&self) -> Extent2D {
        self.extent
    }
    /// Actual native plane storage, without an inferred RGB interpretation.
    pub fn storage(&self) -> D3d11Storage {
        self.storage
    }
    /// The retained frame's array slice, not a stream frame number.
    pub fn pool_slice(&self) -> u32 {
        self.slice
    }
    /// Declared frame color matrix, interpreted by a later GPU converter.
    pub fn color_space(&self) -> ffmpeg::color::Space {
        self.frame.color_space()
    }
    /// Declared frame transfer, interpreted by a later GPU converter.
    pub fn color_transfer(&self) -> ffmpeg::color::TransferCharacteristic {
        self.frame.color_transfer_characteristic()
    }
    /// Declared full/limited input range, without guessing from native storage.
    pub fn color_range(&self) -> ffmpeg::color::Range {
        self.frame.color_range()
    }
    /// Current device failure. Owners must withdraw and reopen after removal.
    pub fn check_device(&self) -> Result<(), DecodeError> {
        // SAFETY: the owned frame and COM reference retain this exact device.
        unsafe {
            self.texture
                .GetDevice()
                .and_then(|device| device.GetDeviceRemovedReason())
        }
        .map_err(native_error)
    }
}

pub(super) struct HardwareDevice {
    buffer: *mut ffmpeg::ffi::AVBufferRef,
    adapter: AdapterId,
    admission: Arc<Admission>,
    _local: PhantomData<Rc<()>>,
}
impl Drop for HardwareDevice {
    fn drop(&mut self) {
        // SAFETY: this is our reference. Codec contexts and retained frames own other refs.
        unsafe { ffmpeg::ffi::av_buffer_unref(&mut self.buffer) };
    }
}
impl HardwareDevice {
    fn new(adapter: AdapterId, codec: ffmpeg::codec::Id, budget: GpuFrameBudget) -> Result<Self, DecodeError> {
        let hardware = ffmpeg::ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA;
        // SAFETY: static codec/config descriptors remain valid throughout FFmpeg's lifetime.
        let supported = unsafe {
            let codec = ffmpeg::ffi::avcodec_find_decoder(codec.into());
            if codec.is_null() {
                return Err(DecodeError::UnsupportedCodec);
            }
            (0..64).any(|index| {
                let config = ffmpeg::ffi::avcodec_get_hw_config(codec, index);
                !config.is_null()
                    && (*config).device_type == hardware
                    && (*config).pix_fmt == ffmpeg::ffi::AVPixelFormat::AV_PIX_FMT_D3D11
                    && (*config).methods & ffmpeg::ffi::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i32 != 0
            })
        };
        if !supported {
            return Err(DecodeError::UnsupportedHardware);
        }
        // FFmpeg's D3D11VA device option selects the current DXGI enumeration index.
        // Resolve the requested LUID afresh; verify the real frame LUID again after decode.
        let index = adapter_index(adapter)?;
        let selected = CString::new(index.to_string()).unwrap();
        let mut buffer = ptr::null_mut();
        // SAFETY: libavutil owns context allocation and its API-specific ABI; no private
        // AVD3D11VADeviceContext declaration is mirrored in Rust.
        let result = unsafe {
            ffmpeg::ffi::av_hwdevice_ctx_create(&mut buffer, hardware, selected.as_ptr(), ptr::null_mut(), 0)
        };
        if result < 0 {
            return Err(ffmpeg::Error::from(result).into());
        }
        if buffer.is_null() {
            return Err(ffmpeg::Error::Bug.into());
        }
        Ok(Self {
            buffer,
            adapter,
            admission: Arc::new(Admission {
                budget,
                exhausted: AtomicBool::new(false),
            }),
            _local: PhantomData,
        })
    }
    pub(super) fn configure(&self, context: &mut ffmpeg::codec::context::Context) -> Result<(), DecodeError> {
        self.admission.exhausted.store(false, Ordering::Release);
        // SAFETY: unopened uniquely owned codec context; FFmpeg releases its acquired ref.
        unsafe {
            let reference = ffmpeg::ffi::av_buffer_ref(self.buffer);
            if reference.is_null() {
                return Err(ffmpeg::Error::Other {
                    errno: ffmpeg::error::ENOMEM,
                }
                .into());
            }
            (*context.as_mut_ptr()).hw_device_ctx = reference;
            (*context.as_mut_ptr()).get_format = Some(select_hardware);
            (*context.as_mut_ptr()).opaque = Arc::as_ptr(&self.admission).cast_mut().cast();
            // Reader keeps current/next, publication may retain a frame, and exact seeks
            // create a new codec/pool. Retention remains explicitly bounded by the owner.
            (*context.as_mut_ptr()).extra_hw_frames = 8;
        }
        Ok(())
    }
    pub(super) fn failure(&self, error: DecodeError) -> DecodeError {
        if self.admission.exhausted.load(Ordering::Acquire) {
            DecodeError::LimitExceeded
        } else {
            error
        }
    }
    pub(super) fn frame(&self, frame: ffmpeg::frame::Video) -> Result<D3d11Frame, DecodeError> {
        if frame.format() != ffmpeg::format::Pixel::D3D11 {
            return Err(DecodeError::UnsupportedHardware);
        }
        // SAFETY: a successfully received D3D11 AVFrame owns this texture and array index.
        // Borrow then AddRef the interface; retain the AVFrame too to pin its pool slot.
        let (texture, slice) = unsafe {
            let raw = (*frame.as_ptr()).data[0].cast();
            let texture = ID3D11Texture2D::from_raw_borrowed(&raw)
                .ok_or_else(|| DecodeError::Hardware("missing D3D11 frame texture".into()))?
                .clone();
            let slice = usize::try_from((*frame.as_ptr()).data[1] as isize)
                .ok()
                .and_then(|index| u32::try_from(index).ok())
                .ok_or_else(|| DecodeError::Hardware("invalid D3D11 frame slice".into()))?;
            (texture, slice)
        };
        let mut descriptor = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: copied read-only descriptors from a retained live texture/device.
        let adapter = unsafe {
            texture.GetDesc(&mut descriptor);
            let device = texture.GetDevice().map_err(native_error)?;
            device.GetDeviceRemovedReason().map_err(native_error)?;
            let adapter = device
                .cast::<IDXGIDevice>()
                .map_err(native_error)?
                .GetAdapter()
                .map_err(native_error)?;
            let desc = adapter.GetDesc().map_err(native_error)?;
            AdapterId {
                low: desc.AdapterLuid.LowPart,
                high: desc.AdapterLuid.HighPart,
            }
        };
        if adapter != self.adapter
            || slice >= descriptor.ArraySize
            || descriptor.MipLevels != 1
            || descriptor.SampleDesc.Count != 1
            || descriptor.Width < frame.width()
            || descriptor.Height < frame.height()
        {
            return Err(DecodeError::Hardware("D3D11 frame identity or extent mismatch".into()));
        }
        let storage = match descriptor.Format {
            DXGI_FORMAT_NV12 => D3d11Storage::Nv12,
            DXGI_FORMAT_P010 => D3d11Storage::P010,
            _ => return Err(DecodeError::UnsupportedHardware),
        };
        let extent = Extent2D::new(frame.width(), frame.height()).map_err(|e| DecodeError::Hardware(e.to_string()))?;
        Ok(D3d11Frame {
            texture,
            slice,
            extent,
            storage,
            adapter,
            frame,
            _local: PhantomData,
        })
    }
}

impl Decoder {
    /// Preserve native storage: HAP blocks or D3D11 decode on one exact adapter.
    /// HAP's selected adapter is applied when its GPU publisher is constructed.
    /// Unsupported codecs/profiles fail; an owning caller can reopen with `open` instead.
    /// Pool reservations precede allocation and follow FFmpeg pool ownership across seeks.
    pub fn open_native(
        path: &Path,
        limits: DecodeLimits,
        adapter: AdapterId,
        budget: GpuFrameBudget,
    ) -> Result<Self, DecodeError> {
        Self::open_native_with_interrupt(path, limits, adapter, budget, || false)
    }
    /// Hardware open with the same cancellation/demux boundary as software decode.
    pub fn open_native_with_interrupt(
        path: &Path,
        limits: DecodeLimits,
        adapter: AdapterId,
        budget: GpuFrameBudget,
        interrupt: impl FnMut() -> bool + Send + 'static,
    ) -> Result<Self, DecodeError> {
        Self::open_configured(path, limits, interrupt, move |input, index| {
            let stream = input.stream(index).ok_or(ffmpeg::Error::StreamNotFound)?;
            if stream.parameters().id() == ffmpeg::codec::Id::HAP {
                return Ok(DecoderKind::Hap);
            }
            HardwareDevice::new(adapter, stream.parameters().id(), budget).map(DecoderKind::D3d11)
        })
    }
}

fn adapter_index(wanted: AdapterId) -> Result<u32, DecodeError> {
    // SAFETY: read-only DXGI enumeration, no device creation or fallback selection.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(native_error)?;
    for index in 0.. {
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(native_error(error)),
        };
        let desc = unsafe { adapter.GetDesc1() }.map_err(native_error)?;
        if wanted
            == (AdapterId {
                low: desc.AdapterLuid.LowPart,
                high: desc.AdapterLuid.HighPart,
            })
        {
            if desc.Flags & windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                return Err(DecodeError::UnsupportedHardware);
            }
            return Ok(index);
        }
    }
    Err(DecodeError::Hardware("selected adapter is unavailable".into()))
}
fn native_error(error: windows::core::Error) -> DecodeError {
    DecodeError::Hardware(format!("{} ({:#010x})", error.message(), error.code().0))
}
// SAFETY: FFmpeg passes a terminated static format list. Never select a software format;
// unsupported hardware profiles must fail instead of being mislabeled as hardware decoded.
unsafe extern "C" fn select_hardware(
    context: *mut ffmpeg::ffi::AVCodecContext,
    mut formats: *const ffmpeg::ffi::AVPixelFormat,
) -> ffmpeg::ffi::AVPixelFormat {
    use ffmpeg::ffi::AVPixelFormat::{AV_PIX_FMT_D3D11, AV_PIX_FMT_NONE};
    if formats.is_null() {
        return AV_PIX_FMT_NONE;
    }
    // FFmpeg owns this terminated list; cap the amount of work in the callback.
    for _ in 0..256 {
        let format = unsafe { *formats };
        if format == AV_PIX_FMT_D3D11 {
            return if unsafe { initialize_frames(context) } {
                format
            } else {
                AV_PIX_FMT_NONE
            };
        }
        if format == AV_PIX_FMT_NONE {
            return format;
        }
        formats = unsafe { formats.add(1) };
    }
    AV_PIX_FMT_NONE
}

fn admitted_pool(width: i32, height: i32, surfaces: i32, max_pixels: i64) -> bool {
    width > 0 && height > 0 && (1..=32).contains(&surfaces) && i64::from(width) * i64::from(height) <= max_pixels
}
struct Admission {
    budget: GpuFrameBudget,
    exhausted: AtomicBool,
}
struct PoolLease {
    previous_free: Option<unsafe extern "C" fn(*mut ffmpeg::ffi::AVHWFramesContext)>,
    previous_opaque: *mut std::ffi::c_void,
    _reservation: Arc<dyn Send + Sync>,
}
unsafe extern "C" fn release_pool(context: *mut ffmpeg::ffi::AVHWFramesContext) {
    // FFmpeg calls this only after its pool/storage cleanup; preserve any earlier owner.
    let lease = unsafe { Box::from_raw((*context).user_opaque.cast::<PoolLease>()) };
    unsafe {
        (*context).user_opaque = lease.previous_opaque;
        (*context).free = lease.previous_free;
        if let Some(free) = lease.previous_free {
            free(context);
        }
    }
    drop(lease);
}
fn pool_bytes(width: i32, height: i32, surfaces: i32, format: ffmpeg::ffi::AVPixelFormat) -> Option<u64> {
    use ffmpeg::ffi::AVPixelFormat::{AV_PIX_FMT_NV12, AV_PIX_FMT_P010LE};
    let pixels = u64::try_from(width).ok()?.checked_mul(u64::try_from(height).ok()?)?;
    let bytes = match format {
        AV_PIX_FMT_NV12 => pixels.checked_mul(3)?.div_ceil(2),
        AV_PIX_FMT_P010LE => pixels.checked_mul(3)?,
        _ => return None,
    };
    bytes.checked_mul(u64::try_from(surfaces).ok()?)
}
// SAFETY: called only by the codec's format callback on its uniquely owned live context.
// Ask FFmpeg for API-specific parameters, then admit the fixed pool before allocation.
unsafe fn initialize_frames(context: *mut ffmpeg::ffi::AVCodecContext) -> bool {
    if context.is_null() {
        return false;
    }
    let mut frames = ptr::null_mut();
    let result = unsafe {
        ffmpeg::ffi::avcodec_get_hw_frames_parameters(
            context,
            (*context).hw_device_ctx,
            ffmpeg::ffi::AVPixelFormat::AV_PIX_FMT_D3D11,
            &mut frames,
        )
    };
    if result < 0 || frames.is_null() {
        return false;
    }
    let parameters = unsafe { &mut *(*frames).data.cast::<ffmpeg::ffi::AVHWFramesContext>() };
    if !admitted_pool(
        parameters.width,
        parameters.height,
        parameters.initial_pool_size,
        unsafe { (*context).max_pixels },
    ) {
        unsafe { ffmpeg::ffi::av_buffer_unref(&mut frames) };
        return false;
    }
    let admission = unsafe { &*(*context).opaque.cast::<Admission>() };
    let Some(reservation) = pool_bytes(
        parameters.width,
        parameters.height,
        parameters.initial_pool_size,
        parameters.sw_format,
    )
    .and_then(|bytes| admission.budget.reserve(bytes)) else {
        admission.exhausted.store(true, Ordering::Release);
        unsafe { ffmpeg::ffi::av_buffer_unref(&mut frames) };
        return false;
    };
    let lease = Box::new(PoolLease {
        previous_free: parameters.free,
        previous_opaque: parameters.user_opaque,
        _reservation: reservation,
    });
    parameters.free = Some(release_pool);
    parameters.user_opaque = Box::into_raw(lease).cast();
    if unsafe { ffmpeg::ffi::av_hwframe_ctx_init(frames) } < 0 {
        unsafe { ffmpeg::ffi::av_buffer_unref(&mut frames) };
        return false;
    }
    unsafe { (*context).hw_frames_ctx = frames };
    true
}

#[cfg(test)]
mod tests;
