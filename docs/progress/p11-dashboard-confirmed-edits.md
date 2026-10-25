# P11 confirmed dashboard edits

Golden UI now retains confirmed free-layout work at the workbench session boundary. Widget
disposal no longer cancels a confirmed tail or removes its visible final geometry. Separate
widgets wait for distinct managed history groups. This advances the dashboard checkpoint;
it does not close P11 or qualify native input.

## Owners and behavior

`store/session/edit-operations.ts` owns bounded FIFO admission, operation lifetimes and immutable
parameter drafts. The reactive adapter only notifies presentation readers after the current DOM
update. The session observes authoritative graph publications, resets on snapshot replacement
and disposal, and preserves its existing intent pipeline and action-progress reporting.

Each free-layout controller supplies its captured host, graph-transition revision and typed
source/path fence. Unconfirmed disposal cancels; confirmed disposal preserves final work.
Mode exit leaves already confirmed work intact, while source retirement prevents its unsent tail.
Cancellation still waits for the admitted prefix before closing the owned group. At most 64
operation records can be retained. This admission covers participating free-layout operations;
the existing edit-session helper still leaves unrelated foreign active groups unowned.

`free-layout-committer.ts` publishes only edited parameter fields. Returning to an initial value
keeps that field projected until settlement. A rejected final sample is distinguishable from a
successful no-op. Presentation readers check parameter UUIDs and consume drafts without mutating
the authored graph. Retained free-surface geometry applies immutable preview-map deltas to the
affected widget owners, keeping spatial queries and Frame consistent with mounted controls.

Successful final drafts remain until graph publication reaches the applied receipt's event
boundary. History admission can pass on once the preceding group closes, independently of that
presentation catch-up. Reset/source invalidation and rejected final samples clear drafts. This
uses actual applied-ACK watermarks when present; it does not invent causality for staged receipts
or establish broader unknown-outcome/project-operation semantics.

## Evidence

The [initial failed probe](evidence/p11-dashboard-confirmed-edit-failure-2026-10-25.json)
records a **24 CSS-pixel snapback** immediately after confirming a move while its first write
receipt was held. The final [full browser evidence](evidence/p11-dashboard-confirmed-edits-2026-10-25.json)
passes **213 correctness groups and five sparse authored-node tiers**, including **14 new groups**
and all prior 199 groups. Its 105 source fingerprints and production assets identify the run.

Move and resize each use a mounted control in a 600-widget free container and the actual session
intent pipeline. The fixture calls the same component operations as native adapters, controls
real begin/write receipts, and can deliver applied ACKs before their typed parameter events.
No keyboard, pointer or drag events are synthesized. New groups verify:

- Final geometry through finish, offscreen disposal and remounting, with no observed jump.
- A second widget's begin/write waits until the first widget's end; each group closes separately.
- Drafts survive completed receipts until typed graph events arrive, including disposal/remounting.
- Frame uses projected geometry; observed center error is below 0.007 CSS pixels.
- Confirmed work survives mode exit; removed source identity cancels unsent work.

Root UI tests pass **344 cases**: audio 27 / graph 52 / Golden UI 150 / app 115. Twelve additional
unit cases cover service admission/ownership, drafts/reset/capacity/event boundaries, touched-only
projection, rejected final samples and indexed draft geometry. The geometry case verifies one
affected owner among 1,000 widgets and unchanged authored values. Targeted TypeScript diagnostics
for the three added/extended unit sources report no errors, including their dependencies.
Browser and app Svelte checks report **zero errors and zero warnings**. Owned Prettier,
whitespace and both Cargo formatters pass.

Because session composition changed, the existing manager/outliner/picker browser regression also
ran through 100k entries: five correctness tiers, all 15 action distributions and variable-row
focus passed. Its [raw samples](evidence/p11-dashboard-session-collection-regression-2026-10-25.json)
remain separate from dashboard timing, which is still open.

The locked default app build passes in **2m04s**, rebuilding the frontend bundle. Executable
SHA-256: `23f76287f1c3d54c122c1ef72aaf259b26551e79fdd9dc59a0f966ce40c88770`.

Inventory is **2,148 source files / 27 oversized**. DashboardCanvas is 966 lines, the free controller
529, the pure session owner 233 and the workbench facade 955. All touched runtime/test sources
remain below 1,000 lines, without a new exception.

## Remaining acceptance

Continue ordered move/constrained resize and its backend enable/placement boundary, active
containers and varied CSS units. Qualify composed ownership across reparenting, host/project
replacement, save/undo barriers and pending confirmed tails. The current raw intent barrier does
not yet establish a barrier over all unsent confirmed work; the composed gate remains open.

Then run the fixed three-warmup / 31-sample action matrix for discovery, selection, edits,
navigation, framing and exact dense picking across every authored-node tier. This correctness
run establishes no latency distribution. Native gestures/usability retain P17 and full-product
throughput P16. Tree keyboard, graph keyboard and complete composed-workflow checkpoints remain.

Reproduce with `npm run test:browser:dashboard --workspace golden_ui`.
`-- --confirmed-only` selects the 14 diagnostics and does not replace the full run.
**P11 remains 2/6 fixed checkpoints (33% rounded); whole-plan scope coverage remains 57% (57.41% exact).**
