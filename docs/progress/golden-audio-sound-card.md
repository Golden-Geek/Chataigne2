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

- Current phase: Phase 14 - performance, robustness, and product evidence.
- Status: deterministic implementation, local qualification, the Windows WASAPI real-device
  endurance gate, and bundled release headless startup are complete; mounted browser inspection
  and cross-platform results remain outstanding.
- Phase 9 checkpoint: `de2bfdf5` (`feat(chataigne): add persistent Sound Card module model`).
- Phase 10 checkpoint: `ddf380a5` (`feat(chataigne): connect Sound Card nodes to golden_audio`).
- Phase 11 checkpoint: `2b66ee79` (`feat(chataigne): expose Sound Card commands and scripting`).
- Phase 12 checkpoint: `2db280bf` (`feat(audio-ui): add reusable Golden audio device inspector`).
- Phase 13 checkpoint: `0bb953b1` (`feat(ui): add scalable Sound Card editor`).
- Phase 14 checkpoint: this change (`perf(audio): add Sound Card runtime qualification`).
- Phase 14 managed-device qualification checkpoint: this change
  (`fix(audio): harden managed device recovery`).
- Phase 14 analysis endurance checkpoint: `f6661d6c`
  (`fix(audio): preserve analysis frames through scheduler stalls`).
- Phase 14 bundled-host checkpoints: `85e6cc36`
  (`fix(host): preserve discovery routes with bundled UI`) and `a245641b`
  (`fix(host): avoid bundled headless self-probe`).
- Stop boundary: Phase 14 cannot be marked fully complete until mounted normal/narrow browser
  inspection and cross-platform runs have exact-commit evidence.

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
`CPAL_ASIO_DIR`; LLVM/Clang and Visual C++ are build prerequisites. The official `audiosdk/asio`
source is pinned by full Git revision in `tools/bootstrap/toolchain.json`, and one shared local/CI
resolver validates the exact SDK layout before exporting it to the build. The repository already
contains the GPLv3 license text, and the project owner confirmed that the entire workspace is GPLv3.
Workspace Rust and npm package metadata now consistently use the SPDX identifier `GPL-3.0-only`.
Steinberg's currently published open-source ASIO SDK is also GPLv3, so the earlier MIT
incompatibility concern does not apply.

Decision:

- implement and qualify the ASIO backend under the GPLv3 path without checking SDK archives into
  this repository;
- resolve the pinned official Git source into the bounded external tool cache and expose
  `CPAL_ASIO_DIR` only to the build;
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
| Final render-plan destruction on callback | One-pending-plan acknowledged exchange plus retained retired-plan slot; allocation/deallocation guard | Mitigated; deterministic ownership qualification PASS |
| Independent input/output clock drift | Bounded ring, adaptive ASRC, PI controller, discontinuity fade, drift/bridge observations | Mitigated; backend-neutral qualification PASS |
| Decoder completion after stop/replacement | Monotonic command sequence and cancellation generation watermarks; stale worker results discarded off callback | Mitigated; deterministic ordering suite PASS |
| Large meter/matrix/spectrum UI | Packed latest-only telemetry, Canvas, viewport virtualization, bounded refresh, teardown tests | Open |
| Legacy unmapped gitlinks | Do not use or modify them; record pre-existing tooling failure | Open, unrelated |

## Backend qualification

| Platform | Backend | Build | Discovery | Stream I/O | Recovery | Package/startup |
| --- | --- | --- | --- | --- | --- | --- |
| Windows x64 | WASAPI | PASS | PASS | PASS - default-output open/start/100 ms silence/stop smoke | PASS - exact `f6661d6c` release build sustained the medium workload for one hour through 5 planned stop/reopen cycles with 0 warnings, XRuns, deadline misses, bridge pressure, playback failures, or analysis drops | PARTIAL - exact `a245641b` unsigned release bundled-headless startup passed; desktop launch and signed install/uninstall remain `NOT RUN` |
| Windows x64 | ASIO | PASS - pinned `audiosdk/asio` source compiled through CPAL 0.18.1 / `asio-sys` 0.3.0 | PASS - local probe reports ASIO available alongside WASAPI | PASS - default 2-channel output opened, started at 48 kHz, rendered silence for 100 ms, and stopped | NOT RUN | NOT RUN |
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

