# Dashboard UI ownership

Golden UI renders app-agnostic dashboard pages and widgets. The backend owns authored
nodes, parameters, bindings and edit transactions. App UI registers product renderers through
`dashboard-node-widget-registry.ts`; Golden UI does not import app types or policy.

Dashboard moves and explicit resizes use generated `moveDashboardWidget` and
`resizeDashboardWidget` intents. `golden_engine/src/ui_sync/dashboard_layout.rs` owns destination
layout policy, authoritative item-index resolution, supported dimensions and sizing enablement.
Move plus placement, or size plus enablement, settles as one undoable transaction. Invalid
placement, identity, read-only fields and hierarchy changes reject without partial authored edits.
Layout field lookup follows local declared children; it does not build a whole-tree snapshot.
The UI supplies viewport-derived geometry and sends one intent through the captured session.

Start at `packages/golden-ui/components/panels/dashboard/DashboardCanvas.svelte` for composition
and widget templates. Its `canvas/` modules own distinct presentation lifetimes:

| Owner                                                            | Responsibility                                                                                                     |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `surface.svelte.ts`, `SurfaceContent.svelte`                     | Child layout, tabs/accordion view state, native drop adapters, registered view operations and recursive rendering. |
| `surface-drop.svelte.ts`, `surface-move.svelte.ts`               | Active drop/move and pending creation/commit presentation lifetimes.                                               |
| `camera.svelte.ts`, `page-view.ts`                               | Viewport measurements, pan/zoom, persisted camera state and Home/frame commands.                                   |
| `free-layout.svelte.ts`, `ordered-layout.svelte.ts`              | Free gestures and thin ordered-operation composition through shared presentation commands.                         |
| `ordered-move.svelte.ts`, `constrained-resize.svelte.ts`         | Independent move/resize lifetimes, captured admission, native adapters and cancellation.                           |
| `surface-item-order.ts`                                          | Page-shared authored item indices for bounded mounted-anchor queries.                                              |
| `interaction-geometry.svelte.ts`, `interaction-chrome.svelte.ts` | Transform targets, hit testing, insertion/ancestor decoration and move-preview publication.                        |
| `marquee.svelte.ts`                                              | Surface presses and marquee selection.                                                                             |
| `widget-content.svelte.ts`, `binding-drop.svelte.ts`             | Live target rendering, generic controls and binding intents.                                                       |
| `context-menu.svelte.ts`                                         | Capability-dependent menu presentation and explicit edit intents.                                                  |
| `geometry.ts`, `dom.ts`, `contracts.ts`                          | Pure coordinate calculations, DOM measurements and shared presentation types.                                      |
| `free-surface-geometry.ts`                                       | Retained presentation bounds and spatial queries, independently of mounted widgets.                                |
| `free-surface-groups.ts`, `FreeSurfaceOverview.svelte`           | Retained bounding hierarchy and explicit dense-view presentation.                                                  |
| `presentation-budget.ts`, `SurfaceChildBrowser.svelte`           | Hierarchical detail allowances and searchable child previews.                                                      |
| `surface-retention.svelte.ts`                                    | Shared focus, native pointer/range and active-gesture retention across layout types.                               |
| `free-surface-viewport.svelte.ts`, `view-context.svelte.ts`      | Clipped membership, focus/range/interaction retention, page-scoped view state and logical framing.                 |

Controllers receive live state through getter contexts. Methods that need a mutually dependent
controller defer access until use; constructors must not eagerly read a later controller.
Each owner disposes its event listeners, observers, animation frames and edit sessions.
`SurfaceContent.svelte` adds no wrapper, preserving direct-child layout and hit-test boundaries.

Shared CSS lives under `canvas/styles/`, namespaced by `.gc-dashboard-canvas`. It covers both
the root shell and descendants across recursive component boundaries, in the original cascade
order. Context-menu portal selectors keep their separate `gc-dashboard-context-menu` namespace.

Rendering uses the authored anchor and position directly. Changing an anchor does not cause a
mounted widget to rewrite its position. Viewport-dependent presentation must not repair domain
state or make authored meaning depend on whether a widget is mounted.

