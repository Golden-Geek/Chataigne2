# P11 free-layout edit ownership

Golden UI now gives each free move/resize operation its own commit queue and source-path fence.
Native pointer handlers adapt to explicit presentation commands; backend intents still own
authored mutations and history. This is partial active-operation qualification, not closure of
the dashboard checkpoint or a native-input claim.

## Owners and behavior

`canvas/free-layout-committer.ts` owns the latest unsent sample, an indexed acknowledged baseline
and admitted writes for one gesture. Samples coalesce before a frame and while a write is in
flight. Typed parameter edits travel as a batch. Rejection does not advance the baseline;
cancellation drops unsent work and waits for the admitted prefix before closing its owned edit
session. A final reparent action can execute before that same session closes. Separate queue
instances prevent an old acknowledgement from changing a successor's baseline.

`gesture-fence.ts` captures edited widget/parameter identities and their unique ancestry.
Subsequent validation uses persistent node/parent map deltas. Layout and insertion dependencies
are included. Mode, layout, source/parameter replacement and path changes stop new writes.
Idle widgets do not subscribe to graph changes for gesture validation. The controller owns
native pointer identity separately from operation geometry; cancellation stops instead of
flushing a final sample. Starting an already-selected widget preserves its logical group.
Container metrics are read from the parent surface.

`store/session/edit-session.ts` coalesces one owner's begin/end acknowledgements. An end requested
before begin is acknowledged waits for begin, then closes the matching host/history epoch.
A replacement host or foreign epoch is left alone. The public `createUiEditSession` adapter
sends through its captured host. Existing foreign active groups remain unowned by that helper;
independent gesture admission across widgets still needs its qualification below.

The [architecture](../architecture/dashboard-ui.md) records the maintained boundaries. No wire
DTO, engine behavior or app-specific policy moved into the UI package.

## Evidence

The final production browser run passes **199 correctness groups and five sparse tiers**,
including **20 new free-operation groups**. All **97 frozen source fingerprints** match the
current files, including the shared intent/history/session paths and new unit sources.
The [full evidence](evidence/p11-dashboard-free-gestures-2026-10-25.json) records assets, browser
and machine. The [two failed probes](evidence/p11-dashboard-free-gesture-failures-2026-10-25.json)
preserve the initially incorrect snapshot-history closure expectation and its diagnostic state.
All prior sparse/dense/hierarchy/framing/move-presentation/creation gates remain passing.

Move and resize each use a mounted widget inside a 600-widget free container. The checks verify
visible geometry, delayed-begin closure on mode exit, rejection of further updates after mode,
layout, removal, reparenting and identity retirement, preservation of admitted writes without
draining the retired tail, cancellation before a frame, final coalescing and two-widget logical
selection. The fixture calls production start/update/finish operations and the real serialized
session pipeline. It delays actual begin/write receipts and publishes typed parameter events.
No keyboard, pointer or drag event is synthesized.

The replacement fixture also resets history. Its retired operation must leave that replacement
epoch alone, so that case expects no `endEdit`; other owned cases require exactly one begin and
one end. The original five-second correctness limit and prior geometry/scale gates are unchanged.
This correction does not classify an observation timeout as a stopped process or restart live work.

Root UI tests pass **332 cases**: audio 27 / graph 52 / Golden UI 138 / app 115. The **19 added
unit cases** cover six session-ordering, eight queue/backpressure and five source-path cases.
Targeted TypeScript diagnostics for the four new unit sources report no errors. That check
caught and corrected a duplicate fixture property and a mock's missing argument type, which
Vitest transpilation alone did not reject. Browser/app Svelte checks report zero errors/warnings.
Owned Prettier, whitespace and both Cargo formatters pass.

The final locked default app build passes in **2m02s**, reusing the current UI bundle. The
preceding build rebuilt that bundle after the idle-subscription guard in **2m06s**. Final
executable SHA-256:
`1c48247d9a3e53db4be8b00e71839e09299fd9e6907189056cdc2f1b8e140de8`.
Inventory is 2,142 source files / 27 oversized. DashboardCanvas is 960 lines and the free
controller is 512; all touched runtime/test files remain below 1,000 lines without a new exception.

## Remaining acceptance

The subsequent [confirmed-edit milestone](p11-dashboard-confirmed-edits.md) delivers session-owned
final work/drafts across view disposal and independently admitted widget history. This report's
interruption checks alone did not establish that handoff. Continue ordered moves/constrained
resize, active container and varied-unit geometry, project/save barriers and the remaining
composed gesture lifetimes before declaring teardown complete.

Then run the fixed three-warmup / 31-sample matrix for discovery, selection, edits, navigation,
framing and exact dense picking at every authored-node tier. This correctness run establishes
no latency distribution. Native gestures/usability retain P17 and full-product throughput P16.

Reproduce with `npm run test:browser:dashboard --workspace golden_ui`.
`-- --gestures-only` selects the 20 new diagnostics and does not replace the full run.
**P11 remains 2/6 fixed checkpoints (33% rounded); whole-plan scope coverage remains 57%.**
