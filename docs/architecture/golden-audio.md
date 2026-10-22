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

Sound Card's app-owned `command_service` prepares its five command nodes and five script methods
on a passive `golden_io` worker. Command nodes dispatch frozen snapshot/override leases directly;
scripts retain their engine-owned argument lease. Script success reports preparation-queue admission;
later parse/native failures appear as backend warnings. Command nodes retain the existing structured
command-result event, including the native admission sequence. The former serialized module-request
round trip is removed.

Preparation admits at most 64 live requests globally, with a conservative 4 MiB reservation per
request covering decoded input, temporary preparation and result storage. Snapshot/JSON/script leases
retain their separate canonical budgets. Source parameters/arguments are limited to 256 KiB and
paths to 64 KiB; UI control payloads are checked before decoding at 64 KiB, 256 values and depth eight.
Eight worker lifetime slots include retiring workers. Cancellation never joins on the actor, and
captured source/result final disposal stays worker-owned.

The actor services at most four prepared deliveries per wake. Native submission happens once against
the captured runtime lifetime; a replacement runtime cannot receive an old request. Authored gain
targets require a fresh identity/parent/type check. Gain writeback, bounded diagnostics and command
result publication have separate edit receipts, so publication failures cannot replay native effects.
Only pending gain work requests a command-service snapshot. Disable/removal cancels preparation;
native retirement invalidates captured targets. This does not establish P04's ordering barrier between
gain commands and configuration exchanges. Remaining C01 feedback/failure qualification stays open
in the status ledger.

Sound Card's `routes` boundary owns device-selection resets and initial default seeding. Resets
share a 16-root deletion budget across input/output and wait for the preceding edit completion
before admitting another turn. A flag settles only after a fresh snapshot observes the committed
result. Defaults use indexed channel/route UUID lookup, create at most two detached route trees
per direction, and skip trees already committed by a partial prefix. Module lifetime and node
read fences protect queued writes; disable, destroy and restoration invalidate old route work.
Routing intents reject while their direction is actively resetting or seeding.

Default physical keys admit at most 1024 bytes and channel naming metadata at most 64 KiB per
field before copying. Existing channel/reference names and persisted route UUIDs remain unchanged.
An invalid direction does not abandon accepted writes in the other direction: those settle before
the input error is reported. Missing inventory/channels waits for subsequent structural/native
publication. Channel short-name changes request backend reconciliation even without telemetry.
`routes::prune` scans at most 32 captured roots and queues at most eight deletions per turn.
The sweep retains a canonical snapshot instead of cloning a complete child list. Each turn checks
current ownership/reference values; failed prefixes retry only survivors. Intent, parent, target and
module-lifetime fences protect deletion. A final fresh sweep covers additions/rewires during earlier
turns. Cleanup continues with the driver set to None. Snapshot-capacity rejection uses a 250 ms
deadline and suppresses snapshot requests while waiting.

`routes::lookup` owns ordered connect/disconnect requests. Its worker borrows immutable route fields,
preserves first-match child order and authored UUIDs, and inspects at most 65536 roots per request.
Each module admits 32 requests, with 256 global live request slots, eight worker lifetimes including
retirement, and one weighted 8 KiB command/reply slot per worker. Keys/UUID tokens admit at most
1024/128 retained bytes. Snapshot leases keep their independent canonical budgets. The actor checks
the source revision on resumption, publishes through edit receipts, and retries stale/failed prefixes
at most eight times. Device selection or module lifetime changes cancel old requests explicitly.
An idle queue drops its worker; failed workers can recover for a subsequent request. Waiting for
worker replies requires no route snapshot. Display-name caches are built only for route creation,
with bounded naming metadata, rather than during rename/disconnect validation.

Sound Card's `structure` boundary reconciles gains, levels, optional analysis values and legacy
containers through one typed staged pass. A turn performs at most 32 reconciliation steps and
eight graph writes, with at most 32 node reads in one precondition fence. One edit receipt
backpressures the next turn. Child scans borrow a canonical retained snapshot; four incrementally
built UUID sets hold at most 256 desired identities each. Names admit at most 64 KiB before copying.
Root retirement ends its turn so later stages cannot queue edits against retired descendants.
Gain/level migration replaces the legacy root before pruning its children, preserving the only
copy of its value through a failed suffix. Legacy spectral toggles merge enabled intent into the
typed container before removal; deterministic bool identities are replaced without an absent gap.
Failed prefixes re-read committed state. Disable/destroy/restoration invalidate queued writes.
Authored topology/metadata callbacks and the service's own edits request a fresh pass after the
current scan; value-only/meter updates cannot keep restarting a large scan. Pending receipts and
250 ms capacity/error backoff suppress snapshot requests. Progress uses worker wakes, including
driver None; no periodic actor update is introduced. An idle service owns no retained snapshot.