| `0bb953b1` + Phase 14 working tree | Combined small reference workload | median 8.659 microseconds |
| `0bb953b1` + Phase 14 working tree | Combined medium reference workload | median 42.467 microseconds |
| `0bb953b1` + Phase 14 working tree | Combined large reference workload | median 197.02 microseconds |
| `0bb953b1` + Phase 14 working tree | Combined extreme-offline reference workload | median 1.1588 ms |

The warmed render allocation guard observed zero allocations, zero deallocations, and zero net
bytes. The Phase 14 combined harness extends that guard across routing, resident playback, meters,
pitch capture, and spectrum capture.

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

## Phase 9 implementation

The Chataigne-owned Sound Card model now provides:

- one app dependency on `golden_audio.workspace` without an app-selected backend feature list;
- a generated `Audio / Sound Card` module catalog item, app-owned child schemas, and exactly five
  generated command node types exposed by its `ModuleCommandTester`;
- correctly separated authored containers, removable/duplicable channels, profiles, routes, and
  analyzers, plus read-only derived meter and analysis result structures;
- two default virtual inputs, two default virtual outputs, one-to-one default input/output device
  profiles, meter projections, and pitch/spectrum analyzer outputs materialized in batched
  `NodeTree` operations;
- stable authored UUID identity across rename and reorder, fresh identity on duplication, and
  deterministic UUIDv5 identities for rebuildable meter, analyzer-result, and spectrum-band
  projections;
- same-Sound-Card reference filters for virtual input/output routes and channel-volume commands;
- event-driven derived-structure repair with no Phase 9 periodic poll or audio runtime;
- sparse project persistence for authored profiles, routes, identity, gain, and missing device
  selections, including a tagged missing enum choice after production-style reload preparation;
  and
- a fixture-backed lifecycle suite covering creation, save/reload, duplication, removal, projection
  repair, null-backend readiness, generated catalog membership, command scoping, and cross-module
  reference rejection.

Phase 9 deliberately stops at the persistent app model. It does not construct an audio engine,
open a native backend, poll live values, or execute the five command nodes.

## Phase 10 implementation

The Chataigne-owned runtime adapter now provides:

- one `golden_audio::AudioEngine` lifecycle per live Sound Card module, using every compiled native
  backend plus the deterministic null backend, with explicit shutdown on module removal, project
  replacement, and drop;
- dirty-tree conversion into backend-neutral `AudioConfiguration` values, stable UUID-derived
  channel/route/tap identities, device-profile selection by stable profile key, and one coalesced
  configuration generation per stabilization batch;
- last-valid-plan behavior: invalid foreign references reject the replacement, while missing
  devices and dangling local references keep authored topology intact and surface node warnings;
- Golden-owned periodic discovery, device supervision, stream open/start/stop, recovery status, and
  active compiled-plan ownership on the control worker;
- live device choices that add discovered targets without replacing a persisted missing enum value;
- cached runtime `NodeId` bindings, 30 Hz observation polling, epsilon-filtered parameter writes,
  readiness and data-capability projection, and no steady-state process-tree or state-machine
  snapshot rebuild;
- read-only meter, analyzer, playback, stream, and diagnostic value projection from the coalesced
  Golden observation;
- an app-owned latest-only `chataigne.sound_card.telemetry` envelope whose embedded device and
  analysis contracts, plus the envelope itself, are generated from Rust into TypeScript; and
- an unregistered Svelte 5 `ChataigneAudioDeviceInspectorAdapter` that translates the future
  reusable inspector binding into ordinary `golden_ui` parameter intents. Registration remains a
  Phase 12 responsibility.

Phase 10 deliberately stops before command execution, multiplex admission, script functions,
callbacks, snippets, and templates. Those remain Phase 11 work.

## Phase 11 implementation

The Chataigne-owned command and scripting boundary now provides:

- executable manual, auto-triggered, external-target, and transient multiplex command paths for
  Play File, Stop File, Stop All Files, Set Master Volume, and Set Channel Volume;
- effective-snapshot extraction for every command parameter, so each multiplex lane supplies its
  own file path, playback ID, virtual-output reference, and gain without mutating the authored
  command node;
- typed bounded admission into `golden_audio`, ordered same-ID replacement and cross-ID
  independence, and a structured `chataigne.sound_card.command.result` event for admitted
  sequences or typed failures;
- app-side virtual-output validation that rejects inputs, foreign modules, deleted nodes, empty
  references, and physical-channel strings before crossing the Golden boundary;
- active master/output gain handling on the Golden control worker without render-plan
  recompilation, plus structured diagnostics if a stale target reaches the worker;
