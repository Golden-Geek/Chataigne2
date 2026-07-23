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
