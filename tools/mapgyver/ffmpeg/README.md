# FFmpeg dependency boundary

Golden Video owns file demux and decode through the optional `ffmpeg` feature. MapGyver owns
the authored per-media transport and per-channel File parameters. Decode runs on a worker;
the engine and UI never call FFmpeg or own its contexts.

`windows-x64.json` pins `ffmpeg-next =9.0.0` and an exact shared LGPL Windows x64 build.
Cargo.lock pins the FFI dependency. The archive supplies headers, DLLs and MSVC import libraries.
No executable on the user's PATH is used by production decoding. Preparation verifies the
archive SHA-256 and size; dependencies stay in the ignored repository target directory.
MapGyver enables `video-files` by default; Golden Video retains its optional `ffmpeg` boundary.
Windows app builds use this pinned SDK even when selecting a smaller app feature set, because
their native bundle resources include the runtime. Other-platform workspace CI installs its
platform FFmpeg development libraries; that is build coverage, not MapGyver runtime qualification.

From the repository root, with Visual C++ and LLVM/libclang installed:

```powershell
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 test --locked -p golden_video --lib --no-default-features --features ffmpeg
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 clippy --locked -p golden_video --lib --tests --no-default-features --features ffmpeg -- -D warnings
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 test --locked -p MapGyver --lib -- --test-threads=1
npm run run:mapgyver
```

The helper restores process-local environment variables and never changes system settings.
Without Cargo arguments it only prepares the pinned dependency and prints its location.
`--tauri` selects the repository's local Tauri CLI from the MapGyver app directory; the normal
`package:mapgyver` / `package:check:mapgyver` npm commands use this same prepared environment.
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
`prepare-runtime.ps1` verifies every selected DLL and the archive license by pinned hash/size
before changing `target/mapgyver-ffmpeg-runtime`. It stages the five imported FFmpeg libraries,
LGPL/GPL texts, HAP/Snappy notices and public headers, binding notices and provenance. Unchanged
copies preserve timestamps. The Windows Tauri resource map copies these DLLs beside the app
and notices under `licenses/video` for ordinary builds and installer inputs. Runtime lookup
uses executable-local DLLs; it needs no system FFmpeg installation or FFmpeg PATH entry.

The build helper and npm scripts change only process-local environment, and the workspace
Windows product gate prepares the same pin before compiling default MapGyver. `tests/runtime.ps1`
checks idempotence and rejection of corrupted extracted DLL/license inputs before staging.
The browser fixture's `MAPGYVER_VIDEO_LOCAL_DLLS=1` additionally checks the owned process's
loaded module locations, sizes and hashes against the pin.

Complete corresponding-source delivery and incorporated third-party notices from the upstream
FFmpeg build, final installer checks, exact/coalesced seek/relink/device recovery and sustained
actual-app qualification remain M10/4. Staging these inputs alone does not close that gate.
No release or deployment is performed by these helpers.

HAP demux uses these same libraries. The [pinned reference dependency](../../../crates/golden_video/vendor/hap/README.md)
returns compressed GPU blocks through generated public C bindings, backed by Rust Snappy.
Native File playback now uses BC upload/GPU sampling and the shared three-slot publisher;
HAP Q conversion and alpha stay on the GPU. [M10/2 acceptance](../../../docs/progress/evidence/mapgyver-m10-hap-acceptance-2026-11-09.json)
covers both-GPU color/scale/alpha, external4K files, concurrency/shared owners and actual-app
copy/routing/reload. Bundled sustained qualification remains M10/4.
The explicit ignored `generate_reference_hap_mov_fixtures` test encodes known BC blocks through
the reference encoder and muxes only its three checked-in MOV fixtures with pinned libavformat;
it needs no CLI HAP encoder. Ordinary tests only read those files.

Primary sources: [Rust bindings](https://github.com/zmwangx/rust-ffmpeg),
[FFmpeg send/receive API](https://ffmpeg.org/doxygen/9.0/group__lavc__encdec.html),
[build source](https://github.com/FFmpeg-Builds/FFmpeg-Builds/tree/autobuild-2026-11-08-12-35).
