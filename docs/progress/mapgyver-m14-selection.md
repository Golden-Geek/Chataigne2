# M14/1 editor selection and hover

Updated 2026-11-15. Golden owns reusable input/capture/selection dispatch; app adapters resolve
their edited owners and shared Inspector selection. M14/1 remains **Active, unaccepted**.
Overall acceptance stays88/106 (83%); no new fixed gate is closed by this slice.

The user's2026-11-14 instruction selects M14 before full M10/4 qualification. Every M10
requirement remains unfinished and release-blocking; only execution order changed.

## Delivered behavior

Golden `Viewport2D.onselectionclear` replaces the old background-click callback. It dispatches
unconsumed primary-down and idle Escape, preserving pan capture, tool-owned keys, editable
fields and toolbar controls. Its consumer supplies the owner; Golden contains no MapGyver policy.

Screen Warping selects a Surface on ordinary pointer-down without starting geometry edits.
Keyboard/accessibility clicks remain supported; physical mouse-up cannot repeat the selection.
Background-down and idle Escape call the existing Screen-owner selection path. Alt-drag and
corner editing remain on their existing capture/admission path. Existing selected-only handles
are verified in the DOM fixture, including unavailable-source and invalid geometry.

Composition's backend query now honors explicit edited-owner selection with no selected child,
including stale off-page hints and nested owned Compositions. It no longer selects a first child
implicitly. Locked content and explicit retained hints for unrelated selection remain supported;
their complete independent-editor policy is still unaccepted. The adapter uses authoritative page
selection rather than projected geometry to retain its selected ID, and forwards background/idle
Escape through the shared viewport contract to the Composition media owner.

Golden's public `createViewportPointer` replaces the app-local helper. Composition and Matrix
select a primary hit before mutation admission and retain capture/cancel behavior. Selection
consumes the hit even when editing is refused, preserving viewport keyboard focus when the hit
control is replaced. The browser test reproduces and verifies this focus correction without
manually focusing the viewport before Escape. Physical clicks cannot reselect through the
keyboard/accessibility click fallback.

Matrix Layer background-down and idle Escape select the Matrix media owner; Cell layout selects
its Layer owner without leaving that Layer. Explicit backend owners clear retained Layer/Cell
hints, while owned Cell descendants resolve to their containing Cell. Both queries preserve
bounded visible pages and independent off-page definitions without authoring or history changes.
Unlocked nested Matrix/Composition owner selection now preserves the edited domain instead of
following the nested media's outer Composition placement.

Golden's public `selectedEditorItem` matches the current selected identity against bounded visible
and independently queried definitions. Matrix controls update immediately on a new pointer choice
or deselection; an old cached definition cannot restore handles. Cell selection follows raw
authoritative pages, so parameter projection copies cannot restore the previous selected Cell.

Surface Source uses the existing backend-bound Crop Origin/Size Parameters as its selected
objects, with Surface as edited owner. A body press selects Origin; corner press or keyboard
activation selects Size. Background-down/idle Escape inspect Surface and hide crop handles.
Selection is shared with the ordinary Inspector, including its direct Parameter editor; there
is no extra local selection store. The selectable unselected contour remains available without
a source. Native overlay declarations withdraw corner paint/hit IDs alongside DOM controls.

The crop keeps native capture and gesture admission on press, while sending geometry samples
only after movement. Click release sends no geometry intent. Escape during capture cancels and
consumes the key; a subsequent idle Escape clears selection. Window blur, selection loss and
project/source lifetime replacement stop the owned interaction. The browser fixture covers
refused edit admission, natural focus, typed keyboard selection, capture/cancel and final samples.
Corner selection remains available during metadata refresh: edit admission uses the busy state,
while focus and pointer/keyboard selection remain available. A deterministic busy-state fixture
reproduced the disabled-button failure and now passes for both primary-down and keyboard input.

Golden graph background-down now publishes empty node/edge selection immediately, with no
second callback on release. Idle Escape uses that same public selection contract after connection
cancellation. Text editing, toolbar/finder keys, modifier/marquee selection, middle pan and active
pointer ownership remain isolated. Only selected standard graph nodes show resize handles;
unselected valid nodes get a subtle contour hover without handles or selection changes. The
obsolete mouse-up-only clear flag and its movement bookkeeping are removed. Touched controllers
and the existing browser runner follow the repository formatter.

No additional owner callback or selection store was introduced. Existing Alchemist/State Machine
adapters already select their Formula/manager for empty graph selection. Spatializer's custom
world now selects its module through the same callback, guarded against active endpoint input.
These are shared Golden/consumer changes; MapGyver has no new graph panel from this slice.

Graph pointer cancellation is now separate from commit. Escape first cancels captured input and
any keyboard connection endpoint without deselecting; the next idle Escape selects the owner.
Pointercancel/lost capture/blur and scope/topology/disposal boundaries retire matching ownership
before capture release, discard uncommitted movement/resize/marquee/connection hints, and cannot
submit mutation callbacks. Previously submitted optimistic work remains pending until its own
acknowledgement/rejection. Normal release still commits once. Custom-world capture policy remains
with its specialized adapter.

