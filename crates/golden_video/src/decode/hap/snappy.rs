//! Private Snappy C ABI for the reference codec, backed by the established Rust snap crate.
use std::{ffi::c_char, slice};

// The C caller retains disjoint input/output slices and valid output length pointers.
// All entry points are private-prefixed so another native dependency cannot bind them.
#[unsafe(no_mangle)]
unsafe extern "C" fn golden_hap_snappy_uncompressed_length(
    input: *const c_char,
    len: usize,
    output: *mut usize,
) -> u32 {
    let input = unsafe { slice::from_raw_parts(input.cast::<u8>(), len) };
    match snap::raw::decompress_len(input) {
        Ok(len) => {
            unsafe { *output = len };
            0
        }
        Err(_) => 1,
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn golden_hap_snappy_uncompress(
    input: *const c_char,
    len: usize,
    output: *mut c_char,
    capacity: *mut usize,
) -> u32 {
    let input = unsafe { slice::from_raw_parts(input.cast::<u8>(), len) };
    let available = unsafe { *capacity };
    let required = match snap::raw::decompress_len(input) {
        Ok(len) => len,
        Err(_) => return 1,
    };
    if required > available {
        return 2;
    }
    let output = unsafe { slice::from_raw_parts_mut(output.cast::<u8>(), available) };
    match snap::raw::Decoder::new().decompress(input, output) {
        Ok(len) => {
            unsafe { *capacity = len };
            0
        }
        Err(_) => 1,
    }
}
#[unsafe(no_mangle)]
extern "C" fn golden_hap_snappy_max_compressed_length(len: usize) -> usize {
    snap::raw::max_compress_len(len)
}
#[unsafe(no_mangle)]
unsafe extern "C" fn golden_hap_snappy_compress(
    input: *const c_char,
    len: usize,
    output: *mut c_char,
    capacity: *mut usize,
) -> u32 {
    let input = unsafe { slice::from_raw_parts(input.cast::<u8>(), len) };
    let available = unsafe { *capacity };
    if available < snap::raw::max_compress_len(len) {
        return 2;
    }
    let output = unsafe { slice::from_raw_parts_mut(output.cast::<u8>(), available) };
    match snap::raw::Encoder::new().compress(input, output) {
        Ok(len) => {
            unsafe { *capacity = len };
            0
        }
        Err(_) => 1,
    }
}
