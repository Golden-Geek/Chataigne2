# Revised Matrix model

**M07/1 Accepted — 2026-11-03.** Matrix owns authored document policy under
`apps/mapgyver/src/matrix`; Golden owns typed Parameters, forest edits, Inspector, persistence
and history. The accepted M08 configuration/demand boundary remains a dependency of this model.

## Typed Layer/Group blending — 2026-11-03

`ElementNode` now declares real opacity, blend and render-enabled Parameters. The persisted
`mapgyver.matrix.layer.v3` / `mapgyver.matrix.group.v2` type selects an immutable kind; no
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

## Layer position, canvas and duration — 2026-11-03

Layer v3 adds typed position, integer canvas size and whole-millisecond transition duration.
Backend creation takes the current Matrix canvas; Group nodes have no spatial controls.
Atomic ConfigureLayer and generic edits share validation, history, copy and project values.
Old Layer schemas and obsolete Matrix-wide transition records reject explicitly. Generated
Rust/TypeScript contracts and all current consumers use the new ownership.

Render preparation preserves native child size, clips to the Layer canvas and places that
logical canvas in its parent. Media Resolution can independently resample the exported texture.
A reproduced stale parent-canvas cache was removed, so Resolution edits immediately affect
mapping. Cell override → Column override → Layer duration now controls independent deadlines.
Shared/modal timing and sparse bounded exceptions preserve the existing binding/interruption
limits and retained process demand. Admission checks actual timed Layer requests before staging.

[Layer receipt](evidence/mapgyver-m07-layer-settings-2026-11-03.json): **378 library tests pass**,
two retained exclusions, in **197.56 seconds**. The strengthened interruption-admission case
also passes separately. Strict all-target/native Clippy, generated contracts, Svelte (0/0),
26 UI tests and both formatters pass. Twelve Windows process regressions pass, one retained
exclusion. Actual bundled Inspector position/canvas/duration, per-field Undo/Redo, Group absence
and offscreen editing pass. Cell/Column controls retain their separate regression receipt.

The new native Layer position/canvas/resize/Undo case passes on Intel Graphics and NVIDIA
GeForce RTX 5080 Laptop GPU: **18 complete 32×24 images**, zero nominal or residual pixel error,
with existing resource-retirement assertions. This proves the spatial slice, not full M07/4.
The baseline failed because its new one-route fixture had only two payload slots for the
three-reservation readback path. Using the existing two-route fixture preserves all caps and
checks both outputs plus preview. The only repaired native rerun passed. A browser checkbox
action raced history updates; waiting for the displayed prior state fixed the test without
changing product behavior. No gate or qualification budget is changed.

## Typed Cell placement, opacity and enabled — 2026-11-03

Cell v3 owns three real Parameters: relative pixel position, opacity and enabled. There is no
Cell size; child media keeps its own dimensions. Public ConfigureCell validates before editing
an existing Cell atomically or inserting one detached configured Cell subtree at an empty
address. Ordinary controls use the generic Inspector, including offscreen selection. The action
is now named Inspect Cell settings and retains access to the same owned app controls.

Position/opacity edits invalidate runtime projections without changing the source epoch.
Bindings update settings for the same authored source; replacement/deletion retains outgoing
placement/opacity until that clip retires. Cell opacity multiplies fade weight and Layer opacity,
including the single-input path. Disabled destinations hold current content on Column launch
and skip individual activation. Cell v2 and opaque Cell data reject before recoverable decoding.
Rust-generated DTOs and all consumer/type checks follow the v3 ownership.

[Cell receipt](evidence/mapgyver-m07-cell-settings-2026-11-03.json): **380 library tests pass**,
two retained exclusions, in **153.98 seconds**, including typed sparse creation, atomic history,
copy/project/schema admission, live placement/opacity, disabled direct/Column activation, retained
outgoing settings, opacity multiplication and four public callers. Strict all-target/native
Clippy and generated-contract equality pass. Svelte reports zero errors/warnings; 26 UI tests pass.
Twelve Windows lifecycle regressions pass, one retained exclusion.

The actual bundled generic Inspector passes Cell X/Y, opacity and enabled edits with exact
per-field Undo/Redo, no size widget, offscreen editing in the 258-row scene and no playback side
effects. Layer/Group controls and source/history/Save-New-Load regressions also pass. The first
browser attempt had an ambiguous selector because nested and root Layers shared a label; the
focused intended-row selector passes without changing product behavior.

