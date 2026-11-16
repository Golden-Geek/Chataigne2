# Canvas controls

Golden graph UI owns the public `CanvasToolbar` and keyboard navigation command mapper.
GraphCanvas and Golden UI's `Viewport2D` compose them while retaining their own camera,
pointer and animation lifetimes. Native preview underlays remain owned by the desktop host.
MapGyver supplies contextual action snippets to the same toolbar; Composition, Screen
Warping and Surface Source no longer need duplicated navigation or internal title bars.

`Viewport2D.onselectionclear` dispatches an unconsumed primary background pointer-down or
idle Escape. Navigation capture, child controls, text editing and consumed tool keys retain
ownership; selection no longer waits for a later click. The adapter chooses the edited owner
and updates the shared workbench selection. Screen Warping selects ordinary Surface hits on
pointer-down without opening a geometry edit; Alt-drag and corner capture keep their existing
gesture path. Its keyboard/accessibility click fallback cannot reselect a Surface on physical
mouse-up. See the [M14 selection inventory](../progress/mapgyver-m14-selection.md) for remaining
editor adapters and native acceptance; this public input contract alone does not qualify them.

Golden's public `selectOnPress` action admits primary-down target selection and keyboard/
accessibility activation, ignoring later physical clicks and nested controls. It leaves native
drag admission intact and unregisters its listeners on disposal. `clearViewportSelection`
dispatches plain background-down and tool-first/idle Escape with shared text/control/modifier
guards. `Viewport2D`, Dashboard and Matrix grid consume these reusable selection mechanisms;
adapters supply explicit owners and tool cancellation.

Golden's Inspector registry can opt a specialized editor into `anchorSelection`. A selection
of its descendants keeps that editor mounted while the workbench still selects the actual
document nodes. Inline curves use this hook for key/easing selection and their embedded typed
Inspector; pointer-down no longer destroys the canvas holding capture. Parameter Watchers and
raw data retain the selected Parameter, with an anchored Watcher below the editor to avoid
moving its canvas during input. Explicit Inspector locks keep their document/UUID boundary.
Ownership projection caches immutable selection arrays against published graph deltas, so live
unrelated Parameter updates do not rescan large selections. The curve adapter publishes only
input-owned changes and rejects replaced session/project/snapshot/selection scopes.
Before projecting a new graph publication, it settles a valid pending local selection against
that same input basis. A same-turn metadata update cannot replay the previous workbench owner
over a newly selected key. A newer foreign selection still wins; current topology is checked
before publishing, and ordinary projection does not write authored data.

`inspector/editor-selection` now owns that bridge for both Curve and Gradient. Each adapter
supplies a bounded hierarchy projection, typed local signature and current-node resolution;
the bridge owns input admission and host/document/selection fencing. Gradient's registry entry
anchors its editor while real stops or their Parameter children drive the workbench Inspector.
Background/idle Escape inspect the Gradient owner without choosing an implicit first stop.
Stop markers use the public `selectOnPress` action for pointer and keyboard selection; hover
changes only their unselected contour.

Gradient captures the stable editor surface so sorting stop markers does not replace its capture
target. Pointer cancel, lost capture, blur, access/topology/selection loss, snapshot replacement
and disposal retire input without completing armed deletion. Normal release alone may remove
the stop after current UUID/access checks; submitted writes settle before the pinned history
group ends. Pending closure prevents a new gesture from joining it. Color-popup Escape stays
local to its editor, and its existing double-click command survives capture retargeting.
The shared ColorPicker starts edit ownership before its first pointer sample is published.
The production Inspector fixture verifies these software boundaries; actual backend/history
and native-host acceptance remain separate.

Gradient data ownership stays in Golden: MapGyver currently exposes no authored Gradient editor
path; Chataigne Alchemist is an existing product consumer. Golden's public project facade owns
creation, typed edits, history, subtree copying and project codecs. Backend qualification uses
that facade rather than adding a MapGyver-only authoring route or calling raw-engine duplication,
which deliberately requires the project codec boundary.

Fresh Gradient nodes seed their initial stops during initialization. Reload and subtree copy
preserve authored zero-stop collections through the project decode boundary; in-place history
restoration preserves initialization state. Sparse project codecs distinguish declared schema
children from authored user-item collections. Both live and captured encoders retain user items
and explicit empty overrides against seeded collections; expansion inherits declared children
and replaces user items with the saved list. This requires no opaque persisted flag or special
declared-child identity retention. The public-facade regressions cover standalone and declared
Gradient reload, partial removal, history and subtree copying; native acceptance remains separate.

