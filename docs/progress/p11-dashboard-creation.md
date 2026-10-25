# P11 dashboard creation-presentation lifetimes

Golden UI now owns native drag preview/depth and pending creation in
`canvas/surface-drop.svelte.ts`. Native drop handling and the public surface creation operation
use that same owner. Backend intents remain responsible for creating and binding authored nodes.
This delivery completes the pending-creation cases; active move/resize teardown and the fixed
action/timing matrix remain required before dashboard checkpoint closure.

## Ownership and behavior

`surface-fence.ts` captures the destination UUID, layout and direct child order. Drops also
capture the referenced source UUID. Both drop and move presentation use the shared surface
check. Replacement, source removal, changed layout/order, leaving edit mode and view disposal
retire stale presentation. Ordinary value publications preserve a valid destination.

Every pending creation has its own operation object. Rejection or an exception clears only that
operation, so an older request cannot erase a newer preview. A matching graph publication or
other structural change clears the obsolete insertion preview. Pending presentation retains
the existing two-second expiry. Disposal clears state and timers; an eventual reply cannot
restore it. Accepted backend work keeps its authoritative outcome after its former view retires.
These rules do not add transport cancellation or infer transaction success from UI appearance.

The surface controller now adapts native events, gathers placement hints and invokes focused
owners. Its old pending-child sets and duplicate timeout are removed. Registered surface
creation provides the same operation to other presentation callers. The controller is 788 lines;
DashboardCanvas is 951. See [architecture](../architecture/dashboard-ui.md).

## Qualification

The final production browser run passes **179 correctness groups and five sparse tiers**,
including **55 new creation groups**. All **84 frozen source fingerprints** match the current
files. The [full report](evidence/p11-dashboard-creation-2026-10-25.json) records assets, browser
and machine. Prior sparse/dense/hierarchy/framing/move correctness gates remain passing.

Each layout uses a 600-widget nested container. Free, horizontal, vertical, grid, tabs and
accordion verify overlapping rejections, removed sources, same-ID identity replacement,
destination layout/order changes, mode retirement and run-mode refusal, acknowledged creation,
retained destination budget round trips and offscreen disposal/remounting. One horizontal case
also verifies pending expiry. Success publishes a configured registered-widget subtree and
checks the actual new control; rejection and stale completion do not restore retired previews.

The fixture invokes the production surface operation and actual WorkbenchSession intent queue.
It distinguishes queued operations from transport receipts because that queue serializes
dispatch. It publishes typed node/child events for acknowledgements. These checks exercise
presentation outcomes, not backend transaction semantics or native drag/keyboard/pointer input.
New cases use 600 widgets; existing scale cases separately retain all five authored-node tiers.
They establish no latency distribution or physical-input qualification.

Two fixture failures are preserved. The [deadlock observation](evidence/p11-dashboard-creation-hang-2026-10-25.json)
records the live owned process, changed-source and bundle hashes before termination. The fixture
had awaited a second transport receipt while holding the first acknowledgement; the production
queue correctly serialized dispatch. The [failed snapshot probe](evidence/p11-dashboard-creation-failure-2026-10-25.json)
used an unconfigured node widget and failed the visible-content assertion. Successful fixtures
now publish the registered widget's complete parameter subtree and target reference. No gate
was relaxed to obtain a pass.

Root UI tests pass **313 cases**: audio 27 / graph 52 / Golden UI 119 / app 115. Three new unit
cases cover source/destination identity, insertion order and layout versus ordinary values.
Browser and app Svelte checks report zero errors/warnings. Owned Prettier, whitespace and both
Cargo formatters pass. The locked default app build rebuilds the frontend and passes in
**2m26s**. Executable SHA-256:
`5f39eea3adb014ed1dbc8f4ae3710c336b70168cdc4e139d1be314aed3d84a25`.
Inventory is 2,133 source files / 27 oversized. Every touched runtime/test file remains below
1,000 lines, with no new exception.

## Resume

Qualify active free/ordered move and resize teardown next, including mode/layout/identity/path
changes, queued/in-flight commits and edit-session closure. Then run the fixed three-warmup /
31-sample action matrix for discovery, selection, edits, navigation, framing and exact dense
picking at every tier. Preserve existing feedback, stall and completion limits. Native input
and usability retain P17; whole-product throughput retains P16.

Reproduce with `npm run test:browser:dashboard --workspace golden_ui`.
`-- --creation-only` selects the 55 creation diagnostics and does not replace the full run.
**P11 remains 2/6 fixed checkpoints (33% rounded); whole-plan scope coverage remains 57%.**
