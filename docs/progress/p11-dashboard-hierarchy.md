# P11 dashboard hierarchy budgets

Golden UI now shares ordinary presentation work across dashboard containers. The backend still
owns authored nodes, layout parameters, bindings and transactions. This is another verified
dashboard delivery, not closure of the P11 dashboard checkpoint.

## Ownership and behavior

`presentation-budget.ts` gives a page 512 ordinary widget slots. Each rendered child consumes
one; only containers share the remaining allowance. Invalid/excessive allowances normalize at
the view boundary. Free surfaces respect that allowance for detail and bounding-group overview.
With no remaining slots, one conservative group still describes the unmounted contents.

Oversized flow/tab/accordion collections expose a labeled preview and a searchable child browser.
The browser uses the shared ContextMenu with explicit windowing, including short collections;
all logical children remain discoverable while at most 42 rows mount per open menu. Descriptors
are prepared only when opening it. Choosing a child uses the surface controller's view operation,
opening the relevant tab/accordion as needed, and preserves backend state. Keyboard opening
focuses search; closing after selection returns focus to the browser button.

`surface-retention.svelte.ts` owns one shared policy for focus, pointer targets, active layout
gestures and bounded native text ranges. Keyed slots keep active controls while their container
switches between full content and a preview. Removing focused widgets returns focus to the page. The
tab body uses keyed child identities through destruction, preventing a nullable derived node
from reaching a child during a budget change. Page view state remains UUID-keyed and survives
offscreen unmounting. No extra ancestor wrapper changes the layout or hit-test boundaries.

The [architecture](../architecture/dashboard-ui.md) explains the owners and the
[fixed contract](p11-workspace-qualification-contract.md) records the workloads and budgets.
Focus/selection, native ranges, active gestures and transient Frame ancestry are explicit extra
retention paths; ordinary detail limits do not claim to bound arbitrary forced ancestry depth.

## Correctness evidence

The production runner passes **51 correctness groups and five sparse scale tiers**, including
17 new hierarchy groups alongside the previous sparse/dense cases. It records 68 frozen source
fingerprints, now including the shared menu and row implementation. The private Edge browser
and machine remain recorded in the [full evidence](evidence/p11-dashboard-hierarchy-2026-10-25.json).
The [five failed probes](evidence/p11-dashboard-hierarchy-failures-2026-10-25.json) retain the
runtime errors and their exact source fingerprints. Operations use component/store methods and
DOM focus/geometry; no keyboard/mouse events are synthesized.

Both broad families keep 32 simultaneously visible containers, with at least six children each.
The mixed family cycles free, horizontal, vertical, grid, tabs and accordion layouts.

| Target authored nodes | Actual authored nodes | Authored widget shells, including containers | Free family mounted shells / groups | Mixed family mounted shells / groups |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 2,283 | 224 | 224 / 0 | 174 / 0 |
| 10,000 | 10,005 | 926 | 32 / 480 | 42 / 90 |
| 20,000 | 20,004 | 1,835 | 32 / 480 | 42 / 90 |
| 50,000 | 50,001 | 4,562 | 32 / 480 | 42 / 90 |
| 100,000 | 100,007 | 9,108 | 32 / 480 | 42 / 90 |

The first target deliberately exceeds 1k to retain the minimum 32 x 6 hierarchy. Normal and
stress pressure are not reduced. Each container identity is checked in the actual DOM, not
inferred from a counter. Larger mixed scenes expose 26 bounded child browsers. Far-child reveal,
exact search, row mounting, chosen control identity and search/close focus pass at every larger
tier. The free family uses its remaining 480 slots as overview bounds rather than multiplying
the per-surface 256-group limit by 32.

For each of the six layouts, an actual input retains the same DOM identity and focus when its
page budget drops to zero and returns to 512. Only its owning container and the input widget
remain mounted at zero. Removing that widget returns focus to the page.

A separate 100,000-authored-node fixture contains a binary free hierarchy of depth eight:
255 containers plus 8,974 leaf widgets. Initial detail mounts 255 shells. After all ancestry
unmounts offscreen, Frame reaches the selected leaf, with peak/final mounting of 256 and center
errors of 0.000245 / 0.000138 CSS pixels, below the existing 0.5-pixel tolerance. This proves the
qualified depth-eight free path, not arbitrary depth or all flow/tab framing combinations.

Four new unit cases cover deterministic budget allocation, inherited bounds, retained extras
and invalid/fractional allowances. Root UI tests pass **296 cases**: audio 27 / graph 52 /
Golden UI 102 / app 115. App and browser fixture checks report zero errors/warnings.
Owned Prettier, whitespace and both Cargo formatters pass. The locked default app build rebuilds
the frontend and passes in **2m18s** after the final queued-focus lifetime guard. Executable SHA-256:
`5215b8a05958259afb188b6a6a1b24626e84d092b8cbb2aec73ce59a898d15f6`.

Retained failed probes exposed an accidental sparse-selection pin and a nullable tab-child
lifetime during unmounting. The fixes keep sparse selection logical and make tab child identity
keyed through destruction. The fixture's removal operation now uses the actual graph parent,
so nested removal exercises a valid structural event.

## Remaining checkpoint scope

Continue inactive-tab and bulk framing inside oversized flow/tab/accordion previews, native-range
retention and browser-header focus retirement through budget changes, and the remaining edit/drop lifecycle matrix. The fixed
three-warmup / 31-sample action matrix must then qualify discovery, selection, edits, navigation,
framing and exact dense picking at every tier. Existing normal/stress feedback, stall and
completion gates remain unchanged. These correctness runs contain no latency distributions.
Native physical input/usability remains P17 and whole-product throughput remains P16.

Reproduce the full run with `npm run test:browser:dashboard --workspace golden_ui`.
`-- --hierarchy-only` runs the 19 hierarchy diagnostics, including the two existing presentation
round trips; it does not replace the full delivery run.
**P11 stays 2/6 checkpoints (33% rounded); whole-plan scope coverage stays 57%.**
