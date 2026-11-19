# Safe Parameter control admission (M15/1)

**Accepted — 2026-11-17.** [Acceptance receipt](evidence/mapgyver-m15-controls-2026-11-17.json).

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
Sparse expansion now prepares only the typed inputs requested by a pure owner hook before
materializing declarations. Media reads Kind; Golden decodes the declared Parameter and
includes requested node/parent inputs in the baseline cache key. Missing Video constraints
therefore come from the original DSL, while an explicit valid constraint override survives.
Immutable captured saves use the same sparse Parameter payload helpers as live saves.

## Verified scope — 2026-11-17

- Current app library:738 passed,8 ignored, including direct inputs/Undo, Proxy and both
  Binding directions, grouped binding Undo, script admission/nonfinite rejection, changed
  live bounds, invalid-oscillator/healthy-sibling behavior and Video Speed's exact new-save
  constraint roundtrip, omitted-metadata restoration and explicit valid overrides.
  Log: `target/m15-declaration-restore-app.log`, exit0.
- Parameters:32 passed,1 ignored, including each numeric kind with changed bounds,
  scalar/integer/vector/Color/CSS admission, projections, steps and invalid data.
  Log: `target/m15-parameter-types-final.log`, exit0.
- Current Engine:700 passed,3 ignored, in the existing software-only `test-fast` profile.
  `target/m15-declaration-restore-engine.log`, exit0. Strict app/Engine library/test Clippy
  passes with warnings denied; `target/m15-declaration-restore-clippy.log`, exit0.
- Fresh default bundled build passes: `target/m15-declaration-restore-build.log`, exit0.
  Browser fixture Node/Prettier and root/Core formatting pass.

The earlier failed native cohort at `target/m15-native-controls-20261117-1645` exposed
the missing Speed constraints. It remains failed evidence, not acceptance.

## Acceptance — 2026-11-17

The current bundled fixture passes on owned PID51440, normal exit0. Surface Opacity,
Composition Opacity and Video Speed each pass Animation/Manual, mode Undo/Redo, manual0.5
editing, normal save/reload and an omitted-Speed-constraints copy of the same private project.
All 198 observed values are finite and bounded; every target continues animating.
Speed stays on its 0.01 grid.
The actual captured save contains that step before removal.768 requests/receipts are
observed with no rejection or page error. Original mode/history/reload requirements,
numeric-type/changed-bound admission, script/binding/transport paths and healthy-sibling
safety have verified scope in the owning regressions above.

App SHA256: `9ED731A0ED33AF9E73A8F1E1CBC0F76D23BC1D54EE3142C42156BACFAD254DCC`.
Local artifacts: `target/m15-native-controls-20261117-1742`.
This is control/persistence correctness; the native Video node has no loaded file and
establishes no decoder, frame-rate, GPU-cost, codec or recovery qualification.

M15/1 has no remaining acceptance item. Next functional delivery is Matrix selection/custom
colors. The user explicitly selects targeted checks per feedback change and a later deep/
Intel/edge-case pass; original release requirements remain in the internal ledger.

## Gradient and Color animation

- Add a standalone **Gradient** to a Golden User Context, or select **Animation** on a Color parameter. Both open the same Gradient editor and use typed Position, Color, and Interpolation stop parameters.
- Color animation traverses the gradient once per cycle. **Frequency (Hz)** sets the number of cycles per second, **Phase** offsets the starting position in cycles, and **Update Rate (Hz)** sets the sampling rate. Frequency zero holds the selected phase.
- Linear and Smooth interpolate RGBA. None holds the left stop's color up to the next stop. A single stop holds a constant color. An empty animation or a return to Manual restores the authored manual color.
- The engine rebuilds the sampled gradient after subtree edits. Scheduled updates reuse that model, admit RGBA through the target's normal constraints, and publish the live color without writing it into the project or undo history.
