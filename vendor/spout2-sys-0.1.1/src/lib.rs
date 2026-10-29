//! Raw FFI bindings to the Spout2 SDK.
//!
//! This crate compiles the vendored Spout2 C++ sources together with a thin
//! `extern "C"` shim (see `shim/spout_shim.cpp`) into a single static library
//! and exposes the shim's flat C API. Everything here is `unsafe` and
//! Windows-only.
//!
//! Prefer the safe [`spout2`](https://docs.rs/spout2) wrapper crate over calling
//! these functions directly.
#![allow(non_camel_case_types)]

/// The Spout SDK version this crate is vendored against (git submodule pin).
pub const SPOUT_SDK_VERSION: &str = "2.007.017";

/// Detailed texture transfer failed, possibly after partial commands; fence every attempt.
pub const SPOUT_DX_TRANSFER_FAILED: core::ffi::c_int = 0;
/// This call queued the actual GPU copy and flush, without certifying completion/freshness.
pub const SPOUT_DX_TRANSFER_COPIED: core::ffi::c_int = 1;
/// An explicitly versioned receiver observed no changed supported positive counter.
pub const SPOUT_DX_TRANSFER_UNCHANGED: core::ffi::c_int = 2;
/// Texture access timed out; this call did not copy and must not reuse an old new-frame flag.
pub const SPOUT_DX_TRANSFER_BUSY: core::ffi::c_int = 3;
/// Sender layout/session changed; the detailed API acknowledges the update flag.
pub const SPOUT_DX_TRANSFER_UPDATED: core::ffi::c_int = 4;
/// The named sender is disconnected.
pub const SPOUT_DX_TRANSFER_DISCONNECTED: core::ffi::c_int = 5;
/// Unknown/foreign endpoint policy: copies are explicitly unversioned, regardless of registry.
pub const SPOUT_DX_FRAME_COUNT_UNVERSIONED: core::ffi::c_int = 0;
/// Instance-local sender counter, sender name at most239 bytes. Never writes registry.
pub const SPOUT_DX_FRAME_COUNT_SENDER_LOCAL: core::ffi::c_int = 1;
/// Explicit caller guarantee that the selected sender/replacements publish a counter.
/// Only existing counters are opened; their existence alone does not establish trust.
pub const SPOUT_DX_FRAME_COUNT_RECEIVER_VERSIONED: core::ffi::c_int = 2;

#[cfg(windows)]
mod ffi {
    use core::ffi::{c_char, c_int};
    #[cfg(any(feature = "dx", feature = "dx12", feature = "gl"))]
    use core::ffi::{c_double, c_long, c_uint, c_void};

    unsafe extern "C" {
        /// Copy the linked SDK version (e.g. `"2.007.017"`) into `buf`; returns
        /// bytes written excluding the NUL, or 0 on failure.
        pub fn spout_get_sdk_version(buf: *mut c_char, maxlen: c_int) -> c_int;
    }

    // ===================================================================
    // DirectX 11 backend
    // ===================================================================
    /// Opaque handle to a `spoutDX` instance.
    #[cfg(feature = "dx")]
    #[repr(C)]
    pub struct spout_dx_t {
        _private: [u8; 0],
    }