Feedback and remaining native failure/reconfiguration fences still belong to C01. General
snapshot/publication scaling stays in P16; these authored service budgets do not qualify end-to-end
latency or change P04's audio ordering requirement.

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

The configured engine sample rate defines engine-frame units. In the production managed runtime,
one render worker fills output queues; native output callbacks drain those queues and wake it.
Without a consuming output callback the worker uses an `Instant`-paced null path. Input and output
are separate clock domains unless a backend proves otherwise. Input monitoring crosses a bounded
ring through adaptive asynchronous resampling with observable fill, drift, underflow, overflow,
and latency. Queue-driven refill is not a shared show-presentation clock.

The implemented input bridge owns a bounded SPSC sample ring, an asynchronous Rubato resampler with
preallocated input/output storage, and a bounded PI drift controller. The input callback only
validates and copies a complete interleaved callback block. The render side adjusts the resampling
ratio from ring fill, emits silence on starvation, and publishes coalesced fill, drift, latency,
underflow, overflow, timestamp-loss, and discontinuity observations. A device-rate change rebuilds
the resampler and flushes the old clock domain on the control thread; it never changes engine time.

`RenderClockCoordinator` implements a separately tested authority/handoff primitive, but the
2026-10-18 source audit found no production managed-runtime caller. Its tested fades, block-boundary
promotion, and null fallback must not be read as a production integration claim. A single
production authority, explicit epoch/presentation-time conversion, and prepared scheduled
activation are readiness work in P14 of the
[implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md).

The intended contract is prepare/prime, switch at an engine block boundary, and retire old stream
ownership away from realtime work without resetting the sample counter or replaying effects.
Current implementation/qualification gaps are recorded in the
[one status ledger](../progress/implementation-status.md).

## Playback boundary

File playback is a three-stage ownership pipeline:

1. The control worker receives play, stop, stop-all, gain, and configuration commands and assigns
   monotonic sequences. The current separate gain/plan delivery paths do not yet preserve one
   render-side order across configuration changes; P04 owns that repair.
2. A fixed decoder-worker pool privately uses Symphonia to probe and decode, resamples to the engine
   rate, maintains the bounded resident cache, and primes bounded stream rings.
3. The render stage activates prebuilt fixed-slot voices, reads planar resident assets or
   streaming rings, applies bounded start/stop ramps and explicit routes, and returns completed
   ownership for control-thread destruction.

Playback-ID cancellation watermarks make a late decoder result stale. Reusing an ID replaces its
pending or active generation by default without allowing an older load to start. A play request may
disable Force Restart, in which case an occupied ID remains untouched and the control worker emits
a typed no-effect event. Missing-ID stop and empty stop-all requests use the same explicit outcome
instead of silently disappearing. Stop-all advances a global watermark and cancels every pending
worker request. Worker and result queues are bounded; capacity pressure is a structured failure
rather than a new thread or unbounded allocation.

Resident assets are immutable `Arc`-owned planar buffers keyed by canonical path metadata, track,
and engine sample rate. Cache lookup, insertion, invalidation, and eviction happen only on decoder
or control threads. Large files use bounded read-ahead and emit silence on starvation without
blocking the render stage. The current streamed voice advances its source playhead only through
available decoded frames, so starvation inserts delay before resumption. That behavior protects
render progress but does not retain synchronization with independent media. P14 separates
standalone resume behavior from explicit show-locked timestamp/catch-up policy. Decoder arrival
currently determines activation; a source-file offset is not a requested target start frame.

Play requests carry a typed non-negative start offset. Resident assets keep their full-file cache
identity and begin at the corresponding engine-rate frame. Streamed sources perform an accurate
container seek on the decoder worker, reset codec state, and trim only the pre-target decoded
frames before priming the ring.

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
feature set adds dynamically loaded JACK on every desktop platform, ASIO on Windows, and realtime
scheduling. Native PipeWire and realtime DBus support remain explicit Linux qualification
features. A missing JACK server or ASIO driver produces a recoverable backend state and does not
prevent the native host from starting. The backend probe enumerates hosts and devices but does not
open a stream. The separate smoke example opens the default output, writes silence for 100 ms, and
closes it.

