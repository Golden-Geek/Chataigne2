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

Unselected Surfaces have a thin transparent outline and no handles or hover fill. Selection
adds a light fill, stronger outline and handles. Invalid mapping retains a red outline with
only a light selected fill; unrelated areas remain clear.

Screen authoring tracks the history identity represented by its projected geometry. After
Undo/Redo, a new gesture waits for the matching projection instead of copying a stale corner
origin into an edit. Ordinary background refresh remains interactive; an admitted gesture
keeps its ownership until it closes.

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
reply time, alongside the bounded authored-edit queue. Native decoration drawing still uses
the configured video cadence; this change alone does not establish 240 Hz presentation.

## Acceptance

M11/6 is **Accepted**. Software suites and native anchored/reduced-motion navigation,
Composition handles, keyboard/history, Surface Source input, bounded Matrix Frame and
two-editor selection in narrow panes pass. The Matrix toolbar reserves header clearance.
Browser invalid-quad checks preserve the red cue without covering unrelated areas. Native
input and DOM alignment do not establish scanout timing or close retained physical gates.
See the [receipt](../progress/evidence/mapgyver-m11-canvas-controls-2026-11-04.json).
