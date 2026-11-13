//! Public FFmpeg buffer-pool APIs with admission before every new plane allocation.
use super::{DecodeError, ffmpeg};
use crate::image::ImageStorageBudget;
use ffmpeg::ffi;
use std::{
    ffi::c_void,
    ptr,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) struct Allocator {
    pub budget: ImageStorageBudget,
    pool: Mutex<Option<Pool>>,
    exhausted: AtomicBool,
}
struct Pool {
    format: ffi::AVPixelFormat,
    width: i32,
    height: i32,
    lines: [i32; 4],
    planes: [*mut ffi::AVBufferPool; 4],
}
// SAFETY: FFmpeg buffer pools support concurrent checkout/release. Configuration and
// destruction are serialized by Allocator's mutex; frame buffers own independent refs.
unsafe impl Send for Pool {}

impl Allocator {
    pub fn new(bytes: u64) -> Result<Self, DecodeError> {
        if bytes == 0 {
            return Err(DecodeError::InvalidLimits);
        }
        Ok(Self {
            budget: ImageStorageBudget::new(bytes).map_err(|_| DecodeError::InvalidLimits)?,
            pool: Mutex::new(None),
            exhausted: AtomicBool::new(false),
        })
    }
    pub fn configure(&self, context: &mut ffmpeg::codec::context::Context) -> Result<(), DecodeError> {
        let supported = ffmpeg::decoder::find(context.id()).is_some_and(|codec| {
            codec
                .capabilities()
                .contains(ffmpeg::codec::capabilities::Capabilities::DR1)
        });
        if !supported {
            return Err(DecodeError::UnsupportedSoftwareStorage);
        }
        self.exhausted.store(false, Ordering::Release);
        // SAFETY: unopened context; the boxed allocator in DecoderKind remains at a stable
        // address until after every owned codec context has been destroyed.
        unsafe {
            (*context.as_mut_ptr()).opaque = (self as *const Self).cast_mut().cast();
            (*context.as_mut_ptr()).get_buffer2 = Some(get_buffer);
        }
        Ok(())
    }
    pub fn failure(&self, error: DecodeError) -> DecodeError {
        if self.exhausted.load(Ordering::Acquire) {
            DecodeError::LimitExceeded
        } else {
            error
        }
    }
}

impl Pool {
    unsafe fn new(
        context: *mut ffi::AVCodecContext,
        frame: &ffi::AVFrame,
        budget: &ImageStorageBudget,
    ) -> Option<Self> {
        let format = unsafe { (*context).pix_fmt };
        if format as i32 != frame.format {
            return None;
        }
        let descriptor = unsafe { ffi::av_pix_fmt_desc_get(format) };
        if frame.width <= 0 || frame.height <= 0 || descriptor.is_null() {
            return None;
        }
        if unsafe { (*descriptor).flags } & (ffi::AV_PIX_FMT_FLAG_HWACCEL | ffi::AV_PIX_FMT_FLAG_PAL) as u64 != 0 {
            return None;
        }
        let mut width = frame.width;
        let mut height = frame.height;
        let mut alignment = [0i32; 8];
        // SAFETY: live codec and bounded complete metadata output arrays.
        unsafe { ffi::avcodec_align_dimensions2(context, &mut width, &mut height, alignment.as_mut_ptr()) };
        let mut lines = [0i32; 4];
        // Preserve inter-plane stride ratios by aligning the width, not individual rows.
        loop {
            if unsafe { ffi::av_image_fill_linesizes(lines.as_mut_ptr(), format, width) } < 0 {
                return None;
            }
            if lines
                .iter()
                .zip(alignment)
                .all(|(line, align)| line % align.max(1) == 0)
            {
                break;
            }
            width = width.checked_add(width & width.checked_neg()?)?;
        }
        let mut sizes = [0usize; 4];
        let strides = lines.map(|line| line as isize);
        if unsafe { ffi::av_image_fill_plane_sizes(sizes.as_mut_ptr(), format, height, strides.as_ptr()) } < 0 {
            return None;
        }
        let mut pool = Self {
            format,
            width: frame.width,
            height: frame.height,
            lines,
            planes: [ptr::null_mut(); 4],
        };
        for (plane, size) in pool.planes.iter_mut().zip(sizes) {
            if size == 0 {
                continue;
            }
            // FFmpeg's SIMD consumers require tail padding; 64 covers the supported
            // build's maximum stride alignment, separately from visible plane spans.
            let bytes = size
                .checked_add(16 + 64 - 1)
                .filter(|bytes| *bytes <= i32::MAX as usize)?;
            let owner = Box::into_raw(Box::new(budget.clone()));
            *plane = unsafe { ffi::av_buffer_pool_init2(bytes, owner.cast(), Some(allocate), Some(free_pool)) };
            if plane.is_null() {
                drop(unsafe { Box::from_raw(owner) });
                return None;
            }
        }
        Some(pool)
    }
}
impl Drop for Pool {
    fn drop(&mut self) {
        for pool in &mut self.planes {
            // SAFETY: exclusive pool handle. Outstanding frames keep storage alive and
            // their allocation callbacks release admission only after actual destruction.
            unsafe { ffi::av_buffer_pool_uninit(pool) };
        }
    }
}

unsafe extern "C" fn get_buffer(context: *mut ffi::AVCodecContext, frame: *mut ffi::AVFrame, _flags: i32) -> i32 {
    if context.is_null() || frame.is_null() || unsafe { (*context).opaque.is_null() } {
        return -ffmpeg::error::EINVAL;
    }
    let allocator = unsafe { &*(*context).opaque.cast::<Allocator>() };
    let image = unsafe { &mut *frame };
    if image.data.iter().any(|pointer| !pointer.is_null()) {
        return -ffmpeg::error::EINVAL;
    }
    let mut current = allocator.pool.lock().unwrap_or_else(|error| error.into_inner());
    if current.as_ref().is_none_or(|pool| {
        pool.format as i32 != image.format || pool.width != image.width || pool.height != image.height
    }) {
        // Release idle old-layout storage before admitting replacements; held old frames
        // retain their own pool/budget charges while the new pool competes for capacity.
        *current = None;
        *current = unsafe { Pool::new(context, image, &allocator.budget) };
    }
    let Some(pool) = current.as_ref() else {
        return -ffmpeg::error::EINVAL;
    };
    image.extended_data = image.data.as_mut_ptr();
    for index in 0..4 {
        if pool.planes[index].is_null() {
            continue;
        }
        image.buf[index] = unsafe { ffi::av_buffer_pool_get(pool.planes[index]) };
        if image.buf[index].is_null() {
            allocator.exhausted.store(true, Ordering::Release);
            unsafe { ffi::av_frame_unref(frame) };
            return -ffmpeg::error::ENOMEM;
        }
        image.data[index] = unsafe { (*image.buf[index]).data };
        image.linesize[index] = pool.lines[index];
    }
    0
}

unsafe extern "C" fn allocate(opaque: *mut c_void, bytes: usize) -> *mut ffi::AVBufferRef {
    let budget = unsafe { &*opaque.cast::<ImageStorageBudget>() };
    let Some(lease) = budget.reserve(bytes) else {
        return ptr::null_mut();
    };
    super::buffer::allocate(lease)
}
unsafe extern "C" fn free_pool(opaque: *mut c_void) {
    drop(unsafe { Box::from_raw(opaque.cast::<ImageStorageBudget>()) });
}
