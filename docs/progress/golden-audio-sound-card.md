# Golden Audio and Sound Card Progress

This is the evidence ledger for the phased Golden Audio and Chataigne Sound Card implementation.
Results describe the exact local revision and platform that ran them. An untested platform or
backend remains `NOT RUN`.

## Baseline

- Plan baseline: `0c0442e964e3dc3cce0c7a6ae647be2c26d275b2` (22 July 2026).
- Implementation baseline: `c9cf6992326294475dbc4c9e9b1d5b8558130318`.
- Branch: `main`.
- Baseline drift: two commits (`97d7618f`, `c9cf6992`) after the plan baseline. The affected Sound
  Card extension paths were inspected; only shared module tests changed in the relevant backend
  area, while the remaining changes concern state-machine/Alchemist multiplex performance and UI.
- Worktree at start: clean except for the user-owned untracked
  `chataigne2-golden-audio-sound-card-implementation-plan.md`, which is preserved.
- Legacy gitlinks: four mode-160000 paths remain in the index, while `.gitmodules` has no matching
  entries. Consequently `git submodule status` fails before reporting SHAs. Indexed gitlink SHAs:
  `b4b9fe6f` (`src-ui/src/lib/golden_alchemist_ui`), `0dcb6baf`
  (`src-ui/src/lib/golden_ui`), `1bfe1c5b`
  (`submodules/golden_alchemist_core`), and `0cd6e706`
  (`submodules/golden_core`). This pre-existing repository condition is not changed by the audio
  work.
- Local platform: Windows x64, `x86_64-pc-windows-msvc`.
- Toolchain: rustc `1.97.0`, Cargo `1.97.0`, Node.js `26.5.0`, npm `11.17.0`.

## Current status

- Current phase: Phase 0 — baseline, decisions, and progress record.
- Status: complete locally; checkpoint pending.
- Next step: scaffold the backend-independent `golden_audio` crate.

## Decisions

### Package boundaries

- `golden_audio` is one independent workspace crate with no dependency on Golden Core or Chataigne.
- `golden_audio_ui` is a separate npm workspace package using `golden_ui` and generated
  app-agnostic Rust DTOs.
- Chataigne owns all Sound Card nodes, authored defaults, commands, scripts, persistence, and
  domain-specific editor behavior.
- Native audio, decoding, resampling, and analysis dependencies remain private to `golden_audio`.

### Initial engine limits

The limits are configurable and validated in one `EngineLimits` value. Initial desktop defaults to
qualify are:

| Limit | Initial value |
| --- | ---: |
| Engine sample rate | 48,000 Hz |
| Internal block | 128 frames |
| Virtual input channels | 256 |
| Virtual output channels | 256 |
| Sparse routes | 16,384 |
| Simultaneous voices | 256 |
| Analysis taps | 64 |
| FFT size | 16,384 frames |
| Spectrum bands per tap | 256 |
| Resident asset threshold | 32 MiB |
| Total resident cache | 512 MiB |
| Control commands | 4,096 |
| Events | 4,096 |
| Stream/read-ahead frames | 65,536 per stream |
| Gain dezipper | 10 ms, configurable from 5–20 ms |
| Observation rate | 30 Hz, configurable through 60 Hz |

These are capacity defaults, not promises that every reference workload meets its deadline. Phase
14 records measured qualification before any performance claim.

### Dependency baseline

The selected versions fit the pinned Rust `1.97.0` toolchain:

| Dependency | Version | License | MSRV / notes |
| --- | --- | --- | --- |
| CPAL | 0.18.1 | Apache-2.0 | Rust 1.85; stable device IDs and structured errors; optional ASIO, JACK, native PipeWire, and realtime features |
| Symphonia | 0.6.0 | MPL-2.0 | Rust 1.85; explicit audio codec/format features only |
| Rubato | 4.0.0 | MIT OR Apache-2.0 | Rust 1.85; preallocated `process_into_buffer` path |
| rtrb | 0.3.4 | MIT OR Apache-2.0 | Rust 1.38; bounded SPSC queues |
| realfft | 3.5.0 | MIT | RustFFT baseline Rust 1.61 |
| pitch-detection | 0.3.0 | MIT OR Apache-2.0 | Evaluated behind a private adapter; a focused in-crate YIN kernel is preferred if it gives clearer bounded ownership and passes the synthetic suite |

Symphonia will enable only `aac`, `adpcm`, `aiff`, `alac`, `caf`, `flac`, `isomp4`, `mkv`, `mp3`,
`ogg`, `pcm`, `vorbis`, and `wav`, plus the required metadata/SIMD features. Experimental video and
subtitle codecs remain disabled.

### Workspace and ASIO licensing

CPAL 0.18.1 can build ASIO through `asio-sys` and an external Steinberg SDK located by
`CPAL_ASIO_DIR`; LLVM/Clang and Visual C++ are build prerequisites. The repository already contains
the GPLv3 license text, and the project owner confirmed that the entire workspace is GPLv3. Workspace
Rust and npm package metadata now consistently use the SPDX identifier `GPL-3.0-only`. Steinberg's
currently published open-source ASIO SDK is also GPLv3, so the earlier MIT incompatibility concern
does not apply.