Curve background-down and idle Escape now inspect the Curve owner through that adapter.
Golden's focused `curve/input` controller separates release from cancellation for key/Bezier/
segment edits, marquee, drawing and pan. It retires ownership before releasing capture;
Escape cancels tools first, and cancel/lost capture/blur/disposal discard unsent work and local
previews without completing a marquee or drawing. Submitted writes settle before their pinned
session ends. Scope replacement and foreign local selection stop stale queued writes; ordinary
live parameter publications retain capture. Idle Escape reuses `clearViewportSelection`.
The specialized editor and renderer are composed from bounded modules under `common/curve`.

Golden's `parameter-value-access` owns the value-access rule shared by the ordinary Inspector
and specialized Curve gestures. Typed read-only, enabled and control metadata remain authoritative;
the UI does not change control modes to admit an edit. Curve geometry requires its complete
parameter group and matching current UUIDs before opening an edit session. It rechecks access
before queued writes, cancels on revocation and respects document remove/duplicate permission.
Read-only targets remain selectable. The selected key's typed Inspector remains mounted when
navigation moves the key outside the canvas. Curve owner replacement resets retained camera
bounds; live parameter publications preserve them. Curve constants/value projection, key-model
reading and compiled easing evaluation now live in focused modules under `common/curve`;
model projection never mutates document nodes.

`AnimationCurveCanvas` is now a DOM/lifecycle host. Its props derive from the typed
`CurveCanvasFrame`; shared Curve model types replace the former partial duplicate declarations.
`canvas-paint` consumes one draw frame and returns both bounds text and the hit transform from
the exact painted geometry. `canvas-samples` owns clipped/adaptive/discontinuous sampling,
`canvas-grid` owns axis/grid presentation, and `canvas-theme` resolves CSS tokens. The host
retains canvas sizing, theme observation, event forwarding and projection publication. These
renderer files stay below1000 lines.

Golden's `editor-operation-scope` separates a pending command's view lifetime from its original
document lifetime. Host, project, snapshot, loading, owner removal and disposal invalidate the
document lease; changing the edited owner permanently prevents new submissions from that view.
Already submitted work may settle on its original document, while a changed selection suppresses
late selection feedback. The Curve adapter captures the owner identity and edit-session host
before awaiting Begin for drawing, creation, paste and easing commands.

The focused `curve/creation` controller matches the exact created-node ID/UUID from the backend
receipt and observes typed child projection events. It never guesses the first new key or polls
for completion. Initialization uses the original owner's keys even after the view changes; each
remaining write checks the document lease and current parameter access. Clipboard sequences stop
before submitting another key after view replacement. Owned preview cleanup preserves newer
preview objects, and a late creation cannot steal the user's newer selection. This changes no
authored schema or domain defaults; actual backend/history/native acceptance remains separate.

MapGyver's Matrix admission guard protects Matrix structure and its declared Parameters, while
Golden owns editable descendants of a Parameter's control subtree. Walking ancestry stops at
that Parameter boundary for nested control edits. Curve key creation/removal/move/duplication
therefore use ordinary Golden intents; direct Matrix edits, mixed removals and attempts to remove
the declared Parameter remain protected. This policy belongs in the backend and applies to every
generic caller, including the ordinary app's transport. The connected Curve fixture also retains
an unresolved first-drag cancellation; passing key creation does not establish full input acceptance.

Golden's `createEditorScopeObserver` compares the host, project/snapshot generation, connection,
loading visibility and owner identity explicitly. Curve input uses this semantic transition to
cancel its tool; reactive effect cleanup is reserved for disposal. Replacing a loading-metadata
object while visibility stays false therefore preserves capture and queued edits. Real scope,
selection and parameter-access replacement still cancel, and component disposal retires input.
The production Inspector fixture verifies a hidden metadata refresh during a captured drag.
The separate connected first-drag failure remains unresolved after this correction.

