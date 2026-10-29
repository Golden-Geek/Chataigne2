/*
 * spout_shim.h - a flat C ABI over the Spout2 C++ classes.
 *
 * This shim exists so the Rust `spout2-sys` crate can bind a stable, plain-C
 * surface instead of a C++ vtable / mangled symbols / STL return types. It is
 * compiled together with the vendored Spout2 sources into one static library.
 *
 * Conventions across this boundary:
 *   - booleans are `int` (0 = false, non-zero = true)
 *   - strings out are copied into a caller-provided buffer (never returned as an
 *     owned pointer); the function returns the byte length written (excl. NUL)
 *   - strings in are NUL-terminated C strings
 *   - GPU / Win32 handles (ID3D11Texture2D*, ID3D11Device*, HANDLE, HWND) are
 *     opaque `void*`; OpenGL names (texture/FBO) are `unsigned int`
 *   - every function body is wrapped in try/catch so a C++ exception can never
 *     unwind across the FFI boundary
 *
 * The DirectX functions are compiled only when SPOUT2_SHIM_DX is defined, and
 * the OpenGL functions only when SPOUT2_SHIM_GL is defined (set by build.rs
 * from the crate's `dx` / `gl` features). DirectX 12 functions are compiled
 * when SPOUT2_SHIM_DX12 is defined (the `dx12` feature). A single opaque handle
 * type backs both the sender and receiver roles for a backend (they are the same C++ class); the
 * Rust wrapper presents them as distinct types.
 */
#ifndef SPOUT2_SHIM_H
#define SPOUT2_SHIM_H

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Copy the linked Spout SDK version string (e.g. "2.007.017") into `buf`.
 * Returns the number of bytes written excluding the NUL terminator, or 0 on
 * failure. Always available regardless of which backends are enabled.
 */
int spout_get_sdk_version(char* buf, int maxlen);

/* ===================================================================== */
/* DirectX 11 backend (spoutDX)                                          */
/* ===================================================================== */
#if defined(SPOUT2_SHIM_DX)

typedef struct spout_dx_t spout_dx_t;

enum spout_dx_transfer_status {
    SPOUT_DX_TRANSFER_FAILED = 0,
    SPOUT_DX_TRANSFER_COPIED = 1,
    SPOUT_DX_TRANSFER_UNCHANGED = 2,
    SPOUT_DX_TRANSFER_BUSY = 3,
    SPOUT_DX_TRANSFER_UPDATED = 4,
    SPOUT_DX_TRANSFER_DISCONNECTED = 5
};

enum spout_dx_frame_count_mode {
    SPOUT_DX_FRAME_COUNT_UNVERSIONED = 0,
    SPOUT_DX_FRAME_COUNT_SENDER_LOCAL = 1,
    SPOUT_DX_FRAME_COUNT_RECEIVER_VERSIONED = 2
};

/* Instance-only policy before initialization. Unknown foreign endpoints must use
 * UNVERSIONED. RECEIVER_VERSIONED is an explicit caller guarantee, never inferred
 * from semaphore existence; it only opens an existing counter. Counted names are
 * at most239 bytes (SDK counter suffix bound). No registry writes. */
int spout_dx_set_frame_count_mode(spout_dx_t* h, int mode);

/* Exact per-call GPU-copy admission, not completion. Failure may follow partial
 * commands: callers must fence every attempt. Both outputs are reset on
 * every call. Counter availability is separate from Copied/Unchanged; zero/errors
 * are unversioned. UPDATED acknowledges IsUpdated so the next call can copy. */
int spout_dx_receive_texture_into_status(spout_dx_t* h, void** pp_texture,
                                       long* frame, int* counter_available);
int spout_dx_send_texture_status(spout_dx_t* h, void* texture);

/* Non-consuming receiver-counter hint under the same real texture mutex, timeout0.
 * Does not create counters or change last-received state. Unavailable means do not skip. */