Decision:

- implement and qualify the ASIO backend under the GPLv3 path without checking SDK archives into
  this repository;
- keep SDK material in the bounded external tool cache and expose `CPAL_ASIO_DIR` only to the build;
- ship Corresponding Source and required license/copyright notices with distributed binaries; and
- verify Steinberg's ASIO trademark/usage guidance and the exact SDK archive notices during package
  qualification before marking the release row `PASS`.

## Risk register

| Risk | Mitigation / gate | Status |
| --- | --- | --- |
| ASIO requires LLVM, Visual C++, SDK material, GPLv3 source compliance, and trademark/package verification | Bootstrap checks; external cache; no vendored SDK; retain notices and ship Corresponding Source | Open |
| JACK library/server absent | Preserve dynamic loading; report `Unavailable`; app startup must pass without JACK | Open |
| Native PipeWire headers/linkage/package runtime | CPAL native backend remains internal; bootstrap and CI install explicit prerequisites; package evidence required | Open |
| macOS microphone permission and signing | Add usage description/entitlements and test denied/allowed signed package | NOT RUN |
| Final render-plan destruction on callback | One-pending-plan acknowledged exchange plus retained retired-plan slot; allocation/deallocation guard | Open |
| Independent input/output clock drift | Bounded ring, adaptive ASRC, PI controller, discontinuity fade, drift/bridge observations | Open |
| Decoder completion after stop/replacement | Monotonic command sequence and cancellation generation watermarks; stale worker results discarded off callback | Open |
| Large meter/matrix/spectrum UI | Packed latest-only telemetry, Canvas, viewport virtualization, bounded refresh, teardown tests | Open |
| Legacy unmapped gitlinks | Do not use or modify them; record pre-existing tooling failure | Open, unrelated |

## Backend qualification

| Platform | Backend | Build | Discovery | Stream I/O | Recovery | Package/startup |
| --- | --- | --- | --- | --- | --- | --- |
| Windows x64 | WASAPI | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| Windows x64 | ASIO | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| Windows x64 | JACK | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| Windows arm64 | WASAPI | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| macOS x64 | CoreAudio | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| macOS arm64 | CoreAudio | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| macOS | JACK | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| Linux x64 | ALSA | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| Linux x64 | JACK | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| Linux x64 | native PipeWire | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| Linux arm64 | ALSA / PipeWire | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN |

## Performance evidence

No audio engine exists at Phase 0. All render, routing, playback, analysis, memory, queue-pressure,
and soak measurements are `NOT RUN`.

| Revision | Hardware / OS | Backend | Workload | Result |
| --- | --- | --- | --- | --- |
| N/A | Windows x64 baseline | N/A | Initial audio benchmark suite | NOT RUN |

## Commands and evidence

| Command / inspection | Result |
| --- | --- |
| `git branch --show-current` | PASS — `main` |
| `git rev-parse HEAD` | PASS — `c9cf6992326294475dbc4c9e9b1d5b8558130318` |
| `git rev-parse main` | PASS — matches HEAD |
| `git status --short` | PASS — only the user-owned untracked plan |
| `git submodule status` | FAIL — pre-existing unmapped gitlink in `.gitmodules` |
| `git ls-files --stage` gitlink audit | PASS — four legacy gitlinks recorded above |
| `rustc --version --verbose` | PASS — Rust 1.97.0, Windows MSVC x64 |
| `cargo --version` | PASS — 1.97.0 |
| `node --version` | PASS — 26.5.0 |
| `npm --version` | PASS — 11.17.0 |
| `cargo info` for the dependency baseline | PASS — versions, licenses, features, and declared MSRVs recorded |
| `npm install --package-lock-only --ignore-scripts` | PASS — workspace GPLv3 metadata synchronized; npm reported three pre-existing low-severity advisories |
| New documentation link-target check | PASS — all Phase 0 local targets resolve |
| `cargo metadata --no-deps --format-version 1` | PASS — workspace manifests accept `GPL-3.0-only` |
| `npm pkg get license --workspaces --include-workspace-root` | PASS — all current npm workspace packages report `GPL-3.0-only` |
| `git diff --check` | PASS — no whitespace errors; Git emitted only existing Windows line-ending conversion warnings |
| Phase 1 Rust checks/tests/clippy | NOT RUN |
| Root and Golden Core formatting | NOT RUN |
| Chataigne tests | NOT RUN |
| UI checks/tests/build | NOT RUN |
| Product run modes | NOT RUN |
| Cross-platform backend/hardware matrix | NOT RUN |

## Files changed

Phase 0:

- `docs/architecture/golden-audio.md`
- `docs/progress/golden-audio-sound-card.md`
- `docs/architecture/README.md`
- `docs/README.md`
- `Cargo.toml`
- `package.json`
- `apps/chataigne/ui/package.json`
- `packages/golden-ui/package.json`
- `packages/golden-graph-ui/package.json`
- `README.md`
- `package-lock.json`

## Remaining work

Phases 1–15 remain. The immediate next gate is the backend-independent crate with strong types,
validated limits/configuration, null/mock/offline backends, control/event contracts, clean shutdown,
and external-style API tests.
