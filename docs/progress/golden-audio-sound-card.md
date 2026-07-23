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

- Current phase: Phase 8 - RMS, pitch, spectrum, and observation transport.
- Status: implementation and backend-neutral qualification complete; checkpoint pending.
- Last checkpoint: `db226cc2` (`feat(audio): add ordered asynchronous file playback`).
- Next step: add the Chataigne-owned Sound Card schema and persistence boundary.

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
| Decoder workers | 2 |
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
| allocation-counter | 0.8.1 | MIT OR Apache-2.0 | Dev-only callback allocation/deallocation guard |
| Criterion | 0.8.2 | Apache-2.0 OR MIT | Dev-only render and routing benchmarks; Rust 1.86 |

Symphonia will enable only `aac`, `adpcm`, `aiff`, `alac`, `caf`, `flac`, `isomp4`, `mkv`, `mp3`,
`ogg`, `pcm`, `vorbis`, and `wav`, plus the required metadata/SIMD features. Experimental video and
subtitle codecs remain disabled.

Phase 8 selected a focused in-crate YIN kernel instead of adding `pitch-detection`. The local
implementation keeps all difference scratch and thresholds explicit, needs no additional
dependency, and passes the synthetic tone, harmonic, detuning, silence, noise, low-amplitude, and
chirp suite. RealFFT 3.5.0 remains a private optional dependency behind the default `analysis`
feature; it is absent from no-default-feature builds.

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
| Independent input/output clock drift | Bounded ring, adaptive ASRC, PI controller, discontinuity fade, drift/bridge observations | Mitigated; backend-neutral qualification PASS |
| Decoder completion after stop/replacement | Monotonic command sequence and cancellation generation watermarks; stale worker results discarded off callback | Mitigated; deterministic ordering suite PASS |
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

Phase 2 results are scalar, backend-independent release benchmarks. The hardware was an Intel Core
Ultra 9 275HX (24 logical processors) on Windows 11 build 26200. These measurements establish the
local implementation baseline; they are not cross-platform or device deadline claims.

| Revision | Workload | Result |
| --- | --- | --- |
| `baf373c` + Phase 2 working tree | Render: 8 channels, 16 routes, 32 frames | median 356.41 ns |
| `baf373c` + Phase 2 working tree | Render: 8 channels, 16 routes, 1,024 frames | median 9.2262 µs |
| `baf373c` + Phase 2 working tree | Render: 32 channels, 128 routes, 128 frames | median 7.5126 µs |
| `baf373c` + Phase 2 working tree | Render: 128 channels, 1,024 routes, 256 frames | median 120.06 µs |
| `baf373c` + Phase 2 working tree | Render: 256 channels, 16,384 routes, 1,024 frames | median 5.6106 ms |
| `baf373c` + Phase 2 working tree | Routing: requested 0 routes (16 physical patch routes) | median 1.1042 µs |
| `baf373c` + Phase 2 working tree | Routing: 16 routes | median 1.1129 µs |
| `baf373c` + Phase 2 working tree | Routing: 128 routes | median 7.8434 µs |
| `baf373c` + Phase 2 working tree | Routing: 1,024 routes | median 60.218 µs |
| `baf373c` + Phase 2 working tree | Routing: 4,096 routes | median 208.83 µs |
| `baf373c` + Phase 2 working tree | Routing: 16,384 routes | median 694.98 µs |
| `740c306b` + Phase 7 working tree | Playback: 1 resident voice, 128 frames | median 0.251 µs per callback |
| `740c306b` + Phase 7 working tree | Playback: 16 resident voices, 128 frames | median 3.987 µs per callback |
| `740c306b` + Phase 7 working tree | Playback: 64 resident voices, 128 frames | median 15.974 µs per callback |
| `740c306b` + Phase 7 working tree | Playback: 128 resident voices, 128 frames | median 31.487 µs per callback |
| `740c306b` + Phase 7 working tree | Playback: 256 resident voices, 128 frames | median 64.486 µs per callback |
| `740c306b` + Phase 7 working tree | Playback: 8 resident plus 8 streamed voices, 128 frames | median 3.591 µs per callback |
| `740c306b` + Phase 7 working tree | Decode: one-second stereo WAV at 48 kHz | median 154.07 µs |
| `db226cc2` + Phase 8 working tree | Metering: 256 channels, 128 frames | median 105.87 µs |
| `db226cc2` + Phase 8 working tree | YIN: 2,048-frame pitch window | median 590.42 µs on analysis worker |
| `db226cc2` + Phase 8 working tree | Real FFT: 256 frames | median 1.715 µs on analysis worker |
| `db226cc2` + Phase 8 working tree | Real FFT: 2,048 frames | median 5.688 µs on analysis worker |
| `db226cc2` + Phase 8 working tree | Real FFT: 16,384 frames | median 38.662 µs on analysis worker |