- script descriptors and host dispatch for `playFile`, `stopFile`, `stopAllFiles`,
  `setMasterVolume`, and `setChannelVolume`, including stable virtual-output UUID tokens;
- playback lifecycle plus device/backend status callbacks with documented argument shapes;
- transient playback callback delivery, which stays inside the live engine inbox and cannot enter
  UI replay or transport resynchronization; and
- an app-owned Sound Card script template, comment-only function/callback snippets, and scripting
  guide coverage.

Phase 11 deliberately leaves reusable device-inspector presentation and registration to Phase 12.

## Phase 12 implementation

The reusable UI boundary now provides:

- a `golden_audio_ui` npm workspace package whose generated TypeScript device contract comes
  directly from the Rust `golden_audio` DTOs and has a deterministic drift check;
- `AudioDeviceSelector` with separate input/output enablement and backend-grouped native selectors,
  stable and persisted-missing targets, backend/readiness/permission presentation, negotiated
  format and latency summaries, shared recovery/sample-rate/buffer controls, and nonblocking
  refresh;
- structured diagnostics with expandable technical detail, visible focus, semantic labels, native
  keyboard interaction, and live status announcements without color-only meaning;
- a generic binding contract plus a reusable declared-path/node-ID parameter adapter that emits
  ordinary `golden_ui` parameter intents and contains no device or negotiation policy;
- explicit exact-node-type registration, unregister, binding resolution, and deterministic test
  reset helpers with no import-time registry side effects;
- a Chataigne-independent mock adapter and standalone consumer;
- a narrow `golden_ui` default-child filter hook so a custom inspector can render app children
  while omitting only the parameter folder represented by the custom presentation; and
- Chataigne registration for `sound_card_module`, using the generic parameter binding to map the
  existing connection folder and hiding that one duplicated folder below the reusable selector.

Phase 12 deliberately leaves the module-editor descriptor registry, full Sound Card editor,
matrices, meters, playback controls, analysis controls, and product diagnostics to Phase 13.

## Phase 13 implementation

The app-specific editor boundary now provides:

- one Chataigne-owned module-editor descriptor registry used by both the module inspector header
  and dock panel definitions, with Spatializer migrated to stable per-module panel identity and no
  product branch added to `golden_ui`;
- a focused Svelte 5 `SoundCardEditorPanel` that composes the Phase 12 `AudioDeviceSelector` and
  keeps module/profile selection in dock panel state;
- generic-inspector-backed virtual input/output authoring, reorder, rename, output faders, and
  master volume, so all persistent edits continue through public Golden UI intents;
- active device-profile history plus physical-input, physical-output, monitoring, and playback
  sparse matrices whose Canvas projection scales independently of matrix area;
- atomic `CreateUserItem` route creation with source, destination, and gain `initial_params`,
  existing-gain `SetParam`, route `RemoveNode`, grouped pointer painting through
  `BeginEdit`/`EndEdit`, acknowledgement-keyed optimistic state, and rejection rollback;
- authored monitoring visibility while input is disabled, with inactive signal flow communicated
  in text and styling;
- frame-coalesced Canvas meters and spectrum, semantic meter/table fallbacks, pitch/spectrum
  observations, analysis authoring, available diagnostics, and explicit ResizeObserver/animation
  frame teardown;
- an app-owned playback lifecycle projection generated from Rust, read-only active voice rows,
  and ephemeral stop-one/stop-all control events that do not edit project state;
- a deterministic mock Sound Card evidence harness covering devices, routes, meters, playback,
  pitch, spectrum, and diagnostics without hardware; and
- tests for descriptor registration and panel identity, atomic route intents, grouped edit
  sessions, gain/removal paths, undo/redo, inactive monitoring, packed telemetry rendering,
  256-by-256 matrix DOM bounds, playback control routing, and Canvas frame cancellation.

The in-app browser surface was unavailable during this revision, so mounted visual inspection at
normal and narrow desktop sizes is `NOT RUN`. Phase 14 still owns reference-workload profiling,
the additional underrun/overrun/starvation/drift/bridge/render diagnostics, hardware/backend soaks,
cross-platform qualification, and mounted product evidence.

## Phase 14 implementation

The reusable engine now provides a ready-to-run managed render path in addition to its existing
external-callback surface. Chataigne opts into that path when it creates a Sound Card engine.
Golden Audio owns the render worker, acknowledged render/stream bridge swaps, playback renderer,
analysis controller, and device callback handlers. Native input callbacks write to the bounded
adaptive clock bridge; native output callbacks drain a bounded prefilled queue and wake the render
worker. When no callback consumes output, the same worker advances from the paced null clock so
playback, meters, and analysis retain one monotonic timeline.

