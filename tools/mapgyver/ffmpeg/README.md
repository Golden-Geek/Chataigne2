# FFmpeg dependency boundary

Golden Video owns file demux and decode through the optional `ffmpeg` feature. MapGyver will
own the authored per-media transport and per-channel File parameters. Decode runs on a worker;
the engine and UI never call FFmpeg or own its contexts.

`windows-x64.json` pins `ffmpeg-next =9.0.0` and an exact shared LGPL Windows x64 build.
Cargo.lock pins the FFI dependency. The archive supplies headers, DLLs and MSVC import libraries.
No executable on the user's PATH is used by production decoding. Preparation verifies the
archive SHA-256 and size; dependencies stay in the ignored repository target directory.

From the repository root, with Visual C++ and LLVM/libclang installed:

```powershell
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 test --locked -p golden_video --lib --no-default-features --features ffmpeg
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 clippy --locked -p golden_video --lib --tests --no-default-features --features ffmpeg -- -D warnings
```

The helper restores process-local environment variables and never changes system settings.
Without Cargo arguments it only prepares the pinned dependency and prints its location.

The initial public `decode::SoftwareDecoder` retains original presentation timestamps and
reference-counted software planes, including pixel format and color metadata. It enforces
explicit image/packet/work bounds, drains delayed codec frames and propagates demux failures.
It makes no implicit color conversion or GPU readback. Its limits do not claim to cap all
third-party temporary allocations; queue admission and cancellation belong to the owning worker.

This is the first M10/1 boundary. App transport, seek/step, worker/recovery integration and
GPU-resident D3D11 frames remain required. Do not label software decoding as hardware playback.
M10/4 owns final DLL packaging, dependency notices, corresponding-source delivery and sustained
actual-app qualification. No release or deployment is performed by the preparation helper.

Primary sources: [Rust bindings](https://github.com/zmwangx/rust-ffmpeg),
[FFmpeg send/receive API](https://ffmpeg.org/doxygen/9.0/group__lavc__encdec.html),
[build source](https://github.com/FFmpeg-Builds/FFmpeg-Builds/tree/autobuild-2026-11-08-12-35).
