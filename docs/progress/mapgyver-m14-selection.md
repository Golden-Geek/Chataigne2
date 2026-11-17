# M14/1 editor selection and hover

Updated 2026-11-17. Golden owns reusable input/capture/selection dispatch; app adapters resolve
their edited owners and shared Inspector selection. M14/1 is **Active, unaccepted**.
Overall acceptance is92/106 (87%), M14 4/10 after accepted gates3–5,7.

**Current Source result:** Shared workbench admission, unexpected capture-loss cancellation,
projected-control focus and tool-first native Escape are implemented. The preview browser suite,
68 preview tests, both UI typechecks, production controls and the complete original headless
Source contract pass. Native Intel confirms captured Escape, retired pointer input and exact
Undo. Reconnect still splits one drag across history: one Undo retains its first accepted sample
instead of restoring the pre-drag scene. Keep the failing native regression and correct shared
request/edit-group lifetime on disconnect next. Remaining Source lifetime and physical/native
image checks are unaccepted. See the [checkpoint](evidence/mapgyver-m14-source-capture-2026-11-17.json).
The final NVIDIA Screen audit also rejects a queued old-Screen geometry request after New
(`screen reference is missing`). The lifetime callback returned, but this rejection prevents
complete current native qualification. Correct the same queued-request ownership boundary;
older passing Screen receipts retain only their exact builds and scopes.

**Retained Screen result:** Screen Warping cancels both native and DOM capture when document input
becomes unavailable. The real baseline retained capture across reconnect; the corrected app
passes overlap selection in both authored orders, offscreen and bounded off-page typed Inspector editing, captured
Escape/Undo, real reconnect without late writes, New during a held drag and exact Load recovery
on Intel and NVIDIA. Both ordinary app processes exit0, two native previews install per GPU,
and observed rejected requests/page errors are zero. The original headless contract, production
control browser suite, UI typecheck and default app build pass. The resumed full seven-stage
flow passes on both GPUs, including129 Surfaces with128 visible definitions and one independent
selected Surface/129 bindings. See the
[bounded receipt](evidence/mapgyver-m14-screen-selection-2026-11-17.json).
The current producer fingerprint is captured before launch; final pass receipts are written
only after normal app exit. Both cohorts use d9ddebcf… and exit0.

The earlier33483.7-second interrupted off-page run remains failed. Its exact snapshot stays in
stash `d0029450f8410976a11ed646fa203cd0eb0bbe05` as evidence; the named block is now integrated
and verified after waiting for restored editor ownership and persisted lock state. Do not
restore that stash automatically. Renderer admission stays128; this metadata/selection scope
does not qualify129-Surface rendering. Earlier
[Matrix lifetime acceptance](evidence/mapgyver-m14-matrix-selection-2026-11-16.json) remains valid.

The native desktop skill's21:28 availability check still returns native-pipe error2; no desktop
input was sent. Native previews install on the actual selected GPUs, but physical input and
independent pixel/hit alignment remain unverified. Next: correct Source reconnect history,
then its remaining lifetime and Composition/Matrix scope. Other unfinished
MapGyver inventory rows remain Pending; no fixed gate credit until all required scope passes.

**2026-11-16 user priority revision:** this gate selects only remaining MapGyver scope.
Noncritical Chataigne-only Curve, Spatializer and other consumer improvements are deferred
until Chataigne work resumes; see the [exact patch and findings handoff](chataigne-editor-followups.md).
Historical consumer results below remain evidence, not a queue for MapGyver work. M14/3–5,7
are Accepted and pushed; their scopes are not being reopened. Shared defects required by a
current MapGyver path remain actionable; unrelated Chataigne qualification does not block
MapGyver delivery. Earlier percentages and next-action notes below are historical, not current
execution instructions; this section and the main ledger own the return queue.

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

The stronger real-backend Screen test now checks Surface selection and background owner
inspection while the pointer is still down. Its original baseline instead expected the old
Escape behavior; the corrected owner-Escape assertion passes, but reselection immediately
afterward failed. Source also incorrectly admitted selection through the editor's mutation/read
busy flag. The correction gives `SurfaceHandles` explicit selection availability, fenced by
connection/loading and active gestures, while geometry still honors `disabled`. Surface bodies,
corners and keyboard selection remain available during metadata reads; background clearing uses
the same lifetime/gesture availability. The deterministic busy/unavailable-input fixture passes.
The rebuilt ordinary executable now passes this real-backend path, including peer/Inspector
selection before release, repeated owner Escape, unchanged authored state/history and drag Undo.
Physical native hits/pixels remain separately unaccepted.

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

The real-backend Composition continuation reproduced a refused reselection immediately after
background owner clearing. Its controls returned on mutation/read busy state before the shared
pointer could select; Matrix spatial controls had the same source pattern. Golden's pointer
start now requires a typed target with independent `selectable`/`editable` permissions, and both
consumers pass lifetime/tool availability separately from edit admission. A metadata read may
refuse editing without blocking selection. Busy Composition/Matrix DOM checks,16 viewport
unit cases,10 Composition controller cases and four typechecks pass. The rebuilt ordinary app
also passes the complete strengthened Composition integration suite. Matrix's real-backend
and physical acceptance remain separate; neither is inferred from these DOM/type checks.

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

Actual-domain acceptance now covers Alchemist and State Machine in the ordinary bundled
Chataigne workbench. `apps/chataigne/ui/tests/browser/graph-selection-product.mjs` uses public
intents to create real domain nodes, then normal panel and graph input. Its default mode tests
Alchemist; `<binary> --state-machine` tests State Machine with the same assertions. Both verify exact Inspector UUID before
release, retained DOM/capture, hover without selection/controls, background Formula/manager inspection,
tool-owned captured Escape followed by idle-owner Escape, and no late-input edits. Project Save
bytes and Undo/Redo lengths remain identical; no rejected operations/page errors occur and the
owned host exits normally. No production change was needed.

The initial Home button was covered by the Formula Properties overlay. The completed selection
scene uses the normal Hide properties action. Toolbar/inset overlap remains required work when
M14/2 navigation is active; this result does not accept that layout. Log:
`target/m14-graph-product-selection-2026-11-16.log`, SHA256
`efe9f3771dc0928d4e9609711772c6a7296df48a9963eddddcf6792ed90204bd`.
Receipt: `target/qualification/m14-graph-selection-product-262620ac-a58b-4daf-9fd5-f728ab2ec036/browser.jsonl`,
SHA256 `27336eb1d549a8d79a8aebf60f1bde3a5ed25a323221d4002fa7ea3bfb7bb710`.
The producer for those earlier checks is the `d2fd83b5` working-checkout Chataigne binary recorded below.
The shared contract's two-domain run also passes:

- `target/m14-state-selection-product-2026-11-16.log`, SHA256
  `0d336fb9f0152597672bf1cb1cf6c0c78b4126f2680367c658741c4769ef7a95`;
  receipt `target/qualification/m14-state-machine-selection-product-7e4d016f-b6a1-4e57-80e2-d040c5e866f2/browser.jsonl`.
