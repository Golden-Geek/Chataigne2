# Chataigne2 golden_audio and Sound Card Module

## Codex implementation plan

Baseline: Chataigne2 main at commit [0c0442e964e3dc3cce0c7a6ae647be2c26d275b2](https://github.com/Golden-Geek/Chataigne2/commit/0c0442e964e3dc3cce0c7a6ae647be2c26d275b2), dated 22 July 2026.

This plan supersedes earlier Sound Card plans written against commit 689431f. The current baseline includes the later tests-and-performance pass and its transient command-delivery changes.

### Baseline evidence

- [Repository AGENTS.md](https://github.com/Golden-Geek/Chataigne2/blob/0c0442e964e3dc3cce0c7a6ae647be2c26d275b2/AGENTS.md)
- [Workspace Cargo manifest](https://github.com/Golden-Geek/Chataigne2/blob/0c0442e964e3dc3cce0c7a6ae647be2c26d275b2/Cargo.toml)
- [ModuleBase](https://github.com/Golden-Geek/Chataigne2/blob/0c0442e964e3dc3cce0c7a6ae647be2c26d275b2/apps/chataigne/src/module/mod.rs)
- [Multiplex-aware command execution](https://github.com/Golden-Geek/Chataigne2/blob/0c0442e964e3dc3cce0c7a6ae647be2c26d275b2/apps/chataigne/src/module/command/mod.rs)
- [golden_ui node-inspector registry](https://github.com/Golden-Geek/Chataigne2/blob/0c0442e964e3dc3cce0c7a6ae647be2c26d275b2/packages/golden-ui/components/panels/inspector/node-inspector-registry.ts)
- [Current app panel registration](https://github.com/Golden-Geek/Chataigne2/blob/0c0442e964e3dc3cce0c7a6ae647be2c26d275b2/apps/chataigne/ui/src/routes/%2Bpage.svelte)
- [CPAL 0.18.1 API](https://docs.rs/cpal/0.18.1/cpal/) and [official backend/features table](https://github.com/RustAudio/cpal#supported-platforms)
- [rust-jack dynamic-loading default](https://github.com/RustAudio/rust-jack/blob/main/Cargo.toml)
- [Symphonia 0.6 format/codec matrix](https://docs.rs/symphonia/0.6.0/symphonia/)
- [Rubato real-time processing guidance](https://docs.rs/rubato/latest/rubato/)

## 1. Mission

Implement a production-grade, cross-platform audio foundation named golden_audio and a Chataigne2 Sound Card module built on it.

The result must provide:

- Reusable audio-device discovery, stream ownership, format negotiation, reconnection, clock handling, virtual channels, patching, mixing, playback, metering, pitch detection, spectral analysis, diagnostics, offline rendering, and deterministic test backends in golden_audio.
- A reusable golden_audio_ui Svelte 5 package, built on golden_ui, that provides the canonical Golden custom inspector for input/output backend and device selection.
- A Chataigne-specific Sound Card node, persistence model, commands, script API, callbacks, value exposure, and Svelte 5 editor in apps/chataigne.
- First-class WASAPI, ASIO, CoreAudio, ALSA, and JACK support.
- Native PipeWire support owned and shipped by golden_audio on Linux unless an evidenced blocker is recorded. PipeWire is not allowed to leak into application code even if it has to remain an internally selectable build feature.
- Normal official Chataigne builds with one ordinary golden_audio dependency. Chataigne must not import CPAL, JACK, PipeWire, ASIO bindings, Symphonia, Rubato, or any other backend implementation crate.
- A hard real-time render path suitable as a foundation for later DAW work.
- Preservation of the current Chataigne module, command, script, persistence, UI, host, watch, and packaging architecture.

This is not a request for a minimal proof of concept. It is also not permission to build a full DAW, plugin host, sequencer, or timeline now. Build the smallest complete professional audio engine that satisfies this module while preserving the boundaries needed for those future products.

## 2. Mandatory execution rules

Codex must follow these rules throughout the implementation.

1. Read AGENTS.md completely before editing. Re-read the relevant section whenever ownership is uncertain.
2. Confirm the current main SHA before work begins. If main moved beyond the baseline, record the new SHA and inspect every affected path before editing. Do not reset or discard unrelated user changes.
3. Create docs/progress/golden-audio-sound-card.md in Phase 0. Update it continuously with:
   - Baseline SHA and branch.
   - Current phase and status.
   - Decisions made and why.
   - Files changed.
   - Commands actually run.
   - Exact pass, fail, blocked, or NOT RUN results.
   - Measured performance evidence.
   - Remaining risks and next step.
4. Never claim that code compiles, a backend works, hardware was tested, or a platform passed unless that exact revision was actually compiled or tested. Use NOT RUN when evidence does not exist.
5. Keep the repository buildable at every committed checkpoint. A phase may contain temporary local breakage, but its commit cannot.
6. Finish every phase with one focused, reviewed commit. Do not restore the removed supercommit helpers. Suggested commit subjects are included below.
7. Stop on a failing phase gate, fix the phase, update the progress document, and rerun the failed gate before continuing.
8. Use the repository’s generated app-node registration. Do not add a handwritten parallel node catalog or intermediate mod.rs registry merely to expose the module.
9. Use Svelte 5 runes and direct event properties only. No legacy Svelte event syntax.
10. Put every test in the repository-prescribed tests directory beside its feature. Do not add inline Rust test modules or adjacent star-test files.
11. Keep implementation files below 1,000 lines. Split by cohesive responsibility before reaching that limit.
12. Do not put Chataigne NodeId, ProcessCtx, ParamValue, script callback names, UI DTOs, or product policy in golden_audio.
13. Do not perform audio work, device polling, decoding, FFTs, or millisecond polling on Chataigne’s engine thread.
14. Do not expose backend Cargo features through Chataigne. Internal golden_audio features are permitted for CI, null-backend tests, and constrained consumers; the official dependency uses golden_audio defaults.
15. Keep these workflows working without audio-specific flags after the documented bootstrap prerequisites are installed:
    - cargo run
    - cargo xtask watch
    - cargo run -- --dev
    - cargo run -- --headless
16. Never solve a native-backend startup failure by making the whole app require that server or driver at runtime. Missing ASIO drivers, JACK, or PipeWire must become structured Unavailable status, not an application crash.

## 3. Ownership boundary

The target dependency direction is:

~~~text
CPAL / native SDKs / codecs / DSP crates
                    |
                    v
              golden_audio
               |           |
               |           +------> generated reusable UI contract
               |                                |
               v                                v
       Chataigne Sound Card adapter       golden_audio_ui
               |                          based on golden_ui
               +---------------+----------------+
                               |
                               v
                    Chataigne inspector/editor
~~~

### golden_audio owns

- Backend and host enumeration.
- Device and physical-channel descriptions.
- Stable app-agnostic audio identities.
- Stream creation, format conversion, start, stop, recovery, and two-phase switching.
- Engine clock, input/output clock bridges, drift compensation, and null clock.
- Virtual input and output buses.
- Input patch, monitoring patch, playback patch, output patch, faders, and master gain.
- Immutable compiled render plans.
- Hard real-time rendering and callback-safe control exchange.
- File probing, decoding, cache policy, streaming read-ahead, resampling, and voice lifecycle.
- RMS, peak/clip diagnostics, pitch, FFT band energy, smoothing, and observation snapshots.
- Bounded queues, saturation semantics, resource limits, diagnostics, and metrics.
- Null, mock, and offline backends.
- Backend-specific prerequisites, capabilities, and availability explanations.

### Chataigne owns

- The sound_card_module node type and all child node types.
- Mapping persistent node UUIDs to AudioChannelId and other golden_audio identities.
- Device-profile persistence as nodes.
- Virtual-channel authoring and route authoring semantics.
- Module lifecycle, dirty-config collection, and application of coalesced observations.
- Command nodes and multiplex override handling.
- Script methods, callbacks, templates, and snippets.
- Chataigne reference filters and file constraints.
- The thin mapping from its node tree and intents to the generic audio inspector contract.
- App-owned Sound Card telemetry extensions that are not generic device-selection state.
- The complete Sound Card editor, module-editor registration, and Chataigne-specific routing/analysis UI.

### golden_audio_ui owns

- The reusable golden_ui custom inspector for audio input/output device selection.
- Generic device selector view-model rendering for backend groups, available devices, persisted missing devices, enable state, readiness, negotiated format, buffer, latency, permissions, and structured errors.
- Reusable Svelte 5 controls and accessibility behavior for the same selector outside an inspector.
- A small adapter interface that lets a Golden application bind its persistence/node model without teaching the package Chataigne types or policy.
- Tests and a standalone mock-adapter harness proving that the inspector works without Chataigne.

### Explicitly forbidden boundaries

- golden_audio must not depend on golden_core, golden_io, Tauri, Svelte, or apps/chataigne.
- golden_audio_ui may depend on golden_ui and generated app-agnostic golden_audio contracts, but must not depend on Chataigne.
- golden_ui must not acquire audio-specific device or backend policy; golden_audio_ui is its audio-domain companion.
- Chataigne must not copy golden_audio DSP or device code.
- The UI must not invent channel IDs, route labels, default gains, profile keys, or cleanup policy.
- The generic Golden packages must not acquire Sound Card policy. Add a generic public hook only when an existing reusable boundary genuinely lacks one.

## 4. Fixed product semantics

These decisions are implementation requirements, not open questions.

### 4.1 Default module state

- Output is enabled and targets the platform’s system-default output through the platform-default backend.
- Input is disabled by default to avoid surprise microphone capture and acoustic feedback.
- The module starts with two virtual inputs and two virtual outputs.
- Input and output device patches default to one-to-one channel mapping when a compatible physical channel exists.
- The monitoring matrix starts empty. No microphone is routed to an output until the user explicitly adds a monitoring route.
- Master and per-output-channel volume default to 0 dB.
- The internal engine sample rate defaults to 48,000 Hz.
- The internal processing block defaults to 128 frames. Hardware callbacks may be any size and are adapted without allocation.

### 4.2 Stable channels

- A virtual channel’s identity is an AudioChannelId derived by the Chataigne adapter from the UUID of its persistent virtual-channel node.
- Renaming, reordering, device replacement, backend replacement, reconnection, or physical-channel changes never change that identity.
- Physical device channel indices never appear in commands, scripts, processors, or user links.
- Deleting a virtual channel invalidates it intentionally. Dependent route cleanup is backend-owned and must occur atomically with the deletion or preserve an explicitly visible unresolved route; the UI must not silently implement cleanup policy.

### 4.3 Device selection and recovery

- Persist a target as backend plus stable device identity, not display name or enumeration index.
- Keep a fallback fingerprint and last-known label for devices whose backend cannot provide a durable identifier.
- A missing selected device remains selected and appears as Missing. The default recovery policy waits for it and outputs silence; it does not silently route to unrelated hardware.
- A separately selectable Follow System Default policy may follow operating-system default changes.
- Device-specific patch profiles survive device loss and restore when the matching device returns.
- New device profiles receive deterministic one-to-one routes without changing virtual channels.
- Input and output may target different devices and different backends.

### 4.4 Playback IDs and ordering

- playback_id is a required, non-empty, trimmed string.
- Play File with an unused ID starts a new voice after asynchronous preparation.
- Play File with an existing or loading ID is last-command-wins replacement. The old voice ramps out and the new voice ramps in.
- Stop File is idempotent and applies to active and loading voices.
- Stop All Files cancels every active and loading request that precedes it in command order.
- A decoder result that completes after replacement or Stop All is stale and must be discarded on a non-real-time thread.
- Play, replace, stop, and stop-all ordering is represented with monotonically increasing command sequence numbers and generation watermarks.
- When output is unavailable, playback continues against the null clock. Reconnection resumes at the current playhead; it must not retrigger from zero.

### 4.5 Volume and gain

- Authored units are decibels. Convert to linear gain outside the callback.
- Accept -120 dB through +24 dB. Treat -120 dB as digital silence.
- Every audible gain change uses a configurable dezipper with a 10 ms default and a permitted 5–20 ms range.
- Route gain, output-channel fader, and master gain are distinct.
- No hidden compressor or limiter changes signal semantics. Internal f32 buses may exceed full scale. Device conversion saturates only where required and reports clip/overload diagnostics.

### 4.6 Module enabled state

- Disabling the module ramps down, cancels voices, closes streams, and retains all authored configuration.
- Re-enabling reconnects devices but does not resume voices that were stopped by disable.
- Commands received while disabled fail with a structured reason and emit the normal failure callback where applicable.

## 5. Signal and clock model

~~~text
Physical inputs
    |
    v
Device format conversion
    |
    v
Input clock bridge and adaptive ASRC
    |
    v
Input patch profiles
    |
    v
Stable virtual inputs -------> RMS / pitch / spectrum taps
    |
    v
Sparse monitoring matrix
    |
    +---------------------------+
                                |
Decoded and resampled voices -> Playback patch -> Stable virtual outputs
                                                    |
                                                    v
                                         Per-channel faders
                                                    |
                                                    v
                                               Master gain
                                                    |
                                                    v
                                          Output patch profile
                                                    |
                                                    v
                                      Output ASRC / format conversion
                                                    |
                                                    v
                                            Physical outputs
~~~

### Clock rules

- The configured engine sample rate defines timeline time. Hardware sample rates are boundary details.
- The output callback is the render demand clock while an output stream is ready.
- A software null clock becomes the render clock when output is disabled, missing, or switching.
- Exactly one render clock is authoritative at a time.
- Input and output are separate clock domains even when they refer to the same device unless a backend explicitly proves a shared clock.
- Input monitoring uses a bounded ring and adaptive asynchronous resampling. Control ring fill with a stable PI-style drift controller and expose measured ppm, fill, underflow, overflow, and estimated latency.
- Output rate conversion is fixed-ratio unless device timestamps prove drift that requires correction.
- Switching clocks is a two-phase operation: prepare and prime the new clock, switch at a block boundary, then retire the old stream off the callback.
- Reset a broken bridge with a short fade rather than replaying stale buffered input.

## 6. Hard real-time contract

The render callback and input callback must never:

- Allocate, reallocate, or free heap memory.
- Lock a mutex, rwlock, condvar, or blocking channel.
- Wait for another thread.
- Log or format strings.
- Open, close, enumerate, probe, decode, seek, or read files or devices.
- Resolve UUIDs, node references, channel names, or Chataigne tree state.
- Build a route plan.
- Clone or drop the final owning reference to heap data.
- Panic for recoverable data, device, or queue errors.

Use:

- Planar f32 internal buffers.
- Preallocated scratch storage sized from explicit EngineLimits.
- Bounded SPSC queues at every callback boundary.
- A single active immutable RenderPlan.
- Boxed plan exchange with an acknowledgement invariant: the control side submits at most one pending plan; the callback swaps it only at a block boundary and returns the retired plan to the control side. If the return queue is unexpectedly full, retain the retired plan in a fixed callback slot and decline another swap. Never drop it in the callback.
- Fixed voice slots. Completion marks a slot retired; only a non-real-time owner clears its buffers and drops asset ownership.
- Preallocated stream rings and analysis frame slots.
- Coalesced gain mailboxes for high-frequency target changes, with ordered sequence barriers around play and stop operations.
- Denormal protection and zeroing validated by benchmarks rather than assumed.

Add a test-only allocation/deallocation guard around render_block and callback-format conversion. The test must fail on both allocation and deallocation after warm-up. Add a source-level review checklist for forbidden primitives in callback-owned modules.

## 7. golden_audio public contract

Keep the public API small and backend-neutral. Exact naming may change during implementation, but the following capabilities and ownership must remain.

~~~rust
pub struct AudioEngineBuilder {
    pub config: AudioEngineConfig,
    pub limits: EngineLimits,
    pub backend_policy: BackendPolicy,
}

pub struct AudioEngine { /* owns workers and streams */ }
pub struct AudioControl { /* cloneable, nonblocking producer */ }
pub struct AudioEventReceiver { /* polled outside RT */ }
pub struct AudioObservationReader { /* latest coalesced snapshot */ }

pub enum AudioCommand {
    ApplyConfiguration { generation: u64, config: AudioConfiguration },
    PlayFile(PlayFileRequest),
    StopFile { playback_id: PlaybackId },
    StopAllFiles,
    SetMasterGain { gain: GainDb },
    SetChannelGain { channel: AudioChannelId, gain: GainDb },
    SetEnabled(bool),
    Shutdown,
}

pub enum AudioEvent {
    ConfigurationApplied { generation: u64 },
    ConfigurationRejected { generation: u64, error: AudioError },
    BackendStatusChanged(BackendStatus),
    DeviceStatusChanged(DeviceStatus),
    PlaybackStarted(PlaybackInfo),
    PlaybackFinished(PlaybackInfo),
    PlaybackStopped(PlaybackStopInfo),
    PlaybackFailed(PlaybackFailure),
    Diagnostic(DiagnosticEvent),
}
~~~

Required strong types include:

- AudioChannelId
- AudioRouteId
- AudioDeviceId
- PhysicalChannelKey
- BackendId
- PlaybackId
- VoiceId or generational voice slot
- AnalysisTapId
- ConfigGeneration
- CommandSequence
- GainDb
- SampleRate
- FrameCount

Public errors must be structured, non-exhaustive where appropriate, and preserve a stable category plus a human explanation:

- InvalidConfiguration
- UnsupportedFormat
- BackendUnavailable
- DeviceMissing
- DeviceBusy
- PermissionDenied
- StreamNegotiationFailed
- QueueFull
- CapacityExceeded
- DecodeFailed
- PlaybackNotFound where useful, although Stop File remains idempotent
- ShuttingDown
- InternalInvariant

Do not expose CPAL, Symphonia, Rubato, JACK, PipeWire, or ASIO types in public signatures.

## 8. Proposed golden_audio layout

~~~text
crates/golden_audio/
├── Cargo.toml
├── README.md
├── benches/
│   ├── render.rs
│   ├── routing.rs
│   ├── playback.rs
│   └── analysis.rs
├── src/
│   ├── lib.rs
│   ├── ids.rs
│   ├── limits.rs
│   ├── config.rs
│   ├── error.rs
│   ├── diagnostics.rs
│   ├── backend/
│   │   ├── mod.rs
│   │   ├── traits.rs
│   │   ├── null.rs
│   │   ├── mock.rs
│   │   └── cpal/
│   │       ├── mod.rs
│   │       ├── discovery.rs
│   │       ├── identity.rs
│   │       ├── negotiation.rs
│   │       ├── stream.rs
│   │       └── status.rs
│   ├── device/
│   │   ├── mod.rs
│   │   ├── descriptor.rs
│   │   ├── profile.rs
│   │   ├── supervisor.rs
│   │   └── recovery.rs
│   ├── control/
│   │   ├── mod.rs
│   │   ├── command.rs
│   │   ├── event.rs
│   │   ├── worker.rs
│   │   └── plan_exchange.rs
│   ├── render/
│   │   ├── mod.rs
│   │   ├── plan.rs
│   │   ├── compiler.rs
│   │   ├── buffers.rs
│   │   ├── route_kernel.rs
│   │   ├── gain.rs
│   │   ├── clock.rs
│   │   ├── bridge.rs
│   │   ├── convert.rs
│   │   └── offline.rs
│   ├── playback/
│   │   ├── mod.rs
│   │   ├── request.rs
│   │   ├── asset.rs
│   │   ├── cache.rs
│   │   ├── decoder.rs
│   │   ├── streaming.rs
│   │   ├── voice.rs
│   │   └── scheduler.rs
│   ├── analysis/
│   │   ├── mod.rs
│   │   ├── frame_pool.rs
│   │   ├── rms.rs
│   │   ├── pitch.rs
│   │   ├── spectrum.rs
│   │   ├── smoothing.rs
│   │   └── observation.rs
│   └── tests/
│       ├── mod.rs
│       ├── allocation.rs
│       ├── plan_exchange.rs
│       └── ...
└── tests/
    ├── offline_render.rs
    ├── recovery.rs
    ├── playback_ordering.rs
    ├── synthetic_analysis.rs
    └── fixtures/
~~~

Keep one crate initially. Split only if a measured compile-time, dependency, or ownership problem justifies it. The reusable backend-independent core must remain buildable with no native audio backend.

Add the reusable companion UI package:

~~~text
packages/golden-audio-ui/
├── package.json
├── index.ts
├── README.md
├── generated/
│   ├── AudioBackendStatus.ts
│   ├── AudioDeviceDescriptor.ts
│   ├── AudioDeviceInspectorState.ts
│   └── AudioStreamStatus.ts
├── inspector/
│   ├── GoldenAudioDeviceInspector.svelte
│   ├── audio-inspector-adapter.ts
│   └── tests/
├── components/
│   ├── AudioDeviceSelector.svelte
│   ├── AudioBackendGroup.svelte
│   ├── AudioStreamSummary.svelte
│   ├── AudioDeviceStatus.svelte
│   └── tests/
└── testing/
    └── mock-audio-inspector-adapter.svelte.ts
~~~

The npm package name is golden_audio_ui, matching the existing golden_ui naming convention. It has golden_ui and Svelte 5 as peer/workspace dependencies and no Chataigne dependency.

golden_audio remains UI-framework independent. It owns the app-agnostic serializable device/status DTO source and canonical field/variant meanings. Generate the TypeScript contract consumed by golden_audio_ui; do not hand-maintain a second backend-status or device-description union in TypeScript. Type-generation support may be an optional codegen-only dependency and must not enter the callback or native runtime.

## 9. Dependency policy

At the recorded baseline, evaluate and normally pin compatible releases around:

- CPAL 0.18.1 for platform hosts and streams.
- Symphonia 0.6.0 for probing and decoding.
- Rubato 4.0.0 for prepared real-time and worker-side sample-rate conversion.
- rtrb 0.3.4 for bounded SPSC queues.
- realfft 3.5.0 for real-valued FFT analysis.
- pitch-detection 0.3.0 behind a private adapter if its correctness and steady-state profile pass the synthetic-signal suite. Otherwise implement a focused YIN kernel inside golden_audio and document why.

Before adding each dependency:

- Inspect current release notes, license, MSRV, features, target support, and transitive native requirements.
- Add it at the workspace dependency level when shared policy benefits from one pin.
- Run cargo deny and cargo machete.
- Avoid all-features shortcuts that silently enable unrelated experimental codecs.
- Select explicit Symphonia codecs and generate the Chataigne file-extension list from the same Rust source of truth.
- Keep experimental Symphonia video/subtitle features disabled.

The dependency choice is private. Future replacement must not break the public golden_audio contract.

## 10. Native backend and distribution contract

### Official backend matrix

| Platform | Required built-in backends | Default automatic backend |
| --- | --- | --- |
| Windows | WASAPI, ASIO, JACK | WASAPI |
| macOS | CoreAudio, JACK | CoreAudio |
| Linux | ALSA, JACK, native PipeWire | PipeWire when available, otherwise ALSA |

JACK remains explicitly selectable on every desktop platform even when it is not the automatic default.

### Cargo feature design

- golden_audio default features represent the official full native desktop build plus playback and analysis.
- A no-default-features build provides null/mock/offline functionality for deterministic CI and reuse.
- Target-specific CPAL dependency declarations select only meaningful features:
  - Windows: asio, jack, and realtime priority support.
  - macOS: CoreAudio default plus jack.
  - Linux: jack, pipewire, and realtime-dbus in addition to ALSA.
- Chataigne declares only golden_audio.workspace = true.
- Do not add Chataigne features named asio, jack, pipewire, cpal, or native-audio.

### ASIO

- golden_audio owns ASIO host enumeration, driver discovery, device descriptions, negotiation, stream setup, callback conversion, errors, and recovery.
- Vendor ASIO drivers remain external runtime components.
- Add LLVM/Clang and ASIO SDK requirements to tools/bootstrap/toolchain.json and the toolchain documentation.
- Extend tools/dev.ps1 to verify a usable libclang and Visual C++ toolchain and to explain remediation.
- Use CPAL’s supported SDK discovery for normal builds. Make release CI deterministic by prefetching the selected SDK version into the bounded non-checkout tool cache and setting CPAL_ASIO_DIR for that job.
- Record the chosen Steinberg SDK license path and distribution obligations before calling an ASIO package shippable. Do not check an SDK into the repository unless the selected license explicitly permits it.
- Test package startup on a clean Windows VM with no ASIO driver. The app must start and report ASIO Unavailable.
- Test actual I/O with at least one vendor ASIO driver and 32, 64, 128, and 256 frame buffers where the driver supports them.

### JACK

- Use CPAL’s JACK host and preserve rust-jack dynamic loading.
- Absence of libjack or a running JACK server must not prevent startup.
- Use a stable, unique client name derived from application and module identity without exposing Chataigne types to golden_audio.
- Discover ports/channels without persisting transient port indices.
- Test JACK2 or PipeWire-JACK on Linux, JACK on macOS, and JACK on Windows.
- Test server restart while the module is active and verify automatic recovery.

### PipeWire

- Keep native PipeWire implementation inside golden_audio.
- Add libpipewire and D-Bus development prerequisites to Linux bootstrap checks for supported package managers.
- Package the client library or declare it as a package runtime dependency according to the existing Chataigne release format; never assume developer packages exist on user machines.
- Verify native PipeWire enumeration and streaming separately from PipeWire’s JACK compatibility layer.
- A missing server is an Unavailable backend status and allows fallback to ALSA only when the persisted policy permits fallback.

### Platform permissions

- Add and qualify the macOS microphone usage description and any Tauri/macOS entitlements needed for input.
- Surface Windows microphone privacy denial distinctly from missing hardware.
- Document Linux realtime-priority and audio-group behavior; use rtkit where supported and continue with a warning if priority elevation is denied.

## 11. Render-plan compiler

AudioConfiguration is an authored, stable-ID graph. RenderPlan is a dense, callback-ready indexed representation.

The compiler must:

1. Validate all IDs, references, rates, frame limits, channel limits, route limits, finite values, and duplicate definitions.
2. Resolve stable IDs to compact indices outside real time.
3. Convert dB gains to finite linear gains.
4. Allocate and size every buffer outside real time.
5. Compile sparse routes into destination-major contiguous spans.
6. Optionally choose a dense kernel above an evidenced density threshold. Do not add handwritten SIMD until benchmarks prove it useful.
7. Compile analysis taps and voice destinations to indices.
8. Produce deterministic output for semantically equivalent input.
9. Return warnings for unresolved physical channels and hard errors only for invalid authored topology.
10. Preserve the last valid plan when a new generation fails.

Use property tests to compare the optimized route kernel against a slow scalar reference over randomized channel counts, routes, gains, block sizes, silence, NaN-rejection cases, and overlapping sums.

## 12. Playback engine

### Decode and cache path

- Probe and decode only on worker threads.
- Decode small assets to immutable planar f32 PCM at the engine sample rate.
- Stream large assets through a bounded read-ahead ring.
- Make resident threshold, total cache budget, max voices, read-ahead frames, and decoder workers configurable in EngineLimits.
- Start with safe desktop defaults such as 256 voices, a 512 MiB total resident cache, a 32 MiB per-asset resident threshold, and bounded per-voice read-ahead. Confirm these with memory and soak tests before freezing them.
- Cache keys include canonical source identity, relevant file metadata, selected track, and engine sample rate.
- File change invalidates the cached generation without touching an active immutable generation.
- Never create one operating-system thread per voice.

### Voice path

- Use a preallocated generational voice-slot pool.
- A prepared resident voice references immutable PCM retained by a non-real-time asset owner.
- A streamed voice owns a preallocated SPSC sample ring whose decoder side may block on file I/O but whose render side never does.
- Voice start, replacement, stop, end, starvation, and failure are explicit states.
- Apply short start and stop ramps.
- A starved stream emits silence for missing frames, increments a counter, and may recover; it never blocks the callback.
- Completion and stop notifications are returned to the control thread before any asset or stream buffer is freed.

### Default playback patch

- Stereo maps channel 1 and 2 to virtual output 1 and 2 at 0 dB.
- Mono maps to the first two virtual outputs at -3.0103 dB each when two outputs exist, otherwise to output 1 at 0 dB.
- Multichannel content maps by channel position while destinations exist.
- Unmapped source channels are silent and produce a rate-limited warning.
- Users can override these rules with sparse playback-route nodes.

### Supported files

Enable and test the selected Symphonia formats needed for WAV, AIFF, CAF, FLAC, MP3, AAC/M4A, OGG/Vorbis, MKV, and WebM where supported by the pinned release. Do not advertise WMA or any extension not decoded by the selected features.

## 13. Analysis engine

### RMS and peak

- Accumulate per-virtual-input pre-monitor values and per-virtual-output post-fader values in the render path with constant, allocation-free work.
- Publish linear RMS, dBFS RMS, peak dBFS, and clip state.
- Publish input_global_max_rms, output_global_max_rms, and global_max_rms. The final value is the maximum of all currently published enabled virtual-input and post-fader-output RMS values.
- Define the RMS observation window in milliseconds and make it independent of callback block size.
- Publish at 30 Hz by default, configurable to 60 Hz.

### Pitch

- golden_audio supports multiple analysis taps even if the initial module creates one pitch analyzer.
- Each pitch analyzer selects one stable virtual input.
- Use a configurable minimum frequency, maximum frequency, power threshold, YIN threshold, and confidence threshold.
- Publish valid, frequency_hz, confidence, midi_note, note_name, and cents.
- Validate using generated sine waves, harmonics, detuned notes, silence, noise, low-amplitude signals, chirps, and mixed fundamentals.
- Specify acceptable error in cents for each fixture and never report a confident pitch for silence or below-threshold noise.

### Spectrum

- Each spectrum analyzer selects one stable virtual input.
- Support FFT sizes from 256 through 16,384 frames, power-of-two only.
- Support Hann and Blackman-Harris windows initially.
- Support 0, 50, and 75 percent overlap.
- Support linear and logarithmic band spacing.
- Support configurable min/max frequency, 1–256 bands, and attack/release smoothing.
- Compute normalized band power and publish linear amplitude plus dBFS.
- Preserve Chataigne band-node identities by band index when range or spacing changes; update center/low/high metadata instead of recreating every node.
- Execute FFT and pitch work on analysis workers using preallocated frame slots and newest-result semantics.
- If workers fall behind, count dropped frames and continue audio. Analysis is never allowed to delay rendering.

## 14. Chataigne node model

Add the module beneath apps/chataigne/src/module/modules/audio/sound_card. Let build.rs discover it through the current generated registry.

### Suggested tree

~~~text
Sound Card
├── Connection
│   ├── Input Enabled
│   ├── Input Device
│   ├── Output Enabled
│   ├── Output Device
│   ├── Recovery Policy
│   ├── Engine Sample Rate
│   ├── Buffer Policy
│   ├── Fixed Buffer Frames
│   ├── Connected / Can Receive / Can Send from ModuleBase
│   └── Read-only negotiated input/output format and readiness
├── Parameters
│   ├── Master Volume
│   ├── Virtual Inputs
│   │   └── Virtual Input Channel...
│   ├── Virtual Outputs
│   │   └── Virtual Output Channel
│   │       └── Volume
│   ├── Device Profiles
│   │   ├── Input Profile...
│   │   │   └── Input Patch Route...
│   │   └── Output Profile...
│   │       └── Output Patch Route...
│   ├── Monitoring Routes
│   │   └── Monitor Route...
│   ├── Playback Routes
│   │   └── Playback Route...
│   └── Analysis
│       ├── Pitch Analyzer
│       └── Spectrum Analyzer
├── Values
│   ├── Input Levels
│   │   └── Channel Meter projection...
│   ├── Output Levels
│   │   └── Channel Meter projection...
│   ├── Global Levels
│   ├── Pitch Results
│   ├── Spectrum Bands
│   ├── Playback Status
│   └── Diagnostics
└── Command Tester
~~~

### Required node types

- sound_card_module
- sound_card_virtual_input
- sound_card_virtual_output
- sound_card_input_profile
- sound_card_output_profile
- sound_card_input_patch_route
- sound_card_output_patch_route
- sound_card_monitor_route
- sound_card_playback_route
- sound_card_channel_meter
- sound_card_pitch_analyzer
- sound_card_spectrum_analyzer
- sound_card_spectrum_band
- Five command node types listed in Section 15

### Persistence rules

- Virtual channels, device profiles, routes, analyzer configuration, spectrum bands, and authored volumes are ordinary sparse project nodes.
- Runtime handles, negotiated formats, live device lists, voice slots, and meter observations are never serialized.
- Device enum options retain a Missing entry for persisted but unavailable selections.
- Meter projection nodes are backend-materialized from virtual channels, repaired in a single NodeTree/AddNodeTree operation, and not user-creatable.
- Structural repair is idempotent and does not churn UUIDs on load.
- Duplicate-module tests must prove the duplicate receives distinct virtual-channel UUIDs and AudioChannelIds.
- Project round-trip tests cover present devices, missing devices, disabled input, authored matrices, custom playback routes, analyzer settings, and renamed/reordered channels.

### Runtime adapter

- SoundCardModule owns an optional nonblocking golden_audio handle, dirty flags, the last submitted configuration generation, stable NodeId-to-AudioChannelId maps, and coalesced observation state.
- Initialization builds the runtime outside the audio callback, materializes missing projection nodes, and submits one complete configuration.
- Parameter and structural callbacks only mark the relevant configuration dirty or submit a lightweight ordered control change.
- Coalesce structural changes and compile off-thread. Do not rebuild a render plan once per matrix cell during a drag.
- Poll golden_audio events and observations at 60 Hz or lower through the current compiled kernel/scheduling mechanism.
- Apply observation parameters in a batch and only when changed beyond an explicit epsilon.
- Emit a packed latest-only UI telemetry event for the custom editor from the same observation generation. Rust owns the DTO and generates TypeScript.
- Set ModuleBase connected to true only when every enabled direction is ready and at least one direction is enabled.
- Set can_receive from input readiness and can_send from output readiness.
- Rate-limit repeated warnings and attach structured warnings to the owning connection or analysis nodes.

## 15. Commands and scripts

### Command nodes

| Command | Parameters | Exact behavior |
| --- | --- | --- |
| Play File | Audio File, Playback ID | Read the multiplex-aware execution snapshot, validate non-empty ID, enqueue asynchronous preparation, and use replacement semantics |
| Stop File | Playback ID | Idempotently stop active or loading matching ID |
| Stop All Files | None | Insert an ordered cancellation watermark |
| Set Master Volume | Volume dB | Set a smoothed target without recompiling topology |
| Set Channel Volume | Virtual Output reference, Volume dB | Resolve only a stable virtual-output channel; never accept a physical index |

Every command must:

- Use command_execute_snapshot so multiplex lanes can override every parameter, including path and playback ID.
- Validate values from that effective snapshot, not the persistent base node.
- Return immediately after bounded admission.
- Emit a structured failure if admission fails.
- Have focused tests for manual trigger, auto trigger where applicable, external target-module use, multiplex overrides, queue saturation, disabled module, and missing output.

### Script methods

Expose:

~~~javascript
playFile(path, playbackId)
stopFile(playbackId)
stopAllFiles()
setMasterVolume(volumeDb)
setChannelVolume(channel, volumeDb)
~~~

setChannelVolume accepts the module’s script-visible virtual-output node handle or stable channel token, resolved by Chataigne before calling golden_audio.

### Script callbacks

Add:

~~~javascript
playbackStarted(playbackId, path, info)
playbackFinished(playbackId, info)
playbackStopped(playbackId, reason, info)
playbackFailed(playbackId, path, error)
audioDeviceStatusChanged(direction, status)
audioBackendStatusChanged(backend, status)
~~~

Retain the standard module parameter/value/connection callbacks. Add:

- apps/chataigne/src/module/script_templates/sound_card_module.js
- Comment-only snippets beneath the existing snippets directory.
- Documentation updates in docs/guides/module-scripting.md.
- Tests for descriptor methods, template expansion, callback names, argument shape, and no replay of stale transient playback events.

## 16. Reusable golden_audio device custom inspector

Create packages/golden-audio-ui as the reusable presentation companion to the Rust crate.

### Required exports

- GoldenAudioDeviceInspector.svelte, conforming to golden_ui’s NodeInspectorComponentProps contract and rendering through defaultHeader/defaultContent/defaultChildren.
- AudioDeviceSelector.svelte, the same device UI as a composable section for richer panels.
- AudioDeviceInspectorAdapter and AudioDeviceInspectorBinding types.
- registerGoldenAudioDeviceInspector(nodeType, adapterFactory), an explicit helper that stores the application adapter and calls golden_ui’s existing registerNodeInspector for that concrete node type.
- unregister and test-reset helpers with no implicit module-import side effects.
- Generated TypeScript DTOs for backend, device, direction, readiness, negotiated stream format, buffer, latency, permission, and structured errors.
- A mock adapter for package tests, Storybook-style/manual development, and product evidence.

The current golden_ui registry already resolves an exact node_type before a user_item_kind. Use that public behavior. Chataigne registers sound_card_module through the reusable helper, which safely overrides the general module item-kind inspector only for Sound Card nodes.

### Adapter boundary

The inspector receives a generic reactive binding from the application:

~~~typescript
export interface AudioDeviceInspectorBinding {
    readonly state: AudioDeviceInspectorState;
    setInputEnabled(enabled: boolean): Promise<IntentResult>;
    selectInputTarget(target: AudioDeviceTargetId): Promise<IntentResult>;
    setOutputEnabled(enabled: boolean): Promise<IntentResult>;
    selectOutputTarget(target: AudioDeviceTargetId): Promise<IntentResult>;
    setRecoveryPolicy(policy: AudioRecoveryPolicy): Promise<IntentResult>;
    setSampleRate(rate: number): Promise<IntentResult>;
    setBufferPolicy(policy: AudioBufferPolicy): Promise<IntentResult>;
    setFixedBufferFrames(frames: number): Promise<IntentResult>;
    refreshDevices(): Promise<void>;
}
~~~

This is a UI adapter, not a second audio engine API:

- golden_audio defines the backend-neutral state meanings and generates the data types.
- golden_audio_ui renders those meanings and owns interaction/accessibility behavior.
- The consuming application maps persistence and transport to the binding.
- Chataigne’s adapter resolves its connection parameters and sends golden_ui edit intents.
- The adapter may not implement fallback, assign device identity, negotiate formats, or rewrite invalid selections. Those decisions remain in golden_audio or the owning backend node.

Do not require every consumer to use Chataigne’s exact tree. Provide a reusable Golden node adapter helper that accepts declared parameter paths/IDs, while permitting another Golden application to implement the binding directly.

### Inspector UX

The generic custom inspector includes:

- Separate input and output enable controls.
- Backend-grouped device selectors using stable target IDs.
- System Default and persisted Missing selections.
- Clear Compiled, Available, Unavailable, Missing Server, Missing Driver, Permission Denied, Busy, and Failed status.
- Read-only active backend/device, sample rate, channel count, sample format, buffer frames, and estimated latency.
- Buffer policy, fixed-buffer control, recovery policy, and engine sample rate when the adapter exposes them.
- Manual refresh that requests discovery but never blocks the UI.
- Expandable diagnostic detail with copyable technical information; the primary UI remains human-readable.
- Keyboard navigation, visible focus, semantic labels, screen-reader status announcements, and no color-only state.

The inspector must work when:

- Input is disabled and output is enabled.
- Output is disabled and input is enabled.
- Both directions are disabled.
- The selected device is missing.
- A backend was compiled but its server/driver is absent.
- Discovery is in progress.
- The application is offline/headless and exposes only the null backend.

### Chataigne integration

- Add a thin Sound Card inspector adapter under apps/chataigne/ui.
- Register the reusable inspector for sound_card_module during current +page setup.
- Reuse AudioDeviceSelector inside SoundCardEditorPanel’s Devices section. Do not recreate a Chataigne-only selector.
- Let the generic inspector render the default children below its device section, excluding only child controls it already renders through the established defaultContent/defaultChildren snippets.
- Keep matrices, virtual-channel authoring, playback, analysis, and Chataigne diagnostics in the app-specific full editor.

### Package tests

- Exact node-type registration wins over the generic module inspector and unregister restores it.
- Input/output edits call only the adapter methods with stable target IDs.
- Missing, unavailable, permission-denied, busy, discovery, and ready states render correctly.
- Failed intents roll UI state back and present the structured error.
- Refresh cancels/ignores stale completion after inspector destruction.
- Keyboard and accessibility tests cover both selectors and status changes.
- A mock consumer renders and edits the inspector without importing any Chataigne file.
- Generated DTOs are current; a codegen-drift check fails CI if they differ.

## 17. Sound Card editor

Add apps/chataigne/ui/src/lib/panels/modules/SoundCardEditorPanel.svelte and focused components below a sound-card subdirectory.

### Generalize module editors first

Replace the Spatializer-only conditional in ModuleInspectorPanelHeader.svelte with an app-owned descriptor registry.

The registry maps module node type to:

- Panel type.
- Stable panel-ID prefix.
- Title builder.
- Action label.
- Icon.
- User-panel definition.

Use the same descriptor source from the inspector header and +page.svelte panel registration. Register Spatializer and Sound Card through it. Do not put this Chataigne registry in golden-ui.

### Editor sections

1. Devices, composed from golden_audio_ui AudioDeviceSelector
   - Input/output enable.
   - Grouped backend/device selection.
   - Missing and unavailable status.
   - Negotiated rate, channels, sample format, buffer, estimated latency, and connection state.
   - Backend availability explanations without exposing raw backend errors as the primary message.
2. Virtual channels
   - Add, remove, rename, reorder.
   - Output faders and master fader.
   - Stable identity retained during reorder and rename.
3. Device patch
   - Input physical-to-virtual matrix.
   - Output virtual-to-physical matrix.
   - Active device profile selector/history.
4. Monitoring
   - Virtual-input-to-virtual-output sparse gain matrix.
   - Authored routes remain visible but inactive while input is disabled.
5. Playback
   - Playback patch matrix.
   - Active voice count and read-only lifecycle list.
   - Ephemeral stop controls may use SendNodeEvent because they do not edit project state.
6. Analysis
   - Pitch settings and current pitch display.
   - Spectrum settings and Canvas spectrum.
   - Linear/log spacing and band meters.
7. Diagnostics
   - XRuns, underruns, overruns, decoder starvation, dropped analysis frames, queue pressure, drift ppm, bridge fill, render timing, and last structured errors.

### Matrix and meter implementation

- Virtualize matrix rows and columns. Do not mount one interactive component for every cell in a 256 by 256 matrix.
- Use Canvas for meters, spectrum, and dense matrix visualization.
- Use DOM controls for focused editing, keyboard access, labels, and assistive technology.
- Create a route with CreateUserItem and initial_params so source, destination, and gain arrive in one backend transaction.
- Update an existing gain with SetParam.
- Remove a route with RemoveNode.
- Wrap drag painting and multi-cell operations in BeginEdit/EndEdit and send batched intents.
- Never allocate labels or route IDs in TypeScript.
- Keep optimistic UI state keyed by the backend acknowledgement and roll it back on rejection.
- Use requestAnimationFrame and packed telemetry; do not create one Svelte store update per FFT bin.
- Add component tests for route creation, editing, removal, grouped undo, missing devices, telemetry teardown, keyboard use, and panel reopening.

## 18. Detailed implementation phases

### Phase 0 — Baseline, decisions, and progress record

Tasks:

- Confirm main SHA, clean/dirty worktree, submodule SHAs, and current toolchain.
- Read AGENTS.md, ARCHITECTURE.md, docs/reference/repository-layout.md, docs/reference/toolchain.md, module authoring/scripting guides, current MIDI runtime, command execution, UI intent, golden_ui node-inspector registry and component-props contract, module header, panel registration, npm package conventions, packaging, and CI files.
- Create docs/progress/golden-audio-sound-card.md.
- Create docs/architecture/golden-audio.md with the ownership and real-time contracts from this plan.
- Record dependency versions and licenses.
- Record the ASIO SDK licensing/distribution decision or mark the release gate blocked without blocking backend implementation.
- Add a short risk register for ASIO build prerequisites, JACK absence, PipeWire linkage, macOS microphone permission, callback plan destruction, device-clock drift, decoder cancellation, and UI telemetry scale.
- Record initial configurable limits rather than scattering constants.

Gate:

- Documentation links resolve.
- No code behavior changed.
- Progress record contains the exact baseline and NOT RUN entries for all future platforms.

Commit: docs(audio): lock golden_audio architecture and execution baseline

### Phase 1 — Crate scaffold and backend-independent contract

Tasks:

- Add crates/golden_audio to workspace members and workspace dependencies.
- Add the crate with no native default enabled until Phase 5.
- Implement strong IDs, gain/rate/frame wrappers, limits, configuration DTOs, errors, backend traits, null backend, mock backend, offline clock, command/event contract, clean shutdown, and the codegen-only foundation for app-agnostic device-inspector DTOs.
- Add crate README with public boundary and minimal null/offline example.
- Verify golden_audio has no dependency on any Golden or Chataigne crate.
- Add serde only where durable app-agnostic config benefits; do not make serialization the runtime model.

Tests:

- ID equality, ordering, formatting, and serde round-trip.
- Gain validation and dB/linear conversion.
- Limit validation.
- Null backend discovery/open/close.
- Clean shutdown and double shutdown.
- Public API compile test from an external-style integration test.

Gate:

- cargo check -p golden_audio --no-default-features --all-targets
- cargo test -p golden_audio --no-default-features
- cargo clippy -p golden_audio --no-default-features --all-targets -- -D warnings

Commit: feat(audio): add backend-neutral golden_audio foundation

### Phase 2 — Render buffers, routing, gains, and offline renderer

Tasks:

- Implement planar buffer storage, interleaved boundary conversion, buffer chunking, zeroing, saturating output conversion, and sample format tests.
- Implement authored virtual channels and four route classes.
- Implement deterministic RenderPlan compilation and destination-major sparse kernels.
- Implement monitoring, playback mixing, per-output faders, master, and output patch in the exact signal order.
- Implement sample-accurate gain ramps.
- Add the offline renderer and scalar reference renderer.
- Add allocation/deallocation instrumentation after warm-up.

Tests:

- Empty/silent graph.
- One-to-one and many-to-one routes.
- Fan-out and feedback-safe monitoring defaults.
- Route gain and total gain order.
- Variable hardware block sizes including 1, 17, 64, 127, 128, 511, and larger than internal scratch chunk.
- f32, i16, u16, i24/u24 where the backend exposes them.
- Randomized optimized-versus-reference property tests.
- No allocation or deallocation during render.
- No NaN or infinity reaches a device buffer from validated configuration.

Benchmarks:

- 8 by 8, 32 by 32, 128 by 128, and 256 by 256 channel configurations.
- Sparse route counts from 0 through 16,384.
- Block sizes 32 through 1,024.
- Silence, unity, and changing gains.

Gate:

- Focused tests pass.
- Benchmark baselines are written to the progress document without inventing a pass threshold after seeing only favorable results.

Commit: feat(audio): implement deterministic realtime render core

### Phase 3 — Callback-safe control plane and plan lifecycle

Tasks:

- Implement the bounded application-to-control queue and control worker.
- Implement the acknowledged one-pending-plan exchange.
- Implement coalesced gain mailboxes and ordered sequence barriers.
- Implement fixed voice-slot and analysis-frame ownership primitives without playback/analysis algorithms yet.
- Implement structured queue pressure and saturation events.
- Add a debug/test real-time thread guard.
- Prove all active and retired plan destruction occurs off callback during normal operation and controlled shutdown.

Tests:

- Rapid configuration generations apply only the newest valid generation.
- A failed generation leaves the previous plan active.
- Return-queue-full defensive path retains ownership without dropping.
- Commands remain ordered around plan swaps.
- Gain updates coalesce without crossing play/stop sequence barriers.
- Queue-full results are explicit and bounded.
- Shutdown during pending swap, device loss, and producer disconnect.
- Miri or sanitizer coverage for the plan exchange if any unsafe code is introduced.

Gate:

- No lock, allocation, deallocation, or log in callback tests.
- Stress test performs millions of swaps/controls with no leak, double drop, or stale use.

Commit: feat(audio): add bounded realtime control and plan exchange

### Phase 4 — Device model, mock recovery, and format negotiation

Tasks:

- Implement backend-neutral device descriptors, directions, channel keys, stable IDs, fingerprints, supported configurations, status, and profile keys.
- Define the canonical serializable AudioDeviceInspectorState projection and generate its TypeScript DTOs from Rust; keep actions out of the data DTO.
- Implement a deterministic negotiation policy for sample format, channel count, sample rate, and buffer.
- Implement the device supervisor, retry backoff with jitter, explicit missing status, two-phase switch state machine, and mock hotplug events.
- Implement default-device following as an explicit policy.
- Add input and output independent selection.
- Keep discovery off Chataigne’s engine thread.

Tests:

- Stable ID survives rename and re-enumeration.
- Fallback fingerprint disambiguates duplicate names as far as available metadata permits and reports ambiguity instead of guessing.
- Missing selected device remains selected.
- Strict policy does not silently fall back.
- Follow-default policy follows only the operating-system default.
- Profile lookup restores the matching route set.
- Negotiation is deterministic and rejects unsupported fixed requests clearly.
- Mock connect, disconnect, busy, permission denied, format change, server restart, and flapping.

Commit: feat(audio): add device discovery model and recovery supervisor

### Phase 5 — CPAL, ASIO, JACK, PipeWire, and toolchain integration

Tasks:

- Add target-specific CPAL 0.18.x dependencies and private backend adapter.
- Map every CPAL host to BackendId and structured capability/status.
- Implement discovery, physical channel descriptions, negotiation, input/output streams, timestamps, callback conversion, error callbacks, and supervisor recovery.
- Enable WASAPI and ASIO plus JACK on Windows.
- Enable CoreAudio plus JACK on macOS.
- Enable ALSA, JACK, native PipeWire, and realtime-dbus on Linux.
- Make golden_audio default features the official full desktop feature set.
- Update tools/bootstrap/toolchain.json, docs/reference/toolchain.md, tools/dev.ps1, tools/dev.sh, Linux prerequisite lists, CI images/jobs, dependency caches, and release scripts.
- Ensure no audio prerequisite or feature wiring appears in apps/chataigne.
- Add a backend probe example or xtask command that reports compiled, available, unavailable, and failed hosts without opening a stream.
- Verify cargo run commands need no audio-specific flag after setup.

Tests and evidence:

- Null/offline tests still work with no default features.
- Full-feature compile on Windows x64, macOS x64/arm64, Linux x64, and supported Linux arm target.
- Clean startup with no ASIO driver.
- Clean startup with no JACK library/server.
- Native PipeWire unavailable and available cases.
- At least one real stream smoke per required default backend.
- Record remote checks as NOT RUN until CI or hardware evidence exists.

Gate:

- Phase is not complete if ASIO or JACK is postponed.
- PipeWire may be marked blocked only with a concrete compile/package blocker, an owned follow-up issue, and no leakage into app code.
- cargo run, cargo xtask watch, cargo run -- --dev, and cargo run -- --headless receive bounded smoke tests on the development platform.

Commit: feat(audio): ship native desktop audio backends

### Phase 6 — Clock bridges, null clock, switching, and recovery

Tasks:

- Implement input ring, adaptive ASRC, drift controller, output conversion, render clock ownership, null clock, clock handoff, underrun/overrun policy, and estimated latency.
- Use Rubato’s preallocated process-into-buffer path.
- Keep the engine sample rate stable across hardware changes.
- Implement stream prime, fade-down/fade-up, and retire behavior.
- Promote callback thread priority through backend-supported mechanisms.

Tests:

- Simulated input clocks at -1,000 through +1,000 ppm.
- Fill controller convergence without oscillation.
- Underflow, overflow, discontinuity, timestamp loss, and abrupt device-rate change.
- Output lost during active render switches to null clock with monotonic sample time.
- Output reconnect resumes current timeline.
- Input-only, output-only, full duplex, different devices, and different hardware rates.
- Repeated device switching leaves no threads or streams behind.

Soak:

- One-hour mock drift/reconnect test with zero unbounded growth.
- Real-device 30-minute smoke on each available default backend.

Commit: feat(audio): add clock-domain bridging and seamless recovery

### Phase 7 — File playback and voice lifecycle

Tasks:

- Add selected Symphonia codecs and generated extension metadata.
- Implement probe, decoder scheduler, cache, resident decode, streaming read-ahead, worker-side resampling, voice slots, routing, start/stop ramps, sequence/generation cancellation, callbacks, and null-clock advancement.
- Implement cache eviction only on non-real-time threads.
- Implement bounded admission and useful capacity errors.
- Add deterministic audio fixtures in permitted formats; keep fixtures small.

Tests:

- Every advertised extension probes and decodes.
- Unsupported/corrupt/truncated files fail without panic.
- Mono, stereo, and multichannel default routing.
- Same-ID replacement before load, during load, while playing, and at end-of-file.
- Stop before load completes.
- Stop All races with many decoders.
- Rapid repeated play/stop from multiplex-like sequences.
- Stream starvation and recovery.
- Output loss/reconnect does not restart playback.
- Cache hit, invalidation, eviction, and budget enforcement.
- No callback allocation or final asset drop.

Benchmarks:

- 1, 16, 64, 128, and 256 resident voices.
- Mixed resident and streamed voices.
- Decoder throughput and cache memory.

Commit: feat(audio): add ordered asynchronous file playback

### Phase 8 — RMS, pitch, spectrum, and observation transport

Tasks:

- Implement render-path RMS/peak accumulation.
- Implement analysis frame pool and worker scheduling.
- Implement pitch adapter/YIN, real FFT, windows, band generation, normalization, attack/release, and latest observation snapshots.
- Add diagnostics for dropped analysis frames and worker time.
- Ensure disabling an analyzer removes its work without rebuilding unrelated device state.

Tests:

- Exact RMS for DC, sine, square, silence, and multichannel signals.
- dBFS floor and clip behavior.
- Pitch fixture error/confidence suite.
- FFT bin-centered and off-bin tones.
- Linear/log bands, min/max range, Nyquist clipping, smoothing, and FFT-size changes.
- Worker overload drops analysis but never audio.
- Observation generations never mix old topology with new values.

Commit: feat(audio): add realtime-safe metering and analysis

### Phase 9 — Chataigne Sound Card schema and persistence

Tasks:

- Add golden_audio.workspace to apps/chataigne with no backend feature list.
- Implement the module and child node types under the audio/sound_card path.
- Extend ModuleBase and current authoring permissions correctly.
- Add custom same-module reference filters for virtual input/output routes and channel-volume commands.
- Materialize two input/two output defaults, meter projections, default device profiles, and analysis output structures with NodeTree/AddNodeTree.
- Persist authored profiles and retain missing enum choices.
- Configure ModuleCommandTester with only the five Sound Card command types.
- Add project fixtures and sparse round-trip tests.

Gate:

- Generated app-node catalog contains the module and commands without handwritten duplication.
- Module creates, saves, reloads, duplicates, removes, and recovers with a null backend.
- Existing module tests still pass.

Commit: feat(chataigne): add persistent Sound Card module model

### Phase 10 — Chataigne runtime adapter and live values

Tasks:

- Implement conversion from tree snapshots to golden_audio configuration.
- Keep UUID/NodeId resolution in Chataigne.
- Implement dirty-generation coalescing, runtime lifecycle, enable/disable, event polling, observation batching, warnings, and ModuleBase readiness.
- Implement device option refresh without replacing persisted missing selections.
- Implement backend-owned dependent-route cleanup or explicitly visible unresolved routes with atomic undo behavior.
- Add packed latest-only Sound Card UI telemetry DTO/event and generated TypeScript.
- Implement the thin Chataigne AudioDeviceInspectorAdapter that maps Sound Card connection/value nodes to the generic golden_audio inspector state and golden_ui intents. Keep it unregistered until the reusable package exists in Phase 12.
- Add diagnostic/value nodes and update epsilon behavior.

Tests:

- A matrix drag produces coalesced configuration generations.
- Invalid route does not replace last valid audio plan.
- Missing devices preserve topology and patches.
- Device return restores.
- Channel rename/reorder leaves IDs and routes intact.
- Channel delete behavior is atomic and undoable.
- Values update at bounded rate and do not rebuild state-machine snapshots.
- Module removal and project change cleanly shut down workers.

Commit: feat(chataigne): connect Sound Card nodes to golden_audio

### Phase 11 — Commands, multiplexing, scripts, and callbacks

Tasks:

- Add all five command node types.
- Implement command_execute_snapshot use for every parameter.
- Implement runtime admission and structured results.
- Add script descriptors, host dispatch, callbacks, template, snippets, and guide updates.
- Use transient playback lifecycle events for script delivery so reconnect/replay cannot refire stale callbacks.

Tests:

- Focused command semantics from Section 15.
- Multiplex lanes play different paths and IDs from one command node.
- Same lane replacement and cross-lane independence.
- setChannelVolume rejects input channels, foreign modules, deleted channels, and physical strings.
- Script method and callback integration.
- Stop commands work while output is missing.

Commit: feat(chataigne): expose Sound Card commands and scripting

### Phase 12 — Reusable golden_audio_ui device custom inspector

Tasks:

- Add packages/golden-audio-ui to the npm workspace through the existing packages wildcard.
- Add its golden_ui and Svelte 5 peer/workspace dependencies without duplicating the UI toolchain.
- Wire the golden_audio Rust-to-TypeScript device contract into the existing codegen/check workflow.
- Implement AudioDeviceSelector, backend groups, stream summary, status/error presentation, and GoldenAudioDeviceInspector with Svelte 5 runes.
- Implement the explicit adapter registry and registerGoldenAudioDeviceInspector helper on top of golden_ui’s existing exact-node-type registry.
- Add the mock adapter and standalone consumer harness.
- Register sound_card_module from Chataigne with the thin adapter written in Phase 10.
- Render default Sound Card inspector children below the reusable device UI without rendering the same connection parameters twice.
- Document how another Golden application adopts the package with either a node-path adapter or a direct binding.

Tests:

- Package builds and type-checks independently of Chataigne.
- Exact sound_card_module registration overrides the generic module item-kind inspector only for that type.
- Registration/unregistration and test reset are deterministic.
- All ready, disabled, discovering, missing, unavailable, permission-denied, busy, and failed states.
- Stable target IDs, failed-intent rollback, stale refresh cancellation, keyboard navigation, focus, labels, and live status.
- Mock consumer imports golden_audio_ui and golden_ui but no Chataigne package.
- Generated bindings drift check.
- Existing generic golden_ui inspector tests remain green.

Commit: feat(audio-ui): add reusable Golden audio device inspector

### Phase 13 — Module-editor registry and full Sound Card UI

Tasks:

- Generalize module editor descriptors.
- Migrate Spatializer without changing its UX.
- Add SoundCardEditorPanel and focused Svelte 5 components.
- Compose golden_audio_ui AudioDeviceSelector for Devices; do not implement a second selector.
- Add virtual channels, three matrices, faders, monitoring state, analysis, and diagnostics.
- Implement viewport virtualization, Canvas rendering, grouped edits, optimistic acknowledgement handling, teardown, and accessibility.
- Add a deterministic mock-audio UI mode used only for tests/product evidence.

Tests:

- Existing Spatializer editor still opens.
- Sound Card panel opens for the correct module and retains panel identity.
- Matrix create/update/remove and undo/redo.
- Device missing/reappearing UI.
- Input-disabled monitoring UI retains authored cells but shows inactive signal flow.
- Packed meter/pitch/spectrum telemetry.
- Large 256 by 256 matrix does not create 65,536 DOM controls.
- No subscription, timer, animation frame, or Canvas leak after close/reopen.
- npm check, lint, test, and build.

Commit: feat(ui): add scalable Sound Card editor

### Phase 14 — Performance, robustness, and product evidence

Tasks:

- Add Criterion or the repository-standard benchmark harness.
- Add allocation, render-time, queue-pressure, memory, reconnect, and observation metrics.
- Add property and stress tests.
- Add deterministic product-evidence scenario using null/mock devices, synthetic input, active playback, monitoring routes, meters, pitch, and spectrum.
- Render and inspect the Sound Card UI at normal and narrow desktop sizes.
- Profile before optimizing. Keep scalar fallback and document any platform-specific acceleration.

Reference workloads:

| Workload | Channels/routes | Voices | Analysis |
| --- | --- | --- | --- |
| Small | 8 input, 8 output, 16 routes | 4 | RMS |
| Medium | 32 input, 32 output, 128 routes | 32 | RMS, pitch, 64 bands |
| Large | 128 input, 128 output, 1,024 routes | 128 | RMS, 4 pitch taps, 256 bands |
| Extreme offline | 256 input, 256 output, 16,384 routes | 256 | Maximum configured taps |

Qualification targets:

- Zero render-path allocations/deallocations after warm-up.
- Zero callback locks and blocking calls.
- On named reference hardware, render p99 below 50 percent of the hardware block deadline for the applicable workload and p99.99 below 80 percent.
- Zero XRuns caused by the engine in a one-hour supported reference workload after warm-up.
- Bounded memory at all configured capacity limits.
- Chataigne engine tick and UI event rates remain within existing performance contracts.

Record hardware, OS, backend, driver, rate, buffer, compiler profile, and commit with every number.

Commit: perf(audio): qualify Sound Card scale and realtime behavior

### Phase 15 — Cross-platform release qualification and documentation

Tasks:

- Run the full Windows, macOS, and Linux matrix at the exact commit.
- Qualify package startup with optional servers/drivers missing.
- Qualify required real hardware/backend combinations.
- Verify macOS permissions and signed package behavior.
- Verify Linux packaged native-library behavior.
- Verify ASIO/JACK/PipeWire availability reporting.
- Run dependency, license, unused-dependency, formatting, Clippy, Rust tests, UI tests, package checks, and all run modes.
- Update ARCHITECTURE.md, docs/README.md, repository layout, toolchain, module authoring, scripting, operations, troubleshooting, release-readiness, and golden_audio README.
- Mark the progress document complete only when every mandatory backend row is PASS. Leave any unavailable hardware row NOT RUN and do not call the feature complete.

Commit: docs(audio): complete cross-platform Sound Card qualification

## 19. Validation command matrix

Use commands appropriate to the phase and platform. Do not blindly enable an impossible target feature on another OS.

### Formatting

~~~text
cargo fmt --all
cargo fmt --manifest-path crates/golden_core/Cargo.toml --all
cargo fmt --all -- --check
cargo fmt --manifest-path crates/golden_core/Cargo.toml --all -- --check
~~~

### Focused Rust

~~~text
cargo check -p golden_audio --no-default-features --all-targets
cargo test -p golden_audio --no-default-features
cargo check -p golden_audio --all-targets
cargo test -p golden_audio
cargo clippy -p golden_audio --all-targets -- -D warnings
~~~

Run full/all native features on the matching platform and null features everywhere.

### Chataigne and workspace

~~~text
GC_SKIP_UI_BUILD=1 cargo test -p Chataigne2
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
~~~

### UI

~~~text
npm run check --workspace golden_audio_ui
npm run test --workspace golden_audio_ui
npm run build --workspace golden_audio_ui
npm run check
npm run lint
npm test
npm run build
~~~

If golden_audio_ui uses the root lint configuration rather than package-local scripts, add one root workspace script and invoke that single documented command. Do not leave the reusable package outside normal CI merely because Chataigne happens to import it.

### Product modes

Run each through a bounded smoke harness that waits for readiness, records output, and terminates cleanly:

~~~text
cargo run
cargo xtask watch
cargo run -- --dev
cargo run -- --headless
npm run package:check
~~~

### Dependency and release hygiene

~~~text
cargo deny check
cargo machete
~~~

### Platform-specific backend evidence

- Windows: WASAPI shared/exclusive where supported, ASIO, JACK, no-ASIO-driver startup, no-JACK startup.
- macOS: CoreAudio input/output, JACK, microphone denied/allowed, no-JACK startup, x64 and arm64 package.
- Linux: ALSA, JACK, native PipeWire, server restart, realtime-priority denied/allowed, packaged client libraries.

## 20. Required test inventory

Before completion, the repository must contain automated coverage for:

- Configuration validation and stable identity.
- Route compiler/reference equivalence.
- Gain smoothing and signal order.
- Allocation/deallocation-free render.
- Plan swap ownership.
- Queue saturation and command ordering.
- Device identity, missing targets, default following, reconnect, and flapping.
- Clock drift, rate mismatch, null clock, and handoff.
- File formats, corruption, cancellation, replacement, streaming starvation, and cache limits.
- RMS/pitch/spectrum correctness.
- Chataigne sparse persistence and duplicate-module identity.
- Command multiplex overrides.
- Script methods/callbacks.
- Standalone golden_audio_ui custom-inspector state, adapter, registration, accessibility, and generated-contract drift.
- UI matrix authoring, undo/redo, virtualization, telemetry, and cleanup.
- Full product startup with a mock/null backend.

Add fuzz or property testing for authored route configurations and corrupt media probing if the repository’s CI budget permits. Never run an unbounded fuzzer as an ordinary CI step; retain a deterministic corpus and bounded smoke duration.

## 21. Definition of done

The project is complete only when all of the following are true:

- golden_audio is a documented standalone workspace crate with no Chataigne or Golden Core dependency.
- golden_audio_ui is a documented reusable Svelte 5 package built on golden_ui, with a working custom device inspector and no Chataigne dependency.
- The generic custom inspector handles input/output enablement, stable backend/device selection, missing/unavailable devices, negotiated stream details, and errors through an application adapter.
- Chataigne registers the reusable inspector for sound_card_module and composes the same selector into the full Sound Card editor rather than duplicating it.
- Chataigne has one normal dependency on golden_audio and no native-backend feature wiring.
- WASAPI, ASIO, CoreAudio, ALSA, and JACK are implemented and qualified.
- Native PipeWire is implemented and qualified, or an explicit evidenced blocker remains without compromising the ownership boundary.
- Missing optional runtime components never prevent app startup.
- Virtual channel references remain stable across device/backend changes and project reloads.
- Device patch, monitoring matrix, playback patch, faders, and master gain work with persisted undoable edits.
- File playback obeys exact ID and ordering semantics, including multiplexed paths/IDs and output loss.
- RMS, pitch, and spectrum values are linkable Chataigne parameters and the custom UI remains efficient at scale.
- The callback satisfies the allocation, deallocation, lock, blocking, logging, and ownership rules.
- Device-clock drift and null-clock transitions are tested.
- Commands, scripts, callbacks, templates, snippets, and documentation are complete.
- Spatializer’s existing editor UX remains intact after registry generalization.
- All four required run workflows still work without audio-specific flags.
- Every phase has a progress entry, exact test evidence, and a focused commit.
- No mandatory row is marked complete on the basis of an assumption, compilation on a different platform, or an untested mock.

## 22. Final handoff format

At the end, Codex must report:

1. Exact final commit SHA and branch.
2. Architectural summary and public golden_audio API.
3. File-level summary by crate/app/UI/tooling/docs.
4. Backend qualification table with PASS, FAIL, BLOCKED, or NOT RUN.
5. Test and command table with exact results.
6. Performance table with hardware and workload.
7. Remaining risks and deliberately deferred DAW features.
8. A statement that existing user changes were preserved.

Do not end with “everything should work.” End with evidence.