The warmed render allocation guard observed zero allocations, zero deallocations, and zero net
bytes. Phase 14 still owns the full reference workload, queue-pressure, memory, soak, and callback
deadline qualification.

## Phase 1 implementation

The new `crates/golden_audio` workspace crate provides:

- strong UUID-backed virtual channel, route, and analysis identities;
- validated backend, device, physical-channel, and playback string identities;
- monotonic configuration/command generations and generational voice IDs;
- validated sample rates, frame counts, decibel gains, engine configuration, and centralized
  `EngineLimits`;
- authored backend-neutral channel, route, device-selection, and analysis configuration;
- structured public errors, diagnostics, commands, events, and coalesced observations;
- app-agnostic device/stream/inspector DTOs with optional Rust-to-TypeScript code generation;
- public backend and stream traits, a deterministic null backend, and a controllable mock backend;
- an offline sample clock; and
- a bounded nonblocking control producer, owned control worker, single event receiver, clean
  idempotent shutdown, and last-valid-generation behavior.

The Phase 1 control worker deliberately does not render or decode. Playback requests produce a
structured unsupported-foundation failure until the playback phase. Callback ownership and the
plan exchange replace the current worker-side synchronization in Phase 3; no callback exists in
Phase 1.

## Phase 2 implementation

The backend-independent render core now provides:

- bounded preallocated planar scratch buffers and chunked processing for oversized callbacks;
- deterministic route compilation with stable channel ordering, destination-major route spans,
  unresolved-endpoint warnings, and explicit physical/playback route sources;
- stateful per-route, per-channel, and master gain ramps;
- the required signal order: input patch, monitoring and playback mix, virtual-output faders,
  master gain, then physical-output patch;
- non-finite containment and saturating interleaved conversion for `f32`, `f64`, signed and
  unsigned 16/24/32-bit boundary formats;
- an offline renderer and a deliberately simple scalar reference implementation used for
  sample-exact comparison;
- release-mode render/routing benchmarks and a source-level realtime review checklist; and
- tests covering deterministic compilation, ordering, ramps, conversion boundaries, oversized
  callbacks, offline timing, and warmed callback allocation/deallocation behavior.

## Phase 3 implementation

The callback/control ownership boundary now provides:

- a bounded `rtrb` application-to-control ingress with a cloneable, nonblocking `try_lock` producer,
  a parked control worker, monotonic command sequences, explicit saturation errors, and structured
  command-queue pressure events;
- generation ordering that rejects stale configurations and compiles each candidate before
  replacing the last valid generation;
- an acknowledged single-pending-plan exchange that swaps only at block boundaries, returns old
  plans to the control thread, and retains a retired plan in a fixed callback slot when the return
  queue is unexpectedly full;
- atomic coalescing gain mailboxes plus fixed-capacity play, stop, stop-all, and plan-swap sequence
  barriers that prevent high-rate parameters crossing ordered operations;
- fixed generational voice slots whose asset payloads are returned through a bounded queue and
  retained in-place under saturation;
