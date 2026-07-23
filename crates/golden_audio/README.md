# golden_audio

`golden_audio` is the reusable audio engine and device foundation for Golden applications. It owns
backend discovery, stream supervision, stable app-agnostic identities, immutable render plans,
routing, playback, analysis, observations, diagnostics, and deterministic null/mock/offline
operation.

It deliberately has no dependency on Golden Core, Chataigne, Tauri, or a UI framework. Native
backend, codec, resampler, and DSP dependencies are private implementation details and never appear
in public signatures.

The initial backend-independent build is available with no features:

```sh
cargo test -p golden_audio --no-default-features
```

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
