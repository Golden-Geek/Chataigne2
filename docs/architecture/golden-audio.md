# Golden Audio Architecture

`golden_audio` is the reusable, application-agnostic audio engine. Chataigne owns the Sound Card
product model and maps persistent project nodes to this engine. `golden_audio_ui` is the reusable
Svelte presentation companion for driver/device selection, stream status, and channel-routing
presentation.

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

`golden_audio` owns host-scoped device discovery, stable device identities, format negotiation,
streams, recovery, clock bridging, internal buses, immutable render plans, routing and gain kernels,
playback, analysis, bounded control exchange, diagnostics, offline rendering, and deterministic
null/mock backends.

Chataigne owns the `sound_card_module` tree, the one-driver product policy, stable node-to-audio
identity mapping, authored channels and direct routes, lifecycle integration, command and script
surfaces, project persistence, reference filters, telemetry adaptation, and the Sound Card editor.

`golden_audio_ui` owns generic audio driver/device controls, status rendering, the SVG channel patch
bay, accessibility behavior, and application binding contracts. An application adapter may
translate node paths and typed intents, but may not invent fallback policy, device identity, format
negotiation, channel counts, route cleanup, or domain defaults.

## Stable authored model and compiled runtime

`AudioConfiguration` is an authored graph containing stable IDs. A worker validates it and compiles
it to a dense immutable `RenderPlan`:

1. Stable channel, route, device, playback, and analysis IDs are resolved off the audio callback.
2. Decibel gains are validated and converted to finite linear targets.
3. Sparse routes are compiled into deterministic destination-major spans.
4. All render, conversion, bridge, voice, and analysis-frame storage is preallocated from explicit
   `EngineLimits`.
5. A rejected generation leaves the last valid plan active.

Channel identity is independent of label, order, driver, device, and physical-channel index.
Chataigne derives it from the persistent channel node UUID. The engine may describe these stable
logical endpoints as virtual channels internally, but product UI, project labels, commands, and
scripts use the simpler input/output Channel terminology. Logical channel references use stable
UUID-derived IDs. Backend-neutral `PhysicalChannelKey` values are the persisted endpoints of patch
routes; backend-native device handles and array indices do not enter the authored graph.

## Signal and clock model