- preallocated analysis-frame ownership circulating through free and ready SPSC queues;
- shared allocation-free pressure counters for every callback boundary and a debug/test
  `RealtimeScope` guard that rejects control-thread reclamation from callbacks; and
- controlled retirement values that transfer final plan, voice, and analysis ownership for
  destruction away from callback threads.

The crate still forbids all locally authored unsafe code. Miri/sanitizer coverage was therefore not
required by the Phase 3 conditional gate.

## Phase 4 implementation

The backend-neutral device layer now provides:

- explicit durable-ID versus fallback-fingerprint identity, last-known labels, deterministic
  profile keys, and ambiguity reporting that never guesses between duplicate devices;
- physical-channel descriptors and validated supported stream configuration ranges for each
  direction, format, channel count, sample-rate range, and buffer range;
- deterministic negotiation with strict or preference policies for channel count, sample rate,
  sample format, and buffer size, including structured unsupported-request context;
- generic serializable device-profile storage keyed independently of device labels or enumeration
  order;
- independent input/output supervisor state, explicit missing/busy/permission/unavailable states,
  deterministic exponential backoff with bounded jitter, and prepare/prime/switch/commit phases;
- strict selected-device recovery and separately explicit operating-system-default following;
- descriptor-change detection that re-enters preparation without replacing the active stream
  before commit;
- a richer mock backend with ordered connect, disconnect, default-change, format-change,
  server-restart, and flapping events plus controllable busy and permission failures; and
- a canonical action-free `AudioDeviceInspectorState` projection with all referenced TypeScript
  DTOs generated from Rust.

`DirectionConfiguration` now owns an `AudioDeviceSelection` instead of a bare target. This is the
intentional schema boundary that keeps fallback identity, last-known label, and profile key together
with the persisted selection.

## Phase 5 implementation

The private native-host layer now provides:

- a CPAL 0.18.1 adapter that maps compiled hosts, durable device IDs, structured descriptions,
  defaults, physical channels, supported configurations, negotiated streams, timestamps, and
  structured errors into Golden contracts without exposing a CPAL type;
- backend-neutral `AudioStreamHandler` callbacks for input and output, pre-silenced output, fixed
  atomic runtime-error publication, and stream-owned preallocated conversion scratch for CPAL's
  24-bit wrapper formats;
- integer and floating-point callback conversion for i8/i16/i24/i32/i64, u8/u16/u24/u32/u64, f32,
  and f64; DSD is deliberately not advertised because the current PCM engine has no valid DSD
  representation;
- compiled-host and runtime-capability probing that distinguishes native availability,
  missing-server JACK/PipeWire, missing-driver ASIO, and host failures without opening a stream;
- a separate explicit device smoke example that opens the default output, renders silence for
  100 ms, and stops cleanly;
- safe feature bundles for native desktop, ASIO, JACK, native PipeWire, real-time priority/DBus, and
  the full desktop qualification set;
- canonical audio toolchain metadata plus Windows ASIO prerequisite checks and Linux
  ALSA/JACK/PipeWire/DBus prerequisite checks; and
- a three-platform CI host job, bounded non-checkout ASIO SDK cache, and matching Linux
  release/product-gate packages.

Normal desktop builds intentionally keep the native OS host as the default. Enabling ASIO by
default would make an otherwise clean Windows checkout fail before bootstrap when LLVM/Clang is
absent. The official full host set is therefore a named qualification feature exercised by CI and
release gates; application code still has one ordinary `golden_audio` dependency and never selects
CPAL features.

## Phase 6 implementation

The backend-neutral clock and recovery layer now provides:

- a bounded interleaved input ring with whole-callback admission, explicit overflow and disconnect
  results, timestamp-loss and discontinuity counters, and atomically sampled observations;
- a preallocated Rubato asynchronous polynomial resampler using `process_into_buffer`, with the
  configured engine rate kept stable while device rates change;
- a bounded proportional-integral drift controller with configurable target fill, gains, integral
  clamp, and correction limit;
