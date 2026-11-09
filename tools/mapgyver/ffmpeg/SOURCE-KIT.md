# MapGyver source-built Video SDK

This kit associates the exact source archives and build recipe with the five default
FFmpeg libraries described in `runtime.json`. App software/strict and both-GPU functional
qualification precede promotion of their pinned bytes. This kit alone does not accept the
remaining M10/4 recovery/performance requirements or qualify a Windows release.

`ffmpeg-source.tar.gz` is the complete unmodified FFmpeg source at the pinned commit.
`w64devkit-source.tar` contains the toolchain's upstream sources, including GCC and MinGW-w64
runtime sources. `toolchain-recipe` preserves its supplied Dockerfile, custom tool sources,
README and version identity. `licenses` contains FFmpeg LGPL/GPL texts, the MinGW runtime
notices and the GCC Runtime Library Exception with its GPL text. The source/build manifest
records archive sizes and SHA256 hashes and complete configure arguments.

All required Video codec families retain their built-in decoders. The build keeps CPU SIMD
assembly, software scaling and D3D11/DXVA2 acceleration. It excludes optional external codec/
filter libraries, encoding, capture devices, CLI programs and network support. This avoids
the unrelated external-library closure in the earlier general-purpose development SDK.
Decoder presence is not acceptance of unverified codec/profile variants.

Rebuild on Windows from this directory, with Python available for identity checks:

```powershell
New-Item -ItemType Directory -Path recipe/target/ffmpeg-source/downloads -Force
Copy-Item -LiteralPath ffmpeg-source.tar.gz -Destination recipe/target/ffmpeg-source/downloads
Copy-Item -LiteralPath w64devkit-source.tar -Destination recipe/target/ffmpeg-source/downloads
powershell -NoProfile -File recipe/tools/mapgyver/ffmpeg/build-source.ps1
powershell -NoProfile -File recipe/tools/mapgyver/ffmpeg/prepare-source-kit.ps1
```

The recipe downloads the exact hash-pinned portable compiler and assembler tools when absent.
It reads their archives as data with Windows' installed archive tools; it runs no installer and
changes no system configuration. Build state stays under the recipe's ignored/local `target`.
The `VERSION` release override prevents FFmpeg from inheriting an enclosing repository's Git
identity. `SOURCE_DATE_EPOCH`, logical install prefix and PE timestamp policy are fixed.
The recipe copies build-only DLL aliases instead of requiring Windows symlink privileges.
Its emitted MSVC import libraries are consumed through the ordinary `FFMPEG_DIR` boundary.

The checks load each exact SDK DLL, compare its public version/configuration/license exports
and decoder catalog with the pinned recipe, and emit runtime hashes. Complete app behavior,
device recovery, sustained performance and final artifact/source publication have separate
acceptance requirements. A clean reconstruction in a second directory reproduces all five
DLL bytes; cross-machine byte-for-byte reproduction remains unclaimed.