```text
physical input -> format conversion -> input clock bridge -> input routing -> input channels
                                                                          |          |
                                                                          |          +-> analysis
                                                                          v
                                                                   monitoring routes
decoded/resampled voices -> playback routing -----------------------------+
                                                                          |
                                                                          v
                                                                  output channels
                                                                          |
                                              output faders -> master -> output routing
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

- The render plan resolves stable tap IDs and logical input-channel IDs to compact indices.
- The callback accumulates input-channel RMS/peak before monitoring and output-channel RMS/peak
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

Each Sound Card has one shared Audio Driver selection. Selecting `None` registers, discovers, and
opens no native host; both hardware directions are disabled while the null clock keeps engine time
available to non-hardware work. Selecting a driver registers and discovers only that exact host.
Changing it retires the old host before the replacement becomes eligible, so an unselected ASIO,
JACK, PipeWire, CoreAudio, ALSA, or WASAPI host cannot reserve devices or start a driver as a side
effect of creating the module.

Input Device and Output Device are selected independently within the active host, and each direction
has a `None` choice. An explicit device selection stores stable identity, fallback fingerprint, and
last-known label. If it disappears, that direction stops, retains the requested target, and waits
for the same device; it never falls through to unrelated hardware. A system-default selection may
follow operating-system default changes where the host provides that concept. Recovery restores an
explicit user choice when it becomes available again.

Sample Rate and Buffer Size expose `Automatic` plus backend-projected compatible choices. The
backend derives those choices from the active input/output capabilities, intersects them when both
directions are active, and uses the remaining direction when the other is `None`. The UI never
guesses a rate, buffer, fallback, or compatibility policy.

Discovery and stream supervision run outside Chataigne's engine thread. Optional servers, drivers,
permissions, or priority elevation failures become structured statuses and diagnostics, never
application startup failures.

## Native host boundary

CPAL 0.18.1 is confined to `golden_audio::backend::cpal`. The compiled host catalog exposes
side-effect-free IDs and labels, while only the selected host becomes a registered
`AudioBackend`. Callback data crosses the boundary only as Golden sample buffers and
`AudioCallbackTimestamp`. The adapter maps stable CPAL device IDs, descriptions, physical-channel
counts, supported formats, default devices, and structured errors into Golden contracts.

The workspace vendors CPAL 0.18.1 with one narrow ASIO lookup patch. Upstream's default
`device_by_id` walks the installed device iterator, which loads ASIO drivers in registry order.
The patched ASIO host builds a single-name iterator for the requested `DeviceId`, so catalog
discovery stays name-only and probing or opening one selection never initializes preceding
drivers. The exact delta and removal condition are recorded in the
[vendor patch note](../../vendor/cpal-0.18.1/CHATAIGNE_PATCH.md). Remove the vendor patch when
upstream provides the equivalent exact lookup.

`AudioBackend::id` is the side-effect-free identity path used by registration, validation, and
routing. Descriptor and device discovery may initialize a native host, so the managed engine calls
them only for the selected host and only from its control worker. Constructing a module with Audio
Driver `None`, or choosing another compiled driver, therefore never probes ASIO or any other
unselected host.

The callback owns its `AudioStreamHandler`. Primitive formats are borrowed directly without
allocation. CPAL's 24-bit wrapper samples are converted through stream-owned, preallocated `f32`
scratch storage, so those dependency types do not enter the public API. Output buffers are silenced
before user processing, and callback errors publish fixed atomic codes for control-side inspection;
callbacks do not allocate, format, log, or destroy control-owned state.

The reusable crate's ordinary `desktop` feature uses the native OS host. Chataigne's default
feature set additionally enables ASIO on Windows. Platform qualification features add JACK, native
PipeWire, and real-time DBus support without changing application dependencies. The backend probe
enumerates hosts and devices but does not open a stream. The separate smoke example opens the
default output, writes silence for 100 ms, and closes it.

## Sound Card product model

The module tree presents three small sections:

1. **Connection** contains Audio Driver, Input Device, Output Device, Sample Rate, Buffer Size, and
   direct Input Routing / Output Routing containers. No Device Profile concept is exposed or
   persisted as an editable user model. Routing nodes remain structurally stable for persistence,
   while their custom inspectors are absent when that direction's device is `None`.
2. **Parameters** contains master and per-channel input/output volumes plus a Processing container.
   Direction parameter roots remain stable and are hidden by their inspectors while unused. Pitch
   Detection and Spectral Analysis are booleans, disabled by default.
3. **Values** mirrors active input/output master and channel levels. Pitch Detection and Spectral
   Analysis value containers exist only while their corresponding processing booleans are enabled;
   unused direction Value roots are removed from the tree.

The user authors Input Channels and Output Channels through ordinary parameter edits. The backend
owns stable channel identities, default labels, descendant reconciliation, route validation and
cleanup, and topology materialization. One stabilized edit batch reconciles a count change, and
newly available stereo endpoints receive default one-to-one left/right routing. The UI may edit a
channel label and request a route, but it does not allocate nodes, labels, IDs, or hidden route
parameters.

## Chataigne runtime integration

The Sound Card module owns the application adapter and queues reusable `AudioEngine` construction
for the selected Audio Driver on a dedicated lifecycle worker. Node creation does not wait for
native host initialization, and Audio Driver `None` does not construct a hardware backend.
Completed runtimes cross back through a pending-result channel; stale driver/format generations and
retired runtimes return to that worker for shutdown, so replacement, removal, and project drop do
not join audio workers on Chataigne's engine thread. Startup failures use bounded reconnect backoff.

Tree snapshots are requested only for dirty authored configuration after the requested runtime is
ready. The adapter converts persistent UUIDs and references into Golden IDs, submits one generation
for the complete stabilized edit batch, and retains the last valid engine plan when conversion
fails.

Fresh module creation keeps authored controls and read-only projections separate. Golden Core
accumulates lifecycle-generated channel/routing descendants and publishes the completed Sound Card
as one subtree transaction instead of rebuilding a UI projection for every declared child.
Processing-result descendants are materialized only for enabled processing features. The module
repairs derived structure once per structural event frame; its individual child callbacks only mark
the audio configuration dirty.

The Golden control worker owns selected-host discovery, device supervision, active streams, and
compiled-plan publication. Chataigne opts into Golden's managed runtime: one dedicated render worker
owns the active render plan, playback renderer, and enabled analysis processors. Native input
callbacks write through the bounded adaptive clock bridge, while native output callbacks drain a
bounded prefilled queue and wake the worker. An absent output callback selects the paced null-clock
path, so playback and enabled analysis continue without moving host timing into Chataigne.

The output queue derives its capacity from the negotiated device buffer and holds three callback
periods, rounded to whole internal render blocks. Initial and recovery prefill follows the engine
clock until the first native callback; a full priming queue pauses the timeline instead of
rendering unconsumed blocks. Once callbacks begin, queue occupancy drives refill. Retiring or
abandoning a bridge is a lifecycle transition rather than an XRun, while underflow or overflow on
an active bridge remains observable and fails managed-device qualification.

Backend callback errors cross as one-shot atomic stream statuses. A ready stream warning becomes a
structured diagnostic; a missing, invalidated, or failed stream retires its callback bridge,
enters the supervisor's bounded retry backoff, and is reopened from fresh discovery. This polling
stays on the control worker and never moves backend status inspection onto the render thread.

Render plans and input/output bridge endpoints cross to the worker through acknowledged ownership
exchanges and retire back to the control thread. Callback handlers only convert or transfer
preallocated samples and update atomic pressure counters; they do not lock, allocate, decode,
compile, log, or destroy final owners.

Chataigne polls the coalesced observation at 30 Hz, updates cached value-node IDs only when values
move beyond the configured epsilon, and publishes one latest-only app telemetry envelope. Missing
explicit device selections remain persisted; unresolved local routes remain authored and receive
visible warnings so removal and undo stay atomic.

The app-owned Svelte adapter maps Sound Card connection paths and typed routing requests to public
backend intents. `golden_audio_ui` owns generic driver/device presentation, status/error
presentation, the SVG patch-bay component, and explicit exact-node-type registration helpers.
Importing either the adapter or reusable package has no registry side effect.

Input and output routing use a two-column SVG patch bay. Input device channels are on the left and
editable Sound Card Input Channels are on the right; editable Sound Card Output Channels are on the
left and output device channels are on the right. It renders one element per endpoint and one curve
per authored route, so DOM/SVG work is `O(endpoints + routes)`, never the Cartesian product of both
channel sets. Connect, disconnect, rename, and channel-count operations cross the public UI boundary
as typed Sound Card intents. The backend applies each accepted operation atomically and owns route
identity, validation, stereo defaults, and cleanup; acknowledgement-keyed optimistic presentation
rolls back on rejection.

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

With a hardware driver selected, Input Device starts at `None` and Output Device starts at that
host's system default. Input and output each start with two user-facing Channels and compatible
physical endpoints receive parallel left/right routing. Master and channel gains start at 0 dB.
Sample Rate and Buffer Size start at `Automatic`; Pitch Detection and Spectral Analysis start
disabled and therefore have no Values containers. These are backend-owned Chataigne defaults, not
policy recreated in reusable UI code.

See the implementation and evidence ledger in
[Golden Audio and Sound Card progress](../progress/golden-audio-sound-card.md).
