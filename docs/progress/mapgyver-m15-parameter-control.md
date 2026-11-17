# Safe Parameter control admission (M15/1)

Golden owns numeric admission, dependent control evaluation and Parameter persistence.
MapGyver owns the Surface/media/Video reproducers and ordinary Inspector verification.
The [architecture](../architecture/mapgyver-parameters.md) defines those boundaries.

Confirmed failures before their corrections:

- Surface Opacity Animation at750ms aborts the tick: `value -1 is lower than min 0`.
- Video Speed Animation at17ms aborts on its declared0.01 step.
- Through actual app intents, a source changes4→−4 but Proxy Opacity stays1 without a
  diagnostic: UI settlement consumed the notification before control evaluation.
- After ordinary save/reload, Video Speed has no range or step. Sparse encoding omitted
  locked constraints against a generic constructor baseline that cannot restore them.

## Implemented behavior

Shared admission adapts finite numeric range/step before bounded conversion; integers stay
wide until admission. Scripts and projected controls/bindings use the same boundary.
Nonnumeric domains and owner invariants remain strict. Nonfinite input rejects without
mutation; an overflowing oscillator holds its last valid target while siblings continue.
Already admissible stepped values stay exact across repeated admission and history.

Bounded edit settlement evaluates controls before dispatch drains their source notifications.
Dependent writes share the owning transaction and Undo. New sparse saves preserve constraints
when no matching owner declaration supplies them; generic constructor defaults are insufficient.

## Verified scope — 2026-11-17

- Current app library:737 passed,8 ignored, including direct inputs/Undo, Proxy and both
  Binding directions, grouped binding Undo, script admission/nonfinite rejection, changed
  live bounds, invalid-oscillator/healthy-sibling behavior and Video Speed's exact new-save
  constraint roundtrip. Log: `target/m15-parameter-reload-verified.log`, exit0.
- Parameters:32 passed,1 ignored, including each numeric kind with changed bounds,
  scalar/integer/vector/Color/CSS admission, projections, steps and invalid data.
  Log: `target/m15-parameter-types-final.log`, exit0.
- Current Engine, including the final sparse-codec change:700 passed,3 ignored, in the
  existing software-only `cargo test-fast` profile. Log: `target/m15-sparse-engine-final.log`, exit0.
- Current strict libraries/tests Clippy passes with `MapGyver/native-qualification` and
  `-D warnings`; `target/m15-sparse-clippy-final.log`, exit0. Both final checks terminated
  at17:05UTC. Browser fixture Node/Prettier checks and root/Core formatting pass.
  The last fresh default bundled app build predates the sparse change; a rebuild remains required.

The actual bundled Inspector fixture completed Animation/Manual, mode Undo/Redo and Manual
edits for Surface Opacity, Composition Opacity and Video Speed. Reloaded Surface/Composition
kept animating; reloaded Video Speed failed its step assertion. The owned app shut down
normally. This failed cohort is retained at `target/m15-native-controls-20261117-1645`.
Its app SHA256 is `E5180048A7CBC3F5349A4B1B817F86E25A725C1D0DB7D3780A94386A9B93AA62`.
It predates the sparse correction and establishes no native proof for that correction.

## Exact remaining requirement

M15/1 stays Active. Restore canonical conditional Parameter metadata when a saved record
already omits it; the encoder correction fixes new saves and does not repair those records.
Then rebuild and rerun the existing bundled Inspector fixture, including all three active
controls through save/reload. Do not open M15/2 or claim acceptance yet.

The16:16–17:00UTC batch stops new implementation/experiments at17:00, finishes existing
validation and checkpoints verified progress. Next product delivery after control safety is
Matrix selection/custom colors. Performance, recovery and unusual-scenario qualification
stay in the second pass; no new benchmark/probe framework was added.