- `target/m14-alchemist-selection-product-2026-11-16.log`, SHA256
  `1dcb61f59802f613ffea824245414a09c8351665a3ae8f60dbabf01f16d07a03`;
  receipt `target/qualification/m14-alchemist-selection-product-9f19b443-06e7-4ec1-90ab-bfc8b49e5615/browser.jsonl`.

Those two owned hosts exit0 with no page errors/rejections; saved documents/history remain exact.
Independent editors, offscreen/permissions, other graph-domain adapters and physical native scope
still require acceptance.

The independent-panel continuation found peer idle Escape did not clear selection after normal
release. Golden now prevents the native pointer default on owned body/title presses, retaining
canvas keyboard ownership through workbench group/Inspector updates; embedded controls are excluded.
That correction passed peer focus/Escape but exposed rapid State-title revisit entering rename
instead of drag capture. The new graph-host boundary clears pending title/resize activation on
foreign control/panel presses, keyboard input and focus/window loss. The first pointer/focus-only
correction passed cross-panel ownership, then failed immediate reselection after Escape; capturing
keyboard input completed that same sequence-lifetime rule. Both failures remain in their logs.

The complete paired contract now passes against the ordinary bundled app: independent State
Machine/Alchemist owners and shared Inspector, retained DOM/capture, tool-first and idle-owner
Escape, and rapid reselect. It pans the selected State offscreen, verifies permission-denied
rename submits no edit, performs a permitted Inspector rename and a typed Description Parameter
edit, and verifies one history item and exact saved-document restoration after each Undo.
The final true same-title double click enters rename; Escape cancels it without a write.

Run the retained extended contract with `<binary> --state-machine --independent-panels`;
ordinary Alchemist/State Machine modes preserve their original scope. The flag names the extended
acceptance contract and waives no requirement. Shared graph browser contracts,60 graph unit tests
and UI typechecks0/0 pass. Both ordinary default-feature apps build; syntax/Prettier and root/Core
formatting pass. The touched editing controller is normalized by repository Prettier; its only
semantic addition is the activation reset. Rust/protocol is unchanged. Other domain/edge/read-only
and physical native scopes still require acceptance; this closes no whole M14 gate.

Logs: `target/m14-graph-independent-product-2026-11-16.log` (baseline peer Escape failure),
`target/m14-graph-independent-product-fixed-2026-11-16.log` (later revisit/capture failure, SHA256
`8c597848002a497c41cabe6bda45470754a794c52b1105ef2148d8f15d35a03c`),
`target/m14-graph-focus-alchemist-product-2026-11-16.log` (pass),
`target/m14-graph-focus-browser-2026-11-16.log` (pass), and
`target/m14-graph-focus-product-build-2026-11-16.log` (exit0, SHA256
`b44b9111ac82e42db4f68f2370643f54405f7583b1993d14d7d6d2c961a50ca3`).
Paired correction receipt:
`target/qualification/m14-state-machine-selection-product-3075f2c8-7659-4a35-ac9d-62805285cd2c/browser.jsonl`.
Earlier focus-correction ordinary Chataigne SHA256:
`19b8f6da6985be632694e50902e5f72b8614fa1c9b884f02dfe77e5743a54e28`;
MapGyver SHA256: `5f0285b53e4a81e134531b36a32a3ba77d1c04c99926efe116928d419f4b4ab5`.
No new native/GPU qualification or complete fixed-gate acceptance is inferred.

Final extended product log: `target/m14-graph-sequence-parameter-product-2026-11-16.log`, SHA256
`c8de6f9c5e236c9f305039beb606dc577e22361967310e71dd3020a03224b549`.
Receipt: `target/qualification/m14-state-machine-independent-selection-product-1576e1b5-c629-496b-8fdb-e2abaf98b847/browser.jsonl`.
It exits0 with no page errors or rejected operations and normal owned-host shutdown.
The earlier pointer-only correction is retained in
`target/m14-graph-sequence-independent-product-2026-11-16.log`; the first complete pass before
adding the actual typed Parameter assertion is in `target/m14-graph-sequence-final-product-2026-11-16.log`.
Browser checks: `target/m14-graph-sequence-keyboard-browser-2026-11-16.log` (exit0);
unit checks: `target/m14-graph-sequence-unit-2026-11-16.log` (60 passed).
Final build: `target/m14-graph-sequence-keyboard-build-2026-11-16.log`, SHA256
`d833e7aea8bc444acf62fa911941b274c20f0d7fd0ca676dba4a59edf7bae299`.
Current ordinary Chataigne SHA256:
`a1cdbe7a33d2c3731c79b3a656479b5953bed7cfac84cdbb1d0a001b8915214b`;
MapGyver SHA256: `d94eb329a2e139a1aebc999b80c8276d0dc7ff1a66437bb0be7745d716a2be7f`.
These preserve unrelated working-tree edits outside this milestone. Native fixtures remain older;
refresh the required inputs before physical acceptance. No new native/GPU cohort ran.

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
The bounded editor/Canvas2D decomposition described below does not close the full acceptance gate.

Curve geometry now shares Golden's ordinary Inspector value-access rule. Read-only, disabled
or externally controlled typed Parameters remain selectable, while their complete geometry
group refuses capture and history admission. Current UUID/access checks stop queued writes
after permission loss; remove/duplicate actions honor document permissions. A key moved outside
the plot keeps its typed Inspector and can still be edited there. Replacing the Curve owner
resets retained camera bounds without recentering ordinary live Parameter updates. Constants,
value conversion, key-model projection and easing evaluation have been extracted into focused
modules; their existing behavior and every easing kind remain covered.

The Curve Canvas2D renderer is now a326-line DOM/lifecycle host over shared model/frame types.
Paint779, grid398, samples298 and theme199 lines form bounded modules. Painting returns both
bounds and hit geometry from one frame; the host owns sizing, theme observation, input forwarding
and publication. No new selection store, compatibility type alias or authored setting was added.
The editor/controller is separately decomposed below; neither split accepts actual-host behavior.

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
also refuses to join an existing edit session while command creation is pending. Actual-host
acceptance remains required after the subsequent controller decomposition.

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

The selection adapter now settles admitted local selection before projecting a graph refresh.
Previously a same-turn owner metadata update replayed the old workbench owner over the newly
selected key, before its outbound selection effect ran. The production Inspector regression
reproduces that failure before the correction. The corrected adapter publishes once against its
captured host/document/selection basis and checks current topology; a newer foreign selection
still wins. This is a verified ordering correction, not a fix for the connected capture failure.

Gesture/session ownership is now extracted into focused `curve/edit-gesture.svelte.ts` and
`curve/parameter-writer.ts` modules. Admission pins the host and document/selection/access basis;
one scheduled frame retains the latest key and Bezier samples. Terminal input takes its edit
session and current pending writes before closure. Creation/easing commands use the same typed
writer and current access/coercion rules. The editor retains geometry and presentation adapters.
The remaining controller is now split into13 focused editor modules: transient state/context,
model, selection, previews, constraints, insertion, sculpting, camera, hits, drawing, commands
and pointer dispatch. The component falls from4615 to687 lines; every new module is at most737.
Typed narrow factory contexts retain canonical document/parent bindings and avoid runtime import
cycles. The11 lifecycle effects remain in their original order at the component boundary.
Review of177 moved initializers found no behavior changes after accounting for context prefixes
and formatting. No new app behavior or whole gate is claimed by this structural cleanup.