Matrix grid now consumes Golden's public `selectOnPress` and `clearViewportSelection` alongside
shared contour hover. Cells, row/column/group headers and their labels select before release;
physical clicks cannot select again. Keyboard/accessibility activation, nested triggers, rename,
double-click launch, text keys, disabled input and per-grid drag cancellation retain ownership.
Background/idle Escape inspect the Matrix owner. `Viewport2D` and Dashboard reuse the same
clearing helper. The Matrix controller publishes explicit focus and coordinates together,
including same-coordinate Group changes and owner clearing; pending older reads cannot restore
the old focus. Coordinate-only transfer updates keep their existing focus contract. Existing
backend explicit-owner/no-child handling is reused without a DTO or Rust change.

Inline curves now share real key/easing selection with the Golden workbench and embedded
Inspector. Golden's opt-in registry owner anchor keeps the curve canvas mounted while its
descendants are selected, including real pointer capture and multiple keys. External Parameter
selection retains its Watcher and raw data without moving the canvas; explicit locks retain
their document/UUID scope. A focused adapter rejects pending input after session, project,
snapshot, loading or foreign-selection replacement. Ownership projection uses bounded ancestry
and published graph deltas instead of rescanning large selections on unrelated live edits.
Curve background-down now clears children and explicitly inspects its owner, including when
local selection was already empty. Idle Escape does the same; tool Escape cancels first.
The extracted Golden input controller separates normal release from cancellation for key,
Bezier and segment edits, marquee, drawing and pan. Cancellation retires ownership before
capture release, drops queued writes and immediately removes local previews. Submitted work
settles before its pinned edit session ends. Late/foreign input cannot complete canceled work.
Pan cancellation restores its basis; scope replacement stops stale input. Unchanged owner UUID
projection prevents ordinary live graph publications from canceling a valid drag.
The existing oversized editor/controller still needs further cohesive decomposition; the
bounded Canvas2D renderer and new terminal/types modules do not close the full acceptance gate.

Curve geometry now shares Golden's ordinary Inspector value-access rule. Read-only, disabled
or externally controlled typed Parameters remain selectable, while their complete geometry
group refuses capture and history admission. Current UUID/access checks stop queued writes
after permission loss; remove/duplicate actions honor document permissions. A key moved outside
the plot keeps its typed Inspector and can still be edited there. Replacing the Curve owner
resets retained camera bounds without recentering ordinary live Parameter updates. Constants,
value conversion, key-model projection and easing evaluation have been extracted into focused
modules; their existing behavior and every easing kind remain covered. The editor/controller
is still oversized and requires further cohesive splits.

The Curve Canvas2D renderer is now a326-line DOM/lifecycle host over shared model/frame types.
Paint779, grid398, samples298 and theme199 lines form bounded modules. Painting returns both
bounds and hit geometry from one frame; the host owns sizing, theme observation, input forwarding
and publication. No new selection store, compatibility type alias or authored setting was added.
This closes the renderer's structural debt only; the remaining oversized editor/controller stays explicit.

Pending Curve draw/create/paste/easing commands now capture their original owner, session and
document before awaiting an edit-session acknowledgement. A reproduced draw previously fitted
Curve B after starting on A and switching owners while Begin was held. The corrected path
discards that unsent draw. Golden's reusable operation lease separates permanently lost view
ownership from the document lifetime, allowing already submitted original-document work to
settle without selecting into a replacement view. Host/project/snapshot/loading/disconnect,
owner removal and disposal invalidate the document lease; foreign selection suppresses feedback.

The extracted creation controller uses the exact created-node ID/UUID receipt and typed graph
projection events instead of guessing the first new key or polling. A concurrent decoy key is
ignored. Admitted creation finishes initialization on its original owner even after the view
changes, with current access checks before each write; later paste entries are not submitted
into the replacement view. Scoped preview cleanup preserves newer previews. Geometry admission
also refuses to join an existing edit session while command creation is pending. The4786-line
editor/controller still needs further cohesive decomposition and actual-host acceptance.

Matrix Parameter control subtrees now admit ordinary Curve CRUD in the actual backend. Previously
Matrix's ancestry guard rejected real key creation with `app_intent_rejected` / “Use Matrix
operations to edit Matrix structure and its paired exports.” The guard now recognizes Golden's
Parameter-control ownership boundary while protecting declared Parameters, Matrix elements and
paired exports. The connected production UI confirms an exact created-node receipt, initialized
typed key Inspector and one backend history item. This is a usable creation fix, not full Curve
input acceptance: the same fixture then selects the key before release and retains its canvas,
but its first drag opens/closes Begin/End without a Parameter write. That failure remains required.

Curve's lifetime effect now uses Golden's semantic `createEditorScopeObserver` rather than
canceling on every reactive dependency cleanup. The observer compares session, project, snapshot,
connection state, loading visibility and owner. Hidden loading-metadata replacement preserves an
admitted pointer and its queued edits; real lifetime changes still retire it. Selection/access
revocation and final disposal retain their separate cancellation paths. This is a verified
metadata-refresh correction; the connected first-drag failure is still reproduced.