    #[cfg(feature = "dx")]
    unsafe extern "C" {
        pub fn spout_dx_create() -> *mut spout_dx_t;
        pub fn spout_dx_destroy(h: *mut spout_dx_t);

        /// Selects instance-only frame counting before initialization; zero means rejected.
        pub fn spout_dx_set_frame_count_mode(h: *mut spout_dx_t, mode: c_int) -> c_int;
        /// Returns an exact SPOUT_DX_TRANSFER status for this GPU send call.
        pub fn spout_dx_send_texture_status(h: *mut spout_dx_t, texture: *mut c_void) -> c_int;
        /// Copies only when this call reports COPIED. Counter outputs reset every call;
        /// availability is independent of copy status, and zero/errors mean unknown freshness.
        /// UPDATED acknowledges upstream IsUpdated, allowing the next receive to proceed.
        pub fn spout_dx_receive_texture_into_status(
            h: *mut spout_dx_t,
            pp_texture: *mut *mut c_void,
            frame: *mut c_long,
            counter_available: *mut c_int,
        ) -> c_int;
        /// Non-consuming, zero-timeout synchronized peek; unavailable means never skip receive.
        pub fn spout_dx_peek_frame_count(h: *mut spout_dx_t, frame: *mut c_long) -> c_int;

        pub fn spout_dx_open_directx11(h: *mut spout_dx_t, device: *mut c_void) -> c_int;
        pub fn spout_dx_close_directx11(h: *mut spout_dx_t);
        pub fn spout_dx_get_device(h: *mut spout_dx_t) -> *mut c_void;
        pub fn spout_dx_get_context(h: *mut spout_dx_t) -> *mut c_void;

        pub fn spout_dx_set_sender_name(h: *mut spout_dx_t, name: *const c_char) -> c_int;
        pub fn spout_dx_set_sender_format(h: *mut spout_dx_t, dxgi_format: c_uint);
        pub fn spout_dx_release_sender(h: *mut spout_dx_t);
        pub fn spout_dx_send_texture(h: *mut spout_dx_t, texture: *mut c_void) -> c_int;
        pub fn spout_dx_send_image(
            h: *mut spout_dx_t,
            data: *const u8,
            width: c_uint,
            height: c_uint,
            pitch: c_uint,
        ) -> c_int;
        pub fn spout_dx_is_initialized(h: *mut spout_dx_t) -> c_int;
        pub fn spout_dx_get_name(h: *mut spout_dx_t, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_dx_get_width(h: *mut spout_dx_t) -> c_uint;
        pub fn spout_dx_get_height(h: *mut spout_dx_t) -> c_uint;
        pub fn spout_dx_get_fps(h: *mut spout_dx_t) -> c_double;
        pub fn spout_dx_get_frame(h: *mut spout_dx_t) -> c_long;
        pub fn spout_dx_hold_fps(h: *mut spout_dx_t, fps: c_int);

        pub fn spout_dx_set_receiver_name(h: *mut spout_dx_t, name: *const c_char);
        pub fn spout_dx_release_receiver(h: *mut spout_dx_t);
        pub fn spout_dx_receive_texture(h: *mut spout_dx_t) -> c_int;
        pub fn spout_dx_receive_texture_into(h: *mut spout_dx_t, pp_texture: *mut *mut c_void) -> c_int;
        pub fn spout_dx_receive_image(
            h: *mut spout_dx_t,
            pixels: *mut u8,
            width: c_uint,
            height: c_uint,
            rgb: c_int,
            invert: c_int,
        ) -> c_int;
        pub fn spout_dx_select_sender(h: *mut spout_dx_t, hwnd: *mut c_void) -> c_int;
        pub fn spout_dx_is_updated(h: *mut spout_dx_t) -> c_int;
        pub fn spout_dx_is_connected(h: *mut spout_dx_t) -> c_int;
        pub fn spout_dx_is_frame_new(h: *mut spout_dx_t) -> c_int;
        pub fn spout_dx_get_sender_texture(h: *mut spout_dx_t) -> *mut c_void;
        pub fn spout_dx_get_sender_handle(h: *mut spout_dx_t) -> *mut c_void;
        pub fn spout_dx_get_sender_format(h: *mut spout_dx_t) -> c_uint;
        pub fn spout_dx_get_sender_name(h: *mut spout_dx_t, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_dx_get_sender_width(h: *mut spout_dx_t) -> c_uint;
        pub fn spout_dx_get_sender_height(h: *mut spout_dx_t) -> c_uint;
        pub fn spout_dx_get_sender_fps(h: *mut spout_dx_t) -> c_double;
        pub fn spout_dx_get_sender_frame(h: *mut spout_dx_t) -> c_long;

        pub fn spout_dx_get_sender_count(h: *mut spout_dx_t) -> c_int;
        pub fn spout_dx_get_sender_name_at(h: *mut spout_dx_t, index: c_int, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_dx_get_active_sender(h: *mut spout_dx_t, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_dx_get_sender_info(
            h: *mut spout_dx_t,
            name: *const c_char,
            width: *mut c_uint,
            height: *mut c_uint,
            share_handle: *mut *mut c_void,
            format: *mut c_uint,
        ) -> c_int;
    }

    // ===================================================================
    // DirectX 12 backend
    // ===================================================================
    /// Opaque handle to a `spoutDX12` instance.
    #[cfg(feature = "dx12")]
    #[repr(C)]
    pub struct spout_dx12_t {
        _private: [u8; 0],
    }

    #[cfg(feature = "dx12")]
    unsafe extern "C" {
        pub fn spout_dx12_create() -> *mut spout_dx12_t;
        pub fn spout_dx12_destroy(h: *mut spout_dx12_t);

        pub fn spout_dx12_open_directx12(
            h: *mut spout_dx12_t,
            device: *mut c_void,
            command_queue: *mut *mut c_void,
        ) -> c_int;
        pub fn spout_dx12_close_directx12(h: *mut spout_dx12_t);
        pub fn spout_dx12_get_d3d12_device(h: *mut spout_dx12_t) -> *mut c_void;
        pub fn spout_dx12_get_device(h: *mut spout_dx12_t) -> *mut c_void;
        pub fn spout_dx12_get_context(h: *mut spout_dx12_t) -> *mut c_void;
        pub fn spout_dx12_get_d3d11on12_device(h: *mut spout_dx12_t) -> *mut c_void;

        pub fn spout_dx12_wrap_resource(
            h: *mut spout_dx12_t,
            d3d12_resource: *mut c_void,
            initial_state: c_uint,
            out_wrapped11: *mut *mut c_void,
        ) -> c_int;
        pub fn spout_dx12_send_wrapped_resource(h: *mut spout_dx12_t, wrapped11: *mut c_void) -> c_int;
        pub fn spout_dx12_release_wrapped_resource(wrapped11: *mut c_void);
        pub fn spout_dx12_receive_resource(h: *mut spout_dx12_t, pp_d3d12_resource: *mut *mut c_void) -> c_int;
        pub fn spout_dx12_create_texture(
            h: *mut spout_dx12_t,
            device: *mut c_void,
            width: c_uint,
            height: c_uint,
            initial_state: c_uint,
            format: c_uint,
            out_texture: *mut *mut c_void,
        ) -> c_int;

        pub fn spout_dx12_set_sender_name(h: *mut spout_dx12_t, name: *const c_char) -> c_int;
        pub fn spout_dx12_set_sender_format(h: *mut spout_dx12_t, dxgi_format: c_uint);
        pub fn spout_dx12_release_sender(h: *mut spout_dx12_t);
        pub fn spout_dx12_send_image(
            h: *mut spout_dx12_t,
            data: *const u8,
            width: c_uint,
            height: c_uint,
            pitch: c_uint,
        ) -> c_int;
        pub fn spout_dx12_is_initialized(h: *mut spout_dx12_t) -> c_int;
        pub fn spout_dx12_get_name(h: *mut spout_dx12_t, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_dx12_get_width(h: *mut spout_dx12_t) -> c_uint;
        pub fn spout_dx12_get_height(h: *mut spout_dx12_t) -> c_uint;
        pub fn spout_dx12_get_fps(h: *mut spout_dx12_t) -> c_double;
        pub fn spout_dx12_get_frame(h: *mut spout_dx12_t) -> c_long;
        pub fn spout_dx12_hold_fps(h: *mut spout_dx12_t, fps: c_int);

        pub fn spout_dx12_set_receiver_name(h: *mut spout_dx12_t, name: *const c_char);
        pub fn spout_dx12_release_receiver(h: *mut spout_dx12_t);
        pub fn spout_dx12_receive_image(
            h: *mut spout_dx12_t,
            pixels: *mut u8,
            width: c_uint,
            height: c_uint,
            rgb: c_int,
            invert: c_int,
        ) -> c_int;
        pub fn spout_dx12_select_sender(h: *mut spout_dx12_t, hwnd: *mut c_void) -> c_int;
        pub fn spout_dx12_is_updated(h: *mut spout_dx12_t) -> c_int;
        pub fn spout_dx12_is_connected(h: *mut spout_dx12_t) -> c_int;
        pub fn spout_dx12_is_frame_new(h: *mut spout_dx12_t) -> c_int;
        pub fn spout_dx12_get_sender_handle(h: *mut spout_dx12_t) -> *mut c_void;
        pub fn spout_dx12_get_sender_format(h: *mut spout_dx12_t) -> c_uint;
        pub fn spout_dx12_get_sender_name(h: *mut spout_dx12_t, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_dx12_get_sender_width(h: *mut spout_dx12_t) -> c_uint;
        pub fn spout_dx12_get_sender_height(h: *mut spout_dx12_t) -> c_uint;
        pub fn spout_dx12_get_sender_fps(h: *mut spout_dx12_t) -> c_double;
        pub fn spout_dx12_get_sender_frame(h: *mut spout_dx12_t) -> c_long;

        pub fn spout_dx12_get_sender_count(h: *mut spout_dx12_t) -> c_int;
        pub fn spout_dx12_get_sender_name_at(
            h: *mut spout_dx12_t,
            index: c_int,
            buf: *mut c_char,
            maxlen: c_int,
        ) -> c_int;
        pub fn spout_dx12_get_active_sender(h: *mut spout_dx12_t, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_dx12_get_sender_info(
            h: *mut spout_dx12_t,
            name: *const c_char,
            width: *mut c_uint,
            height: *mut c_uint,
            share_handle: *mut *mut c_void,
            format: *mut c_uint,
        ) -> c_int;
    }

    // ===================================================================
    // OpenGL backend
    // ===================================================================
    /// Opaque handle to a `Spout` instance.
    #[cfg(feature = "gl")]
    #[repr(C)]
    pub struct spout_gl_t {
        _private: [u8; 0],
    }

    #[cfg(feature = "gl")]
    unsafe extern "C" {
        pub fn spout_gl_create() -> *mut spout_gl_t;
        pub fn spout_gl_destroy(h: *mut spout_gl_t);

        pub fn spout_gl_create_opengl(h: *mut spout_gl_t, hwnd: *mut c_void) -> c_int;
        pub fn spout_gl_close_opengl(h: *mut spout_gl_t) -> c_int;

        pub fn spout_gl_set_sender_name(h: *mut spout_gl_t, name: *const c_char);
        pub fn spout_gl_set_sender_format(h: *mut spout_gl_t, dw_format: c_uint);
        pub fn spout_gl_release_sender(h: *mut spout_gl_t);
        pub fn spout_gl_send_fbo(
            h: *mut spout_gl_t,
            fbo: c_uint,
            width: c_uint,
            height: c_uint,
            invert: c_int,
        ) -> c_int;
        pub fn spout_gl_send_texture(
            h: *mut spout_gl_t,
            tex_id: c_uint,
            tex_target: c_uint,
            width: c_uint,
            height: c_uint,
            invert: c_int,
            host_fbo: c_uint,
        ) -> c_int;
        pub fn spout_gl_send_image(
            h: *mut spout_gl_t,
            pixels: *const u8,
            width: c_uint,
            height: c_uint,
            gl_format: c_uint,
            invert: c_int,
            host_fbo: c_uint,
        ) -> c_int;
        pub fn spout_gl_is_initialized(h: *mut spout_gl_t) -> c_int;
        pub fn spout_gl_get_name(h: *mut spout_gl_t, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_gl_get_width(h: *mut spout_gl_t) -> c_uint;
        pub fn spout_gl_get_height(h: *mut spout_gl_t) -> c_uint;
        pub fn spout_gl_get_fps(h: *mut spout_gl_t) -> c_double;
        pub fn spout_gl_get_frame(h: *mut spout_gl_t) -> c_long;
        pub fn spout_gl_get_handle(h: *mut spout_gl_t) -> *mut c_void;
        pub fn spout_gl_hold_fps(h: *mut spout_gl_t, fps: c_int);

        pub fn spout_gl_set_receiver_name(h: *mut spout_gl_t, name: *const c_char);
        pub fn spout_gl_release_receiver(h: *mut spout_gl_t);
        pub fn spout_gl_receive(h: *mut spout_gl_t) -> c_int;
        pub fn spout_gl_receive_texture(
            h: *mut spout_gl_t,
            tex_id: c_uint,
            tex_target: c_uint,
            invert: c_int,
            host_fbo: c_uint,
        ) -> c_int;
        pub fn spout_gl_receive_image(
            h: *mut spout_gl_t,
            pixels: *mut u8,
            gl_format: c_uint,
            invert: c_int,
            host_fbo: c_uint,
        ) -> c_int;
        pub fn spout_gl_is_updated(h: *mut spout_gl_t) -> c_int;
        pub fn spout_gl_is_connected(h: *mut spout_gl_t) -> c_int;
        pub fn spout_gl_is_frame_new(h: *mut spout_gl_t) -> c_int;
        pub fn spout_gl_get_sender_name(h: *mut spout_gl_t, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_gl_get_sender_width(h: *mut spout_gl_t) -> c_uint;
        pub fn spout_gl_get_sender_height(h: *mut spout_gl_t) -> c_uint;
        pub fn spout_gl_get_sender_format(h: *mut spout_gl_t) -> c_uint;
        pub fn spout_gl_get_sender_fps(h: *mut spout_gl_t) -> c_double;
        pub fn spout_gl_get_sender_frame(h: *mut spout_gl_t) -> c_long;
        pub fn spout_gl_get_sender_handle(h: *mut spout_gl_t) -> *mut c_void;
        pub fn spout_gl_select_sender(h: *mut spout_gl_t, hwnd: *mut c_void) -> c_int;

        pub fn spout_gl_get_sender_count(h: *mut spout_gl_t) -> c_int;
        pub fn spout_gl_get_sender_name_at(h: *mut spout_gl_t, index: c_int, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_gl_get_active_sender(h: *mut spout_gl_t, buf: *mut c_char, maxlen: c_int) -> c_int;
        pub fn spout_gl_get_sender_info(
            h: *mut spout_gl_t,
            name: *const c_char,
            width: *mut c_uint,
            height: *mut c_uint,
            share_handle: *mut *mut c_void,
            format: *mut c_uint,
        ) -> c_int;
    }
}

#[cfg(windows)]
pub use ffi::*;