Gradient is now included in the original “every 2D-like editor” inventory. This corrects an
omission without changing the ten fixed M14 gates or overall denominator. It shares Curve's
extracted `inspector/editor-selection` bridge and the public press/keyboard selection action.
Real stop/field selection keeps the Gradient owner mounted; background and idle Escape inspect
the owner, with no automatic first-stop selection. Hover changes only an unselected marker's
contour. The existing typed stop Inspector, color popup and interpolation tools remain usable.

The actual Chataigne product-host continuation reproduced same-stop reselection failure: after
selecting its Position Parameter, pressing that Stop left the workbench selecting Position.
Both objects project to the same local stop, so a reactive signature alone cannot express the
new exact input target. Golden's shared Inspector bridge now exposes an explicit `select` path
with the same host/document/snapshot/input fences. Gradient primary-down and keyboard actions
use it even when the projected stop is unchanged. Ordinary model projection remains non-authoring
and cannot acquire input authority. The deterministic browser regression fails before this
correction and passes afterward; the rebuilt actual product now passes the same scenario.

The two-panel continuation also exposed a sparse metadata reset defect. Expanding a Stop clears
its constructor's collapsed hint, but the sparse overlay discarded that default value and Load
reapplied the constructor hint. Metadata deltas now retain explicit default resets for tags,
semantics and presentation, including complete records matched to declared baselines. Full
snapshots keep their compact omission of empty/default fields. The complete actual product
flow now preserves the authored expanded state through Save/New/Load. Optimized regression
revalidation passes as recorded below; this is not whole-gate acceptance.

Gradient's captured input now distinguishes cancellation from release. Previously pointercancel
ran the release handler and could submit armed deletion. The stable editor owns capture while
markers reorder. Cancel/lost capture/blur, access or topology/selection loss, snapshot replacement
and disposal retire the gesture without deleting; only normal release can complete deletion.
Current host, document, owner/stop/Parameter UUIDs, ancestry and typed access fence writes.
Submitted writes settle in their pinned group; another gesture cannot join a pending closure.
Color-popup Escape belongs to its own editor. Captured double-click color opening and its first
grouped sample are verified; ColorPicker now notifies Begin before publishing that sample.

Golden's backend qualification exposed a real empty-list persistence defect: after deleting
all stops, Undo/Redo retained the empty list but project reload recreated the black/white defaults.
`GradientNode` now distinguishes fresh construction from project decoding; in-place authored-data
restoration keeps runtime initialization state unchanged. Both sparse encoders retain user-item
records even when their values match defaults, and retain a declared owner's record when its
default user collection was removed. Reconstruction inherits declared schema children while
replacing user items with the saved collection. Five public-facade regressions pass, including
standalone empty copying/reload, partial/empty declared reload and copying the containing owner.
The persisted format and ordinary declared-child identity policy stay unchanged. Core and
MapGyver suites, strict lint, ordinary build and formatting pass. Native acceptance is separate.

Source ownership also resolves the test host: no MapGyver-owned Gradient authoring path was found;
Chataigne's Alchemist constructs declared Gradient children. Backend checks therefore use Golden's
supported public project facade, including its duplication codec, rather than inventing a
MapGyver feature. The bundled Chataigne UI now also verifies actual Outliner/anchored Inspector
selection, capture/cancel, typed editing and live drag/history. Independent real-panel and native
acceptance remain separate from this verified single-panel product-host scope.

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
| Screen Warping / Screen | Actual owned WebView/Inspector on Intel/NVIDIA proves before-release Surface/background selection, overlap in both authored orders against accepted native hit metadata, independent corners/locked peers, offscreen and bounded off-page typed editing, captured Escape/Undo, reconnect capture retirement, held-drag New and exact Load. The complete seven-stage flow passes with128 visible definitions plus independently selected Surface/129 bindings. Zero observed rejected requests/page errors; two installed previews and normal exit0 per GPU. Browser admission-revocation regression also passes | Physical native input and independent image/control/hit agreement; renderer admission remains128 and larger rendering is not qualified |
| Surface Source / Surface | Original complete headless parameter/crop/Inspector/history and production DOM availability-withdrawal checks pass. Shared native capture-loss/focus/tool-first Escape corrections pass browser regressions; actual Intel confirms captured Escape and exact Undo with independent owners | Reconnect splits accepted samples into separate Undo steps: fix shared request/group ownership and retain the failing native regression. Complete unavailable-source peer, offscreen and New/Load stages on both GPUs, plus physical native image/hit/input agreement |
| Composition / Composition media | Real bundled UI/backend verifies before-release child/background selection and Inspector, no implicit first child, independent locked peers, owner/idle Escape, exact no-history selection,130-child off-page editing, reconnect/copy/removal/project replacement; shared selection/edit admission fixes busy reselection; hover and unavailable-bounds DOM checks pass | Physical native input/pixel agreement, overlapping hit order and remaining complete active-gesture/lifetime acceptance |
| Matrix Layer spatial / Matrix media | Actual MapGyver WebView on Intel/NVIDIA verifies before-release selection/Inspector, hover without handles, owner clearing and independent locked Layer/Cell scopes. Captured Escape, exact Undo, no late writes after reconnect, New during held Cell input and exact Load recovery pass with zero rejected requests; installed previews and actual GPU identity are observed | Physical native input/pixel/hit agreement, overlap and active/outgoing selection parity remain open; earlier off-page typed Inspector evidence retains its tested scope |
| Matrix Cell spatial / Layer | The same both-GPU actual-app flow includes disabled-Cell selection, Layer-owner clearing without navigation, recognized Matrix focus and unrelated locked-content retention. Capture cancels before idle-owner Escape; real reconnect and project replacement retire input while history/playback remain exact | Physical native input/pixel/hit agreement, overlap, off-page spatial and active/outgoing selection parity remain open |
| Golden graph canvases | Reusable input/cancel contract and actual Alchemist/State Machine Inspector selection before release pass. Actual paired panels retain independent owners/focus/capture; offscreen State Inspector name and typed Description edits, name permission, exact Undo/save restoration and genuine double-click rename pass | Other domain/edge/read-only and physical/native agreement, specialized custom-world capture acceptance; Alchemist toolbar/Properties overlap returns with M14/2 |
| Golden Dashboard / edited page | Primary-down widget/background selection; selected-only resize zones; border hover; per-page tool-first/idle-owner Escape; free/ordered move/resize and marquee/pan cancellation, nested keyboard and two-editor isolation verified in production browser fixture | Actual workbench/Inspector, permission/offscreen/project and current-host acceptance remain open; unrelated Dashboard backend edits are preserved |
| Animation curves / curve and key nodes | Shared key/easing/field and embedded typed Inspector; background/idle owner, tool-first cancel, selected-only Bezier/contour hover, permission/offscreen/locked-owner checks; bounded editor/renderer; delayed draw/create/paste owner/document fencing, exact creation receipt and original-owner completion without selection theft | Fix connected pointer ownership and complete actual backend/history/native and full lifetime acceptance; all editor/renderer modules stay under1000 lines |
| Gradient / Gradient owner and stops | Actual bundled Chataigne UI/backend proves before-release Outliner/Inspector selection, capture/cancel, typed/live edits and exact Undo/Redo; two locked real panels preserve independent owners/focus and local color Escape, retire capture on reconnect, reload locks, fence New during a live drag and restore exact authored values/presentation; optimized Engine/MapGyver/Gradient suites pass | Physical native input/pixels and remaining complete offscreen/copy/lifetime acceptance stay open. Rematerialized declared UUIDs leave locks unavailable rather than retargeting |
| Watcher projection canvases | `WatcherProjectionCanvas` renders buffer samples/latest points without pointer, key or mutation callbacks; it has no authored canvas targets | Read-only projection applicability resolved from source. Ordinary Inspector owns its settings; retain panel focus/lifetime checks in actual workbench acceptance |
| Media Preview / followed or locked media | `MediaPreviewPanel` renders `ChannelPreview`/`NativePreview` without authored geometry targets; toolbar changes preview choice, not object placement | Read-only image applicability resolved from source. Existing follow/lock/routing requirements stay intact; actual cross-panel/native qualification remains open |
| Matrix grid / Matrix, Layer, Column and Cell | Real bundled UI/backend proves primary-down Cell/Layer/Column/Group selection and exact Inspector identity before release, owner/text-tool Escape, unchanged history/playback, offscreen Cell editing, two locked independent grids with visible Inspector, layout reload, New during held input and exact Load recovery; complete contextual CRUD/rename/structural history passes | Physical native input/drag/pixels and complete active-drag/reconnect/lifetime acceptance remain open; tested project scope is New during a held selection and exact Load recovery |