Gesture/session ownership is now extracted into focused `curve/edit-gesture.svelte.ts` and
`curve/parameter-writer.ts` modules. Admission pins the host and document/selection/access basis;
one scheduled frame retains the latest key and Bezier samples. Terminal input takes its edit
session and current pending writes before closure. Creation/easing commands use the same typed
writer and current access/coercion rules. The editor retains geometry and presentation adapters.
This removes scattered scope/session/queue bookkeeping, but the4615-line component remains
oversized and requires the remaining cohesive splits. No new app behavior or whole gate is claimed.

## Applicable editor inventory

Screen, Source, Composition and Matrix now share Golden's contour action and pure paint
projection. Hover changes only the outline, preserving selection, geometry, handle admission,
disabled dashes and invalid red cues. Native Screen/Source declarations consume the same paint;
the existing native enter/leave path supplies hover. Press/leave/cancel/capture loss/blur and
disposal clear it; explicit Composition/Matrix owner identity replacement clears retained hover.
Actual native input/pixels remain unqualified for this new scope.

Unselected Composition items no longer expose Move gizmos. Known bounds use their actual
contour; unavailable Composition/Matrix bounds retain a selection-only square at the known
pivot. Selection reveals only that item's real controls, with no invented source size. A new
keyboard test reproduced focus loss when a selection target was replaced; the shared viewport
now retains focus before keyboard activation, preserving immediate idle Escape in both editors.

| Editor / owner | Current scope | Exact remaining work |
| --- | --- | --- |
| Golden `Viewport2D` | Browser verified primary-down/idle Escape, child/text/tool isolation, navigation and camera lifetime | Complete independent-editor, capture/cancel and app adapter acceptance |
| Screen Warping / Screen | Pointer-down/owner/idle Escape and selected-only DOM handles; shared contour hover and matching native paint declarations verified | Actual native input/pixel and workbench/Inspector agreement, overlapping hits, off-page and active-gesture Escape checks |
| Surface Source / Surface | Shared typed Crop Parameter selection, owner clearing, selected-only controls; shared hover/native paint declarations; browser capture/cancel and earlier ordinary-host Inspector/history checks | Actual current native pixel/hit agreement, two-editor/offscreen and project/snapshot interaction acceptance |
| Composition / Composition media | Root/nested explicit owner clears old hints; immediate choice/clear and unlocked nested owner retention; selected-only gizmos, actual-bounds hover and unavailable-bounds selection target; pointer/keyboard replacement focus verified | Actual Inspector/native/independent-editor and snapshot acceptance; overlap/off-page and full active-gesture behavior |
| Matrix Layer spatial / Matrix media | Shared owner clear and no-child projection, immediate controls/cached-definition clearing; shared hover, selected-only controls and keyboard/pointer unavailable-bounds selection verified | Actual adapter/Inspector/native checks, independent-editor and active-gesture behavior |
| Matrix Cell spatial / Layer | Shared Layer-owner clear without navigation; backend descendant/off-page selection; shared hover, selected-only controls and unavailable-bounds keyboard/pointer selection verified | Actual adapter/Inspector/native checks, active/outgoing/off-page interaction acceptance |
| Golden graph canvases | Browser verifies primary-down nodes/edges, immediate background/idle-Escape empty selection, selected-only resize/hover, modifier/marquee/pan/text/finder isolation; captured cancellation/focus/identity/lifecycle boundaries and pending-work preservation; Spatializer owner callback wired and consumers typecheck | Actual domain Inspector/native/independent-editor/offscreen/permission and specialized custom-world capture acceptance |
| Golden Dashboard / edited page | Primary-down widget/background selection; selected-only resize zones; border hover; per-page tool-first/idle-owner Escape; free/ordered move/resize and marquee/pan cancellation, nested keyboard and two-editor isolation verified in production browser fixture | Actual workbench/Inspector, permission/offscreen/project and current-host acceptance remain open; unrelated Dashboard backend edits are preserved |
| Animation curves / curve and key nodes | Shared key/easing/field and embedded typed Inspector; background/idle owner, tool-first cancel, selected-only Bezier/contour hover, permission/offscreen/locked-owner checks; bounded renderer; delayed draw/create/paste owner/document fencing, exact creation receipt and original-owner completion without selection theft | Finish oversized editor/controller decomposition and complete actual backend/history/native and full lifetime acceptance; renderer and extracted modules stay under1000 lines |
| Watcher projection canvases | `WatcherProjectionCanvas` renders buffer samples/latest points without pointer, key or mutation callbacks; it has no authored canvas targets | Read-only projection applicability resolved from source. Ordinary Inspector owns its settings; retain panel focus/lifetime checks in actual workbench acceptance |
| Media Preview / followed or locked media | `MediaPreviewPanel` renders `ChannelPreview`/`NativePreview` without authored geometry targets; toolbar changes preview choice, not object placement | Read-only image applicability resolved from source. Existing follow/lock/routing requirements stay intact; actual cross-panel/native qualification remains open |
| Matrix grid / Matrix, Layer, Column and Cell | Shared primary-down/keyboard selection and hover for Cells/headers/labels; background/idle owner clear; immediate focus and stale-reply fencing; connected Golden browser fixture verifies controls, rename/launch, disabled input, independent drag cancellation and listener disposal | Actual backend/Inspector-driven selection, native drag/pixels, virtualized/offscreen/project and independent real-panel acceptance remain open |