The callback bridges allocate, compile, decode, and destroy nothing. Input/output bridge and render
plans retire through acknowledged exchanges and are reclaimed on the control thread. Integration
tests drive deterministic callback-backed input and output streams: one proves decoded playback
reaches the backend output callback, one proves synthetic backend input reaches monitoring,
input/output meters, and the output callback, and one proves the null backend advances without
false callback XRuns. Warmed input and output handler calls observe zero allocations,
deallocations, and bytes.

Phase 14 also adds:

- one app-agnostic combined workload harness covering routing, playback, meters, pitch, and
  spectrum at the documented small, medium, large, and extreme-offline capacities;
- a Criterion benchmark plus an exact-percentile release qualification runner with environment,
  profile, revision, memory, queue-pressure, and analysis-pressure metadata;
- render blocks/frames, total/max render time, deadline misses, callback XRuns, input/output
  underflow/overflow, playback queue/cache, and control queue observations;
- generated Rust-to-TypeScript render/playback observation contracts and Sound Card diagnostics
  for timing, XRuns, queue pressure, and resident cache use;
- a deterministic `sound-card.v1` product-evidence scenario using the null backend and medium
  combined workload;
- a mounted `/evidence/sound-card` route backed by the deterministic editor harness; and
- a backend-neutral managed-device soak contract and release runner with explicit warm-up,
  negotiated-buffer queue sizing, paced prefill, periodic stop/reopen recovery, JSON evidence, and
  strict signal/deadline/XRun/analysis acceptance checks.

The release percentile run used Windows x64, an Intel64 Family 6 Model 198 Stepping 2 processor
(24 logical processors), Rust release mode, 48 kHz, and 128-frame blocks. Setup and warm-up were
excluded; each row contains 10,000 measured blocks:

| Workload | p50 | p99 | p99.99 | Maximum | p99 / block | p99.99 / block | Estimated resident memory |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Small | 8.6 us | 9.6 us | 73.7 us | 73.8 us | 0.36% | 2.76% | 3,859,072 bytes |
| Medium | 41.6 us | 50.5 us | 92.6 us | 123.4 us | 1.89% | 3.47% | 3,961,856 bytes |
| Large | 192.9 us | 241.8 us | 492.2 us | 547.2 us | 9.07% | 18.46% | 4,268,032 bytes |
| Extreme offline | 984.2 us | 1,411.9 us | 1,745.1 us | 1,752.4 us | 52.95% | 65.44% | 17,604,608 bytes |

Small, medium, and large clear the local release-run p99/p99.99 ratios. Extreme offline is
explicitly outside the hardware-deadline target. The unpaced runner intentionally overloads the
analysis worker and recorded bounded dropped analysis frames for medium and large; those
drops are stress evidence, not real-time-paced XRun results. Profiling did not justify a
platform-specific acceleration path, so the scalar implementation remains authoritative.

The Windows native probe found WASAPI available and the default-output smoke opened, started,
wrote silence for 100 ms, and stopped successfully. That is not a real-signal or endurance result.
The managed-device runner then exposed startup underflow and recovery-prefill overflow in the
initial callback bridge. The queue now holds three negotiated callback periods, prefill advances
at engine-clock pace, a full priming bridge pauses until its first callback, and retired bridges do
not report false overflow. Native backend error statuses are consumed once by the control worker:
ready warnings become strict soak diagnostics, while invalidated or failed streams retire their
bridges and enter the supervisor retry/reopen path. A 15-second release rerun on the same WASAPI
output sustained the medium
32-channel/128-route/32-voice workload through three planned stop/reopen cycles with zero callback
XRuns, backend warnings, render deadline misses, control pressure, playback failures, and dropped
or stale analysis frames.

The first exact-commit one-hour run at `898ad103` kept every realtime, playback, and recovery
counter clean but correctly failed the strict gate after four analysis frames were discarded
during rare host-scheduler stalls. The analysis worker had been coalescing queued frames even when
it had enough processing capacity. At `f6661d6c`, it instead processes the bounded queue in order
and reserves four preallocated frames per tap to absorb short scheduler stalls; sustained overload
remains bounded and observable.

