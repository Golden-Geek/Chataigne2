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
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 run --locked -p MapGyver -- --dev

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

The Windows helper sets `FFMPEG_DIR` for Cargo. For direct Cargo or IDE builds on Windows,
set `FFMPEG_DIR` to the SDK path printed by the helper with no arguments. MapGyver's Windows
Tauri resource configuration copies the five prepared DLLs and license materials beside the
Cargo executable. Workspace Cargo configuration supplies the Windows x64 MSVC stack reserve.

To create the Windows ARM64 SDK, run
`bash tools/mapgyver/ffmpeg/build-windows-arm64.sh <new-sdk-directory>` on x86_64 Linux
with Python, curl, tar and Make. The builder reuses MapGyver's pinned FFmpeg source and
verifies the pinned LLVM-MinGW archive before compiling. The SDK contains the five ARM64
DLLs, MSVC import libraries, public headers, build provenance and license notices. The
[Windows ARM64 workflow](../../../.github/workflows/mapgyver-windows-arm64.yml) links
the import libraries and checks the DLL exports on native Windows ARM64. Set
`MAPGYVER_FFMPEG_ARM64_SDK` to the extracted SDK and use the native ARM64 Rust, Node
and MSVC toolchains. With workspace dependencies and frontend assets prepared, run:

```powershell
$env:MAPGYVER_FFMPEG_ARM64_SDK = 'C:/dev/mapgyver-ffmpeg-arm64'
npm ci
npm run build:mapgyver
powershell -NoProfile -File tools/mapgyver/ffmpeg/with-ffmpeg.ps1 --tauri build --target aarch64-pc-windows-msvc --bundles nsis -- --locked
```

The ARM64 wrapper stages its DLLs separately and prepares the pinned `sse2neon.h`
for Spout. It scopes the MSVC stack reserve to the build's linker environment so
the FFmpeg binding generator receives it too. The installer is under
`target/aarch64-pc-windows-msvc/release/bundle/nsis`.
The bundled NDI runtime is currently Windows x64 only.

On macOS, `brew install ffmpeg pkgconf` provides the headers and libraries for ordinary Cargo
development builds. FFmpeg discovery uses `pkg-config` when `FFMPEG_DIR` is unset.

Build the self-contained macOS app on the intended architecture with Xcode command-line tools,
the pinned Rust and Node toolchains, and `nasm` on Intel. From the workspace root run `npm ci`
and `npm run package:mapgyver:macos`. Add `-- --debug` for the CI configuration. The app is
under `target/<profile>/bundle/macos`. The helper checks the FFmpeg source hash, builds the
five playback libraries and packages them in `Contents/Frameworks`, with the source pin,
recipe and notice in the bundle. The bundle declares macOS 13 as its minimum and uses ad hoc
signing by default for local verification. CI relocates and tests the app on macOS 15.

For a Linux development build, use the distro packages for `libavcodec-dev`,
`libavformat-dev`, `libavutil-dev`, `libswresample-dev` and `libswscale-dev` with
`pkg-config`, libclang and the Tauri/Golden desktop build dependencies. Then run
`cargo build --locked --package MapGyver --bin MapGyver`. The default Linux app uses
Golden's portable wgpu path for Media Preview and Screen Warping.

On a Debian-based Linux build host, run `sudo bash tools/mapgyver/install-linux-build-dependencies.sh`
from the workspace root. With the pinned Rust and Node toolchains, run `npm ci`, then
`npm run package:mapgyver:linux`. The command writes a `.deb` under
`target/release/bundle/deb`. Add `-- --debug` for a debug package under
`target/debug/bundle/deb`. Debian's package tooling derives the FFmpeg and desktop runtime
dependencies from the executable. Build on the target distribution and architecture so the
package uses the libraries available there.

The [Linux ARM package workflow](../../../.github/workflows/mapgyver-linux-arm.yml) builds
an `arm64` `.deb` in native Debian 13 userland for the Raspberry Pi OS Trixie baseline.
An x64 Debian 13 host builds the `armhf` package with the pinned Node for frontend assets
and Rust's `armv7-unknown-linux-gnueabihf` target for the desktop. Install that Rust target
with `rustup target add armv7-unknown-linux-gnueabihf`, pass `--armhf` to the dependency
installer, and run `npm run package:mapgyver:linux -- --armhf`. The package is under
`target/armv7-unknown-linux-gnueabihf/release/bundle/deb`. The armhf build covers ARMv7 or
newer CPUs with a 32-bit hard-float userland. CI uses QEMU to exercise the installed armhf
package in clean Debian 13. Raspberry Pi OS launch, VideoCore rendering and HDMI output
require checks on Pi hardware.

Install the file with `sudo apt install ./<package>.deb`, then launch **MapGyver 2** from the
application menu or run `mapgyver2`. CI checks installation without development packages
and exercises the installed desktop executable in a fresh distribution image.

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