This inventory retains the complete gate. DOM callback fixtures do not establish actual
Inspector transactions, native pixels, GPU parity or all editor selection policies.

## Validation

Gradient independent product panels / metadata resets (2026-11-16): the extended bundled
Chataigne flow passes with0 page errors/rejected requests and normal owned-host exit0. It retains
every single-panel check and adds two visible locked real Inspectors, exact Outliner selection
before release, independent retained Stop focus and local color-popup Escape. Closing the live
workbench socket retires held capture; moving after reconnect cannot author stale geometry.
Layout reload restores each lock. New during an accepted drag withdraws both owners while the
pointer remains held, and Load restores exact authored values, Stop identities, ordering and
expanded/collapsed state with clean history. Locks resolve only their document/UUID; a declared
Gradient rematerialized under a new UUID stays unavailable instead of binding positionally.
Projected type/declaration help text is excluded from authored-state comparison; this fixture
does not author descriptions. Presentation remains asserted.

The first product run retained all earlier checks and failed only the expanded Stop's Load
comparison (`target/m14-gradient-product-lifetime-2026-11-16.log`). The first metadata correction
passed692 MapGyver tests and698 Engine tests (including three new regressions), but failed the
existing compact full-snapshot serialization assertion. The refined correction preserves that
original assertion and encodes explicit resets in sparse overrides. Strict Engine library/test
Clippy exits0 with `-D warnings`, both ordinary app builds and root/Core fmt pass. Three new
regressions cover full-record decoding, sparse reset overlays and live/immutable Gradient saves.
The complete optimized rerun passes699 Engine tests (3 ignored),692 MapGyver tests (5 ignored)
and5 Gradient integration tests, including all three new regressions and the unchanged compact
serialization test. Terminal session51332 exits0. Command:
`cargo test --locked -p golden_engine -p MapGyver --lib --test gradient_editor --no-fail-fast`.
Log: `target/m14-gradient-metadata-software-complete-2026-11-16.log`, SHA256
`ce7ded2c0feb2acd5e8dd22132e25ee4a6a93db88385621757ca1084e371e6da`.
The prior run stopped at an intermittent Video stepping fixture rejection (timestamps not ready)
before executing Engine/Gradient tests; retain it in
`target/m14-gradient-metadata-software-final-2026-11-16.log`. The rerun changes only Cargo's
continue-after-failure policy, preserving profile, tests, concurrency and budgets. Follow up the
fixture's prepared-frame synchronization after Speed before treating its reliability as resolved.

The passing product receipt is
`target/qualification/m14-gradient-product-29133495-1eee-4718-b1b7-d88d2c17ef0f/browser.jsonl`.
Passing controller log: `target/m14-gradient-product-lifetime-fixed-2026-11-16.log`, SHA256
`f382314e7c5d5a399e68c81f4277ba0de8549d947691bc1fd669a8238f4b7d18`.
Strict log: `target/m14-gradient-metadata-clippy-2026-11-16.log`; the initial full-suite failure
remains in `target/m14-gradient-metadata-software-2026-11-16.log`. No native GPU/input/pixel or
stopped Curve probe was run, and M14/1 remains unaccepted.

Gradient product host and reselection (2026-11-16): the ordinary Chataigne build and its
default-feature `hosted_gradient_stop_edit_changes_materialized_gradient_resource` test pass.
The first test build failed because inherited `CPAL_ASIO_DIR` pointed to an incomplete Temp SDK;
the existing bootstrap selected its already-cached pinned SDK (`496a0765b8bb9c26f764f22f9a9712a937177db2`)
for the command only. No feature was removed or machine environment persisted. The existing
owned-workbench helper now supports the app's bundled UI as well as production fixtures and
checks normal shutdown for that bundled mode; its existing Spatializer consumer also passes.

The actual declared Alchemist Gradient reaches the ordinary Outliner/Inspector, preserves
selection-only state/history and capture before release, rejects foreign cancellation and cancels
armed removal without deleting. Real Parameter selection keeps the Gradient owner mounted;
typed edits and an accepted live drag each have exact single-step Undo/Redo. Active Escape
preserves accepted edits/selection and fences held motion; idle Escape selects the owner.
Opening the Stop folder authors its normal presentation metadata, so numeric history is measured
after that explicit folder action. Current rendered position and loading completion are awaited
after history replacement before pressing the next handle. Initial fixture corrections covered
sparse leaf children, normal folder expansion and history baselines; the product's unchanged
same-stop failure is retained in `target/m14-gradient-product-ui-history-2026-11-16.log`.

`target/m14-gradient-reselection-baseline-2026-11-16.log` reproduces the same missed selection in
the existing production Gradient browser fixture. The fixed complete Curve/Gradient suite passes,
including primary-down and keyboard reselection with no writes/history; all three Curve paint
hashes remain unchanged. Six selection unit cases and Chataigne/MapGyver/Golden browser
typechecks (0 errors/0 warnings) pass, as do scoped Prettier/syntax, root/Core fmt and both
ordinary app/UI builds. No Rust/DTO behavior changed, stopped Curve probe or native GPU run occurred.