The corrected release build ran for 3,600.055 seconds on the default WASAPI Realtek output at
48 kHz stereo with a negotiated 480-frame callback buffer. It rendered 172,758,784 frames and
completed all five planned stop/reopen cycles. The strict JSON report passed with zero backend
warnings, callback XRuns, render deadline misses, control queue pressure, input/output bridge
underflows or overflows, playback failures, dropped analysis frames, or stale analysis frames. The
maximum render call was 1,398 microseconds, and every recovery returned to `Ready` without retry.

Continued bundled-release qualification exposed two reusable host issues outside the app-owned
Sound Card layer. The static SPA fallback shadowed `/.well-known/chataigne`, and the discovery
document advertised `/ws` while the authoritative server and UI clients use `/api/ui/ws`.
`85e6cc36` reserves backend namespaces from the fallback and shares the canonical WebSocket path.
The next release probe found bundled headless mode waiting five seconds for its own listener before
starting that listener. `a245641b` removes that self-preflight; requested dev-server readiness
remains owned by the dev-server launcher.

The exact `a245641b` Windows release binary then started loopback-only in bundled headless mode
without warnings or error markers. `/api/ui/health` reported both backend and engine read model
ready, `/.well-known/chataigne` returned the correct relative discovery document,
`/api/ui/ws` returned the expected HTTP 426 without an upgrade, `/evidence/sound-card` returned
HTTP 200, and its referenced JavaScript asset returned HTTP 200. The local binary is intentionally
unsigned; installed-package, signing, desktop-window, and uninstall qualification remain release
environment gates.

The configured browser surface again reported no browser, so mounted normal/narrow visual
inspection remains `NOT RUN`; no desktop-control fallback was used. Other Windows backends,
macOS, and Linux remain `NOT RUN`.

## Commands and evidence

