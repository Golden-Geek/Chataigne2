# P11 workspace qualification contract

The actual-app Formula scale extension follows the fixed
[fixture contract](../../apps/chataigne/ui/tests/browser/app-graph-scale/README.md#formula-contract):
the same five backend-tree tiers, dimensions, four action families, 3/31 samples and timing/mounting
ceilings as State. Its dense acyclic Constant/Math chain and free-second-input connection case
are fixed before measurement. Runtime evaluation throughput retains P13/P16.

Spatializer's [fixed extension](p11-spatializer-graph-scale.md) separates authored endpoint counts
from generated matrix cells. Its four modes have 60 timing cases and 16 correctness pressure
groups, with explicit endpoint, overview, region, finder and related-row mounting bounds.
Declaration/publication calibration alone does not qualify actions or close the graph checkpoint.

P11 has six fixed acceptance checkpoints derived from the active plan: manager/catalog windowing,
historical log windowing, dashboard viewport awareness, tree focus/keyboard navigation, graph
keyboard operations, and complete composed workflow qualification. Delivery commits do not add
checkpoints. Partial consumer work does not close a checkpoint that still has required consumers.

The first collection milestone qualifies shared row windowing in actual ManagerListPanel,
OutlinerPanel and NodePickerModalHost. Creation-catalog menus remain part of the first checkpoint;
this milestone does not claim that all P11 collections or keyboard workflows are complete.

## Collection workload and gates

Before formal timing runs, fix 1k/10k baselines, 20k/50k normal and 100k stress collection sizes.
Each graph fixture has that many manager items plus ten structural/nested nodes. Two actual panels
share a workbench session, and a picker uses the same graph. The manager/outliner fixture is
70rem wide and 36rem high; variable-height rows use a 30rem x 32rem viewport. The private browser
viewport is 1680 x 1200. Manager rows include 4rem supplements; generic variable rows are 2/4rem.
Do not shrink these dimensions or remove tall rows after failure.

Each list mounts at most 42 rows, including overscan and one pinned focused row. Test scrolling,
far selection, search and clearing search, hidden-row hoisting, expansion beyond the initial
depth, rejected expansion, unmount/remount, and focus retention/fallback. Confirm actual row text,
selection, rectangles and source identity; a data-count attribute alone is insufficient.

Measure manager filtering, composed manager/outliner selection and picker filtering with three
warmups and 31 retained samples per action/tier. Record verified first DOM/layout feedback,
completion and animation callback intervals/tails. Normal feedback p95 and maximum callback stall
must be <=200ms; stress ceilings are <=400ms. Completion p95 must be <=2,000ms normal / 4,000ms
stress. These are operation-feedback/stall gates, not a 60-FPS or physical-input claim. Setup and
fixture construction are separately recorded. The fixed canvas frame gates remain P10 evidence.

Use production Svelte/Vite, an owned headless browser and component/store operations used by UI
handlers. No synthetic keyboard/mouse input or desktop control is permitted. DOM focus and scroll
geometry are inspected in the isolated fixture. Native input/usability remains P17; full P11
keyboard and broader composed-workflow acceptance stays open until its own required cases pass.

Retain failures and raw samples. Do not relax limits or reduce density to obtain a pass. Source,
bundle, tool and machine evidence must identify what was actually exercised. Later collection,
dashboard and keyboard milestones add their explicit workloads before qualifying them.

## Graph discovery extension

Fix the existing 1k/10k baseline, 20k/50k normal and 100k stress tiers before timing the
graph finder. Use the production GraphCanvas in a 70rem x 36rem fixture, a 1680 x 1200
private browser, 256-column grid with 24rem spacing and one chain edge per node after the
first. Include input/output sockets on every node. The finder searches logical labels and
IDs independently of canvas culling, mounts at most 32 results, and supports next/previous
pages without truncating the logical graph. One focused canvas node may be pinned outside
the viewport. In this fixed scene require at most 128 mounted canvas nodes, including that pin.

Fix four action/tier combinations: far exact-label search, clearing search, next/previous
page round trip, and far-node reveal from overview. Retain three warmups and 31 samples per
combination (20 combinations). Inspect actual result text/IDs, search progress, focused title,
selection, camera geometry and mounted bounds. Use the existing normal 200ms feedback/stall
and 2,000ms completion gates; 100k stress uses 400ms/4,000ms. Keep raw failures. Source setup
is separately recorded. No density or timing limit may be reduced after a failure.

Correctness also covers changing queries before a scan finishes, deletion/renaming while
open, source replacement, cancellation/unmount, focus retention during pan/overview changes,
and selected/unselected overview activation. Call the exact public operations used by UI
controls; inspect owned-browser DOM focus without synthesizing input. This extension qualifies
logical discovery only. Graph closure still requires collapsed/compound sockets, actual app
adapters, authoritative mutations/rejections/reload and movement/connection timing. Native
keyboard usability remains P17.

## Graph mutation and socket extension

Fix the same five graph tiers (1k/10k baseline, 20k/50k normal, 100k stress), 70rem x 36rem
canvas and 1680 x 1200 private browser before timing. Retain the 256-column, 24rem-spaced grid
and chain edges. The last three nodes expose ordinary, nested compound and collapsed sockets;
the last node is the destination, so unrelated source records remain present during editing.
Use actual GraphCanvas public operations and acknowledgement callbacks which publish updated
node positions, collapsed state and directed edges through GraphPresentationAdapter. This is
shared presentation/callback evidence; actual app/backend receipt and reload remain separate gates.

Fix six action/tier combinations: single movement, three-node movement, output-first connection,
input-first connection, compound expansion plus leaf connection, and collapsed-node expansion plus
connection. Use three warmups and 31 retained samples each (30 combinations / 930 samples).
Verify actual source positions/edges, directed endpoint identities, focused controls, pressed state,
mounting and visible request/failure feedback. Keep the existing normal 200ms feedback/stall and
2,000ms completion ceilings; 100k stress uses 400ms/4,000ms. Do not reduce workload or relax gates.

Socket correctness includes all seven button variants, disabled output/input ports, hidden endpoints,
nested expansion, occupied ancestor/descendant exclusion and forced-open connected branches, input-
first/output-first order, cancellation when a socket is hidden, no-callback/read-only guards,
collapse/move rejection and equal-value stale rejection across owner replacement. Input/output IDs
on the same node must remain independent; endpoint keys must not alias when IDs contain separators.
Retain failures, their repaired regressions and raw samples. No input synthesis is permitted;
physical keyboard/webview usability remains P17.

### Actual-app graph scale extension

The [native app fixture contract](../../apps/chataigne/ui/tests/browser/app-graph-scale/README.md)
fixes the real State panel's tree/item dimensions, chain density, four operation families, five
tiers, mounted bounds, source/DOM/SVG checks, sample counts and unchanged timing gates before its
first run. Prototype records come from the owning Rust/native cases and load through the actual
backend. Partial consumer or tier runs do not close graph qualification. Formula and Spatializer
must retain their own domain counts, admission limits and actual-app checks; composed workflows
remain a separate checkpoint.

## Tree keyboard extension

Before keyboard timing, fix the existing five collection tiers and unchanged manager/outliner/picker
scene. Qualify shared production TreeRows operations: Up/Down, Home/End, Right expansion/child,
Left collapse/visible parent, Enter/Space activation and modifier selection. Navigation changes
focus independently of selection. Include hidden/filtered ancestors, rejected expansion, far focus,
collapse/removal fallback, identity replacement and newer navigation superseding pending focus.
Each list retains at most 42 mounted rows in this scene, including the roving and focused row.

Measure far Home/End navigation and branch expansion/collapse in each of the three consumers:
30 action/consumer/tier combinations, three warmups and 31 retained samples each. Keep the existing
200ms feedback/stall and 2,000ms completion normal limits; 100k stress uses 400ms/4,000ms.
Inspect actual DOM focus, row text, geometry and selection. Call the production navigation operation
directly; do not synthesize input. App wrapper integration and constrained picker/lifecycle cases
must also pass before the tree checkpoint closes. Physical keyboard usability remains P17.

### App-consumer and constrained-picker closure

Use the actual ModulePanel, FormulaLibraryPanel, StateProcessorManager and complete
AlchemistEditorPanel property surface at the same five collection tiers in a 70rem x 36rem scene,
with a 1680 x 1200 private browser. Include each app-owned supplement and a leaf-owned private
subtree that must remain hidden. Each manager mounts at most 42 rows. Fix 40 timing combinations:
far Home/End focus or selection/reveal, and branch expansion/collapse, for each app consumer/tier.
Use three warmups, 31 retained samples and the unchanged collection timing limits. Property-host
integration uses public session selection/reveal and backend metadata operations in the actual
editor; shared TreeRows keyboard handling is qualified by the preceding matrix. No physical-key
claim follows from these browser operations. Native usability remains P17.

Constrained picker correctness covers all five tiers: nonselectable branches, hidden-row hoisting,
view exclusion and empty views, outside-root candidates, stale selected UUIDs, required multiple
projections, automatic single projections, optional projections, immediate picks, confirmation after
live constraint changes, request replacement/cancellation, source replacement, focus cycling and
focus restoration. All activation paths must use the same current constraints and projection
validation. Keep the same picker/underlying panels and 42-row bounds. Retain raw failures and
source/asset fingerprints; app-consumer or picker-only success cannot close the entire checkpoint.

## Creation-catalog extension

The shared ContextMenu is the creation-catalog surface used by NodeAddButton and node/graph
context menus. Qualify 1k/10k baseline, 20k/50k normal and 100k stress descriptors produced by
the actual `buildCreatableItemMenu` builder. A root and nested menu expose the same catalog;
each open large layer mounts at most 42 rows in a 22rem-wide, at most 32rem-high viewport.
Menus of at most 128 items retain compact column layout. Do not reduce those counts or replace
the actual menu surface after failures.

Confirm far-item identity/text, focused DOM retention across far reveal and reversal, filtering
focus fallback, exact nested filtering, submenu retirement when its parent disappears, and
compact-menu rendering. Search and reveal use component operations; no synthetic input.
Catalog filtering has three warmups and 31 measured samples per tier with the same fixed
feedback/stall/completion gates above. Complete collection-checkpoint acceptance also requires
the preceding manager/picker/outliner evidence and current owning regression/build checks.

## Historical log extension

Qualify actual LoggerPanel and WorkbenchSession with 1k/10k baseline, 20k/50k normal and 100k
stress retained records in a 60rem x 32rem scene. Include long wrapped multiline messages,
source/tag/content filters, historical reveal, follow-latest and paused append, focus identity,
eviction/clear, consecutive/global duplicate modes and an unlimited-source snapshot.
Full logical history remains available while only the viewport mounts. Ordinary mounting is
bounded by 42 rows here; native text selection may retain up to 64 additional record rows,
with a hard total of 112 in this fixture. Qualify multiline and adjacent-record native selection
through distant scrolling and append. Copy logs uses all filtered logical entries independently
of mounted rows; it is the bulk-history copy path. Native selection is a bounded DOM cohort,
not a request to mount an arbitrarily large selected history.

Measure filtering and a four-record append batch with three warmups and 31 samples per tier,
using the existing fixed feedback/stall/completion ceilings. Confirm actual visible row content,
source identity and geometry after settlement. Input remains component/store operations and DOM
selection/focus APIs in the private browser; no synthesized keyboard/mouse events.

## Dashboard sparse editing extension

The first viewport milestone qualifies sparse free-layout membership and logical direct-child
geometry. It does not close the full dashboard checkpoint. Use a 1680 x 1200 private browser,
90vw x 90vh panel, 1200 x 850 authored page, 12rem x 5rem widgets in a 40-column grid with
240 x 112 authored coordinate spacing. Editing permits staging beyond the page; running retains
the authored page clip. Counts include parameters and structural nodes, independently of widget
counts: target 1k/10k baseline, 20k/50k normal and 100k stress authored nodes.

Ordinary sparse views mount at most 96 widget shells in this scene. Focus may retain one
additional widget; native text ranges may retain at most 64 widgets independently of the total
authored selection. Verify actual text, authored slot bounds, border-box shell bounds, source
identity, distant framing and rectangle selection, keyed reorder, focus retirement, native range
retention/release, local/root font changes and live position updates. Frame centering and visible
slot preservation have a 0.5 CSS-pixel tolerance. Compare against the delivered `654a5f15` renderer,
including a 1,000-child clipped free container and all six layout modes in edit/run smoke checks.

Also verify tab/accordion state across an offscreen round trip, framing a child beneath an
unmounted container, bounded ancestry preparation across 100 offscreen containers and a newer
navigation request superseding pending Frame work. Preparation batches at most 32 unresolved
selections; record actual peak shells and selected-union centering within the same tolerance.

Setup-ready values are individual construction observations, including graph projection and
layout settlement. They are not ordinary-action latency distributions. Current sparse correctness
does not establish dense-view bounds, the full hierarchical/gesture matrix, latency or dashboard
checkpoint completion. Native pointer retention wiring does not qualify physical gestures.

Before closing the checkpoint, fix the dense/hierarchical workloads and mounting/presentation
budgets, then retain three warmups and 31 measured samples per action/tier for discovery,
selection, edits, navigation and framing. Preserve the existing feedback/stall/completion ceilings:
200ms / 200ms / 2,000ms normal and 400ms / 400ms / 4,000ms stress. Measure animation pacing
separately. Keep failures and source/asset/machine records. Native gestures/usability retain P17.

## Dashboard dense-view extension

Use the same authored-node tiers and panel as the sparse cases. Exercise both fully overlapping
12rem x 5rem widgets and the existing 40-column grid at minimum supported zoom (0.25).
Test edit and run modes. The existing minimum zoom and run-mode page clip mean this grid view
contains a viewport slice, not every authored widget; report that distinction explicitly.
Ordinary detailed presentation admits at most 192 intersecting
widgets after at most 2,048 spatial candidate visits. A query that cannot prove completeness
within that work budget switches to an explicitly labeled overview; it must never present a
truncated candidate sample as the complete dashboard.

The overview represents all potentially visible geometry through at most 256 retained bounding
groups. Group bounds and counts describe their authored members, not an exact visible count.
Keep up to 16 of the latest selected direct children as actual controls, plus existing focus,
pointer, native-range and transient framing retention. Bulk logical selection remains complete.
Selecting an overview position must resolve the exact foremost authored widget there, independent
of group approximation and candidate visit limits. Reordering, live geometry changes, removal,
zooming back to detail and page replacement must preserve that contract. Verify real control
content, source identity and geometry; a mounted-count attribute alone is insufficient.

The budget applies per free surface. Qualification of simultaneous nested surfaces, flow layouts,
physical gestures and the fixed action/timing matrix remains required before dashboard closure;
this extension alone cannot establish a global dashboard DOM bound or latency acceptance.

## Dashboard hierarchy extension

Ordinary widget detail shares a page budget of 512 shells. Each rendered child consumes one;
remaining slots are divided deterministically among rendered container children. Free surfaces
use their inherited allowance when deciding detail versus overview. Overview groups also share
that allowance, with one conservative group when no detail slots remain. The pressure fixture
permits at most 1,024 ordinary shells plus overview rectangles in total. Other layouts preserve their
normal presentation while it fits; oversized child collections expose a searchable, windowed
child browser and bounded selected previews. Approximate previews must be explicitly labeled.
Selection, focus, native ranges, active gestures and the existing batched Frame preparation keep
their explicit retention paths; record their additional shells separately from ordinary detail.

Before closure exercise simultaneous free containers, mixed free/flow/tab/accordion branches,
deep ancestry, focus/range retention across budget changes, offscreen Frame and presentation
round trips. Use the same 1k/10k/20k/50k/100k authored-node tiers. Include a 32-container grid,
each with at least six children, and a binary free hierarchy of depth eight. Dense child lists
must keep all logical identities discoverable while mounting at most 42 browser rows per open
menu. Preserve the prior sparse/dense correctness gates and 0.5 CSS-pixel geometry tolerance.
Measure ordinary and retained/transient peaks; do not equate a per-surface bound with this page
budget. Native input remains P17; action/timing acceptance still requires the fixed sample matrix.