Passing actual-product receipt:
`target/qualification/m14-gradient-product-4a89d922-b691-4469-b262-359e12a79b20/browser.jsonl`;
its controller exits0 with0 page errors/rejected requests and normal owned-host shutdown.
Logs: `target/m14-gradient-reselection-product-final-2026-11-16.log` (SHA256
`d61ab3c8648c2ed8cac8c00f39483bc92a5cb892fc84187ed72a94e3d7b50525`),
`target/m14-gradient-reselection-fixed-2026-11-16.log` and
`target/m14-gradient-reselection-existing-fixture-final-2026-11-16.log`.
No headless input/pixel result substitutes for physical native qualification; M14/1 remains open.

Matrix grid (2026-11-16): `--matrix-grid-ui` passes against the unchanged ordinary app,
with0 page errors/rejected requests and normal host exit0. Actual Inspector UUIDs and selected
grid controls agree before pointer release for empty addresses (their existing Layer owner),
authored Cells, Layer/Column headers and Group labels. Hover leaves owner inspection unchanged;
background-down and idle Escape inspect the Matrix owner and clear every grid highlight.
Keyboard Cell selection and Escape from a focused Cell work; Escape within inline Group rename
discards the draft while retaining Group selection. Selection-only snapshots and complete
history remain exact, and no playback column is requested. All existing owned-media creation/
removal, complete structural Undo/Redo and offscreen Cell editing checks remain and pass;
128 added Layers keep fewer than30 row headers mounted while the original Cell Inspector stays
bound and accepts an edit.

The baseline's fixed top-left populated-Cell hit landed on Play. Corrected fixture body hits
use current observed Cell bounds, since row height changes with added content; no force clicks,
fabricated selection or bypassed handlers are used. An initial new assertion incorrectly assumed
empty grid addresses already had authored Cell nodes; it now verifies the existing backend Layer
fallback separately from real Cell creation. Failed logs remain
`target/m14-matrix-grid-{baseline,selection,selection-final,selection-final-hits}-2026-11-16.log`.
Passing log: `target/m14-matrix-grid-selection-current-bounds-2026-11-16.log`, SHA256
`a03d31081ba1bbcce1e6208bf1f7438d71b0d9994e42ced9570c568d90ab9e74`.
The complete grid continuation also passes two simultaneously visible real Matrix grids plus
the ordinary Inspector. A Layer press selects its exact Inspector UUID before release while
the other locked grid retains its content and selection; idle Escape clears only the active
Matrix's focus. Full authoring/history snapshots stay unchanged. Layout reload restores both
locked targets and their independent selections. New during a held row press withdraws both
targets and clears Inspector selection, and releasing cannot reselect the old node. Load restores
both exact authored snapshots, no recovery warnings, zero Undo/Redo, no active edit and no
requested playback column. This is held-selection fencing, not native HTML drag qualification.

The helper waits for persisted panel state and activates restored tabs before querying their
mounted bodies. Row selection deliberately retains the existing column coordinate; the fixture
verifies that current contract. Its initial Load/history checks were corrected to the public DTOs
(resolved path/recovery and a boolean active-edit flag). Earlier failed logs are retained as
`target/m14-matrix-grid-lifetime{,-final,-persisted,-targets}-2026-11-16.log`.
Complete passing log: `target/m14-matrix-grid-lifetime-complete-2026-11-16.log`, SHA256
`3f48270a2550623a7f428982e3e4910f5a8cb7dd2b61666ae57d28fe06bc350a`.
Scoped Prettier/syntax and root/Core formatting pass. These fixture-only slices establish no
new native input/pixel acceptance, complete active-drag or reconnect qualification.

Surface Source (2026-11-16): the unchanged `--surface-source-ui` baseline passes, and its
extended real-backend fixture passes with0 page errors/rejected crop requests and normal
owned-host exit0. Both Origin and Size select their actual typed Parameter and update the
generic Inspector while the pointer is held. Foreign pointer cancellation preserves capture;
matching pointercancel, capture release and dispatched window blur stop held input, preserve
selection and leave the full Screen snapshot/history unchanged for a no-movement gesture.
These software boundary checks do not prove native OS event delivery. Active Escape retains
Crop selection and fences subsequent held motion while preserving the already accepted edit.
Two simultaneously visible locked Source editors retain different Surface targets, share
Origin/Size selection with the visible Inspector and clear to the correct owner without
authoring history. Existing four-corner/origin/clamping, complete Undo/Redo, live Inspector
feedback, source loss/Undo and panel-switching checks all remain and pass.

Logs: `target/m14-surface-source-baseline-2026-11-16.log` and
`target/m14-surface-source-selection-2026-11-16.log`; extended log SHA256
`a3c227f3e3f0b865048526129a777d2081fe7cce278af5d3b7919553803e0ddb`.
The unchanged ordinary executable is the Matrix readiness build identified below. This slice
changes only the integration fixture; no runtime, DTO or native GPU cohort changed. Scoped
Prettier/syntax and root/Core formatting checks pass. M14/1 remains unaccepted.

Matrix Layer/Cell (2026-11-16): `--matrix-layout-selection-ui --matrix-parameters-ui` passes
together on the rebuilt ordinary app, with0 page errors/rejected requests and normal exit
(`target/m14-matrix-selection-combined-final-2026-11-16.log`). The focused spatial flow checks
actual selected rectangles/handles and generic Inspector Position before release, hover without
selection, background and repeated idle Escape to the correct owner, disabled Cell selection,
two visible locked editors with a visible Inspector, recognized Matrix focus clearing and
unrelated-selection retention. Complete Matrix authoring/history stays exact and playback
remains unlaunched. Matrix panels now include local geometry closure in `aria-busy`; a backend
End acknowledgement alone does not establish frontend idle. Target binding and new-tab identity
are awaited explicitly. The original Inspector suite retains every per-field Undo/Redo,
off-page Layer/Cell, optional Size, owned Color and mixed-selection assertion.

Fixture corrections preserve actual UI boundaries: read back Inspector values and await the
loading overlay after history replacement, use an observed lower Cell hit away from controls/
sticky headers, and search for virtualized mixed-selection targets. Earlier failures remain in
`target/m14-matrix-selection-{baseline,readiness,baseline-fixed}-2026-11-16.log`; corrected
Inspector proof is `target/m14-matrix-selection-parameters-2026-11-16.log`. The initial spatial
failure and busy-state correction are retained in `target/m14-matrix-layout-selection*-2026-11-16.log`.
MapGyver typecheck0 errors/0 warnings, production ordinary/UI build, scoped Prettier and root/
Core formatting/check pass. No Rust/protocol behavior changed or physical native cohort ran.
Full M14/1 remains unaccepted; these close the listed Matrix software/real-backend checks only.

Shared spatial admission / Composition (2026-11-16): the unchanged real-backend baseline reached
copy after off-page Inspector, independent locks/layout and reconnect, then failed its obsolete
automatic-first-child assumption (`target/m14-composition-selection-baseline-2026-11-16.log`).
The first held-input assertion incorrectly waited for a deferred backend summary; the correction
checks actual control identity/Inspector while held and requires backend agreement after release.
That corrected test reproduced a swallowed second selection during the owner read
(`target/m14-composition-selection-controls-2026-11-16.log`), with the Inspector still on Panel B.
The new explicit shared selection/edit permissions fix that source defect.

