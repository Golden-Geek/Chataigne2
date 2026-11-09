# FFmpeg dependency boundary

Golden Video owns worker-local demux, decode and frame lifetime behind its optional `ffmpeg`
feature. MapGyver enables Video by default and owns transport, File Parameters and demand.
Engine/UI threads do not call FFmpeg or own its contexts.

`source-build.json` pins the full FFmpeg commit, portable GCC/MinGW toolchain, NASM, source
archives and configure arguments. `windows-x64.json` pins the five qualified runtime DLL
hashes/sizes and the Rust binding contract. Cargo.lock pins the FFI dependencies. A clean
reconstruction in a second directory reproduces all five DLL bytes. Cross-machine byte
identity and final product qualification have separate acceptance requirements.

The shared SDK keeps built-in playback decoders, SIMD, software scaling and D3D11/DXVA2.
Optional external codec/filter libraries, encoders, devices, CLI programs and network are
excluded. Registered DXV/WebP decoders still require their planned per-variant qualification.

From the repository root on Windows, with MSVC, LLVM/libclang and Python available:

```powershell
# Once per checkout (also reconstructs the SDK if target/ was cleaned):
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1
cargo run -p MapGyver -- --dev

powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 test --locked -p MapGyver --lib -- --test-threads=1
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 clippy --locked -p MapGyver --lib --tests -- -D warnings
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 test --locked -p golden_video --lib --features ffmpeg
npm run run:mapgyver
npm run watch:mapgyver
```

The helper builds the exact source SDK when absent, prepares its complete source kit and
verifies runtime inputs before staging. Downloads use bounded retries and atomic installation;
corrupt cached inputs are preserved and replaced. Windows' archive reader extracts the
toolchain as data, with no installer or system changes. Build-only DLL aliases are copied,
so Windows symlink privileges are unnecessary. A fixed release override prevents parent Git
tags from entering the libraries; source epoch, logical prefix and PE timestamp policy are fixed.
MSVC consumes generated public import libraries.

Workspace Cargo configuration supplies the prepared SDK location when `FFMPEG_DIR` is unset
and the Windows MSVC stack reserve. MapGyver's existing Windows Tauri resource configuration
copies the five prepared DLLs and license materials beside the Cargo executable. Plain
`cargo run` therefore needs no wrapper-specific PATH or FFmpeg environment after preparation.
Explicit `FFMPEG_DIR` values retain precedence; the runtime must still match the product pin.
Non-Windows FFmpeg consumers select their native SDK explicitly.

Without Cargo arguments, the helper prepares inputs and returns only the SDK path in the
PowerShell success stream; progress goes to the host. With
`--tauri`, it invokes the repository-local Tauri CLI in the app directory. Run/watch/package npm
commands and Windows workspace CI use this same boundary. It restores process-local variables
and directory, preserving caller Rust flags and their encoded form. The 16 MiB MSVC stack
reserve supports bindgen's Windows SDK traversal.

Prepared locations:

- SDK: `target/ffmpeg-source/stage/mapgyver-video-sdk`.
- [Corresponding-source kit](SOURCE-KIT.md): `target/ffmpeg-source/source-kit`.
- Verified runtime staging: `target/mapgyver-ffmpeg-runtime`.

The kit includes exact FFmpeg/compiler sources, reconstruction recipes, runtime identity and
FFmpeg/GCC/MinGW notices. The Windows Tauri map puts DLLs beside ordinary app builds and
notices/provenance under `licenses/video`. App startup uses executable-local libraries without
a system FFmpeg installation or FFmpeg PATH entry. Source-archive distribution/publication
accompanies the final Windows artifact; these helpers perform no release or deployment.

`tests/source-inputs.ps1` checks retry/cache recovery. `tests/runtime.ps1` checks stage
idempotence and rejection of corrupt extracted DLL/license inputs before mutation.
`tests/source_sdk.py` checks live public DLL version/configuration/license/codec exports and
header identities. Browser qualification with `MAPGYVER_VIDEO_LOCAL_DLLS=1` inspects actual
loaded module paths, sizes and hashes; an explicit fixture pin can qualify a candidate before
promotion.

HAP uses the same demux libraries and the [reference codec](../../../crates/golden_video/vendor/hap/README.md)
with Rust Snappy. BC blocks, YCoCg conversion and alpha stay on the GPU. The ignored fixture
generator uses reference HAP encoding and libavformat MOV muxing; ordinary tests read fixtures.
[M10/2](../../../docs/progress/evidence/mapgyver-m10-hap-acceptance-2026-11-09.json) owns its
functional acceptance. [Video ownership](../../../docs/architecture/mapgyver-video.md) describes
clock/step/source/retirement boundaries. M10/4 retains exact/coalesced seeks, relink/device
recovery and sustained 1080p/4K/resource qualification before M14.

Primary sources: [FFmpeg](https://github.com/FFmpeg/FFmpeg),
[Rust bindings](https://github.com/zmwangx/rust-ffmpeg),
[w64devkit](https://github.com/skeeto/w64devkit).