- input latency estimates that combine observed ring fill, resampler delay, and configured output
  buffering without querying devices from the render path;
- a deadline-based null-clock driver with bounded catch-up;
- one render-clock coordinator that admits exactly one authoritative null or output source, ignores
  non-authoritative callbacks, advances a single monotonic sample timeline, and emits sample-accurate
  handoff gains;
- prepare/prime-compatible output replacement with fade-down, block-boundary authority switch,
  fade-up, and explicit retired-stream accounting; and
- immediate promotion of an already primed replacement if the old output disappears during handoff.

Device-rate reconfiguration is a control-thread operation: it rebuilds and preallocates the
resampler, flushes samples from the old clock domain, resets drift history, records a
discontinuity, and leaves the engine sample rate and timeline unchanged. Output loss immediately
moves authority to the null clock unless a primed replacement is ready, so active playback state
does not retrigger.

## Phase 7 implementation

The playback layer now provides:

- one Rust-owned format registry for WAV, AIFF, CAF, FLAC, MP3, MP4/M4A, Ogg, Matroska, and WebM
  families, with the same extension union emitted by TypeScript code generation;
- private Symphonia probe/decode sessions with structured corrupt, truncated, unsupported, and
  cancellation errors, plus worker-side conversion to the configured engine rate;
- a fixed decoder-worker pool with bounded request/result queues, per-ID and stop-all cancellation
  watermarks, and stale-result rejection for same-ID replacement;
- immutable planar resident assets and an off-realtime cache with fingerprint invalidation, exact
  byte accounting, threshold and total-budget enforcement, deterministic eviction, and active
  `Arc` safety;
- bounded streaming read-ahead rings that are primed before activation, recover from starvation,
  and share cancellation and terminal status with decoder workers;
- fixed generational voice slots with explicit mono, stereo, and multichannel routing, preallocated
  stream scratch, sample-accurate start/stop ramps, and control-thread reclamation;
- public engine integration that orders play, replacement, stop, and stop-all with the existing
  command sequence while publishing lifecycle events only for the surviving generation; and
- sustained playback and decoder benchmarks whose setup and teardown are outside measured callback
  time.

The render path never probes, opens, reads, decodes, resamples, mutates the cache, allocates, or
drops the final asset reference. Output loss leaves the same voice playhead advancing under the
authoritative null clock; restoring output does not enqueue or restart playback.

## Phase 8 implementation

The analysis layer now provides:

- render-plan compilation from stable tap and virtual-input IDs to compact source indices;
- callback-side virtual-input RMS/peak before monitoring and virtual-output RMS/peak after
  output/master gain, with explicit millisecond windows and independent 1–60 Hz publication;
- atomic seqlock meter banks that publish linear RMS, RMS dBFS, peak dBFS, clip state, input/output
  maxima, and the combined global maximum without callback locks or allocation;
- a bounded frame pool with one extra newest-frame retention slot, so worker overload replaces stale
  pending work and never delays rendering;
- one dedicated analysis worker with preallocated YIN difference/input scratch and preplanned
  RealFFT input, output, window, and scratch storage;
- pitch results with validity, frequency, confidence, MIDI note, note name, and cents;
- Hann and Blackman-Harris FFT windows, 0/50/75-percent overlap, normalized single-sided linear or
  logarithmic bands, Nyquist clipping, and attack/release smoothing;
- generation-fixed latest observation snapshots and atomically independent analyzer enablement; and
- captured, processed, dropped, stale, total worker-time, and maximum worker-time diagnostics.

