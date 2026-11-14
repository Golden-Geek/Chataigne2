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
editor adapters, hover and native acceptance; this public input contract alone does not qualify them.

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
adds a light fill, stronger outline and handles. Invalid mapping retains a red outline with
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