## Sound Card product model

The module tree presents three small sections:

1. **Connection** contains Audio Driver, Input Device, Output Device, Sample Rate, Buffer Size, and
   direct Input Routing / Output Routing containers. No Device Profile concept is exposed or
   persisted as an editable user model. Routing nodes remain structurally stable for persistence,
   while their custom inspectors are absent when that direction's device is `None`.
2. **Parameters** contains ranged master and per-channel input/output gains plus a Processing
   container. Every channel gain is labeled `<semantic channel name> Gain` and follows the user's
   exact free-form channel name. Processing owns the 5 Hz default Levels Update Rate, configurable
   from 1 through 30 Hz. Direction parameter roots remain stable and are hidden by their inspectors
   while unused. Pitch Detection is disabled by default. Spectral Analysis is a disabled-by-default
   container rather than a boolean; it is empty until it owns spectral-band definitions.
3. **Values** contains a Levels folder whose conditional direction children are ordered Input then
   Output. Every master and channel level is read-only and ranged. Pitch Detection and Spectral
   Analysis are each represented by one value container, materialized only while the corresponding
   processing feature is enabled; Spectral Analysis has no per-band descendants yet. Values has no
   duplicate Input Levels, Output Levels, or Global Levels roots and no Diagnostics folder. Playback
   Status remains present, and its Active Voices and Loading Voices counters update from the
   coalesced playback observation.

The user authors Input Channels and Output Channels through ordinary parameter edits. The backend
owns stable channel identities, default labels, descendant reconciliation, route validation and
cleanup, and topology materialization. A channel's exact free-form human name, such as `Input 1` or
`Output 1`, is the semantic label presented by routing and gain UI; internal declaration IDs such
as `input_1` or `output_1` never become user-facing labels. One stabilized edit batch reconciles a
count change, and newly available stereo endpoints receive default one-to-one left/right routing.
The UI may edit a channel label and request a route, but it does not allocate nodes, labels, IDs, or
hidden route parameters.

## Chataigne runtime integration

The Sound Card module owns the application adapter and queues reusable `AudioEngine` construction
for the selected Audio Driver on a dedicated lifecycle worker. Node creation does not wait for
native host initialization, and Audio Driver `None` does not construct a hardware backend.
Completed runtimes cross back through a pending-result channel; stale driver/format generations and
retired runtimes return to that worker for shutdown, so replacement, removal, and project drop do
not join audio workers on Chataigne's engine thread. Lifecycle command/result retention is bounded,
each lifecycle worker reserves one of eight cleanup slots for its complete lifetime, and detached
runtime shutdown has four fixed slots that return ownership on overload. A module with rejected
retirement retains that exact runtime and stops requesting replacements until cleanup admission
recovers. Startup failures use bounded reconnect backoff, and delayed retries share one scheduler
with at most 64 outstanding wakes instead of creating one sleeper thread per failure.

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

The output queue derives its capacity from the negotiated device buffer as
`max(ceil(3 * callback_frames / internal_block_frames), 8)` internal blocks. Eight default
128-frame blocks have 21.3 ms capacity at 48 kHz; the default 2,048-frame input bridge target is
42.7 ms. These are configured capacities/targets, not measured end-to-end latency. Initial and
recovery prefill follows the engine
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