`target/m14-composition-selection-final-2026-11-16.log` exits0, with0 page errors/rejected requests
and normal host exit. It verifies actual control/Inspector selection and background owner before
release, no implicit first child, independent locked peers, idle Escape/idempotence, exact authored
state/history preservation and retained peer control identity,130 children with128-item pages and off-page Inspector editing,
layout reload, real reconnect, copy/removal/Undo/Redo, New during held input and same-document
Load. The fixture explicitly selects a child for Copy/drag, preserving M14's owner/no-child rule.

Supporting checks pass:16 viewport unit cases,10 Composition controller cases, production DOM
Screen/Source/Composition/Matrix selection suite with spatial metadata busy, and typechecks0/0
for MapGyver, Chataigne, Golden UI browser fixtures and Golden Video UI. Logs:
`target/m14-spatial-admission-{pointer,controller,browser,mapgyver-types,chataigne-types,golden-types,video-types}-2026-11-16.log`.
Ordinary build rebuilds its production UI; scoped Prettier and root/Core formatting/check pass.
No Rust/protocol behavior changed; preceding Rust validation retains its unchanged source scope.
Matrix real-backend/physical and all remaining native inventory stay Pending. No new desktop
connection retry, GPU/performance cohort or Curve probe occurred. M14/1 remains unaccepted.

Screen selection continuation (2026-11-16): the original headless integration baseline failed
its obsolete Surface-after-Escape expectation; the strengthened version passed held Surface/
background selection and owner Escape, then failed reselection. Logs:
`target/m14-screen-selection-baseline-2026-11-16.log`,
`target/m14-screen-selection-2026-11-16.log`. No extra probe variant was run. The source-supported
admission correction passes the production DOM fixture, including busy primary-down/corner
keyboard selection, refused geometry and unavailable-input rejection;17 controller tests;
MapGyver typecheck0 errors/0 warnings; scoped Prettier; production UI build; root/Core formatting.
Logs: `target/m14-screen-busy-selection-2026-11-16.log`,
`target/m14-screen-selection-controller-2026-11-16.log`,
`target/m14-screen-selection-types-2026-11-16.log`,
`target/m14-screen-selection-ui-build-2026-11-16.log`.

The user saved/closed the dev window to release its FFmpeg DLLs; the ordinary rebuild then
passed. Final `--screen-selection-ui --screen-keyboard-ui` exits0 with no page errors or rejected
requests and normal host exit (`target/m14-screen-selection-final-2026-11-16.log`). Selection
proves Surface/background/peer/Inspector agreement before release, owner Escape/idempotence,
no authored/history changes, one drag Undo and exact geometry restoration. Keyboard verifies
100 repeats plus Shift at zoom, chord release, exact one-step Undo, blur/modifier isolation
and New while held. Its old zero-Screen New assertion was corrected to the accepted M13/5
policy: exactly one fresh default Screen/Surface whose complete snapshot stays unchanged after
old held input. The first combined run retains that obsolete assertion failure in
`target/m14-screen-selection-connected-2026-11-16.log`; final full suites pass.

Computer Use still returns native-pipe error2 after documented retry/reset; no desktop input
was sent. Physical native handle/hit/pixel acceptance returns when the helper reconnects,
using coherent ordinary/native inputs where consumed. This is software/real-backend acceptance,
not a physical native cohort or whole M14/1 acceptance.

Golden backend slice (2026-11-16): the public project facade verifies exact initialized creation,
live typed values before release, one history step, exact Undo/Redo, copied stop values with new
UUIDs, removal, rejected-initialization rollback, standalone empty subtree copy/reload and
runtime initialization preservation. Targeted result is **5 passed/0 failed**:
`target/m14-gradient-collection-regression-2026-11-16.log`. Partial and empty declared collections
survive reload, the remaining stop keeps its UUID, and copying the containing owner preserves
its empty Gradient. The declared child is resolved through its owning UUID plus `gradient`
declaration, respecting intentional rematerialization of unreferenced default child identities.
The earlier four-pass/one-fail value regression is retained in
`target/m14-gradient-backend-declared-path-2026-11-15.log`; its empty-value assertion now passes.

Source-frozen validation after both root/Core formatters passes:

- Full Golden Engine library:696 passed,3 existing ignored; integration:5 passed
  (`target/m14-gradient-core-2026-11-16.log`). This includes persistence, Gradient and the
  ten-thousand-node sparse roundtrip.
- MapGyver library:692 passed,5 existing ignored
  (`target/m14-gradient-mapgyver-2026-11-16.log`).
- Strict Clippy for Golden Engine library and `gradient_editor`:exit0 with `-D warnings`
  (`target/m14-gradient-clippy-2026-11-16.log`).
- Ordinary `cargo build --locked -p MapGyver --bin MapGyver`:exit0
  (`target/m14-gradient-build-mapgyver-2026-11-16.log`); root/Core formatting and final
  `--check` both pass. It waited behind the concurrent user dev run without stopping it.

No new connected/native cohort was run. Earlier passing
five-case Gradient/eighteen-case persistence checks are superseded by the full Core run.
The earlier standalone empty-list baseline is retained at
`target/m14-gradient-backend-complete-2026-11-15.log`; corrected standalone proof is in the fixed/
final-copy logs. Source ownership requires the public project facade for duplication rather than
the raw engine's deliberate `duplicate_node_transport_required` rejection.

This persistence milestone closes the reproduced backend defect; it does not accept a fixed
M14 gate. Investigation began2026-11-15 22:25UTC with a23:25 cutoff; the source correction was
made before cutoff. The interrupted overnight check had no terminal result or remaining process;
the source-frozen validation rerun passes. No additional probe variants or investigation scope
were opened. M14/1 Active, M14 0/10,88/106 (83%).

Gradient selection/cancellation (2026-11-15):66 focused unit cases pass, including Gradient
ownership, typed-field projection, foreign-target rejection and topology/cache scope. Four
typechecks report0 errors/0 warnings. The combined production Inspector browser passes with0
page errors and all three existing Curve paint hashes unchanged. Gradient checks cover selection
before pointer-up, stable capture, foreign pointer rejection, Enter/idle Escape, unselected hover,
background owner, typed-field focus, pointercancel/Escape/lost capture/blur/access/snapshot/disposal
without armed deletion, normal-release deletion intent, pending Begin/closure admission, disabled
Parameter selection, two locked editors, color opening/first grouped sample and editor-local Escape.
Its document/disposal controls are invoked out of band during capture, avoiding an accidental
normal pointer release; snapshot generation is explicitly checked. Locked identities come from
the current fixture graph. These corrected fixture assumptions are not production workarounds.
Logs: `target/m14-gradient-*-2026-11-15.log`. Actual backend/history/copy/reload now pass through
the public project facade above; product-host/native acceptance remains Pending. This browser
transport fixture alone does not establish actual deletion/history.
Scoped Prettier/app lint, both production UI builds and root/Core formatting pass. Gradient's
component is858 lines; the shared selection bridge, projections and new fixtures remain under1000.
No native/GPU cohort or new Curve connected variant was run; its prior capture failure remains
in the return queue. M14/1 stays Active, M14 0/10,88/106 (83%).