This inventory retains the complete gate. DOM callback fixtures do not establish actual
Inspector transactions, native pixels, GPU parity or all editor selection policies.

## Validation

Curve gesture ownership extraction (2026-11-15):64 focused cases, production Inspector browser
and four typechecks0/0 pass. Existing frame hashes, selection/capture/cancel, metadata refresh,
permissions, pending operations and locked-owner behavior are unchanged. App/shared formatting,
both UI builds and root/Core formatting pass. Logs: `target/m14-curve-gesture-*-2026-11-15.log`.
The coherently rebuilt ordinary app's existing connected check still fails at the same
`captured:false, focused:true` assertion after successful real creation. Receipt:
`target/m14-curve-gesture-connected-2026-11-15/failure.json`. That required check is unchanged;
no new scene, parameter or runtime probe was introduced. No Rust/protocol behavior changed,
and no native/GPU acceptance is inferred. M14/1 Active, M14 0/10,88/106 (83%).

Curve lifetime slice (2026-11-15):64 focused Golden cases pass, including repeated equal-scope
wrappers and each real lifetime boundary. The production Inspector browser retains capture across
a hidden loading-metadata refresh and then submits live typed writes; existing cancellation,
permission, offscreen, exact creation/paste and disposal checks still pass with0 page errors.
Four typechecks report0 errors/0 warnings; app/shared formatting, both UI builds and root/Core
formatting pass. The unchanged Rust ownership slice retains692 passing tests/5 ignored and
strict Clippy evidence. No Rust or protocol behavior changed in this UI slice.

The existing connected run `target/m14-curve-lifetime-connected-2026-11-15/failure.json` remains
**failing** at live drag after successful real key creation. The semantic observer does not resolve
that failure; later connected owner-Escape/Undo/Redo assertions remain unexecuted. This source
correction is supported by its captured metadata-refresh regression, not by a claim that it fixes
the connected failure. No extra probe variant or native/GPU cohort was added. The20:15UTC start
and60-minute cumulative investigation limit remain in force; the next implementation decision
needs source evidence for the actual cancellation path. Logs: `target/m14-curve-lifetime-*-2026-11-15.log`.
M14/1 remains Active, M14 0/10, overall88/106 (83%).

Connected capture assertion (2026-11-15): the existing scene now requires the actual primary-down
pointer capture and keyboard focus before testing movement, reusing the Inspector fixture's
pointer-ID check. The current ordinary executable fails that earlier requirement with
`captured:false, focused:true, activeTag:CANVAS`; selection, canvas identity and real creation
still pass. Receipt: `target/m14-curve-capture-connected-2026-11-15/failure.json`; the owned host
shuts down normally. This is one post-press observation, not proof that no transient blur or
DOM lifetime transition occurred. Later movement/history acceptance remains unexecuted.
The test scene, executable, inputs and deadlines are unchanged; no profiler or alternate
environment was introduced. Stop extending this runtime investigation. Next deliver the
remaining focused Curve input/controller decomposition under the same M14/1 gate, preserving
this exact failing acceptance check and revisiting ownership when the extracted source provides
a justified fix. The parked cache attempt remains untouched; no gate is accepted by this check.

Matrix/Curve backend slice (2026-11-15): the full current MapGyver library suite passes
692 tests with5 ignored; strict `cargo clippy --locked -p MapGyver --lib --tests -- -D warnings`
passes. New public-intent tests cover control-subtree creation, exact creation receipt, one-step
initialization Undo/Redo, complete typed Curve model/identity reload, duplicate/move/removal and
atomic rejection of mixed Matrix/Curve removals and declared-Parameter edits. Sparse reload
rematerializes declared children, so reload compares the complete typed Curve model and UUIDs;
Undo/Redo retain exact sparse-project comparison. The initial test assumptions about duplication
receipts and byte-identical sparse reload were corrected, without changing production codecs.

`apps/mapgyver/tests/browser/curve-selection.mjs` uses the existing ordinary headless host and
production bundled UI, with public HTTP setup and real workbench input. Run it with
`MAPGYVER_TEST_BINARY` pointing to the rebuilt ordinary executable and a fresh output directory.
The retained actual-backend rejection is in `target/m14-curve-backend-ready-2026-11-15/failure.json`.
After the backend fix, both `target/m14-curve-backend-fixed-2026-11-15/failure.json` and
`target/m14-curve-backend-final-2026-11-15/failure.json` verify key creation/identity/typed Inspector/
single history before failing the live-drag requirement. The complete connected suite is **failing**;
its later UI Undo/Redo assertions have not run. No whole gate or native qualification closes.