Curve `edit-gesture` owns gesture admission, captured document/selection/access scope, latest
key/Bezier frame samples and edit-session handoff. `parameter-writer` owns current typed access/
coercion and unsettled writes for both gestures and pending commands. The terminal input controller
retires tools, flushes or discards samples and waits for those writes before closing the pinned
session. The editor supplies geometry/projection adapters instead of holding that bookkeeping.
`AnimationCurveNodeEditor` is the687-line composition/lifecycle host. Focused editor factories
own model projection, selection, previews, constraints, insertion, sculpting, camera, hits,
drawing, commands and pointer dispatch; each consumes a typed interface limited to its dependencies.
The context exposes lazy references so mutually dependent projections do not create runtime import
cycles. Document and parent selection bindings stay canonical; `editor-state` holds only transient
presentation/input state. Effects stay at the composition boundary in their original order.
All editor modules are below1000 lines. This structural split does not qualify the unresolved
connected capture failure or actual backend/history/native behavior.

Matrix grid uses those actions for Cells, row/column/group headers and their labels, alongside
the shared contour hover. Background/idle Escape select the Matrix media owner. Grid coordinates
and an explicit document focus identity publish together, so clearing or moving between Column
Groups does not wait for a query receipt; older focus replies remain fenced. Coordinate-only
transfer operations may retain their existing focus. The backend still resolves ownership and
explicit no-child selection; UI code does not create domain defaults or authored history.

Golden's public `viewportContour` owns hover lifetime and contour presentation. Its pure
`viewportContourAppearance` provides the same colors and rem widths to DOM controls and native
overlay projection. Screen, Source, Composition and Matrix consume it without hover selection,
handle admission or domain edits. Native preview's existing enter/leave forwarding drives the
same action; there is no additional input transport. Press/capture loss/leave/blur, owner
replacement and disposal clear hover. Invalid contours retain their warning color.

Only selected objects expose transform gizmos. When source bounds are unavailable, Golden's
`ViewportTarget` places a small selection-only outline at the known pivot; it does not invent
media bounds or resize controls. Keyboard activation retains viewport focus before selection
replaces that target, so Escape continues to inspect the owner. Known bounds use the object's
contour directly. App adapters still own geometry, selection identities and authored operations.

Golden graph canvases use their existing `onGraphSelectionChange([], [])` contract for primary
background-down and idle Escape, with no repeated mouse-up clear. Alchemist and State Machine
already resolve that empty selection to their Formula/manager owners; Spatializer's custom world
now selects its module owner through the same callback, preserving active endpoint ownership.
Golden owns modifier/marquee/navigation and connection-tool isolation. Selected graph nodes alone
show resize handles, and unselected valid nodes receive a contour hover cue. Complete native/
Inspector and active gesture cancellation acceptance remains in the M14 inventory.

Graph cancellation now has a separate terminal path from pointer-up commit. Escape, pointercancel,
lost capture, blur and scope/topology/disposal boundaries retire owned capture before releasing it
and withdraw uncommitted previews without mutation callbacks. Earlier pending optimistic edits
retain their lifecycle. This is verified in the production DOM fixture for standard graph gestures;
actual domain/native/custom-world qualification remains in M14/1.

Golden Dashboard keeps its specialized free/ordered layout controllers. Its focused selection
module owns widget pointer/keyboard admission and page Escape dispatch; only selected widgets
mount resize zones, and unselected hover changes the border. Each page's view context routes
cancellation only to its mounted controllers, including nested marquee and navigation input.
Escape cancels those tools first, then selects the page owner when idle. Canceled marquees
restore their prior selection; late pointer release cannot submit the canceled operation.
Nested keyboard selection stops before reaching an ancestor widget. Run mode and text input
retain their native keys. Separate page views have independent cancellation registries.

Golden's public `createViewportPointer` owns coordinate projection, primary-hit consumption
and pointer capture/cancel. It selects a hit before calling the adapter's edit admission;
refused edits retain selection and viewport focus without acquiring capture. Composition and
Matrix consume this helper directly; the former app-local pointer copy is removed. Adapters
retain typed target identities and backend mutations.

Each pointer start takes a typed `ViewportPointerTarget` with separate `selectable` and
`editable` permissions. The shared helper consumes/selects an available hit before evaluating
edit permission or calling `begin`; a refused edit acquires no capture. Composition and Matrix
must pass connection/loading and active-tool availability separately from metadata/read/mutation
busy state. Their selection-only keyboard paths use that same availability, so a pending read
cannot swallow the next selection press. Existing focused gesture controls retain focus.

