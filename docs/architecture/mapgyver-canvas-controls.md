# Canvas controls

Golden graph UI owns the public `CanvasToolbar` and keyboard navigation command mapper.
GraphCanvas and Golden UI's `Viewport2D` compose them while retaining their own camera,
pointer and animation lifetimes. Native preview underlays remain owned by the desktop host.
MapGyver supplies contextual action snippets to the same toolbar; Composition, Screen
Warping and Surface Source no longer need duplicated navigation or internal title bars.

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

Live Surface geometry publishes ordinary typed Parameter changes and a direct authoritative
Surface reply. Geometry samples do not invalidate the entire Screens catalog; source/metadata
and structural operations still do. This avoids making every outliner row and unrelated editor
query compete with each pointer sample. Native render revision tracking remains unchanged.

Screen/Surface editors observe typed leaves only for the backend-resolved, bounded visible
owners and independent selected Surface. A new query reply does not itself request another
identical query. This observation also covers Inspector, script and transport edits outside
the editor's own gesture. Own gestures continue accepting their direct replies.

The app's `ReadQueue.refresh` publishes sequential in-progress observations while retaining
one active read and one latest replacement. Session, project, snapshot and query identify
that live context. During an active edit group its stable edit-session epoch permits live
observations even though Golden advances working history IDs on every sample. Outside the
group, the exact committed history identity fences replies; group closure, context changes
and starting an owned edit invalidate old work. Catalog row supplements and spatial target
discovery do not refresh for each working history ID; metadata signals and group boundaries
still refresh them.
Other readers retain the existing latest-only `request` behavior. Continuous input therefore
cannot starve every valid reply until release, without weakening document/history fences or
copying backend geometry policy into the UI.

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

Golden's public `PanelApi.observeVisibility` reports the actual dock visibility immediately
and on changes, with explicit subscription disposal. Retained Screen/Surface tabs keep
metadata, title and routing observation, but suspend live typed-leaf inspection while hidden.
Showing a tab refreshes its bounded authoritative snapshot before further editing. This keeps
offscreen tabs from adding per-sample queries to another editor's drag while preserving
locked-panel identity and rename feedback.

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