The callback allocation guard crosses both an RMS-window completion and observation publication
with zero allocation or deallocation. Worker-overload tests fill the frame queue and retain the
newest pending frame while the render call continues successfully. Disabling a tap clears its
result and stops capture without changing the render plan or device state.

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
| `cargo check -p golden_audio --no-default-features --all-targets` | PASS |
| `cargo test -p golden_audio --no-default-features` | PASS — 12 tests (11 unit, 1 external-style integration), 0 failed |
| `cargo clippy -p golden_audio --no-default-features --all-targets -- -D warnings` | PASS |
| `cargo check -p golden_audio --features codegen --all-targets` | PASS |
| `cargo run -p golden_audio --features codegen --bin generate_golden_audio_contract -- target/golden-audio-contract-phase1` | PASS — generated 20 TypeScript files in ignored build output |
| `cargo tree -p golden_audio --no-default-features --edges normal` | PASS — only serde, thiserror, UUID, and their transitive dependencies; no Golden Core or Chataigne dependency |
| Root and Golden Core `cargo fmt` plus `--check` | PASS after Phase 1 |
| `cargo info allocation-counter@0.8.1 criterion@0.8.2` | PASS — versions, licenses, and declared Rust compatibility recorded; both are dev-only |
| `cargo test -p golden_audio --no-default-features` after Phase 2 | PASS — 27 tests (25 unit, 2 integration), 0 failed |
| Phase 2 scalar/reference comparison | PASS — sample exact for input patch, monitoring, playback, faders, master, and output patch |
| Phase 2 warmed render allocation guard | PASS — 0 allocations, 0 deallocations, 0 bytes |
| `cargo bench -p golden_audio --no-default-features --no-run` | PASS |
| Phase 2 quick render/routing benchmarks | PASS — results recorded above |
| First routing quick benchmark | FAIL then fixed — requested 0 and 16 routes produced the same Criterion ID; IDs now retain the requested route count and the complete rerun passed |
| `cargo deny check` after GPL metadata correction | PASS after fixing the initial failure — `deny.toml` had not allowed the workspace's `GPL-3.0-only` license |
| `cargo machete` | FAIL — only six pre-existing unused dependencies in `Chataigne2`; no `golden_audio` dependency was reported |
| Root and Golden Core formatting plus `--check` after Phase 2 | PASS |
| `cargo test -p golden_audio --no-default-features` after Phase 3 | PASS — 46 tests (43 unit, 3 integration), 0 failed |
| Phase 3 plan stress | PASS — 1,000,000 acknowledged swaps; 1,000,001 balanced drops; 0 callback drops |
| Phase 3 gain stress | PASS — 1,000,000 updates coalesced to the final sequence without queue growth |
| Phase 3 callback ownership allocation guard | PASS — plan swap, voice retirement, and analysis transfer observed 0 allocations, 0 deallocations, and 0 bytes |
| Phase 3 full-return defensive path | PASS — old plan retained, no callback drop, later acknowledgement reclaimed it off callback |
| Phase 3 ordering tests | PASS — gain changes remain on the correct side of play/stop and plan-swap barriers |
| Phase 3 bounded queue/producer disconnect tests | PASS — explicit full error/counter, FIFO order, and abandoned producer visibility |
| Phase 3 Miri/sanitizer conditional gate | NOT APPLICABLE — no unsafe code was introduced and `golden_audio` retains `#![forbid(unsafe_code)]` |
| Phase 3 `cargo check` and warning-free Clippy, no default features/all targets | PASS |
| Phase 3 codegen-feature check | PASS |
| Phase 3 `cargo deny check` | PASS — advisories, bans, GPLv3 license policy, and sources |
| Root and Golden Core formatting plus `--check` after Phase 3 | PASS |
| `cargo test -p golden_audio --no-default-features` after Phase 4 | PASS — 58 tests (54 unit, 4 integration), 0 failed |
| Phase 4 stable-ID/fallback tests | PASS — rename/re-enumeration retained identity; unique fallback matched; duplicates reported ambiguous |
| Phase 4 selection/recovery tests | PASS — strict missing retained selection, follow-default tracked only OS default, input/output remained independent |
| Phase 4 negotiation tests | PASS — capability enumeration order did not affect selection; unsupported fixed request returned structured `UnsupportedFormat` |
| Phase 4 mock recovery tests | PASS — connect, disconnect, busy, permission denied, format change, server restart, and flapping |
| Phase 4 TypeScript generation | PASS — 25 generated files and every `index.ts` export target exists |
| Phase 4 inspector projection test | PASS — canonical state serializes data only, with no action fields |
| Phase 4 no-default/all-target check and warning-free Clippy | PASS |
| Phase 4 `cargo deny check` | PASS — advisories, bans, GPLv3 license policy, and sources |
| Root and Golden Core formatting plus `--check` after Phase 4 | PASS |
| `cargo test -p golden_audio --no-default-features` after Phase 5 | PASS - 60 tests (55 unit, 5 integration), 0 failed |
| `cargo test -p golden_audio` after Phase 5 | PASS - 63 tests (58 unit, 5 integration), 0 failed |
| Phase 5 Windows native-host probe | PASS - WASAPI available; no stream opened |
| Phase 5 Windows JACK compile and missing-server probe | PASS - dynamic JACK compiled and an empty host reported `MissingServer` without startup failure |
| Phase 5 WASAPI output smoke | PASS - default 2-channel output opened, started at 48 kHz, rendered silence for 100 ms, and stopped |
| Phase 5 callback format coverage | PASS - all PCM CPAL formats mapped; packed 24-bit conversion remains preallocated and private; DSD excluded |
| Phase 5 core-only dependency tree | PASS - CPAL and native dependencies absent with `--no-default-features` |
| Phase 5 default dependency tree | PASS - CPAL is private to `golden_audio` |
| Phase 5 TypeScript generation | PASS - 25 generated files, expanded sample-format union, and no missing index targets |
| Phase 5 no-default and default warning-free Clippy | PASS |
| Phase 5 `cargo deny check` | PASS - advisories, bans, GPLv3 license policy, and sources; existing workspace warnings remain non-fatal |
| Phase 5 toolchain/script/workflow validation | PASS - JSON, PowerShell parse, normalized shell syntax, and all workflow YAML |
| Root and Golden Core formatting plus `--check` after Phase 5 | PASS |
| Windows ASIO compile/runtime | NOT RUN - local LLVM/Clang and verified SDK archive identity are absent; CI job owns the compile gate |
| macOS CoreAudio/JACK qualification | NOT RUN - no exact-commit remote result yet |
| Linux ALSA/JACK/native-PipeWire/realtime-DBus qualification | NOT RUN - no exact-commit remote result yet |
| `cargo test -p golden_audio --no-default-features` after Phase 6 | PASS - 74 tests (68 unit, 6 integration), 0 failed |
| Phase 6 input callback allocation guard | PASS - 100 warmed Rubato/ring write-read blocks, 0 allocations, 0 deallocations, 0 bytes |
| Phase 6 drift simulations | PASS - stable convergence from -1,000 through +1,000 ppm without tail oscillation |
| Phase 6 discontinuity qualification | PASS - underflow, overflow, timestamp loss, discontinuity, and abrupt 48 to 44.1 kHz change remain bounded and observable |
| Phase 6 clock handoff qualification | PASS - output loss to null, reconnect, primed replacement, loss during handoff, and 10,000 switches preserve one monotonic timeline |
| Phase 6 mock soak | PASS - one simulated hour at 48 kHz/128 frames with 10,000-block loss/reconnect cadence and exact frame growth |
| Phase 6 no-default/default checks and warning-free Clippy | PASS |
| Phase 6 `cargo deny check` | PASS - advisories, bans, GPLv3 license policy, and sources; existing workspace warnings remain non-fatal |
| Root and Golden Core formatting plus `--check` after Phase 6 | PASS |
| Phase 6 real backend 30-minute drift/reconnect soaks | NOT RUN - exact-commit hardware/backend qualification remains a Phase 14 gate |
| `cargo test -p golden_audio --no-default-features` after Phase 7 | PASS - 87 tests (81 unit, 6 integration), 0 failed |
| `cargo test -p golden_audio` after Phase 7 | PASS - 102 tests (93 unit, 9 integration), 0 failed |
| Phase 7 real-format fixtures | PASS - every advertised extension maps to and decodes an actual WAV, AIFF, CAF, FLAC, MP3, MP4/M4A, Ogg, Matroska, or WebM fixture |
| Phase 7 ordering and cancellation | PASS - pending and active same-ID replacement, pending stop, stop-all, rapid play/stop, and stale worker results |
| Phase 7 cache and streaming | PASS - cache hit, fingerprint invalidation, eviction, budget enforcement, bounded read-ahead, starvation, and recovery |
| Phase 7 callback ownership | PASS - resident rendering allocates and deallocates nothing; final asset and queued activation ownership return to control |
| Phase 7 TypeScript generation | PASS - 27 generated files, extension metadata emitted, and no missing index export target |
| Phase 7 sustained quick benchmarks | PASS - 1/16/64/128/256 resident voices, mixed resident/streamed voices, and decoder throughput recorded above |
| Phase 7 no-default/default warning-free Clippy and playback benchmark compile | PASS |
| Phase 7 core-only dependency tree | PASS - Symphonia absent with `--no-default-features` and private in the default build |
| Phase 7 `cargo deny check` | PASS - advisories, bans, GPLv3 license policy, and sources; existing workspace warnings remain non-fatal |
| Phase 7 `cargo machete` | FAIL - the same six pre-existing unused dependencies remain in `Chataigne2`; none belong to `golden_audio` |
| `cargo test -p golden_audio --no-default-features` after Phase 8 | PASS - 91 tests (85 unit, 6 integration), 0 failed |
| `cargo test -p golden_audio` after Phase 8 | PASS - 120 tests (108 unit, 12 integration), 0 failed |
| Phase 8 meter qualification | PASS - exact DC, sine, square, silence, multichannel, dBFS-floor, clipping, callback-partition, and input/output signal-point tests |
| Phase 8 pitch qualification | PASS - tones, harmonics, detuning, MIDI/note/cents, silence, noise, low amplitude, and chirp bounds |
| Phase 8 spectrum qualification | PASS - bin-centered/off-bin tones, Hann/Blackman-Harris, linear/log geometry, Nyquist clipping, overlap, normalization, smoothing, and size changes |
| Phase 8 overload and topology | PASS - newest-frame retention, dropped-frame diagnostics, uninterrupted render, independent disable, and generation-isolated snapshots |
| Phase 8 callback allocation guard | PASS - RMS completion and 30 Hz publication observed 0 allocations, 0 deallocations, and 0 bytes |
| Phase 8 TypeScript generation | PASS - 45 generated files, all analysis DTOs emitted from Rust, and no missing index export target |
| Phase 8 quick benchmarks | PASS - 256-channel metering, YIN, and 256/2,048/16,384-frame FFT results recorded above |
| Phase 8 no-default/default warning-free Clippy and analysis benchmark compile | PASS |
| Phase 8 core-only dependency tree | PASS - RealFFT absent with `--no-default-features` and private in the default build |
| Phase 8 `cargo deny check` | PASS - advisories, bans, GPLv3 license policy, and sources; existing workspace warnings remain non-fatal |
| Phase 8 `cargo machete` | FAIL - the same six pre-existing unused dependencies remain in `Chataigne2`; none belong to `golden_audio` |
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

