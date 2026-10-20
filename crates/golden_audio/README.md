# golden_audio

`golden_audio` is the reusable audio engine and device foundation for Golden applications. It owns
backend discovery, stream supervision, stable app-agnostic identities, immutable render plans,
routing, playback, analysis, observations, diagnostics, and deterministic null/mock/offline
operation.

It deliberately has no dependency on Golden Core, Chataigne, Tauri, or a UI framework. Native
backend, codec, resampler, and DSP dependencies are private implementation details and never appear
in public signatures.

The default `desktop`, `playback`, and `analysis` features enable the native operating-system host
through the private CPAL adapter, asynchronous file playback through private Symphonia workers, and
worker-side FFT/pitch analysis through private RealFFT internals. The backend-independent
null/offline core, render voice pool, stream ring, meter primitives, and analysis settings remain
available with no features:

```sh
cargo test -p golden_audio --no-default-features
```

Host capability and an explicit silent output smoke can be checked with:

```sh
cargo run -p golden_audio --example backend_probe
cargo run -p golden_audio --example backend_smoke
```

The managed callback path has a release-mode endurance gate that runs the medium reference
workload against the system default output, cycles the stream every ten minutes, and emits a
machine-readable report:

```sh
cargo run --release -p golden_audio --example managed_device_soak -- \
  --seconds 3600 \
  --recovery-seconds 600 \
  --report target/golden-audio-managed-soak.json
```

The fixture is a temporary stereo PCM signal played at `-96 dB` per voice. The runner excludes
device and playback warm-up, fails on callback XRuns, backend runtime warnings, deadline misses,
queue pressure, analysis drops, non-silent-signal loss, or incomplete recovery, and removes the
fixture on exit.

Real-device qualification uses the default `RequireZero` deadline-miss acceptance. Deterministic
null-backend tests may select `RecordOnly` because a shared test runner cannot provide realtime
scheduling; the report still records every miss, and this mode is not valid hardware evidence.

