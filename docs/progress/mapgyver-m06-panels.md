# Independent Composition panels and phase acceptance

**M06/5 and M06 are Accepted on 2026-11-03.** Composition panels independently follow
document selection or lock to a project-scoped output, persist lock/page choices, and retain
their own requests, geometry gestures and native preview registration.

## Owning layers

- `media/composition/query.rs` resolves selected media, descendants and Placement children in
  the backend. `InspectCompositionEditor` returns at most **128 ordered instances plus one
  independent selected-instance projection**, with full membership count. It does not allocate
  a full instance list or inspect the media catalog. Counting still walks membership; this is
  bounded projection memory, not a claim of constant-time traversal or whole-app scale acceptance.
- Locks require a document UUID and reject cross-project adoption. Missing owners return an
  empty projection and recover through Undo or loading the same saved project. All four public
  caller routes share the immutable query, including rejected reads during a held edit/redo.
- `editor-controller.ts` owns one active bounded read and one newest replacement per panel.
  Session/generation/history fences reject stale replies. Loaded controls remain usable during
  same-history background reads; geometry actions wait for the exact current history projection.
  The native registration stays installed while a new read resolves.
- `editor.svelte.ts` composes a panel's controller and grouped gesture. The panel persists
  presentation choices through public Golden panel APIs. The global Composition provider,
  whole-catalog SourcePicker, custom Properties component and internal heading/create/refresh
  controls are removed. Typed Placement and Resolution settings use the generic Inspector.
- Golden Inspector rename controls now close their edit state before awaiting a commit receipt.
  Enter followed by blur submits one rename, matching the existing outliner behavior.

## Verified acceptance

Final `cargo test --locked -p MapGyver --lib`: **351 passed, 0 failed, 2 explicit qualification
tests ignored**. Generated contracts and four-route query parity pass. Query tests exercise
300 children, bounded/tail/out-of-range pages, off-page selected descendants, unrelated ownership,
document locks, deletion/Undo/reload, invalid bounds and unchanged held history/redo.

Strict native-qualification Clippy passes for the library and all tests. Both Cargo formatters,
touched-file Prettier and diff checks pass. MapGyver Svelte check has **0 errors/warnings**;
**11 focused Composition tests** and **233 Golden UI tests** pass.

The rebuilt ordinary app passes:

- Headless review: 30 absolute geometry samples, three grouped history gestures, structural,
  source/rejection and project contracts. Bundled owned-media drops, independent variants,
  typed Position/degree Rotation, missing recovery and Save/New/Load remain passing.
- Current `--composition-ui` flow in browser **and actual native WebView2**: 130 children,
  off-page generic Inspector edits/history, bounded pages, two simultaneous side-by-side
  panels, independent persisted lock/page choices, layout reload, real WebSocket disconnect/
  reconnect, selected-copy removal/history, peer close, New during accepted held input,
  cross-project lock rejection, same-document Load and tab/native retirement. The final native
  run observes 51 bounded panel queries, zero page errors and zero rejected requests.
- Ported `--composition-project-ui`: actual New shortcut during accepted drag, stale held-input
  disposal, fresh generic Resolution/history, actual File Open upload with saved child IDs,
  generic rename with exact Undo/Redo, final menu New and unchanged review file hash.
- Native handle regression: eight opposite-anchor resize cases, four outside-corner rotation/
  cursor cases, 46 smooth-zoom samples, translation at zoom, Escape, peer isolation and exact
  grouped history all pass with the new controller. Owned apps exit normally with code 0.

Two defects were found and corrected: history could update Inspector values before the panel's
geometry reply, and Enter/blur could duplicate an Inspector rename. Focused history-fence tests
and actual native/loaded-project regressions prove the fixes. The ported project fixture also
needed an unambiguous source row and rendered-canvas synchronization. Its four cumulative runs
ended at source-row ambiguity, stale geometry, duplicate rename history, then full acceptance.
The native panel flow passed initially and after the corrections; the handle regression failed
at the stale-origin case and passed after its fix. No tolerances or requirements were relaxed.

Final executable SHA-256: `32C528E659B23AAA5750D43C8140FAC458F32E609F33911ED53DCB0B5794238C`.
Receipts: `target/m06-panels-{final-tests,query-tests,clippy,ui-check,ui-tests,golden-tests,build,bundled,project,native,native-handles}.log`.

## M06 phase audit

| Gate | Evidence |
| --- | --- |
| 1: owned model/typed Parameters/API/history/project | [Child-media model](mapgyver-m06-model.md), final 351-test suite and current bundled generic authoring/project flows |
| 2: dependency/cycle/missing/deep/shared ownership | [Dependency acceptance](mapgyver-m06-dependencies.md), final caller and dependency regressions |
| 3: complete native Composition pixels/retirement | [Both-adapter native matrix](mapgyver-m06-native.md): 54 images, 41,472 texels, unchanged 2/255 limit |
| 4: anchored native pointer/zoom/history | [Editor acceptance](mapgyver-m06-editor.md) and passing new-controller native handle regression above |
| 5: bounded independent panels/state/lifetime | Actual browser/native panel and File Open/New flows above |

Whole-app Q04 scale, physical display/timing and broader runtime/resource qualification remain
in their existing gates. The older cross-editor `--source-drop-ui` fixture still assumes channel
outliner rows and reference-only instances; reconcile it before final M09 regression qualification.
It is not cited as passing current owned-media acceptance. Next is M08/2's reusable typed app
control/configuration boundary, followed by revised Matrix integration in the plan's order.