Phase 1:

- `Cargo.toml`
- `Cargo.lock`
- `crates/golden_audio/Cargo.toml`
- `crates/golden_audio/README.md`
- `crates/golden_audio/src/`
- `crates/golden_audio/tests/public_api.rs`
- `crates/golden_audio/examples/null_offline.rs`

Phase 2:

- `Cargo.toml`
- `Cargo.lock`
- `deny.toml`
- `crates/golden_audio/Cargo.toml`
- `crates/golden_audio/src/lib.rs`
- `crates/golden_audio/src/render/`
- `crates/golden_audio/tests/offline_render.rs`
- `crates/golden_audio/benches/`

Phase 3:

- `Cargo.toml`
- `Cargo.lock`
- `crates/golden_audio/Cargo.toml`
- `crates/golden_audio/src/control/`
- `crates/golden_audio/src/lib.rs`
- `crates/golden_audio/src/realtime/`
- `crates/golden_audio/src/render/REALTIME_REVIEW.md`
- `crates/golden_audio/src/tests/engine.rs`
- `crates/golden_audio/tests/realtime_contract.rs`

Phase 4:

- `crates/golden_audio/src/backend/`
- `crates/golden_audio/src/config.rs`
- `crates/golden_audio/src/contract.rs`
- `crates/golden_audio/src/device/`
- `crates/golden_audio/src/ids.rs`
- `crates/golden_audio/src/lib.rs`
- `crates/golden_audio/src/tests/backend.rs`
- `crates/golden_audio/tests/device_contract.rs`