int spout_dx_peek_frame_count(spout_dx_t* h, long* frame);

/* lifecycle */
spout_dx_t* spout_dx_create(void);
void        spout_dx_destroy(spout_dx_t* h);

/* device (a NULL device pointer makes spoutDX create and own one) */
int   spout_dx_open_directx11(spout_dx_t* h, void* device);
void  spout_dx_close_directx11(spout_dx_t* h);
void* spout_dx_get_device(spout_dx_t* h);  /* ID3D11Device* */
void* spout_dx_get_context(spout_dx_t* h); /* ID3D11DeviceContext* */

/* sender */
int          spout_dx_set_sender_name(spout_dx_t* h, const char* name);
void         spout_dx_set_sender_format(spout_dx_t* h, unsigned int dxgi_format);
void         spout_dx_release_sender(spout_dx_t* h);
int          spout_dx_send_texture(spout_dx_t* h, void* texture); /* ID3D11Texture2D* */
int          spout_dx_send_image(spout_dx_t* h, const unsigned char* data,
                                 unsigned int width, unsigned int height, unsigned int pitch);
int          spout_dx_is_initialized(spout_dx_t* h);
int          spout_dx_get_name(spout_dx_t* h, char* buf, int maxlen);
unsigned int spout_dx_get_width(spout_dx_t* h);
unsigned int spout_dx_get_height(spout_dx_t* h);
double       spout_dx_get_fps(spout_dx_t* h);
long         spout_dx_get_frame(spout_dx_t* h);

/* frame-rate control */
void         spout_dx_hold_fps(spout_dx_t* h, int fps);

/* receiver */
void         spout_dx_set_receiver_name(spout_dx_t* h, const char* name);
void         spout_dx_release_receiver(spout_dx_t* h);
int          spout_dx_receive_texture(spout_dx_t* h);
int          spout_dx_receive_texture_into(spout_dx_t* h, void** pp_texture);
int          spout_dx_receive_image(spout_dx_t* h, unsigned char* pixels,
                                    unsigned int width, unsigned int height, int rgb, int invert);
int          spout_dx_select_sender(spout_dx_t* h, void* hwnd);
int          spout_dx_is_updated(spout_dx_t* h);
int          spout_dx_is_connected(spout_dx_t* h);
int          spout_dx_is_frame_new(spout_dx_t* h);
void*        spout_dx_get_sender_texture(spout_dx_t* h); /* ID3D11Texture2D* */
void*        spout_dx_get_sender_handle(spout_dx_t* h);  /* HANDLE */
unsigned int spout_dx_get_sender_format(spout_dx_t* h);
int          spout_dx_get_sender_name(spout_dx_t* h, char* buf, int maxlen);
unsigned int spout_dx_get_sender_width(spout_dx_t* h);
unsigned int spout_dx_get_sender_height(spout_dx_t* h);
double       spout_dx_get_sender_fps(spout_dx_t* h);
long         spout_dx_get_sender_frame(spout_dx_t* h);

/* discovery (no graphics device required) */
int spout_dx_get_sender_count(spout_dx_t* h);
int spout_dx_get_sender_name_at(spout_dx_t* h, int index, char* buf, int maxlen);
int spout_dx_get_active_sender(spout_dx_t* h, char* buf, int maxlen);
int spout_dx_get_sender_info(spout_dx_t* h, const char* name, unsigned int* width,
                             unsigned int* height, void** share_handle, unsigned int* format);

#endif /* SPOUT2_SHIM_DX */

/* ===================================================================== */
/* DirectX 12 backend (spoutDX12)                                        */
/* ===================================================================== */
#if defined(SPOUT2_SHIM_DX12)

typedef struct spout_dx12_t spout_dx12_t;

/* lifecycle */
spout_dx12_t* spout_dx12_create(void);
void          spout_dx12_destroy(spout_dx12_t* h);