The native Cell case passes on Intel Graphics and NVIDIA GeForce RTX 5080 Laptop GPU:
**30 complete 32×24 images**, zero nominal/residual pixel error, through source-native size,
Cell × Layer opacity, live placement/opacity edits, Undo, disabled destination hold and enabled
destination activation. Both normal outputs and preview are checked, with existing completion-safe
resource-retirement assertions. The baseline passes without a native repair experiment. This is
the Cell slice's spatial acceptance; full M07/4 is still required.

## M07/1 acceptance — 2026-11-03

New Matrix creation now builds three Layers, three Columns and the paired Layer exports beside
Final in one detached tree. All nine empty addresses remain sparse. The same factory serves the
public API, all four callers and manager/folder catalog creation. Defaults are creation policy:
deleting all rows/Columns and loading the saved project keeps the Matrix empty.

The baseline exposed two real defects. Prepared catalog creation lost nested curated-item roles
when converting to Golden's staging tree; Golden now preserves those roles. Matrix admission
also rejected whole-owner copies; single and atomic multi-owner copies now use Golden's complete
subtree remapping, while child-only edits remain protected. Both fixes have direct regressions.
A bundled creation/Undo check reproduced a queued viewport read after deletion. The generated
MatrixMissing response and current-media/history fencing clear held data without an error or
history change, and recover on Undo. Mismatched/late absence cannot clear a new selection.

| Original model requirement | Inspected acceptance evidence |
| --- | --- |
| Stable sparse document, derived Layer/Group/Final identities and hierarchy | Current sparse-authoring, validation, nested grouping/move/rename/export-identity and bounded-view suites; unchanged 100k element/depth/batch guards. |
| Typed Layer, Group, Column and Cell settings | Layer/Group/Cell receipts above; real Parameter declarations and projections, atomic Configure, direct validation, early old/opaque-schema rejection, native-sized Cell content and no Cell size. |
| Actual owned media, default Reference and independent Alt-copy | Current owned-media/reference-feedback and nested-copy suites; retained [M04 Reference/ownership acceptance](evidence/mapgyver-reference-native-2026-11-03.json). |
| Backend 3×3 defaults and sparse empty addresses | API, four callers, manager/folder catalog, one-step creation Undo/Redo, exact project reload, explicit empty reload and actual bundled nine-cell grid. |
| Complete authored API, history, copying and project behavior | Current four-caller parity, all owner Parameter/history/copy/project suites, single and atomic multi-Matrix copies with independent paired-export identities; protected-child regression remains passing. |
| Bounded editable selection and owner lifetime | Current offscreen Layer/Cell Inspector in the 258-row scene, deleted viewport/late response/Undo tests, actual creation Undo/Redo with no rejected requests and no launch. |

[Closure receipt](evidence/mapgyver-m07-model-acceptance-2026-11-03.json): **385 app library tests
pass**, two retained exclusions, in **137.12 seconds**. **672 shared Golden tests pass**, three
retained exclusions. Strict Golden and app all-target/native Clippy, generated-contract equality,
Svelte (0 errors/warnings), **28 UI tests**, both formatters and diff checks pass. **Twelve Windows
process regressions pass**, one retained exclusion. Fresh bundled default/grid/history,
Layer/Group/Cell/offscreen Inspector and typed disabled-Cell process-hold/Unload checks pass,
including source/history/Save-New-Load.

**Gate change: M07/1 Accepted.** M07 is 1/4; the fixed plan total is 43/57 gates (75%) and 5/11
fully accepted phases (45%). No denominator, qualification budget or original requirement changed.
Exactly one next gate is Active: M07/2, for its full playback acceptance. This model closure does
not accept the remaining grid design or native/scale/timing qualification.

M07/2 separately retains full Cell → Column → Layer transition precedence and playback acceptance.
M07/3 retains the complete square-cell grid, contextual structural operations, naming, removal,
drag sort/reparent and complete native feedback. M07/4 retains actual native pixels, scale,
memory/timing and owner retirement. Neither extra typed nodes nor passing unit counts waive
those requirements or earn partial acceptance points.
