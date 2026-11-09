# FFmpeg dependency boundary

Golden Video owns file demux and decode through the optional `ffmpeg` feature. MapGyver owns
the authored per-media transport and per-channel File parameters. Decode runs on a worker;
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
It also reserves a 16 MiB MSVC main-thread stack for helper-built executables: optimized
binding generation with the Windows headers exhausted the default reserve. Existing Rust
flags are preserved, including Cargo's encoded form, and restored after Cargo exits.

The public `decode::Decoder` retains original presentation timestamps and explicit software,
D3D11 or compressed HAP frame storage, including pixel format and color metadata. It enforces
explicit image/packet/work bounds, drains delayed codec frames and propagates demux failures.
It makes no implicit color conversion or GPU readback. Its limits do not claim to cap all
third-party temporary allocations; queue admission and cancellation belong to the owning worker.

The app-owned `MediaTransport` now provides the shared monotonic playback policy, and the
decoder can seek in original stream timestamps with fresh codec state. Typed controls and
public callers are wired. The `video-files` app feature now connects bounded local-file workers
to timestamp sampling, explicit software SDR conversion through pinned libswscale and reusable
native texture uploads. Missing-file retry and consumer/observer retirement are covered by
app tests. Selected-channel step commits, complete authored-variant metadata and GPU-resident
D3D11 frames are implemented. The native host selects its exact adapter and retains pool/bridge
admission across seeks and observers, with explicit software fallback. See the
[Video ownership](../../../docs/architecture/mapgyver-video.md) for the current integration boundary.
M10/4 owns final DLL packaging, dependency notices, corresponding-source delivery and sustained
actual-app qualification. No release or deployment is performed by the preparation helper.

HAP demux uses these same libraries. The [pinned reference dependency](../../../crates/golden_video/vendor/hap/README.md)
returns compressed GPU blocks through generated public C bindings, backed by Rust Snappy.
Native File playback now uses BC upload/GPU sampling and the shared three-slot publisher;
HAP Q conversion and alpha stay on the GPU. Complete concurrency/interoperability and bundled
acceptance remain required.
The explicit ignored `generate_reference_hap_mov_fixtures` test encodes known BC blocks through
the reference encoder and muxes only its three checked-in MOV fixtures with pinned libavformat;
it needs no CLI HAP encoder. Ordinary tests only read those files.

Primary sources: [Rust bindings](https://github.com/zmwangx/rust-ffmpeg),
[FFmpeg send/receive API](https://ffmpeg.org/doxygen/9.0/group__lavc__encdec.html),
[build source](https://github.com/FFmpeg-Builds/FFmpeg-Builds/tree/autobuild-2026-11-08-12-35).