| Command / inspection | Result |
| -------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
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
| Windows ASIO local compile/runtime | PARTIAL - pinned SDK checkout, 134 ASIO-feature tests, strict all-target Clippy, Chataigne consumer check, ASIO/WASAPI discovery, and the ASIO 48 kHz output smoke PASS; recovery and package qualification remain NOT RUN |
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
| Phase 9 focused Sound Card tests | PASS - 6 tests covering catalog/commands, defaults, sparse persistence, in-tree and externally linked same-module filters, repair, duplication, and removal |
| `cargo test -p Chataigne2 --no-fail-fast` after Phase 9 | PASS - 449 tests, 0 failed |
| `cargo test -p golden_audio` after Phase 9 | PASS - 120 tests (108 unit, 12 integration), 0 failed |
| `cargo check --workspace --all-targets` after Phase 9 | PASS |
| Phase 9 app all-target Clippy with known unrelated workspace lints allowed | PASS - no Phase 9 warning or lint remained |
| Phase 9 strict all-target Clippy | FAIL before reaching the app - pre-existing `chataigne_condition::runtime::evaluate_leaf` argument-count lint; the app-only retry also reported existing Alchemist/state-machine lints |
| Phase 9 `cargo deny check` | PASS - advisories, bans, GPLv3 license policy, and sources; existing workspace warnings remain non-fatal |
| Phase 9 `cargo machete` | FAIL - the same six pre-existing unused dependencies remain in `Chataigne2`; `golden_audio` is used and was not reported |
| Root and Golden Core formatting plus `--check` after Phase 9 | PASS |
| Phase 10 focused Sound Card tests | PASS - 14 tests covering persistence plus stable UUID conversion, generation coalescing, invalid-plan retention, missing-device recovery, unresolved-route warnings, atomic undo, readiness/capabilities, bounded snapshot-free values, and worker shutdown |
| `cargo test -p golden_audio` after Phase 10 | PASS - 121 tests (109 unit, 12 integration), 0 failed |
| `cargo test -p Chataigne2 --no-fail-fast` after Phase 10 | 456 PASS, 1 timing-sensitive pre-existing multiplex performance test exceeded its 10 ms threshold under suite load; its immediate isolated rerun PASSed |
| `cargo check --workspace --all-targets` after Phase 10 | PASS |
| `cargo clippy -p golden_audio --all-targets -- -D warnings` after Phase 10 | PASS |
| Phase 10 app all-target Clippy with only the documented unrelated Alchemist/state-machine lint classes allowed | PASS - no Sound Card warning or lint remained |
| Phase 10 strict app all-target Clippy | FAIL on the same unrelated Alchemist/state-machine lint classes documented in Phase 9 |
| Phase 10 Sound Card Rust-to-TypeScript generation | PASS - 46 deterministic files covering device, stream, analysis, and packed app telemetry declarations |
| `npm run check` in `apps/chataigne/ui` after Phase 10 | PASS - 0 errors and 0 warnings |
| `npm test` in `apps/chataigne/ui` after Phase 10 | PASS - 18 tests in 7 files |
| `npm run build` in `apps/chataigne/ui` after Phase 10 | PASS - static production build completed; only the existing chunk-size advisory was emitted |
| Cargo and npm workspace license metadata after Phase 10 | PASS - all 26 Cargo packages and all four npm workspaces report `GPL-3.0-only` |
| Root and Golden Core formatting plus `--check` after Phase 10 | PASS |
| Phase 11 focused Sound Card tests | PASS - 7 tests covering manual/auto/external command execution, per-lane multiplex overrides, ordered replacement admissions, local output validation, disabled/missing-output behavior, script descriptors/templates/dispatch, callback shapes, and transient retention |
| `cargo test -p golden_audio` after Phase 11 | PASS - 122 tests (110 unit, 12 integration), 0 failed |
| `cargo test -p Chataigne2 --no-fail-fast` after Phase 11 | PASS - 464 tests, 0 failed |
| `cargo clippy -p golden_audio --all-targets -- -D warnings` after Phase 11 | PASS |
| `cargo clippy -p Chataigne2 --all-targets` after Phase 11 | PASS - existing workspace warnings only; no Sound Card warning remained |
| Phase 11 strict app all-target Clippy | FAIL before reaching the app on the existing Alchemist condition argument-count lint; allowing that lint exposed the broader pre-existing Golden Core/Alchemist warning backlog |
| Root and Golden Core formatting plus `--check` after Phase 11 | PASS |
| `npm run check --workspace golden_audio_ui` after Phase 12 | PASS - 0 errors and 0 warnings |
| `npm run check:generated --workspace golden_audio_ui` after Phase 12 | PASS - all 45 generated files match the Rust DTO exporter |
| `npm test --workspace golden_audio_ui` after Phase 12 | PASS - 21 tests in 5 files |
| `npm run check --workspace chataigne-ui` after Phase 12 | PASS - 0 errors and 0 warnings |
| `npm test --workspace chataigne-ui` after Phase 12 | PASS - 18 tests in 7 files |
| `npm run build --workspace chataigne-ui` after Phase 12 | PASS - static production build completed; only the existing chunk-size advisory was emitted |
| Phase 12 npm workspace dependency/license audit | PASS - `golden_audio_ui` resolves only its `golden_ui` dependency and Svelte peer; all five npm workspaces report `GPL-3.0-only` |
| Root `npm run check` and `npm test` after Phase 12 | PASS - package codegen/type checks plus 21 package and 18 app tests |
| Phase 13 focused Rust tests | PASS - 3 tests for playback lifecycle projection, ephemeral UI stop controls, atomic route creation, grouped gain/removal edits, and undo/redo |
| `cargo test -p Chataigne2 --no-fail-fast` after Phase 13 | PASS - 467 tests, 0 failed |
| `cargo clippy -p chataigne_sound_card_protocol --all-targets -- -D warnings` after Phase 13 | PASS |
| `cargo clippy -p Chataigne2 --all-targets` after Phase 13 | PASS - existing Golden Core, Alchemist, desktop-host, and app warning backlog only; no Phase 13 Sound Card warning remained |
| `npm run check --workspace chataigne-ui` after Phase 13 | PASS - 0 errors and 0 warnings |
| `npm run lint --workspace chataigne-ui` after Phase 13 | PASS |
| `npm test --workspace chataigne-ui` after Phase 13 | PASS - 36 tests in 14 files |
| `npm run build --workspace chataigne-ui` after Phase 13 | PASS - static production build completed; only the existing chunk-size advisory was emitted |
| Phase 13 Sound Card protocol regeneration | PASS - generated TypeScript hashes were unchanged after regeneration |
| Root `npm run check` and `npm test` after Phase 13 | PASS - package codegen/type checks plus 21 package and 36 app tests |
| Phase 13 256-by-256 matrix evidence | PASS - one Canvas, 512 axis options, and fewer than 600 focused DOM controls; no per-cell component expansion |
| Phase 13 mounted browser inspection | NOT RUN - the configured browser surface reported no available browser; no unsupported fallback automation was used |
| Root and Golden Core formatting plus `--check` after Phase 13 | PASS |
| Phase 14 combined workload allocation guard | PASS - routing, resident playback, meters, pitch capture, and spectrum capture observed 0 allocations, 0 deallocations, and 0 bytes after warm-up |
| Phase 14 combined quick Criterion benchmark | PASS - small, medium, large, and extreme-offline medians recorded in the performance table |
| Phase 14 exact-percentile release runner | PASS - 10,000 measured blocks per workload; p50/p99/p99.99/max, deadline-ratio, memory, and analysis-pressure evidence recorded above |
| Phase 14 managed callback integration | PASS - callback-backed decoded playback output, synthetic-input monitoring/meter/output, null-clock XRun, and callback allocation tests |
| `cargo test -p golden_audio` after Phase 14 | PASS - 134 tests (118 unit, 16 integration), 0 failed |
| Phase 14 Golden Audio feature matrix | PASS - no-default, playback-only, analysis-only, and default all-target checks |
| Phase 14 Golden Audio strict Clippy | PASS - default and no-default all-target runs with `-D warnings` |
| `cargo test -p Chataigne2` after Phase 14 | PASS - 468 tests, 0 failed |
| Phase 14 Sound Card product evidence | PASS - `sound-card.v1`, semantic digest `fnv1a64:8e5054f8524fa1bc` |
| Phase 14 Windows native probe | PASS - WASAPI available |
| Phase 14 Windows default-output smoke | PASS - stereo 48 kHz stream opened, ran silent for 100 ms, and stopped |
| Phase 14 managed-device runner tests | PASS - option/fixture guards, medium null-backend signal/recovery, supervisor runtime-failure backoff, and callback-stream invalidation/reopen |
| Phase 14 short Windows managed-device recovery | PASS - release, 15 seconds, medium workload, 3 planned recoveries, 0 XRuns/backend warnings/deadline misses/queue pressure/playback failures/analysis drops |
| Phase 14 initial one-hour Windows managed-device soak | FAIL at exact `898ad103` - 4 analysis frames dropped during host-scheduler stalls; all realtime, playback, warning, and recovery counters were otherwise clean |
| Phase 14 corrected one-hour Windows managed-device soak | PASS at exact `f6661d6c` - release, 3,600.055 seconds, medium workload, 172,758,784 frames, 5/5 planned recoveries, 0 warnings/XRuns/deadline misses/bridge pressure/queue pressure/playback failures/analysis drops |
| Phase 14 generated audio/Sound Card contracts | PASS - playback and render observations regenerated in both consumers; reusable generated-contract drift check passed |
| Phase 14 reusable audio UI checks | PASS - 0 Svelte diagnostics and 21 tests |
| Phase 14 Chataigne UI checks | PASS - 0 Svelte diagnostics, Prettier clean, 36 tests, and static production build |
| Phase 14 evidence route production build | PASS - `/evidence/sound-card` emitted in the server/static build |
| Continued root UI verification | PASS - 0 diagnostics, generated-contract drift clean, 21 reusable audio UI tests, 36 Chataigne UI tests, Prettier clean, and production build |
| Bundled discovery route regression | PASS - 21 transport tests; SPA fallback preserves `/api` and `/.well-known`; discovery advertises `/api/ui/ws` |
| Bundled headless startup regression | PASS - 7 desktop-host tests, 1 subprocess helper ignored, and strict owning-crate Clippy with `--no-deps -D warnings` |
| Local unsigned package check | PASS at exact `85e6cc36` - production UI plus Tauri custom-protocol release application built successfully |
| Corrected bundled headless release startup | PASS at exact `a245641b` - health/read-model ready, discovery JSON correct, WebSocket route reachable, Sound Card evidence and referenced asset HTTP 200, no false frontend warning or error marker |
| Phase 14 mounted normal/narrow browser inspection | NOT RUN - the configured browser surface reported no available browser; no unsupported fallback automation was used |
| Phase 14 one-hour real-device workload and recovery soak | PASS - exact `f6661d6c` WASAPI release report, strict `golden-audio-managed-device-soak.v1` contract |
| Product run modes | PARTIAL - exact `a245641b` bundled headless release startup passed; mounted desktop, browser session, and installed package remain `NOT RUN` |
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

