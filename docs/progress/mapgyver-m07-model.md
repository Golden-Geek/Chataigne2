# Revised Matrix model

**M07/1 Active; no gate accepted by this slice.** Matrix owns authored document policy under
`apps/mapgyver/src/matrix`; Golden owns typed Parameters, forest edits, Inspector, persistence
and history. The accepted M08 configuration/demand boundary remains a dependency of this model.

## Typed Layer/Group blending — 2026-11-03

`ElementNode` now declares real opacity, blend and render-enabled Parameters. The persisted
`mapgyver.matrix.layer.v2` / `mapgyver.matrix.group.v2` type selects an immutable kind; no
definition DTO is stored in node data. Existing definitions, bounded selected-row reads,
render-index captures and render plans project the Parameters. Runtime invalidation still uses
the owning Matrix binding, without tree snapshots or per-frame authoring work.

`ConfigureItem` validates once and writes one `SetParamsAtomic` proposal. Direct generic edits
use the same declared values and ancestor validation; API configuration is one Undo step,
ordinary Inspector edits are one step per control. Nested copy uses Golden's complete forest
copy and remaps paired exports/references, including real Parameter identities. Sparse project
reload retains configured identities and values. Unsupported old row schemas and opaque data
in current row records fail at project admission before recoverable decoding can discard them.
This deliberately breaks the old opaque row schema; there is no compatibility codec.

The shared Inspector presents ordinary controls, including all four blend modes. The old
custom blending form is removed. Contextual Matrix operations remain available during the
ongoing grid redesign. Selection reads stay independent of visible rows; an actual Inspector
edit after scrolling its Layer outside a 258-row grid is verified without launching playback.

The saved pre-revision stash remains intact and un-restored. This conversion reuses its typed-row
approach against the current Column/Cell/control model; its older opaque Column/Cell code,
host/UI extensions and presentation changes were not applied.

## Verification

[Receipt](evidence/mapgyver-m07-row-parameters-2026-11-03.json): **375 library tests pass**, zero
failures and two retained qualification exclusions, in **108.52 seconds**. Three new cases cover
real values/atomic Configure, direct edits and capture fencing, invalid/atomic rollback, nested
copy with independent identities, exact history/project replay and early old/opaque-schema
rejection. Existing four-caller parity, render-plan blending and accepted retained-demand suites
also pass. Strict all-native-target Clippy, both generated-contract equality checks and both
Cargo formatters pass. Svelte reports **0 errors/warnings**; **26 app/Matrix UI tests pass**.

The final isolated app passes actual generic Layer/Group opacity, every blend mode, render
enablement and one-step per-field Undo/Redo, plus selection/Inspector editing outside the visible
258-row grid. It preserves no-launch behavior. Current Cell/Column Inspector/sharing/direct/
disabled-destination/Unload and source/history/Save-New-Load regressions pass on the same build.
**Twelve Windows lifecycle cases pass** in **1.15 seconds**, with one retained pinned-descendant
exclusion. These are regression checks; revised native Matrix pixels/scale remain M07/4.

Validation repaired an invalid test assumption that Golden must reject the Boolean string
`"false"`; normal canonical coercion remains supported. Range, enum and atomic-proposal rejection
are still checked. An early test used a nonexistent multi-Parameter UI intent and was corrected
to Golden's public atomic engine edit. Final review removed an unnecessary whole-tree inbox
snapshot. One combined browser attempt failed fetching a dynamic JavaScript chunk; fresh focused
row/control runs pass without adding retry behavior. The older comprehensive Matrix UI fixture's
blending section now follows generic controls; its remaining legacy workflow must be reconciled
and fully qualified under M07/3–4 before claiming full grid acceptance.

## Remaining M07/1 contract

- Typed Layer position, canvas size and transition duration, including their runtime projection.
- Typed Cell relative position, opacity and enabled behavior, with no Cell size; preserve actual
  child-media size, optional transition/app controls and sparse empty addresses.
- New Matrix 3×3 defaults through backend-owned batched structure creation; complete typed
  Parameter/API/copy/history/project behavior and current caller/schema coverage.

M07/2 separately retains full Cell → Column → Layer transition precedence and playback acceptance.
M07/3 retains the complete square-cell grid, contextual structural operations, naming, removal,
drag sort/reparent and complete native feedback. M07/4 retains actual native pixels, scale,
memory/timing and owner retirement. Neither extra typed nodes nor passing unit counts waive
those requirements or earn partial acceptance points.