ASIO, JACK, native PipeWire, and real-time DBus integration are exposed through platform
qualification features documented in the
[toolchain policy](../../docs/reference/toolchain.md#native-audio-prerequisites). Applications
depend on `golden_audio`; they do not import or configure CPAL directly.

Chataigne deliberately enables the `asio`, `jack`, and `realtime` features in its default feature
set. Ordinary desktop products therefore include JACK alongside WASAPI on Windows, CoreAudio on
macOS, or ALSA on Linux; Windows also includes ASIO. Realtime scheduling promotes CPAL device
callbacks and the managed Golden Audio render worker where the platform supports it. Priority
refusal is nonfatal and produces a structured diagnostic. JACK loads its runtime library/server on
demand, so their absence is a recoverable `MissingServer` status. Other `golden_audio` consumers
retain the external-prerequisite-free native default.

On Windows, the repository wrapper prepares the pinned external ASIO SDK and LLVM environment, then
runs the host probe by default:

```powershell
.\tools\asio.ps1
.\tools\asio.ps1 -- cargo test -p golden_audio --features asio
```

## Event delivery

Control events use a bounded best-effort queue by default; full publication increments the
observation's dropped-event counter. Hosts requiring every active occurrence can opt into
`AudioEngineBuilder::with_ordered_events()`. Only the non-realtime control worker waits for
capacity. Keep consuming events to allow control progress. For graceful completion, submit
`AudioControl::shutdown()` and drain through EOF; `AudioEngine::shutdown()` and engine drop
cancel publication before joining. Dropping the receiver also releases blocked publication.
Cancellation preserves the already buffered prefix. Event admission obeys both the configured item
limit and an **8 MiB retained-value budget**, with at most **1 MiB per prepared event**. Ordered
publication waits for either kind of capacity; best-effort publication rejects the new event.
`AudioEvent::retained_bytes()` charges inline values, owned string/path capacity and a conservative
map-node allowance. Queue metadata/spare deque storage, producer preparation scratch, observations,
consumer copies and media resources are separate from that value budget.

Immutable string identities admit 64 KiB through constructors and deserialization; accepted values
are compacted without changing their text. Playback paths admit 128 KiB of retained native path
storage before command acceptance, including spare capacity. Accepted playback IDs and paths stay
verbatim in lifecycle events. Diagnostic/display strings are limited to 4096 bytes (diagnostic codes
and context keys: 256); contexts retain 16 entries with 1024-byte values. Truncation uses a visible
suffix or context marker. Preparation and discarded diagnostic storage remain on the control worker.

Graceful shutdown resolves remaining pending/playing registrations before `ShutdownComplete`.
Voices awaiting render retirement remain tracked by their full voice generation, including across
replacement of a playback ID. Already returned render outcomes are delivered first; remaining
registrations receive `PlaybackStopped`, retaining an earlier stop reason or using `StopAll`.
Fallback outcomes follow original start order. External hosts must quiesce their own callback
renderer; the managed runtime stops its device/render paths before final lifecycle resolution.

`PlaybackFailure::terminated_voice` identifies a voice retired by a decoder failure. Preparation
and control-admission failures carry `None`: a failed load or stop request does not imply that an
existing voice stopped. Consumers must match the full voice identity before removing a playback
projection, including when an old generation fails after its playback ID was replaced.

## Playback ownership

`AudioCommand::PlayFile` is ordered with configuration, gain, stop, and stop-all commands. A fixed
worker pool probes and decodes files, resamples them to the engine rate, and either inserts a small
immutable asset into the resident cache or primes a bounded streaming ring. The audio callback only
reads preallocated resident or streamed voice state.

Decoder workers own independent bounded inboxes; an idle worker cannot block another stream's
refill. Full stream rings park their decoder worker until the render side consumes frames. File IO,
decoding, resampling, rendering, and device callbacks never run on the application's engine thread.

External audio hosts can take the `PlaybackVoiceRenderer` once and invoke it from their
authoritative render callback. Ready-to-run product adapters instead opt into
`AudioEngineBuilder::with_managed_render_runtime`; Golden then owns the render worker, installs
bounded input/output callback bridges, and advances from the paced null clock whenever no output
callback is consuming. Finished and stopped voice ownership is returned to the control thread for
destruction in both modes. Decoder completions carry command generations, so stopping or replacing
a playback ID cannot start a stale decode.

Managed output queues hold three negotiated callback periods rounded to internal blocks, with a
minimum of eight blocks. Startup/recovery prefill is engine-paced; the first consuming callback
switches refill to queue demand. At the default 128-frame/48 kHz configuration the minimum is
21.3 ms of queue capacity, not a measured end-to-end latency guarantee. Shared presentation-time
coordination and scheduled activation remain readiness work described in the
[current architecture](../../docs/architecture/golden-audio.md#signal-and-clock-model).

The supported extension list has one Rust source and is included in the generated TypeScript
contract. Raw AAC and WMA are intentionally not advertised.

## Analysis ownership

The render plan resolves every enabled analysis tap to a dense virtual-input index. A renderer
accumulates input meters before monitoring and output meters after channel/master gain, captures
preallocated tap frames, and publishes meter values through lock-free latest-value storage at the
configured observation rate. It never runs pitch detection or an FFT.

One dedicated worker applies the in-crate YIN implementation or RealFFT with Hann or
Blackman-Harris windows, linear or logarithmic bands, overlap, normalization, and attack/release
smoothing. When the worker falls behind, old analysis frames are replaced by the newest available
frame and audio rendering continues. Observations carry their topology generation; results from
another generation are discarded. A tap can be enabled or disabled independently without
rebuilding device state.

## Null/offline example

```rust
use golden_audio::{AudioEngineBuilder, FrameCount, OfflineClock, SampleRate};

let mut engine = AudioEngineBuilder::default().build()?;
let control = engine.control();
let _events = engine
    .take_event_receiver()
    .expect("event receiver is available once");

let mut clock = OfflineClock::new(SampleRate::new(48_000)?)?;
clock.advance(FrameCount::new(128)?)?;

control.shutdown()?;
engine.shutdown()?;
# Ok::<(), golden_audio::AudioError>(())
```

The authored configuration uses stable IDs. Chataigne or another application is responsible for
mapping its persistent domain identities into these types. Physical device indices are intentionally
absent from the command and project-facing contract.

`AudioCommand::ApplyConfiguration` accepts `Arc<AudioConfiguration>`. Producers can retain the
immutable configuration across full-queue retries; the native control worker obtains ownership
or clones it there. The serialized command shape is unchanged. `AudioConfiguration::retained_bytes_within`
accounts for spare capacities and bounds collection traversal before external lifetime admission.
`AudioDeviceInspectorState::retained_bytes_within` measures device metadata independently of
metering/analysis storage, allowing configuration preparation to use a separate budget.

`SetDeviceProbeInterests` and `AudioControl::set_device_probe_interests` accept
`Arc<Vec<AudioDeviceTargetId>>`; ownership/cloning occurs on the native control worker. The JSON
shape is unchanged. Producers retaining canonical shared payloads can charge targets through the
last queued command. `AudioObservationReader::device_revision()` reads an atomic fence without
the snapshot lock. Device-state changes advance it; metering does not. The matching revision in
`AudioObservationSnapshot` is native metadata omitted from serialization.

See [Golden Audio Architecture](../../docs/architecture/golden-audio.md) for ownership, signal,
clock, recovery, and callback-safety rules.