Selection ordering correction (2026-11-15): the production Inspector guard first fails on
same-turn graph refresh/key selection, then passes after the adapter correction, including a
newer foreign-selection case. All existing browser behavior and frame hashes remain unchanged,
with0 page errors.64 focused unit cases, four typechecks0/0, scoped formatting/app lint, both UI
builds and root/Core formatting pass. Logs: `target/m14-curve-selection-race-*-2026-11-15.log`;
the intentional failing baseline is retained separately.
The rebuilt ordinary app's unchanged connected regression still fails `captured:false, focused:true`
after exact creation, one history step and typed Inspector pass. Receipt:
`target/m14-curve-selection-race-connected-2026-11-15/failure.json`; host shutdown is normal.
Later connected movement/Escape/history remain unexecuted. No new connected scene, native/GPU
cohort, Rust/protocol change or cache-stash restoration occurred. M14/1 remains Active/unaccepted,
M14 0/10,88/106 (83%).

Full controller decomposition (2026-11-15):64 focused cases, four typechecks0/0, scoped formatting,
app lint, both production UI builds and root/Core formatting pass. Production Inspector browser
reports0 page errors and retains all three620×330 frame hashes, captured input, metadata refresh,
typed access, offscreen/locked Inspector and pending-command cases. A read-only TypeScript review
compares177 moved function/derived initializers and11 effects againstc5a77a18; only context
qualification and formatting differ. The command/menu module uses `.svelte.ts` for its derived
menu state; the first browser attempt caught the incorrect plain-TS extension before correction.
Logs: `target/m14-curve-editor-*-2026-11-15.log`.

The coherent ordinary rebuild's unchanged connected regression still fails before movement:
`captured:false, focused:true, activeTag:CANVAS`. Exact created identity, initialized typed Inspector
and one history item pass; later drag/Escape/history cases remain unexecuted. Receipt:
`target/m14-curve-editor-connected-2026-11-15/failure.json`; normal owned-host shutdown is retained.
No new runtime probe, native/GPU cohort, Rust or protocol change was introduced. Source readability
debt is closed for these editor/renderer files; M14/1 and the full actual-host/native inventory
remain unaccepted. M14 0/10,88/106 (83%).

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
focused Curve input/controller decomposition under the same M14/1 gate was completed next.
Preserve this exact failing acceptance check and revisit ownership when the extracted source
provides a justified fix. The parked cache attempt remains untouched; no gate is accepted by this check.

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
capability or whole-gate acceptance. Actual backend/history/native
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
or every lifetime case. The subsequent bounded controller/renderer split does not expand this scope.
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
Prior ordinary build with extracted gesture/session ownership:
`da152e1196b46d896672520433e62ce78752b83a286efceeb54f68cd5ca2bcfc`;
build log SHA256 `579e35ba8dae93d7ca82fdf8e050ad54695206cac9d8f45927bd1041009c4604`
(`target/m14-curve-gesture-build-mapgyver-2026-11-15.log`). Its connected result is the failing
gesture receipt above.
Prior ordinary build with the complete controller decomposition:
`aea3e6ecc8f360f1a9dc4b418a33062378f001f75442902a8ec11f6c39649dc3`;
build log SHA256 `693d5fddb8afd64c84cc86e97cd1bf6b49c4f01804c94994fe6932e0a56f7f18`
(`target/m14-curve-editor-build-mapgyver-2026-11-15.log`). Its connected result is the failing
editor receipt above.
Prior ordinary build with the selection ordering correction:
`75e836dbde6685286bf4b563b388f1600f834598864d2082934f6172b5ca9319`;
build log SHA256 `7dc53aef88f3e99174792e0696c9d4a84acea7b5d0b9fde6756882a2cede5f5a`
(`target/m14-curve-selection-race-build-mapgyver-2026-11-15.log`). Its connected result is the failing
selection-race receipt above.
Prior ordinary build with Gradient/shared-bridge input:
`25988ac43752937acaa9f653f13b3aeb58a23360fd11f5571632c3d1068aa106`;
build log SHA256 `3311cf24845b0be8996aaa2e0cbd2f1562be74e593ffdea9f914066b9b3b4234`
(`target/m14-gradient-build-mapgyver-2026-11-15.log`). The connected Curve failure above retains
its earlier binary scope; no new connected/native qualification is inferred from this rebuild.
Prior ordinary build with Gradient collection persistence (2026-11-16):
`ae6743e0812007b00236b892c282d36a0216915cabd25d49d68bf3147e564d23`;
build log SHA256 `db145a48fcc5448a2bff1045747c0f5c56dc108665e25985469df025f14cbb56`
(`target/m14-gradient-build-mapgyver-2026-11-16.log`). It was built from the working checkout,
including preserved unrelated user edits; the milestone stages only its owned files.
Prior ordinary build with Screen read/selection admission (2026-11-16):
`513c33b92b1f566651122eeeefcde5e4f54866107af4003e1ada5eac886b57ad`;
build log SHA256 `e9d4f5ed163c810d189323d5823c456b6ffd28283798a4f7b462ae1d3f9fbb44`;
final integration log SHA256 `4fe58297865eacf794517fe35bca3d4d6e0ea4fc05fd6a44bf45a3f0936c86b4`.
Logs: `target/m14-screen-selection-build-2026-11-16.log`,
`target/m14-screen-selection-final-2026-11-16.log`.
Prior ordinary build with shared spatial admission (2026-11-16):
`92bc43c8f19189e95e652680e07ad988be5b026fc5cbfdac912a3731e98d535f`;
build log SHA256 `2cdf44b53e23e80081fec1b13168eec9ee9425e287657bedbaa8b5e62e0e72f9`;
integration log SHA256 `ce48db27f0d3aa00864b558c58dc4645d128b5479603f1f40e98743df68c08a0`.
Logs: `target/m14-spatial-admission-build-2026-11-16.log`,
`target/m14-composition-selection-final-2026-11-16.log`.
Prior ordinary build with Matrix interaction readiness (2026-11-16):
`f92d8c669074887e4fe6ce34669e953e2d5be84b14c03f9bd642b09cd8d69229`;
build log SHA256 `9907163eb7e083305cc8702f4add228329847bea90d3cd0e7a2e9669fe861f8b`;
combined integration log SHA256 `bac81f8dee5c0d81d785aea31604515c59d5b52e5c9f747d5c2afa11746cf7fe`.
Logs: `target/m14-matrix-layout-build-2026-11-16.log`,
`target/m14-matrix-selection-combined-final-2026-11-16.log`.
Prior ordinary builds with explicit Inspector selection (2026-11-16):
MapGyver SHA256 `ac42d85c8c28e74b24a7ae3a4f8f8314e979aa094def351283afb2cc360d97cc`;
Chataigne SHA256 `ec22e0f8186d3730031d570b41233572dd04510a63cd9ba758f3494df4b3d88c`.
Both default packages/bins were built together from the preserved working checkout; unrelated
edits are not included in this milestone's staged changes. Build log:
`target/m14-gradient-reselection-product-build-2026-11-16.log`, SHA256
`8dbd2c39cd2a703971cbeaa9e8ca8a2127363696a110bc060a37a242d24b2576`.
Previous paired ordinary builds with sparse metadata reset preservation (2026-11-16):
MapGyver SHA256 `21157ab5d425187eec36af7ff2abc07320bcba043e37efe0d840826327e2f643`;
Chataigne SHA256 `d756d86293e04e72a1e0482c206cd87a468e71b30cbbbbfa260e773ee687c135`.
Build log: `target/m14-gradient-metadata-product-build-2026-11-16.log`, SHA256
`3719e79ebe9f9cdb50e27c2ae725455b69f7d580a9bfae21b6c8c246252eb5e0`.
These working-checkout builds retain unrelated user edits outside the staged milestone.
The native fixture remains older: these are not a coherent new ordinary/
native qualification pair. Refresh both inputs before native acceptance; Video evidence keeps
its original source/build scope.