Free slots and their widget shells share border-box bounds. The retained model uses the same
size/anchor/minimum contract, measured surface dimensions, local `em` size and root `rem` size.
The shared CSS conversion API requires both font measurements; gesture and creation contexts
carry them from the DOM boundary. No authored values are rewritten to accommodate mounting.

The model updates changed owners using `VersionedNodeMap.forEachChange`, which skips shared
trie branches. It consumes the public `golden_graph_ui/spatial-index` primitive. Camera changes
query retained geometry; layout/font/viewport unit changes reindex affected surface bounds.
Marquee selection and framing can reach unmounted direct free children. Framing temporarily
materializes missing ancestry in batches of 32 unresolved selections, captures page-local bounds,
then releases those ancestors before camera animation. Non-free layouts retain their DOM geometry
path. New navigation supersedes pending framing. A page owns the shared view context, including
non-navigable viewers.

Tab and accordion presentation state lives at that page boundary, keyed by authored UUIDs.
Surface remounting resolves saved child UUIDs against the current graph. Removed identities and
page changes retire their cached state. Public view operations delegate to the same surface
controllers as tab/accordion buttons; they do not edit the backend graph.

The viewport owner intersects clipping ancestors. Shared surface retention preserves a focused
control, layout gestures and native pointer targets, and bounds native text-range retention to
64 direct widgets per surface. Pointer targets use
authored identity and are released on up/cancel/blur or owner retirement. Keyed moves restore focus after
DOM reconciliation; removal uses the remembered authored identity to return focus to the page.
Observers and event-driven refresh are disposed with their surface. Disabling a free view clears
its retained graph/index references.

Detailed free views admit at most 192 ordinary widgets after at most 2,048 candidate visits.
If a query cannot prove completeness within that budget, the surface switches to an explicitly
labeled overview instead of showing a truncated sample of controls. A spatially balanced retained
hierarchy produces at most 256 conservative bounding groups without enumerating their members
on camera movement. Membership changes rebuild that hierarchy; individual geometry changes
repair its ancestor bounds. Group counts describe authored members, including offscreen ones.

The overview keeps up to 16 of the latest selected direct children as actual controls, alongside
the existing focus, pointer, range and framing retention. Bulk logical selection is unchanged.
Activation resolves the exact foremost authored widget at that point through retained geometry;
approximate groups never decide control identity. The page returns focus when a focused overview
retires. Widget shells isolate their own stacking context, so an earlier label cannot paint above
a later sibling's controls. These budgets apply per free surface; nested/flow composition still
needs its owning qualification. Coordinates remain CSS-pixel measurements of authored geometry;
the overview caption uses relative units adjusted for camera scale.

A page now shares 512 ordinary detail slots through its hierarchy. Every rendered child costs
one slot; remaining slots divide deterministically among rendered containers. Free surfaces use
that allowance for both detailed membership and overview groups. Oversized flow/tab/accordion
collections show a labeled selected-child preview with a searchable child browser. The browser
uses ContextMenu's explicit `windowed` option, including short collections, and prepares menu
items only while open. It retains complete authored identity/order without mounting every header
or control. Choosing a child uses the surface controller's view operation; it never repairs or
rewrites backend state. Keyed slots retain active controls across budget changes.

Selection, focus, ranges, gestures and Frame preparation can retain additional shells. These are
reported separately from ordinary detail. The current hierarchy evidence does not close arbitrary
depth, all flow/tab framing combinations, native gestures or operation latency qualification.

[Sparse viewport qualification](../progress/p11-dashboard-sparse.md) covers sparse views,
container state retention and batched ancestry framing. [Dense-view evidence](../progress/p11-dashboard-dense.md)
covers overlapping and minimum-zoom fixtures. P11 still owns the complete hierarchical/gesture
matrix and workflow timing before the dashboard checkpoint closes.
The [hierarchy report](../progress/p11-dashboard-hierarchy.md) records shared-budget pressure,
child discovery, focus transitions and depth-eight free framing.
The [framing report](../progress/p11-dashboard-framing.md) covers bounded flow geometry,
inactive panes, native ranges, header focus and local scroll restoration.

