# P11 dashboard action timing contract

Fix this matrix before qualification. Use the existing 1680 x 1200 owned browser, 90vw x 90vh
panel, 1200 x 850 page, 16px root font, 12rem x 5rem widgets and 0.5rem snap grid. Authored-node
tiers are 1k/10k baseline, 20k/50k normal and 100k stress. Report actual authored/leaf/container
counts: the 32-container hierarchy has at least six leaves per container; depth-eight binary free
hierarchy has 255 containers and at least 768 leaves. Their minimum sizes can exceed a small tier.
Do not shrink density, hierarchy, dimensions, samples or budgets after a failure.

Each tier has 32 action/profile combinations, each with three warmups and 31 retained samples:

- Sparse grid editing: selection, rectangle selection, single move, four-widget move, resize,
  pan, zoom, distant Frame and Home (nine).
- Fully overlapping widgets in edit and run modes: exact foremost picking, full logical selection,
  and Frame of the full selected union (six).
- The 40-column grid at minimum zoom 0.25 in edit and run modes: exact picking and pan (four).
  Running keeps the authored-page clip; this is a viewport slice, not a view of every widget.
- A 32-container mixed hierarchy: child discovery and Frame beneath each of the six parent layouts
  (twelve). Discovery opens/filters the real child browser when the collection is windowed; small
  collections use their existing direct child view operation. The chosen child must render.
- Depth-eight free hierarchy: Frame a leaf beneath its ancestors (one).

Preparation resets selection/view or closes a preceding browser outside the measurement. Fixture
construction is recorded separately. The timed operation includes lookup, presentation/store work,
Svelte updates, actual DOM/layout checks and final acknowledgement/history closure where relevant.
Use the same public operations as UI handlers; no synthetic keyboard/pointer events or desktop use.

Observe first correct rendered selection/content/geometry. Navigation feedback requires changed
rendered camera geometry and painted scene content; target arrival is separately checked at
completion. Completion requires the operation promise, finished camera animation, settled managed
work and correct actual DOM geometry, then two animation frames of stable expected state. Record
animation callback intervals and actual presentation peaks throughout the operation. Separate normal
detail from additional frame/interaction/selection cohorts; overlapping reasons never double-count a shell.

Keep feedback p95 <=200ms, maximum callback stall <=200ms and completion p95 <=2,000ms for normal
tiers; stress limits are 400ms / 400ms / 4,000ms. Report callback p95 separately. Frame centering and
source/rendered geometry retain 0.5 CSS-pixel tolerance. Sparse mounting remains <=96 shells; dense
detail <=192 with at most 256 overview groups. Hierarchy permits 512 ordinary shells, at most 16
additional retained/transient shells here, and <=1,040 combined shells/overview rectangles. Child
browser rows remain <=42. Preserve the preceding 599 correctness groups and five sparse tiers.

Retain raw warmups/samples, failures, source/asset/machine fingerprints and setup observations.
Run every fixed combination even if a timing gate fails; correctness failures retain their failing
sample and continue independent cases. These results qualify this dashboard matrix, not native
input, a 60-FPS claim, backend throughput or the remaining P11 keyboard/composed checkpoints.

## Harness ownership diagnostic

The cumulative correctness prefix must release temporary DOM identity references after their last
assertion. Use scoped Playwright handles with `finally` disposal; never retain removed controls in
window globals. Do not change project density, hierarchy, samples, timing gates or the cumulative
qualification sequence to conceal harness retention.

`--heap-fresh` compares a fresh renderer with a 1k-authored-node sparse fixture. `--heap-probe`
replays the full correctness prefix before that same resident fixture. `--retention-probe` checks
ownership after each correctness suite by loading the same 1k-node fixture and collecting discarded
objects. These modes are diagnostic-only: explicit collection never runs in qualifying samples or
the ordinary `--timing` matrix.

Fix the retention regression gate before its complete replay: after each suite, the collected heap
must be <=64MiB for the 1k-node / 90-widget fixture. Event listeners must be no more than the initial
fresh-fixture count plus 32, and the resident view retains the existing <=96 widget-shell budget.
Native DOM totals are reported, including cached template fragments; they are not mounted-node counts.
An initial proposed fresh-total-plus-128 check was rejected before qualification: Svelte's `from_html`
retains lazily initialized template nodes in module closures. Use the actual widget budget, collected
heap and listener ownership to reject retired graphs/controls instead of treating all native nodes as
mounted content. These checks allow compiled code and shared UI initialization.
They are harness-lifetime checks, not a product memory budget. Passing them does not qualify action
latency; the unchanged 599-group correctness prefix and all 160 timing combinations still must pass
without explicit collection.
