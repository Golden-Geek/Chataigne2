# P11 dashboard move-presentation lifetimes

Golden UI now owns move-preview and pending-commit presentation in `canvas/surface-move.svelte.ts`,
separately from surface layout. The backend continues to own graph edits and transaction outcomes.
This delivery advances dashboard correctness; creation-drop and resize lifetimes, the complete
action/timing matrix and native input remain open.

## Behavior and boundaries

`move-fence.ts` captures source identity/parent and both surface identities, child order and
layout. Removing, replacing or reparenting a source, changing insertion order/layout, or leaving
edit mode retires stale presentation. Ordinary value publications retain a valid move.
Only involved surfaces construct a move fence; unrelated surfaces do constant-time routing.

Chrome publications carry opaque operation identities. A widget that never published a preview
cannot cancel another widget's preview during destruction. Pending commits expire after two
seconds or clear on matching structure, explicit rejection or invalidation. Commit publication
returns a cancellation function bound to that publication. A delayed rejection cannot cancel a
newer pending move from the same widget. These are local presentation events, not wire DTOs.

Tab previews preserve the same keyed active control instance while hiding/inerting its pane.
Accordion previews retain their insertion anchor even when ordinary detail is bounded. Shared
retention includes marked gesture ancestors and evaluates its cached cohort before budget/layout
DOM changes, so the owning container remains available during the qualified preview operation.
The surface layout controller drops from 930 to 825 lines; DashboardCanvas remains 945 lines.
The [architecture](../architecture/dashboard-ui.md) records the maintained owners.

## Evidence

The final production browser runner passes **124 correctness groups and five sparse tiers**,
including **55 new move-lifetime cases**. Its **80 frozen source fingerprints** match current
files. The [full report](evidence/p11-dashboard-move-lifetimes-2026-10-25.json) records source,
assets, machine and browser. The [six failed probes](evidence/p11-dashboard-move-failures-2026-10-25.json)
preserve failures and qualified artifact identities. The initial source-removal probe predates
adding the new diagnostic driver to source fingerprinting; its bundle fingerprints are retained.

Each of free, horizontal, vertical, grid, tabs and accordion uses a 600-widget nested container.
The cases verify preview content, exact source DOM-instance survival through a zero/restored
budget, cancellation, source removal, layout/mode changes, run-mode refusal, same-ID identity
replacement, reparenting, acknowledged child order, explicit rejection and stale rejection after
a newer commit. An additional horizontal case verifies pending expiry and hidden-source cleanup.
All prior sparse/dense/hierarchy/framing correctness gates remain passing in the full run.

The fixture calls the actual production chrome publisher and ancestor-marking operation, then
applies typed backend graph publications for acknowledgement/removal/reparenting. It does not
synthesize keyboard, pointer or drag events, emulate a backend transaction or qualify physical
gestures. New lifecycle cases use 600 widgets; the existing scale cases separately retain their
1k/10k/20k/50k/100k authored-node tiers. No action latency distribution is established here.

Failed probes exposed a lingering preview after source removal, stale ancestor retention and
global cancellation during tab-control destruction. One early free-surface diagnostic supplied
an ordered insertion index; it was corrected to the production free-surface null index. A
temporary pre-effect initialization error was fixed by declaring the retained derivation first.
The 0.5 CSS-pixel and existing scale/mounting gates remain unchanged.

Root UI tests pass **310 cases**: audio 27 / graph 52 / Golden UI 116 / app 115. Three added unit
cases cover invalid sources/targets, identity/path retirement and layout/order versus ordinary
value publications. Browser and app Svelte checks report zero errors/warnings. Owned Prettier,
whitespace and both Cargo formatters pass. The locked default app build rebuilds the frontend
and passes in **2m30s**. Executable SHA-256:
`cf4a9bcfabc8615431999ee10bdca14bc24685a7402209b40b7452412bb4b5e1`.
Inventory is 2,129 source files / 27 oversized; every touched runtime/test file stays below
1,000 lines, with no new exception.

## Resume

Continue creation-drop outcome/lifetime and active move/resize teardown coverage, followed by
the fixed three-warmup / 31-sample matrix for discovery, selection, edits, navigation, framing
and exact dense picking. Keep the existing normal/stress feedback, stall and completion limits.
Physical input/usability remains P17 and whole-product throughput remains P16.

Reproduce with `npm run test:browser:dashboard --workspace golden_ui`.
`-- --edit-only` runs the 55 new diagnostics and does not replace the full run.
**P11 remains 2/6 fixed checkpoints (33% rounded); whole-plan scope coverage remains 57%.**