Chataigne projects the coalesced observation at the Sound Card's app-owned Levels Update Rate,
configurable from 1 through 30 Hz with a 5 Hz default. It updates cached value-node IDs only when
values move beyond the configured epsilon and publishes one latest-only app telemetry envelope.
Its native owner validates and copies snapshots into a bounded retained pool; ordinary actor wakes
borrow shared captures without copying the source snapshot. Golden Audio exposes
`AudioObservationReader::inspect` and `AudioObservationSnapshot::retained_bytes_within` for bounded
worker-side inspection before copying, including spare capacities and nested metadata/analysis.
The native owner also prepares telemetry JSON and tracks active voices. The actor publishes shared
payloads with an application receipt and projects values through a bounded cursor; its value cache
advances only after each edit applies. Each prepared binding plan owns a fixed cache allocated on
the worker and charged through final background disposal. Generation tags avoid whole-cache clearing,
and inline scalar/short-text slots cannot grow during actor service. The inspection closure must
not block or reenter the reader.
The native owner also captures immutable inspector metadata with sorted physical-channel indices
for route intents, with admission independent of metering/analysis capture. Metering-only captures
reuse it; actor admission borrows keys and checks the atomic native device revision
before and after lookup. Device changes reject stale route requests until a current capture is
available. Route and device-choice readers share the same retained inventory through background
disposal. Choice synchronization retries missing inventory independently of metering pressure.
Device/numeric option construction, serialization and comparison run on the shared preparation
worker. The actor moves at most four prepared patches, waits for their Core receipt, and prepares
fresh state after a failed prefix. Native enum domains retain their options, membership indexes and
cached allocation charge under a separate lifetime quota through exported constraints and DTOs.
Core checks membership by binary search and reads domain weights without rescanning entries.
Canonical enum values share one index; custom values retain a separate exact-value index. The JSON
option arrays and generated UI types are unchanged. The native plan moves bounded read preconditions with its patches;
Core checks selector/module/connection state and the captured inventory lifetime before the first
patch. A late selector edit, native publication or owner retirement fails the receipt before any
choice patch applies. The native revision uses the domain-neutral `golden_io` clock; read guards
hold no device payload and never lock the observation snapshot. A successful check is an admission
boundary, not a lock on later changes. Core native Parameter authored snapshots and generated
inherited snapshots preserve the original enum domain and native admission through callback
restoration and undo/redo. Kind coercion borrows unchanged inputs or moves accepted storage;
constraint replacement revalidates the current value in place. Project JSON and wire codecs remain
separate. Remaining conversion parsing/formatting, distinct-domain equality, non-enum snapshot
copies, custom JSON codecs, constraint projection/disposal costs and other native-execution fences
remain open.
Capture/telemetry overload has a separate visible warning and leaves control admission independent.
App-owned selection/probe planning, choice planning and configuration conversion run on one bounded preparation worker.
Queued/running requests lease their source snapshots; ready plans retain output storage admission
and published revision fences through retries and final disposal. Bindings promote
only after a matching native configuration acknowledgement; rejected replacements preserve the
previous bindings. Route-warning reconciliation also runs on that worker, comparing applied
presentation state; actor publication has bounded receipts and no optimistic warning cache.
Native device-revision fences cover successful and failed preparation results without locking
snapshots on the actor. Probe targets retain their canonical quota through native command Arcs.
See [host ownership](hosts.md) for budgets and the remaining structure, Core choice publication, feedback inspection,
command/diagnostic and fence boundaries.
Missing explicit device selections remain persisted; unresolved local routes remain authored and
receive visible warnings so removal and undo stay atomic.

Post-gain output RMS at or above 0.01 linear publishes a latest-only outgoing-activity state, so the
module traffic icon remains lit until the signal falls below the threshold. With outgoing traffic
logging enabled, authoritative playback started, finished, stopped, failed, and no-effect events are
logged at the Sound Card module origin.
One bounded native logging worker appends prepared event messages before their callback receipts
advance. Actor service forwards retained handles, waits for native completion wakes, and defers
overload with a coarse deadline. Display logs admit 16 KiB with a truncation marker; typed callback
data stays intact. Logging failure has an explicit warning and cannot replay a committed prefix.

The app-owned Svelte adapter maps Sound Card connection paths and typed routing requests to public
backend intents. `golden_audio_ui` owns generic driver/device presentation, status/error
presentation, the SVG patch-bay component, and explicit exact-node-type registration helpers.
Importing either the adapter or reusable package has no registry side effect.

Input and output routing use a two-column SVG patch bay. Input device channels are on the left and
editable Sound Card Input Channels are on the right; editable Sound Card Output Channels are on the
left and output device channels are on the right. It renders one element per endpoint and one curve
per authored route, so DOM/SVG work is `O(endpoints + routes)`, never the Cartesian product of both
channel sets. Connect, disconnect, rename, and channel-count operations cross the public UI boundary
as typed Sound Card intents. The backend owns intent validation, route identity, stereo defaults,
and receipt-driven derived cleanup; a channel-count edit converges through bounded structure turns.
Acknowledgement-keyed optimistic presentation
rolls back on rejection. During pointer routing, the preview cable snaps to a nearby compatible
connector using a row-scaled capture area; releasing while snapped submits that route and both the
connector and cable provide visible snap feedback.

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
Sample Rate and Buffer Size start at `Automatic`; Pitch Detection and the Spectral Analysis
container start disabled and therefore have no corresponding Values containers. These are
backend-owned Chataigne defaults, not policy recreated in reusable UI code.