Screen Warping follows the same selection-before-admission rule in its native-aware controls.
`SurfaceHandles.selectionAvailable` follows connection/loading availability; the owning adapter
retains the active-gesture guard. Metadata/read busy state blocks geometry, while Surface and
corner pointer/keyboard selection and background owner clearing remain available. Busy state
must not disable the focused corner during a held keyboard edit. Actual bundled UI/backend
selection, peer/Inspector, history and keyboard/project tests cover these boundaries; physical
native hits/pixels remain separate acceptance.

Composition's query distinguishes an explicitly selected edited owner from an absent or
unrelated selection. A selected owner has no selected child, even with an old retained hint
or when a nested Composition is itself an outer instance. The query does not invent a first
child. The adapter follows authoritative page selection and publishes direct pointer choices
immediately, without letting parameter projection receipts reselect an old child. Broader
cross-editor selection retention remains in M14/1's acceptance inventory.

Matrix Layer and Cell spatial adapters use the same background/idle-Escape callback. The
Layer view selects its Matrix media owner; the Cell view selects its Layer owner while
keeping that Layer open. Backend Matrix queries distinguish explicit owner selection from
retained child hints and resolve owned Cell descendants through the existing bounded owner
walk. An unlocked nested Matrix or Composition retains its edited domain when its own media
owner is selected, rather than navigating to its outer Composition placement.

Golden's public `selectedEditorItem` resolves a current selected identity against bounded
visible definitions and one independently queried off-page definition. Cached geometry cannot
invent selection after clearing it or override a newer pointer choice. Matrix adapters keep
their existing project/session fences; Cell selection observes raw authoritative pages rather
than parameter-projected copies, so geometry receipts cannot restore a stale selected Cell.

Surface Source selects the Surface's actual Crop Origin Parameter for the crop body and Crop
Size Parameter for its corners. Their backend-projected UUID bindings drive shared workbench/
Inspector selection and selected-only DOM/native controls; no second selection store or authored
crop node is invented. The retained edited owner is the Surface: background-down or idle Escape
selects that Surface and withdraws corner paint and native corner hit IDs, retaining the crop's
selectable contour. Keyboard activation selects the same typed Parameters without opening an edit.
An active drag consumes Escape to cancel before the shared viewport can deselect; a subsequent
idle Escape clears selection. Native pointer capture and project/source lifetime fencing remain
at the existing preview boundary. Pointer-down retains gesture admission through selection
refreshes, while geometry samples wait for actual movement. Releasing without movement closes
the owned group without sending a geometry intent.
Metadata refresh may block geometry editing without disabling pointer/keyboard selection of a
corner; a disabled native button would lose selection between focus and keyboard activation.

[Screen output cadence](mapgyver-output-cadence.md) is authored independently of editor input
and the native preview's 60 FPS consumer. Shared clocks and endpoint pacing belong in Golden.

Matrix keeps bounded table scrolling. Home and Frame use cached global row/column indices
in the Rust selection snapshot, including when the selected object is outside the visible
window. Column positions are indexed when the cached view is built; selection lookup does
not scan the document. Toolbar clearance preserves access to the column headers.

In the Cell layout editor, the selected Cell's controls paint above overlapping Column
alternatives so its drag area remains reachable in a populated Matrix. Layers keep their
backend compositing order. Reordering these bounded keyed controls preserves SVG-root
pointer capture and never changes authored order, source routing or history ownership.

Unselected Surfaces have a thin transparent outline and no handles or hover fill. Selection
adds a light fill, stronger outline and handles; hover only strengthens the contour. Invalid mapping retains a red outline with
only a light selected fill; unrelated areas remain clear.

Screen authoring tracks the history identity represented by its projected geometry. After
Undo/Redo, a new gesture waits for the matching projection instead of copying a stale corner
origin into an edit. Ordinary background refresh remains interactive; an admitted gesture
keeps its ownership until it closes.

Surface, Composition and Matrix Layer/Cell geometry publish ordinary typed Parameter changes
and direct authoritative replies. Continuous geometry intents do not invalidate the Screens
or media catalog; source/metadata and structural operations still do. Native render revision
tracking is unchanged, while unrelated catalog readers no longer compete with every sample.

All those editor readers and Media Preview use Golden's public `observeEditor` and
`EditorReadQueue.observe` contracts. The observer follows changed Parameter ancestor paths
under backend-resolved owners, including nested Placement nodes and off-page selections.
Persistent-index differences avoid enumerating the document or copying a folder-parsing loop
into every view. Metadata views limit ancestry depth; hidden adapters automatically inherit
their dock's visibility. A query reply cannot trigger an identical query loop.