A bounded attempt to ignore live ancestor metadata in the selection cache passes58 focused tests
and existing Inspector browser/type/build checks, but does not fix the connected drag. It is
**not shipped**: stash `1371a7bda9d03f1f637889dbab90c25c33de4781`
(`mapgyver-m14-curve-ancestor-cache-unproven-2026-11-15`) holds only that attempt and its unit test.
Do not automatically restore or repeat this unchanged hypothesis. The original57-case Golden
implementation remains in the checkout. Current drag investigation began20:15UTC; the observed
baseline and cache experiment are terminal. Next inspect the existing cancellation paths and make
a justified implementation decision; do not extend this chain with another probe variant.
Logs: `target/m14-curve-backend-*-2026-11-15.log`. M14/1 Active, M14 0/10,88/106 (83%).

Curve pending-operation slice (2026-11-15):57 focused operation/Curve/Inspector/viewport unit
cases pass, including permanent view loss, host/project/snapshot/loading/disconnect/owner-removal/
disposal retirement and selection-only feedback suppression. The production Inspector/Canvas
browser fixture reproduces the original wrong-owner draw, then passes delayed draw/paste owner
replacement, valid draw and two-key paste, snapshot replacement, exact creation with a concurrent
decoy, original-owner initialization without selection theft, and disposal. Existing access,
offscreen typed editing, capture/cancel, hover and locked-owner checks still pass with0 page
errors. This is a production UI fixture with a mock transport, not actual backend/history/native
acceptance. Four typechecks report0 errors/0 warnings; formatting, both UI builds and root/Core
formatting pass. Logs: `target/m14-curve-operation-*-2026-11-15.log`; the baseline log retains
the wrong-owner assertion failure. No whole gate closes: M14/1 Active, M14 0/10,88/106 (83%).

Curve renderer structural slice (2026-11-15):48 focused Curve/Inspector/viewport cases pass.
New sampling tests cover visible bounds, screen/value alignment, segment ownership, owner-free
extension lines and step/hold vertical discontinuities; grid formatting and CSS theme fallback/
override behavior remain covered. All existing production Inspector browser interactions,
selected-only/hover paint, permission refusal/revocation, offscreen typed editing, cancellation
and independent locked-owner checks pass with0 page errors. Four app/Golden fixture typechecks
report0 errors/0 warnings; shared/app formatting, both UI builds and root/Core formatting pass.

Before/after owner and keyboard-cleared owner Canvas2D frames match exactly at620×330.
The initial fixture incorrectly expected those two different lifecycle states to equal each
other; they already differ in the existing renderer and are retained separately. Their respective
RGBA SHA256 values are `4c5e4f57b57b492f075cb7202dbeac47b3d987cb13752fd5ffdb858b91fb145f`
and `4579169103f9d6c68f4b149379c8a7e2e26590007d14b33780da9814fe50a69c`, identical before
and after extraction. The selected-key frame changes paint and records its own hash. These are
same-environment browser frame comparisons, not universal cross-platform image goldens or native
GPU acceptance. Logs: `target/m14-curve-renderer-*-2026-11-15.log`; final browser receipt includes
the three frame hashes. No source snapshots or image archives were added.
This is the bounded structural/qualification batch following four product slices, not a new app
capability or whole-gate acceptance. The large editor/controller and actual backend/history/native
inventory remain. M14/1 Active, M14 0/10, overall88/106 (83%); M10 Pending/release-blocking.

Curve model/access slice (2026-11-15):43 focused Curve/Inspector/viewport cases pass, covering
typed model projection without source mutation, current UUID/group admission, every easing kind,
invalid/duplicate sampling inputs and shared manual/template/control/disabled/read-only access.
The production Inspector/Canvas2D browser fixture proves key/segment selection before release
with refused capture/history for read-only, disabled and controlled values; mixed multi-key
groups refuse as a whole. Live disable/control events cancel queued movement before a write.
Protected key double activation cannot remove it. Actual selected-key pixels disappear after
panning offscreen while its real typed Inspector stays editable; its field sends a typed write.
Repeated permission snapshots wait for a fresh completed revision, including repeated Manual.

The initial integration retained an offscreen camera across owner replacement. Resetting camera
ownership in the new scope's effect body fixes it; cleanup alone could observe the prior scope.
Ordinary live updates keep the view and capture, verified by the existing interaction checks.
The initial extraction omitted a constant module's type import; the corrected module and all
four app/Golden fixture typechecks report0 errors/0 warnings. Shared/app formatting, both
production UI builds and root/Core formatting pass. Logs: `target/m14-curve-model-*-2026-11-15.log`;
final unit/browser logs include `-final-`. No Rust/wire DTO or native executable changed.
Mock transport and Canvas2D evidence do not qualify actual backend history, native/GPU pixels
or every lifetime case. Existing oversized controller/renderer code remains explicit debt.
No fixed gate closes: M14/1 Active, M14 0/10, overall88/106 (83%); M10 still Pending/release-blocking.

