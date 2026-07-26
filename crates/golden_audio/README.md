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

ASIO, JACK, native PipeWire, and real-time DBus integration are exposed through platform
qualification features documented in the
[toolchain policy](../../docs/reference/toolchain.md#native-audio-prerequisites). Applications
depend on `golden_audio`; they do not import or configure CPAL directly.

## Playback ownership

`AudioCommand::PlayFile` is ordered with configuration, gain, stop, and stop-all commands. A fixed
worker pool probes and decodes files, resamples them to the engine rate, and either inserts a small
immutable asset into the resident cache or primes a bounded streaming ring. The audio callback only
reads preallocated resident or streamed voice state.

External audio hosts can take the `PlaybackVoiceRenderer` once and invoke it from their
authoritative render callback. Ready-to-run product adapters instead opt into
`AudioEngineBuilder::with_managed_render_runtime`; Golden then owns the render worker, installs
bounded input/output callback bridges, and advances from the paced null clock whenever no output
callback is consuming. Finished and stopped voice ownership is returned to the control thread for
destruction in both modes. Decoder completions carry command generations, so stopping or replacing
a playback ID cannot start a stale decode.

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

See [Golden Audio Architecture](../../docs/architecture/golden-audio.md) for ownership, signal,
clock, recovery, and callback-safety rules.