/* device (NULL device creates and owns a class D3D12 device) */
int spout_dx12_open_directx12(spout_dx12_t* h, void* device, void** command_queue);
void  spout_dx12_close_directx12(spout_dx12_t* h);
void* spout_dx12_get_d3d12_device(spout_dx12_t* h);     /* ID3D12Device* */
void* spout_dx12_get_device(spout_dx12_t* h);           /* ID3D11Device* (11-on-12) */
void* spout_dx12_get_context(spout_dx12_t* h);          /* ID3D11DeviceContext* */
void* spout_dx12_get_d3d11on12_device(spout_dx12_t* h); /* ID3D11On12Device* */

/* DX12 texture bridge */
int spout_dx12_wrap_resource(spout_dx12_t* h, void* d3d12_resource,
                             unsigned int initial_state, void** out_wrapped11);
int spout_dx12_send_wrapped_resource(spout_dx12_t* h, void* wrapped11);
void spout_dx12_release_wrapped_resource(void* wrapped11);
int spout_dx12_receive_resource(spout_dx12_t* h, void** pp_d3d12_resource);
int spout_dx12_create_texture(spout_dx12_t* h, void* device,
                              unsigned int width, unsigned int height,
                              unsigned int initial_state, unsigned int format,
                              void** out_texture);

/* sender */
int          spout_dx12_set_sender_name(spout_dx12_t* h, const char* name);
void         spout_dx12_set_sender_format(spout_dx12_t* h, unsigned int dxgi_format);
void         spout_dx12_release_sender(spout_dx12_t* h);
int          spout_dx12_send_image(spout_dx12_t* h, const unsigned char* data,
                                   unsigned int width, unsigned int height, unsigned int pitch);
int          spout_dx12_is_initialized(spout_dx12_t* h);
int          spout_dx12_get_name(spout_dx12_t* h, char* buf, int maxlen);
unsigned int spout_dx12_get_width(spout_dx12_t* h);
unsigned int spout_dx12_get_height(spout_dx12_t* h);
double       spout_dx12_get_fps(spout_dx12_t* h);
long         spout_dx12_get_frame(spout_dx12_t* h);

/* frame-rate control */
void         spout_dx12_hold_fps(spout_dx12_t* h, int fps);

/* receiver */
void         spout_dx12_set_receiver_name(spout_dx12_t* h, const char* name);
void         spout_dx12_release_receiver(spout_dx12_t* h);
int          spout_dx12_receive_image(spout_dx12_t* h, unsigned char* pixels,
                                      unsigned int width, unsigned int height, int rgb, int invert);
int          spout_dx12_select_sender(spout_dx12_t* h, void* hwnd);
int          spout_dx12_is_updated(spout_dx12_t* h);
int          spout_dx12_is_connected(spout_dx12_t* h);
int          spout_dx12_is_frame_new(spout_dx12_t* h);
void*        spout_dx12_get_sender_handle(spout_dx12_t* h);  /* HANDLE */
unsigned int spout_dx12_get_sender_format(spout_dx12_t* h);
int          spout_dx12_get_sender_name(spout_dx12_t* h, char* buf, int maxlen);
unsigned int spout_dx12_get_sender_width(spout_dx12_t* h);
unsigned int spout_dx12_get_sender_height(spout_dx12_t* h);
double       spout_dx12_get_sender_fps(spout_dx12_t* h);
long         spout_dx12_get_sender_frame(spout_dx12_t* h);

/* discovery (no graphics device required) */
int spout_dx12_get_sender_count(spout_dx12_t* h);
int spout_dx12_get_sender_name_at(spout_dx12_t* h, int index, char* buf, int maxlen);
int spout_dx12_get_active_sender(spout_dx12_t* h, char* buf, int maxlen);
int spout_dx12_get_sender_info(spout_dx12_t* h, const char* name, unsigned int* width,
                               unsigned int* height, void** share_handle, unsigned int* format);

