# Golden Audio Architecture

`golden_audio` is the reusable, application-agnostic audio engine. Chataigne owns the Sound Card
product model and maps persistent project nodes to this engine. `golden_audio_ui` is the reusable
Svelte presentation companion for device selection and stream status.

## Dependency direction

```text
CPAL / native audio SDKs / codecs / DSP crates
                         |
                         v
                   golden_audio
                    |         |
                    |         +---- generated device/status DTOs
                    |                              |
                    v                              v
        Chataigne Sound Card adapter       golden_audio_ui
                    |                       based on golden_ui
                    +---------------+--------------+
                                    |
                                    v
                         Chataigne inspector/editor
```

The dependencies only point downward:

- `golden_audio` does not depend on Golden Core, Tauri, Svelte, or Chataigne.
- `golden_audio_ui` may depend on `golden_ui`, Svelte 5, and generated app-agnostic audio DTOs.
- Chataigne depends on `golden_audio` through its backend-neutral public API. It never imports CPAL,
  JACK, PipeWire, ASIO, Symphonia, Rubato, or other private engine dependencies.
- Audio-specific UI policy stays out of `golden_ui`; Sound Card policy stays out of both reusable
  Golden packages.

## Ownership

`golden_audio` owns device discovery, stable device identities, format negotiation, streams,
recovery, clock bridging, virtual buses, immutable render plans, routing and gain kernels, playback,
analysis, bounded control exchange, diagnostics, offline rendering, and deterministic null/mock
backends.

Chataigne owns the `sound_card_module` tree, stable node-to-audio identity mapping, authored device
profiles and routes, lifecycle integration, command and script surfaces, project persistence,
reference filters, telemetry adaptation, and the full Sound Card editor.

`golden_audio_ui` owns the generic audio device inspector and selector, status rendering,
accessibility behavior, and its application binding contract. An application adapter may translate
node paths and intents, but may not invent fallback policy, device identity, format negotiation,
route cleanup, or domain defaults.

## Stable authored model and compiled runtime

`AudioConfiguration` is an authored graph containing stable IDs. A worker validates it and compiles
it to a dense immutable `RenderPlan`:

1. Stable channel, route, device, playback, and analysis IDs are resolved off the audio callback.
2. Decibel gains are validated and converted to finite linear targets.
3. Sparse routes are compiled into deterministic destination-major spans.
4. All render, conversion, bridge, voice, and analysis-frame storage is preallocated from explicit
   `EngineLimits`.
5. A rejected generation leaves the last valid plan active.

Virtual channel identity is independent of label, order, backend, device, and physical-channel
index. Chataigne derives it from the persistent virtual-channel node UUID. Physical indices never
cross the reusable engine boundary into project references, commands, or scripts.

## Signal and clock model

```text
physical input -> format conversion -> input clock bridge -> input patch -> virtual inputs
                                                                        |          |
                                                                        |          +-> analysis
                                                                        v
                                                                 monitoring matrix
decoded/resampled voices -> playback patch ------------------------------+
                                                                        |
                                                                        v
                                                               virtual outputs
                                                                        |
                                              output faders -> master -> output patch
                                                                        |
                                      output rate/format conversion -> physical output
```

The configured engine sample rate defines timeline time. The ready output callback is the render
clock; a software null clock takes over while output is disabled, missing, or switching. Exactly one
clock is authoritative. Input and output are separate clock domains unless a backend proves
otherwise. Input monitoring crosses a bounded ring through adaptive asynchronous resampling with
observable fill, drift, underflow, overflow, and latency.

Clock switching is two phase: prepare and prime the replacement, switch at an engine block boundary,
then retire the old stream away from the callback. Playback advances on the null clock, so output
loss and recovery never retrigger a voice.

The implemented input bridge owns a bounded SPSC sample ring, an asynchronous Rubato resampler with
preallocated input/output storage, and a bounded PI drift controller. The input callback only
validates and copies a complete interleaved callback block. The render side adjusts the resampling
ratio from ring fill, emits silence on starvation, and publishes coalesced fill, drift, latency,
underflow, overflow, timestamp-loss, and discontinuity observations. A device-rate change rebuilds
the resampler and flushes the old clock domain on the control thread; it never changes engine time.

`RenderClockCoordinator` is the single authority gate. Non-authoritative callbacks cannot advance
the timeline. A ready replacement fades the old output down, becomes authoritative on a block
boundary, fades up, and makes the old generation eligible for control-thread retirement. If the old
stream fails during handoff, a primed replacement is promoted immediately; otherwise the
deadline-driven null clock takes authority without resetting the sample counter.

## Playback boundary

File playback is a three-stage ownership pipeline:

1. The control worker orders play, stop, stop-all, gain, and configuration commands and assigns a
   monotonic sequence to each request.
2. A fixed decoder-worker pool privately uses Symphonia to probe and decode, resamples to the engine
   rate, maintains the bounded resident cache, and primes bounded stream rings.
3. The render callback activates prebuilt fixed-slot voices, reads planar resident assets or
   streaming rings, applies bounded start/stop ramps and explicit routes, and returns completed
   ownership for control-thread destruction.

Playback-ID cancellation watermarks make a late decoder result stale. Reusing an ID therefore
replaces its pending or active generation without allowing an older load to start. Stop-all advances
a global watermark and cancels every pending worker request. Worker and result queues are bounded;
capacity pressure is a structured failure rather than a new thread or unbounded allocation.

Resident assets are immutable `Arc`-owned planar buffers keyed by canonical path metadata, track,
and engine sample rate. Cache lookup, insertion, invalidation, and eviction happen only on decoder
or control threads. Large files use bounded read-ahead and emit silence on starvation without
blocking the callback. Both source forms advance against the same engine sample timeline, including
while the null clock owns output, so reconnect never restarts a voice.