Phase 9:

- `Cargo.lock`
- `apps/chataigne/Cargo.toml`
- `apps/chataigne/src/module/reference_filters.rs`
- `apps/chataigne/src/module/modules/audio/sound_card/`
- `docs/progress/golden-audio-sound-card.md`

Phase 10:

- `Cargo.toml`
- `Cargo.lock`
- `apps/chataigne/Cargo.toml`
- `apps/chataigne/src/module/modules/audio/sound_card/`
- `apps/chataigne/systems/sound_card_protocol/`
- `apps/chataigne/ui/README.md`
- `apps/chataigne/ui/package.json`
- `apps/chataigne/ui/src/lib/modules/audio/sound-card/`
- `crates/golden_audio/src/analysis/observation.rs`
- `crates/golden_audio/src/control/`
- `crates/golden_audio/src/ids.rs`
- `crates/golden_audio/src/tests/engine.rs`
- `docs/architecture/golden-audio.md`
- `docs/progress/golden-audio-sound-card.md`

Phase 11:

- `apps/chataigne/src/module/modules/audio/sound_card/`
- `apps/chataigne/src/module/script_api/`
- `apps/chataigne/src/module/script_templates/sound_card_module.js`
- `apps/chataigne/src/module/script_templates/snippets/sound_card_*.js`
- `crates/golden_audio/src/control/`
- `crates/golden_audio/src/tests/engine.rs`
- `docs/guides/module-scripting.md`
- `docs/progress/golden-audio-sound-card.md`

