//! FFmpeg-owned CPU payload whose reservation follows actual buffer destruction.
use super::ffmpeg::ffi;
use crate::image::StorageLease;
use std::{ffi::c_void, ptr};

pub(super) fn allocate(lease: StorageLease) -> *mut ffi::AVBufferRef {
    let bytes = lease.bytes();
    // SAFETY: positive admitted size; FFmpeg supplies its required SIMD alignment.
    let data = unsafe { ffi::av_mallocz(bytes) }.cast::<u8>();
    if data.is_null() {
        return ptr::null_mut();
    }
    let owner = Box::into_raw(Box::new(lease));
    // SAFETY: exclusively owned payload and lease transfer to the buffer on success.
    let buffer = unsafe { ffi::av_buffer_create(data, bytes, Some(free), owner.cast(), 0) };
    if buffer.is_null() {
        unsafe { ffi::av_free(data.cast()) };
        drop(unsafe { Box::from_raw(owner) });
    }
    buffer
}
unsafe extern "C" fn free(opaque: *mut c_void, data: *mut u8) {
    // SAFETY: FFmpeg calls once after the final buffer/pool reference; free before uncharge.
    unsafe { ffi::av_free(data.cast()) };
    drop(unsafe { Box::from_raw(opaque.cast::<StorageLease>()) });
}