Curve input slice (2026-11-15):28 focused Curve/Inspector/viewport unit cases pass. The production
Inspector/Canvas2D browser fixture verifies immediate background/field owner clearing, tool-first
then idle Escape, additive marquee cancellation, real pointer capture, foreign/second-pointer
isolation, queued key/Bezier/segment cancellation, real lost capture, element/window blur,
snapshot/owner replacement and late release. Normal dragging continues across published typed
Parameter changes; existing key double activation still submits its removal intent. The initial
double-activation assertion expected easing, but the existing behavior is removal; the corrected
test verifies that original contract. Pixel checks show selected-only Bezier controls, actual
hover contour changes with unchanged fill and selection, and handle withdrawal on owner choice.
The fixture uses opaque handle theme colors to identify the actual Canvas2D pixels.

Stronger independent-owner assertions exposed that the earlier fixture placed lock metadata
outside Golden's `__gc_panel_state` envelope. Both editors were mounted, but that earlier test
did not establish actual locking. The corrected public persisted-state fixture verifies distinct
locked owners and independent Escape, without changing production lock behavior. Snapshot data
is cloned before replacing published nodes; tests observe trusted pointer IDs rather than
assuming a browser pointer number. The final browser runner exits0 with0 page errors.
All four app/Golden fixture typechecks report0 errors/0 warnings; app/shared formatting, both
production UI builds and root/Core formatting pass. Logs: `target/m14-curve-input-*-2026-11-15.log`;
final unit and browser logs include `-final-`. No Rust/DTO or native executable changed.
Mock transport acknowledgements and Canvas2D checks do not prove actual backend history,
native pixels, GPU parity or the whole gate. M14/1 remains Active, overall88/106 (83%),M14 0/10.

Curve owner slice (2026-11-15):24 focused Golden Inspector/viewport regression cases pass,
including unchanged ownership projection across20 live graph updates with20,000 selected keys.
The actual production Inspector/curve Canvas2D browser fixture passes real pointer-down before
release with pointer capture and unchanged canvas identity, key/field/multi/external selection,
Watcher geometry, two mounted editors and CSR input guards. Fixture session/intent replies are
mocked: this does not establish actual backend transactions, GPU/native pixels or the whole gate.
The initial SSR effect harness did not execute Svelte effects; those guards now run in the real
CSR fixture instead. Restoring the Watcher exposed a test locator matching both canvases; the
curve-specific locator passes. Explicit Node browser-runner exit is0; PowerShell's earlier redirected
stderr wrapper reported1 despite a passing receipt. Both app and both Golden fixture typechecks
report0 errors/0 warnings. App/shared formatting, both production UI builds and root/Core
formatting pass. Logs: `target/m14-curve-owner-*-2026-11-15.log` (passing unit log is
`m14-curve-owner-unit-accepted-2026-11-15.log`). No Rust/DTO or debug binary changes. Current
bundles include this slice; current-host/native acceptance still requires coherent rebuilt binaries.
M14/1 remains Active, M14 0/10, overall88/106 (83%); M10 remains Pending/release-blocking.

Grid slice (2026-11-15):60 Matrix/spatial and90 Golden Dashboard/viewport unit cases pass,
including immediate explicit focus changes with identical coordinates and held older/latest
replies. The expanded production browser fixture verifies pointer-down-before-release without
duplicate selection, unselected contour hover, header bodies/labels, Enter/Space, owner clearing,
late release, active nested Stop, rename/cancel on selected and unselected rows, double-click
launch, disabled input, two-grid drag cancellation and detached listener cleanup. It uses a
connected public Golden session, not the actual backend Inspector. All142 Dashboard selection/
gesture/ordered groups and the shared viewport browser regression pass. Both app and Golden
fixture typechecks report0/0; app/shared lint, both production UI builds and root/Core formatting
pass. Logs: `target/m14-grid-*-2026-11-15.log`; final combined browser receipt is
`m14-grid-browser-final-2026-11-15.log`. Dashboard receipt:
`target/qualification/p11-dashboard-viewport-2026-11-15T17-37-02-255Z.json`.
Fixture IDs now follow the shared loader's fresh UUIDs. Non-wrapping result rows keep selected
names from moving the test grid between clicks; cell presses use visible body space around Add
Media. Earlier binding/rename failures remain in their logs. The temporary focus experiment was
reverted. Current bundles include these changes; debug executables still embed the earlier UI.
No new native GPU, actual backend/Inspector or whole-gate acceptance is inferred.