The advertised file-extension table is authored once in Rust and emitted into the generated
TypeScript contract. Symphonia remains an optional private implementation dependency behind the
default `playback` feature; the backend-neutral voice and stream primitives remain usable without
default features.

## Analysis boundary

Analysis is split at the callback boundary:

- The render plan resolves stable tap IDs and virtual-input IDs to compact indices.
- The callback accumulates virtual-input RMS/peak before monitoring and virtual-output RMS/peak
  after output/master gain. RMS windows are frame-counted and therefore independent of backend
  callback partitioning; lock-free meter snapshots publish at the configured 1–60 Hz rate.
- Tap capture copies newest bounded windows into a preallocated frame pool. A full worker queue
  retains the newest pending frame and counts pressure instead of blocking audio.
- One dedicated worker performs YIN pitch detection and real-valued FFT analysis. It owns all
  difference arrays, FFT plans/scratch, window coefficients, band geometry, result allocation, and
  observation locks.

Pitch configuration includes frame size, frequency bounds, RMS power threshold, YIN threshold, and
confidence threshold. Results include validity, frequency, confidence, MIDI note, note name, and
cents. The focused in-crate YIN kernel was selected over another dependency because it keeps
scratch ownership explicit and passed the deterministic tone, harmonic, detuning, silence, noise,
low-amplitude, and chirp suite.

Spectrum configuration supports power-of-two sizes from 256 through 16,384 frames, Hann and
Blackman-Harris windows, zero/50/75-percent overlap, linear and logarithmic 1–256-band geometry,
Nyquist clipping, normalized single-sided band amplitude, and attack/release smoothing. Band
identity is the stable band index; frequency-range changes update its low, center, and high metadata.

Analysis snapshots contain one topology generation. The worker discards stale generations, and
readers combine generation-fixed tap results with seqlock-protected meter banks. Disabling one tap
atomically stops capture and clears its latest result without rebuilding the render or device
topology. Diagnostics expose captured, processed, dropped, and stale frame counts plus total and
maximum worker time.

## Hard real-time contract

The render and input callbacks must not:

- allocate, reallocate, or free heap memory;
- lock, wait, block, log, or format text;
- discover or open devices, decode or read files, resolve IDs, inspect Chataigne state, or compile
  routes;
- panic for a recoverable error; or
- drop the final owning reference to plans, assets, streams, or other heap-backed resources.

Callback-owned code uses planar `f32` buffers, preallocated scratch memory, bounded single-producer
single-consumer queues, fixed voice slots, fixed analysis-frame slots, and one active immutable
render plan. The control side submits at most one pending boxed plan. At a block boundary the
callback swaps the plan and returns the retired plan. If the return queue is full, a fixed callback
slot retains ownership and further swaps are declined until the control side catches up.

Tests guard both allocation and deallocation after warm-up. Optimized routing is compared with a
scalar reference. Analysis overload drops analysis frames; playback starvation emits silence;
neither may delay rendering.

## Device recovery contract

A selection stores backend plus stable device identity, with a fallback fingerprint and last-known
label only where the backend lacks a durable ID. A missing selected device remains selected and is
reported as `Missing`. Strict recovery waits and renders silence. Following the operating-system
default is a separate explicit policy.

Input and output selections are independent. Discovery and stream supervision run outside
Chataigne's engine thread. Optional servers, drivers, permissions, or priority elevation failures
become structured statuses and diagnostics, never application startup failures.

## Native host boundary

CPAL 0.18.1 is confined to `golden_audio::backend::cpal`. Public host enumeration returns boxed
`AudioBackend` values, and callback data crosses the boundary only as Golden sample buffers and
`AudioCallbackTimestamp`. The adapter maps stable CPAL device IDs, descriptions, physical-channel
counts, supported formats, default devices, and structured errors into Golden contracts.

The callback owns its `AudioStreamHandler`. Primitive formats are borrowed directly without
allocation. CPAL's 24-bit wrapper samples are converted through stream-owned, preallocated `f32`
scratch storage, so those dependency types do not enter the public API. Output buffers are silenced
before user processing, and callback errors publish fixed atomic codes for control-side inspection;
callbacks do not allocate, format, log, or destroy control-owned state.

The ordinary `desktop` feature uses the native OS host. Platform qualification features add ASIO,
JACK, native PipeWire, and real-time DBus support without changing application dependencies. The
backend probe enumerates hosts and devices but does not open a stream. The separate smoke example
opens the default output, writes silence for 100 ms, and closes it.

## Public surface

The small backend-neutral surface is centered on:

- `AudioEngineBuilder`, `AudioEngine`, `AudioControl`, `AudioEventReceiver`, and
  `AudioObservationReader`;
- authored `AudioConfiguration` and validated `EngineLimits`;
- `AudioCommand` for configuration, ordered playback, smoothed gains, enablement, and shutdown;
- `AudioEvent` and coalesced observation snapshots;
- `AudioBackend`, `AudioStream`, and `AudioStreamHandler` for backend-neutral discovery and
  callback integration;
- strong IDs and validated `GainDb`, `SampleRate`, and `FrameCount` values; and
- structured `AudioError` categories that preserve a stable category plus human-readable context.

No native backend, codec, or DSP implementation type appears in a public signature.

## Product defaults

The Sound Card module starts with output enabled on the system default, input disabled, two virtual
inputs, two virtual outputs, empty monitoring, one-to-one compatible device patches, 0 dB faders and
master, a 48 kHz engine rate, and a 128-frame internal block. These are Chataigne authoring defaults,
not hard-coded policy in the reusable UI.

See the implementation and evidence ledger in
[Golden Audio and Sound Card progress](../progress/golden-audio-sound-card.md).