Phase 12:

- `packages/golden-audio-ui/`
- `packages/golden-ui/components/panels/inspector/`
- `packages/golden-ui/index.ts`
- `packages/golden-ui/store/platform.svelte.ts`
- `apps/chataigne/ui/README.md`
- `apps/chataigne/ui/package.json`
- `apps/chataigne/ui/src/lib/modules/audio/sound-card/audio-device-inspector-adapter.svelte.ts`
- `apps/chataigne/ui/src/routes/+page.svelte`
- `package.json`
- `package-lock.json`
- `README.md`
- `docs/architecture/golden-audio.md`
- `docs/guides/ui-extension.md`
- `docs/reference/contributor-map.md`
- `docs/reference/repository-layout.md`
- `docs/progress/golden-audio-sound-card.md`

Phase 13:

- `apps/chataigne/src/module/modules/audio/sound_card/`
- `apps/chataigne/systems/sound_card_protocol/`
- `apps/chataigne/ui/src/lib/modules/audio/sound-card/generated/`
- `apps/chataigne/ui/src/lib/inspectors/modules/ModuleInspectorPanelHeader.svelte`
- `apps/chataigne/ui/src/lib/panels/modules/module-editor-registry.ts`
- `apps/chataigne/ui/src/lib/panels/modules/module-editor-setup.ts`
- `apps/chataigne/ui/src/lib/panels/modules/SoundCardEditorPanel.svelte`
- `apps/chataigne/ui/src/lib/panels/modules/sound-card/`
- `apps/chataigne/ui/src/routes/+page.svelte`
- `apps/chataigne/ui/.prettierignore`
- `apps/chataigne/ui/vite.config.ts`
- `apps/chataigne/ui/README.md`
- `docs/architecture/golden-audio.md`
- `docs/guides/ui-extension.md`
- `docs/progress/golden-audio-sound-card.md`

Phase 14:

- `crates/golden_audio/src/control/`
- `crates/golden_audio/src/qualification/`
- `crates/golden_audio/benches/reference_workloads.rs`
- `crates/golden_audio/examples/reference_qualification.rs`
- `crates/golden_audio/tests/playback_ordering.rs`
- `apps/chataigne/src/module/modules/audio/sound_card/`
- `apps/chataigne/src/product_evidence/`
- `apps/chataigne/systems/sound_card_protocol/`
- `packages/golden-audio-ui/generated/`
- `apps/chataigne/ui/src/lib/modules/audio/sound-card/generated/`
- `apps/chataigne/ui/src/lib/panels/modules/sound-card/`
- `apps/chataigne/ui/src/routes/evidence/sound-card/`
- `crates/golden_core/hosts/transport/src/ui_server/`
- `crates/golden_core/hosts/desktop/src/`
- `docs/architecture/golden-audio.md`
- `docs/progress/golden-audio-sound-card.md`

## Remaining work

Phase 14 local implementation, deterministic qualification, and the Windows WASAPI one-hour
real-device workload/recovery gate are complete. Windows ASIO source acquisition, compilation,
tests, discovery, and short real output stream smoke now also pass locally against the pinned SDK;
ASIO recovery and installed-package qualification remain external gates. Phase 14 otherwise
remains open for mounted normal/narrow visual inspection and exact-commit cross-platform backend
results. Phase 15 owns final documentation, release, and packaging gates after those external
qualification results exist.