Dashboard slice (2026-11-15):90 focused Golden Dashboard/viewport unit cases pass. The existing
production fixture now has16 selection groups for free/horizontal/vertical primary-down,
selected-only controls, contour-only hover, owner/keyboard/modifier selection, free/ordered
move/resize Escape, real browser pointer late-release rejection, marquee foreign/matching cancel
and blur, navigation cancellation, nested key isolation, two independent editors, and Run mode
text Enter/Escape. All126 existing gesture/ordered-layout groups pass. The initial selection
fixture's default overlapping inspection widget covered its target; explicit non-overlapping
grid data resolves that fixture issue without forcing clicks or changing production hit order.
Golden fixture and both app typechecks report0/0; scoped lint, both production UI builds and
root/Core formatting pass. The selection module keeps `DashboardCanvas` below1000 lines.
Logs are `target/m14-dashboard-*-2026-11-15.log`. Final selection receipt:
`target/qualification/p11-dashboard-viewport-2026-11-15T17-05-30-420Z.json`;
existing regression receipt: `p11-dashboard-viewport-2026-11-15T17-02-36-424Z.json` in that directory.
No current native/actual-app Inspector or whole-gate acceptance is inferred. No Rust/DTO changed.

Contour slice (2026-11-15):15 Golden viewport unit cases and78 Screen/Composition/Matrix cases
pass. The expanded production browser fixture verifies hover without selection/fill/handles,
known/unavailable bounds, only selected gizmos, invalid cues, native Screen hover declarations,
pointer-down-before-release, keyboard replacement focus and Source cancellation. Golden's full
viewport browser regression also passes with0 page errors. Both app typechecks report0/0;
MapGyver lint, scoped shared formatting, both production UI builds and root/Core formatting pass.
Logs: `target/m14-contour-*-2026-11-15.log`, with final browser receipt
`m14-contour-browser-focus-2026-11-15.log`. Earlier failure after keyboard target replacement is
retained in `m14-contour-browser-final-2026-11-15.log`. No Rust/DTO or native binary changed;
the current UI bundles include this slice, while debug binaries below retain earlier embedded UI.
No new native GPU or whole-gate acceptance is inferred.

- `cargo test --locked -p MapGyver --features native-qualification --lib -- --test-threads=4`:
  689 pass,8 ignored. New Matrix cases verify media/channel/Layer owner clears, owned Cell
  descendants, bounded independent off-page focus, nested edited-domain retention and unchanged
  history/project data. Prior Composition/Screen cases retain their acceptance scope.
- Strict all-target MapGyver Clippy passes with `native-qualification` and `-D warnings`.
- `npm run check:mapgyver`:0 errors,0 warnings, including the typed expanded app fixture.
- Matrix/spatial Node suites:59 pass, including immediate owner clearing while a backend reply
  is held, bounded geometry and reader/session/project/history fencing. Golden viewport/selection
  suites:14 pass, including cached-definition clearing, off-page identity and capture/admission.
- The previous36 Composition/spatial Node cases retain their unchanged source scope.
- The prior40 Screen unit passes retain their unchanged source scope.
- Golden viewport browser runner: passes new down-before-release and idle Escape/text/tool
  checks plus existing zoom anchoring, intermediate wheel frames, pan/camera/reduced-motion tests.
- Expanded app browser runner: passes real `SurfaceHandles`, `CompositionPreview`, Matrix
  `RectLayoutControls`/`SpatialViewport` and Golden
  viewport selection, unavailable-source/invalid geometry, denied-edit selection, selected gizmo
  clearing, owner callbacks, natural idle Escape focus, keyboard and pan. Both Matrix modes
  verify down-before-up selection, refused-edit selection and no stale cached handles after clear.
  These are control/callback fixtures, not full Matrix adapter/Inspector or native GPU credit.
- `npm run lint --workspace mapgyver-ui` and scoped shared Prettier: pass.
- Production UI build and ordinary/native-fixture Cargo build: exit0. The desktop build
  explicitly rebuilt its embedded UI. Root/Core formatting and scoped diff checks complete delivery.

The initial Screen background assertion targeted the zoom toolbar; inspecting the actual hit
identified the fixture error. The corrected point is the gap between Surfaces. No product defect
or native failure is inferred from that coordinate mistake. The first new Rust fixtures omitted
the required document identity and created a Reference instead of an owned nested Composition;
corrected fixtures pass without relaxing document validation. Rust query behavior changes here;
wire DTOs/generated contracts remain unchanged and their existing tests pass.
The first Matrix test compile used a channel-reference field on a `MediaId`; the corrected
fixture uses its typed UUID. No runtime API or acceptance requirement changed to make it pass.

Surface Source's later slice runs86 backend Screen tests (all pass), including typed Crop
Parameter/Surface owner resolution with bounded bindings and unchanged project/history. No
runtime Rust/DTO changed in that slice; the earlier689 full-suite passes retain their scope.
Its41 Screen Node cases pass, including native selected-only handle/hit withdrawal. The expanded
browser fixture passes Source owner clearing, keyboard selection, denied editing, capture/cancel
and final samples in addition to Screen/Composition/Matrix. Source typecheck is0/0 and app lint,
strict all-target Clippy and rebuilt ordinary/native binaries pass. Updated native alignment and
gesture fixture setup explicitly selects crop controls; syntax/format checks do not requalify
their native pixels or timing.