The shared queue retains one active read and one latest replacement and publishes valid
observations while another widget's grouped edit continues. Session, project, snapshot,
semantic query and edit-session epoch identify the live context; rolling working-history IDs
do not starve presentation. Group closure restores exact committed-history fencing. Owned
gestures still suppress competing reads and accept their direct replies. Ordinary latest-only
queries use Golden's `ReadQueue.request`. App controllers retain domain mutations and geometry;
reusable synchronization and lifecycle rules belong in Golden rather than individual panels.

Screen keyboard input retains up to 32 pending press/release gestures while an earlier edit
closes or its authoritative geometry projection catches up. Each starts from the current
accepted corner and keeps its own history group; held repeat still coalesces integer offsets
within one group. Readiness changes drain this bounded queue without polling. Selection,
document and connection loss cancel pending input, and focus loss never replays it later.
The app input adapter owns this sequencing; backend geometry still owns output-pixel conversion
and validity. Native gesture ownership is queried synchronously when closing a queued gesture.

Golden UI's `LatestEditGesture` also publishes a bounded transient presentation containing
the owned reference, immutable gesture basis and latest pointer sample. It updates before
transport acknowledgement while the existing one-in-flight/one-latest mutation queue stays
unchanged. Receipts update canonical snapshots without replacing the newer presentation.
Finishing retains the hint through the final acknowledgement and history close; cancellation,
lost ownership and disposal clear it. Presentation never becomes an authored edit basis.

MapGyver projects this hint into control vertices for Screen warp, Surface Source crop,
Composition transforms and Matrix Layer/Cell rectangles. Screen/Source native overlays and
DOM hit controls use the same projected vertices. Domain limits, rounding, validity, persisted
settings and history remain backend-owned; provisional outlines are not authoritative output.
The [functional receipt](../progress/evidence/mapgyver-m13-gesture-presentation-2026-11-07.json)
verifies real held-request input and history. Presented-frame timing and native/DOM pixel
alignment remain separate M13/4 qualification requirements.

Golden video UI's native input adapter admits a hit through the accepted native frame, then
projects explicitly captured movement in that stable native/DOM coordinate basis without
per-move IPC. Terminal hit testing remains native; changed bounds/DPR or ownership cancels
the sequence. This lets Screen/Source control delivery proceed independently of metadata
reply time, alongside the bounded authored-edit queue.

The workbench supplies one reactive visibility context to each mounted panel and its nested
adapters. Golden's observer consumes it automatically; editors do not need visibility props
or subscriptions at every layer. Retained tabs keep metadata, title and routing observation
while suspending live value inspection. Showing a tab resolves its authoritative snapshot,
preserving locked identity and rename feedback.

Golden Video now wakes a separate presentation submission when live placement changes.
It reuses admitted content textures and redraws affected presentation branches/outputs between
media deadlines, with the same completion/retirement and project/device fences. Media time,
content frame counters and unrelated outputs retain their cadence; the footer describes those
content metrics. Pending GPU/endpoint admission retries only on the native renderer worker,
and stops after acceptance. See [Golden Video](../../crates/golden_video/README.md) for ownership
and retry details. Both-GPU slow-media pixel checks prove independent redraws; actual30+fps
presented manipulation (60preferred) and pointer-to-visible latency remain M13/4 acceptance work.

## Acceptance

M11/6 is **Accepted**. Software suites and native anchored/reduced-motion navigation,
Composition handles, keyboard/history, Surface Source input, bounded Matrix Frame and
two-editor selection in narrow panes pass. The Matrix toolbar reserves header clearance.
Browser invalid-quad checks preserve the red cue without covering unrelated areas. Native
input and DOM alignment do not establish scanout timing or close retained physical gates.
See the [receipt](../progress/evidence/mapgyver-m11-canvas-controls-2026-11-04.json).

M13's [pending-outline checks](../progress/evidence/mapgyver-m13-pending-outlines-2026-11-07.json)
now cover Matrix Layer/Cell, Composition translation/rotated resize, Screen and Source on both
GPUs at125%. Captured physical edges agree within one pixel while authored requests are held;
final geometry/history and independent output continuity pass. Composition/Matrix outlines are
DOM controls; native accepted-frame control hits apply to Screen/Source. This scoped coverage
does not establish moving image correspondence,240Hz timing or other DPI/workload acceptance.