`flow-layout.ts` preserves omitted horizontal/vertical extents through CSS-unit prefix sums
and bounded spacers. It rounds each authored slot using the existing CSS serializer before
aggregation; the browser resolves mixed units and auto bases. Retained grid items keep their
original cells. Presentation budgets therefore do not repack the selected controls.

`frame-preparation.ts` captures selected paths and source identities, validates structure across
asynchronous preparation, and exposes at most 32 pending targets per batch. Surface controllers
activate a selected tab or open selected accordion members as view state. Inactive measurement
panes share the active pane's geometry and remain hidden/inert. Free leaves use retained logical
bounds rather than mounting controls. `frame-bounds.ts` projects those bounds through inner
clips; local scroll owners reveal controls before camera framing. Bulk scroll content is
represented by its containing viewport on the clipped axis.

`surface-retention.svelte.ts` observes focus before DOM updates and treats tab/accordion headers
as direct children alongside widget shells. Native ranges retain at most 64 distinct children.
`surface-scroll.svelte.ts` saves the last observed local scroll position in the page's UUID-keyed
view cache, restores it across unmounting and removes listeners on disposal. Retired identities
and page changes clear cached state. The child-browser anchor remains reachable while scrolling.

`surface-move.svelte.ts` owns active move previews and pending commit presentation.
`move-fence.ts` captures source/parent/target identity, layout and order; structural changes,
replacement, reparenting and leaving edit mode retire stale presentation. Widget chrome publishes
opaque operation identities so unrelated component cleanup cannot cancel a move. Commit
publication returns a cancellation function bound to that operation, preventing a late rejection
from cancelling a newer move by the same source. The backend still decides transaction outcomes.
Tab previews keep keyed active controls mounted but hidden/inert; accordion previews retain their
insertion anchor. Shared retention includes marked gesture ancestors and refreshes before a
budget/layout DOM change. The [move-lifetime report](../progress/p11-dashboard-move-lifetimes.md)
records the exercised operations and remaining creation/resize/native coverage.

`surface-drop.svelte.ts` owns native drag depth/preview and pending creation independently of
asynchronous backend requests. Native drop handling and the public surface creation operation
use that same owner. `surface-fence.ts` supplies shared destination identity/layout/order checks
for both drop and move presentation. Drops also validate the referenced source UUID. Mode,
identity, layout or structure changes retire stale previews; disposal clears presentation and
timers. An older rejection cannot clear a newer pending creation. Accepted backend work keeps
its authoritative outcome even if its former view has disappeared. The
[creation report](../progress/p11-dashboard-creation.md) distinguishes these presentation checks
from native gesture and transaction qualification.

Free layout now separates native pointer adaptation from explicit move/resize/update/finish
operations. `free-layout-committer.ts` owns each gesture's latest unsent sample, admitted writes
and acknowledged delta baseline. It batches typed parameter edits, waits for admitted work before
closing history, and discards unsent work on cancellation. A final reparent action can run inside
the same edit session. Baseline lookup is indexed by node identity rather than repeated scans.
`gesture-fence.ts` captures targets, parameter identities and unique ancestry. Later validation
uses persistent node/parent map deltas; layout and insertion dependencies retire stale operations.

`layout-frame.ts` owns the measured parent coordinate frame for active free transforms and
constrained resize. Input, confirmation and unsent-write admission check its bounds, fonts,
scroll and viewport dimensions. A changed frame retires unsent samples while admitted work closes
its history. Window resize has an event listener; there is no recurring measurement poll. Once
confirmed, work releases its DOM frame and uses its captured authored sample through view disposal.
Resize snapping applies only to edited edges, preserving opposite edges and unedited axes even
when their authored positions are off grid. Placement DTOs copy scalar dimensions and enablement
before leaving the presentation layer, so reactive draft objects cannot cross the transport boundary.