Phase 5:

- `Cargo.toml`
- `Cargo.lock`
- `crates/golden_audio/Cargo.toml`
- `crates/golden_audio/examples/backend_probe.rs`
- `crates/golden_audio/examples/backend_smoke.rs`
- `crates/golden_audio/src/backend/cpal/`
- `crates/golden_audio/src/backend/traits.rs`
- `crates/golden_audio/src/device/`
- `crates/golden_audio/src/render/convert.rs`
- `crates/golden_audio/tests/public_api.rs`
- `tools/bootstrap/toolchain.json`
- `tools/dev.ps1`
- `tools/dev.sh`
- `.github/workflows/ci.yml`
- `.github/workflows/product-gate.yml`
- `.github/workflows/release.yml`
- `docs/architecture/golden-audio.md`
- `docs/reference/toolchain.md`

Phase 6:

- `Cargo.toml`
- `Cargo.lock`
- `crates/golden_audio/Cargo.toml`
- `crates/golden_audio/src/lib.rs`
- `crates/golden_audio/src/clock/`
- `crates/golden_audio/tests/clock_contract.rs`
- `docs/architecture/golden-audio.md`
- `docs/progress/golden-audio-sound-card.md`

Phase 7:

- `Cargo.toml`
- `Cargo.lock`
- `crates/golden_audio/Cargo.toml`
- `crates/golden_audio/README.md`
- `crates/golden_audio/src/contract.rs`
- `crates/golden_audio/src/control/engine.rs`
- `crates/golden_audio/src/lib.rs`
- `crates/golden_audio/src/limits.rs`
- `crates/golden_audio/src/playback/`
- `crates/golden_audio/src/realtime/ownership.rs`
- `crates/golden_audio/tests/playback_ordering.rs`
- `crates/golden_audio/benches/playback.rs`
- `docs/architecture/golden-audio.md`
- `docs/progress/golden-audio-sound-card.md`

Phase 8:

- `Cargo.toml`
- `Cargo.lock`
- `crates/golden_audio/Cargo.toml`
- `crates/golden_audio/README.md`
- `crates/golden_audio/benches/analysis.rs`
- `crates/golden_audio/src/analysis/`
- `crates/golden_audio/src/config.rs`
- `crates/golden_audio/src/contract.rs`
- `crates/golden_audio/src/control/observation.rs`
- `crates/golden_audio/src/ids.rs`
- `crates/golden_audio/src/lib.rs`
- `crates/golden_audio/src/realtime/`
- `crates/golden_audio/src/render/`
- `crates/golden_audio/tests/synthetic_analysis.rs`
- `docs/architecture/golden-audio.md`
- `docs/progress/golden-audio-sound-card.md`

## Remaining work

Phases 9-15 remain. The immediate next gate is the Chataigne-owned Sound Card module schema,
backend-neutral `golden_audio` dependency, stable node identity mapping, authoring defaults, and
persistence round trips.