The ordinary-host Source fixture now uses the default Screen and current bounded reference
widgets, waits for fonts and enabled controls, and selects real Crop Parameters before handle/
Inspector checks. Its no-history baseline waits for the preceding Inspector group to close;
the initial assertion included that still-closing group. No history requirement was weakened.
The final2026-11-15 ordinary-host run passes actual down-before-release Inspector/control
selection, background/idle-Escape Surface inspection, no authored/history changes from selection,
all four crop handles, origin/clamping, live Inspector feedback before release, Escape, source
loss/Undo, and Save/New/Load. It reports0 page errors and0 rejected crop requests. The initial
post-Undo keyboard failure is resolved by keeping corner selection available during refresh;
geometry admission remains fenced. This browser/host check establishes no native pixel alignment.

Graph slice validation (2026-11-15):60 Golden graph unit tests pass. The graph component/fixture
Svelte check and both Chataigne/MapGyver app typechecks report0 errors/0 warnings. The production
Edge browser suite passes its existing movement/socket/async/lifetime checks plus five selection
groups: down/hover/controls, idle Escape/focus, modifiers/marquee/pan, connection capture/cancel,
and edges/read-only movement callbacks. The new fixture verifies no duplicate release clear.
Its old extra move-commit argument was removed to match the existing zero-argument API. The
corrected Escape dispatch now reaches idle deselection after checking connection cancellation.

Scoped Prettier/syntax checks and both production UI builds pass. One Chataigne build attempt
lost its generated server index during route analysis; the final isolated build exits0. Its cause
is unproven, and the failed log is retained. No Rust/runtime DTO changed or native GPU check ran.
Graph validation logs are `target/m14-graph-*-2026-11-15.log`; the final Chataigne build uses
`m14-graph-chataigne-build-final-2026-11-15.log`. The graph browser receipt is
`target/qualification/p11-graph-keyboard-2026-11-15T14-25-29-041Z.jsonl`.
Current UI bundles include these changes; the Source debug-binary hashes below retain their
earlier Source scope and do not qualify this graph slice or new native behavior.

Graph cancellation audit: the pre-fix browser baseline confirms one move callback and changed
positions on pointercancel. The corrected production suite verifies movement and resize across
pointercancel/Escape/lost capture/blur, foreign pointer rejection, source scope/topology/disposal
replacement, pending optimistic work, concurrent keyboard/pointer tools, pan/marquee/connection
cancellation and exactly one normal-release commit. It reports0 page errors. The fixture targets
the visible control and observes its real pointer ID; capture loss is processed with the next
pointer event. A constructor initialization-order error was corrected before validation passed.
Final60 graph unit cases, graph component/fixture and both app typechecks (0/0), scoped Prettier,
and both production UI builds pass. Logs: `target/m14-graph-cancel-*-2026-11-15.log`; the intentional
failing baseline is retained separately. Final browser receipt:
`target/qualification/p11-graph-keyboard-2026-11-15T16-18-42-497Z.jsonl`.
No native GPU or actual domain Inspector acceptance is inferred from these callback/DOM tests.

Logs live under `target/m14-selection-*-2026-11-14.log` and
`target/m14-composition-*-2026-11-14.log`.
`target/m14-matrix-*-2026-11-14.log` holds Matrix validation; current Source results are
`target/m14-source-*-2026-11-15.log` with retained Rust/Clippy scope in the2026-11-14 Source logs.
Historical Source desktop build log SHA256:
`0818509733cba660717301553ade2d170bb6867f75ce2f278a8c63a3791a55f9`.
Historical Source ordinary app SHA256:
`484879f411c2597e2cc9fda24e6776ceb45453dfcef6d60561b3d86c8cc42a1f`;
retained native fixture SHA256:
`4b1390515b8c9adc6104f37cec48e884f6fb9dc1f58cc4df965072fc0777d04a`.
Prior ordinary checkpoint rebuild after parking the unproven cache attempt:
`35e7ce1c0df6106c7f7cab323c3efeb98734c278b7a978ed755e4c52eda16da1`;
build log SHA256 `cd5a81086ee1e15d9835b48a3070418dfe9b9b1bfa0933daf1873759fad7bda1`
(`target/m14-curve-backend-build-checkpoint-2026-11-15.log`).
Current ordinary build with extracted gesture/session ownership:
`da152e1196b46d896672520433e62ce78752b83a286efceeb54f68cd5ca2bcfc`;
build log SHA256 `579e35ba8dae93d7ca82fdf8e050ad54695206cac9d8f45927bd1041009c4604`
(`target/m14-curve-gesture-build-mapgyver-2026-11-15.log`). Its connected result is the failing
gesture receipt above. The native fixture remains older: these are not a coherent new ordinary/
native qualification pair. Refresh both inputs before native acceptance; Video evidence keeps
its original source/build scope.

Next deliverable: finish the remaining focused Curve input/controller decomposition and fix
the captured-pointer loss exposed by the existing connected case, then actual backend/history/Inspector/native and
independent-editor lifetime acceptance with coherent rebuilt binaries. No completed slice
or passing software suite substitutes for whole-gate acceptance.