`store/session/edit-session.ts` serializes one owner's begin/end acknowledgements, including an
end requested while begin is pending. Host identity and history epoch guard closure. The public
`createUiEditSession` adapter sends to the captured host. It continues to leave foreign active
groups alone. `store/session/edit-operations.ts` owns free-operation admission across widgets,
confirmed work after view disposal, and immutable parameter drafts. At most 64 operation records
are retained. Unconfirmed disposal cancels; confirmed disposal preserves admitted/final work.
Source identity/path, host, graph transition and resync fences still retire invalid work.
History admission passes to the next operation after the preceding group closes. Successful drafts
remain separately until graph publication reaches the applied acknowledgement's event boundary.
Rejected final samples clear their drafts. Reset cancels all owners, including confirmed work.

Drafts contain only fields edited by the operation, preserve parameter UUIDs, and never mutate
the authored graph. Placement readers and retained spatial geometry consume that presentation
layer, so remounted controls and Frame see the same pending geometry. Preview-map deltas reindex
only affected widget owners. The small reactive adapter publishes after keyed DOM disposal;
the pure service owns no timers, DOM or transport implementation. Intent senders compose applied
receipts with existing action lifecycle reporting through the session pipeline.

Session `runEditBarrier` closes managed admission, cancels active samples and drains confirmed
tails/history before save or other session work. It then flushes raw dispatch and checks snapshot
identity. Project replacement drains managed work before acquiring its existing dispatch suspension.
Concurrent barriers serialize; callbacks do not enter another barrier or open a managed gesture.
Late presentation-only events do not delay completed backend work. Undo/redo check availability
after drainage and report failures through the UI log. Save captures the submitted history identity,
so edits arriving while the request is pending remain unsaved. See the
[barrier contract and evidence](../progress/p11-dashboard-barriers.md) for recovery/host boundaries.

The [free-operation report](../progress/p11-dashboard-free-gestures.md) covers active interruption;
the [confirmed-edit report](../progress/p11-dashboard-confirmed-edits.md) covers view disposal,
independent widget history and late graph publication. Generic ordered lifetimes are qualified
below. The [composition matrix](../progress/p11-dashboard-composition.md) extends these checks to
containers, registered node widgets, mixed destinations and active coordinate-frame changes.
Applied-receipt project/save/history/reparent and host-retirement ordering are qualified by the
barrier matrix. Action timing and broader composed workflows remain open.
These checks do not establish staged/unknown-outcome semantics or complete dashboard acceptance.
See the [fixed contract](../progress/p11-workspace-qualification-contract.md) and the historical
[decomposition comparison](../progress/p11-dashboard-decomposition.md).

`WidgetLayoutEdit` is the presentation-command entrypoint for all layout controllers. The page's
view registry dispatches to the appropriate free or ordered owner. Native adapters keep pointer
identity separately and route cancellation/focus loss to the same cancel operation. Confirmed
ordered moves wait for session admission, then send one atomic backend intent through the captured
host. Resize reuses the coalesced queue and owned history group. Disposal cancels unconfirmed work;
confirmed work completes independently of the component. These owners subscribe to graph changes
only while an operation is active.

Resize drafts explicitly include presentation enablement, alongside UUID-checked values. They do
not change authored metadata. Placement and slot readers retain them until the applied event
boundary; rejection/cancellation removes both parts. Auto sizing starts from measured rendered
bounds, preserves the authored CSS unit, and uses border-box widget shells across layouts.

Insertion queries enumerate mounted controls/headers and resolve their authored indices through
the page's retained `SurfaceItemOrder`. They do not zip sparse headers to logical children or scan
all siblings for every pointer update. Structural/order/role changes refresh the index; value-only
publications use immutable map deltas. An ordered surface with hidden items and no anchor cannot
invent an insertion position. See [ordered lifetime evidence](../progress/p11-dashboard-ordered-lifetimes.md)
for the generic scope, plus the composition matrix for container and registered-widget coverage.
Timing qualification remains required before dashboard acceptance closes; the barrier matrix covers
normal applied receipts, with staged/unknown outcomes explicitly outside that evidence.