### Video boundary Step regression prerequisite

An already-paused first/last-frame Step now returns before queuing Pause and Seek Parameter
writes. The control decides this from prepared timestamps and the actual transport position,
independently of delayed engine position feedback. The new regression holds feedback pending,
verifies an unchanged epoch, then steps immediately in the opposite direction with no extra
readiness wait or authored history. Prepared-frame unit coverage also retains paused Speed,
BOF, File, epoch and source-incarnation fences. The original four-codec mixed-rate/pixel/history
test is unchanged and passes in both post-change full runs. This establishes a boundary defect;
the earlier intermittent receipt lacked the operation stage and is not proven to have this as
its only cause. Keep that receipt; return on a stage-identified reproduction, not another
unchanged probe.

The first added fixture timed out waiting for a metadata host it never started. The corrected
fixture uses existing prepared-control timing injection. Its full optimized default-feature
MapGyver run exits101:692 passed,1 failed,5 ignored. The new regression passes; the failure is
`hosted_video_methods_commit_once_and_report_actual_readiness_and_rejection`, rejected by the
existing process-wide script app-operation reservation pool. That test passes individually
without source/budget changes. Global-pool contention is plausible from source, not established
as the only cause. Deterministic full-suite scripting admission remains in the software return
queue before final qualification. Neither run is described as an entirely green full suite.

Logs and SHA256:

- Corrected full run: `target/m14-video-boundary-software-fixed-2026-11-16.log`,
  `c20c223140e18088a47f8f15c93311b756a3f7ad4f89d5982234872a20949c98`.
- Strict MapGyver lib/tests Clippy, exit0: `target/m14-video-boundary-clippy-2026-11-16.log`,
  `177725e26f57fa00f2e5a9d70b5fe7638557b62a86b19c7391a855b31138e59c`.
- Exact scripting recheck,1 passed: `target/m14-video-boundary-script-recheck-2026-11-16.log`,
  `2b86b847a26d678c327e43a6eea20cb8de8699c73a5266c75d01112ed3b3f9f0`.
- Ordinary default-feature MapGyver build, exit0:
  `target/m14-video-boundary-product-build-2026-11-16.log`,
  `f4ddf38dcb88ea73e8f25bc559aba82f652a10838bf9b1bf2f5896b743d4add4`.

Boundary-Step ordinary MapGyver SHA256 (superseded by the Curve UI build below):
`6d3f0f9292eba04e6b425a8f89439014c3959cf219404033f4e69e103f069ad9`.
Root/Core formatting passes. Core and Gradient are unchanged from their prior699/5 passing
suites. No protocol/UI change, native/GPU check, budget change or fixed-gate acceptance is
claimed. The native fixture remains older and needs a coherent rebuild before native acceptance.

Curve exact-selection slice (2026-11-16): the production Inspector baseline fails when Value is
selected and the already-projected key is pressed: Value remains selected. Curve now publishes
complete selection through the shared bridge's explicit input method before gesture admission.
Pointer/context-menu, marquee completion and Select All use the typed callback; passive cleanup
and asynchronous commands retain guarded projection. The corrected regression selects the key
before release with the same canvas and zero Parameter writes. Full Curve/Gradient browser
coverage, all three unchanged paint hashes,37 focused unit tests and four UI typechecks0/0 pass.
Both ordinary default-feature apps build successfully; root/Core formatting passes.

Logs: `target/m14-curve-exact-selection-baseline-2026-11-16.log` (expected failure),
`target/m14-curve-exact-selection-fixed-2026-11-16.log` (exit0, SHA256
`15878e22d44b1d21216eb0453ac84e45028a38b1c3ceb98be38bd1cd28361ae4`),
`target/m14-curve-exact-selection-unit-2026-11-16.log` (37 passed),
`target/m14-curve-exact-selection-*-check-2026-11-16.log` (four checks0/0),
and `target/m14-curve-exact-selection-product-build-2026-11-16.log` (exit0, SHA256
`27e5045235042570d804fd11ddbe57653d4e1a304bff7dd59a081ae5bc10d35a`).
Curve exact-selection ordinary MapGyver SHA256 (before graph focus correction):
`a84b77774f0770ba3e0519f110e96b0d601e339303d528c04d20d43d9637481e`;
Chataigne SHA256: `f7f4f9bd57c2bb0f2713ebb29f73d2056e54ba8c3e32b4fb3e2031ca4f6e4aa5`.
The separate connected capture failure was not rerun and is not claimed fixed. No native/GPU
check or acceptance-budget change occurred. Finish this verified slice; stop extending Curve
investigation. Return only for a correction supported by its actual terminal boundary or required
native acceptance after helper reconnection. Its original connected/history/native requirements
remain Pending; no fixed gate closes.

Next MapGyver action: remaining Surface Source offscreen/current capture/reconnect/project scope.
Screen's complete seven-stage and Matrix's six-stage lifetime checks pass on both GPUs, including connection ownership
and native/DOM capture retirement. Physical native input/pixels/hits, remaining editor overlap,
off-page and active/outgoing scopes retain their exact requirements. The supported desktop
connection's last check reports native-pipe error2; no helper repair is queued.
The earlier Matrix grid, Surface Source and Composition backend evidence retains only its
tested scope; the complete applicable editor inventory remains required. M14/1 stays Active,
92/106 (87%),M14 4/10; no new fixed gate closes.

Chataigne-only graph, Curve, Gradient and Spatializer followups return only when Chataigne is
selected. Their existing backend/history/copy/reload findings remain in the deferred handoff.
Curve capture is deferred to Chataigne:
return there for a source-supported ownership correction or the required actual-host/native
acceptance; the existing receipt does not identify which terminal boundary fired. Do not extend
the stopped runtime investigation with unchanged variants or restore its failed cache stash.
No completed slice or passing software suite substitutes for whole-gate acceptance.