#endif /* SPOUT2_SHIM_DX12 */

/* ===================================================================== */
/* OpenGL backend (Spout)                                                */
/* ===================================================================== */
#if defined(SPOUT2_SHIM_GL)

typedef struct spout_gl_t spout_gl_t;

/* lifecycle */
spout_gl_t* spout_gl_create(void);
void        spout_gl_destroy(spout_gl_t* h);

/* hidden OpenGL context (for the CPU path when the caller has no GL context) */
int spout_gl_create_opengl(spout_gl_t* h, void* hwnd);
int spout_gl_close_opengl(spout_gl_t* h);

/* sender */
void         spout_gl_set_sender_name(spout_gl_t* h, const char* name);
void         spout_gl_set_sender_format(spout_gl_t* h, unsigned int dw_format);
void         spout_gl_release_sender(spout_gl_t* h);
int          spout_gl_send_fbo(spout_gl_t* h, unsigned int fbo,
                               unsigned int width, unsigned int height, int invert);
int          spout_gl_send_texture(spout_gl_t* h, unsigned int tex_id, unsigned int tex_target,
                                   unsigned int width, unsigned int height, int invert,
                                   unsigned int host_fbo);
int          spout_gl_send_image(spout_gl_t* h, const unsigned char* pixels,
                                 unsigned int width, unsigned int height, unsigned int gl_format,
                                 int invert, unsigned int host_fbo);
int          spout_gl_is_initialized(spout_gl_t* h);
int          spout_gl_get_name(spout_gl_t* h, char* buf, int maxlen);
unsigned int spout_gl_get_width(spout_gl_t* h);
unsigned int spout_gl_get_height(spout_gl_t* h);
double       spout_gl_get_fps(spout_gl_t* h);
long         spout_gl_get_frame(spout_gl_t* h);
void*        spout_gl_get_handle(spout_gl_t* h); /* HANDLE */

/* frame-rate control */
void         spout_gl_hold_fps(spout_gl_t* h, int fps);

/* receiver */
void         spout_gl_set_receiver_name(spout_gl_t* h, const char* name);
void         spout_gl_release_receiver(spout_gl_t* h);
int          spout_gl_receive(spout_gl_t* h); /* connect / poll, no copy */
int          spout_gl_receive_texture(spout_gl_t* h, unsigned int tex_id, unsigned int tex_target,
                                      int invert, unsigned int host_fbo);
int          spout_gl_receive_image(spout_gl_t* h, unsigned char* pixels, unsigned int gl_format,
                                    int invert, unsigned int host_fbo);
int          spout_gl_is_updated(spout_gl_t* h);
int          spout_gl_is_connected(spout_gl_t* h);
int          spout_gl_is_frame_new(spout_gl_t* h);
int          spout_gl_get_sender_name(spout_gl_t* h, char* buf, int maxlen);
unsigned int spout_gl_get_sender_width(spout_gl_t* h);
unsigned int spout_gl_get_sender_height(spout_gl_t* h);
unsigned int spout_gl_get_sender_format(spout_gl_t* h);
double       spout_gl_get_sender_fps(spout_gl_t* h);
long         spout_gl_get_sender_frame(spout_gl_t* h);
void*        spout_gl_get_sender_handle(spout_gl_t* h); /* HANDLE */
int          spout_gl_select_sender(spout_gl_t* h, void* hwnd);

/* discovery (no GL context required) */
int spout_gl_get_sender_count(spout_gl_t* h);
int spout_gl_get_sender_name_at(spout_gl_t* h, int index, char* buf, int maxlen);
int spout_gl_get_active_sender(spout_gl_t* h, char* buf, int maxlen);
int spout_gl_get_sender_info(spout_gl_t* h, const char* name, unsigned int* width,
                             unsigned int* height, void** share_handle, unsigned int* format);

#endif /* SPOUT2_SHIM_GL */

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SPOUT2_SHIM_H */
