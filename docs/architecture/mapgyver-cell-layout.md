# Matrix Cell-content layout

MapGyver owns Cell layout queries, navigation policy and transient editor images. Golden owns
the viewport, native preview registrations and GPU execution. The existing Composition panel
switches between its focused adapters; Matrix playback/structure remains in the separate grid.

Open a Layer's Cells with its compact toolbar action or a double-click on its layout rectangle.
The Cell view keeps the same Frame/Home controls and a Matrix Layers breadcrumb. Its persisted
state contains document-scoped Matrix/Layer/Cell identities and a page, never authored settings.
The tab uses current backend Matrix and Layer labels. A selected Cell drives the standard
Inspector; inactive Cells can be inspected without launching them.

`InspectMatrixCells` uses the cached Matrix index, with authored Cells ordered by Column and
indexed per Layer. It returns at most 128 visible definitions plus an independently selected
Cell. Empty addresses are never materialized. It retains missing selected identities for Undo,
rejects foreign Layer ownership, and reads current labels/settings/source dimensions. The UI's
one-active/latest reader fences history, project, session, snapshot and disposal transitions;
same-project reconnect preserves presentation while controls await a fresh snapshot.

`PreviewSource::MatrixCell` requests one transient image of the selected Cell inside its Layer
canvas. Its renderer key uses the Cell UUID, which cannot collide with an authored channel UUID;
it is absent from discovery and project data. Capture validates ownership through at most 64
ancestors and subscribes to the actual Cell/Layer runtime guard. It captures the selected media
dependency, then applies the existing Matrix position/size/Fit/Fill/Stretch/opacity mapping.
The Cell and whole-Layer playback-enabled flags do not suppress this authoring image. Capture
does not add the Cell's app-control demand or mutate playback. Ordinary source recovery remains
with the shared runtime. Missing/deleted Cells withdraw their image and can recover through Undo.

The transient producer and its dependencies count against existing channel/pass/memory caps;
it adds no persisted media or public output and sends no image frames over control IPC. Renderer
tests inspect immutable capture and both hardware adapters through the same preview source key.

Cell bodies translate through `EditCellGeometry` without requiring source dimensions. A move
preserves the natural Size policy. Eight opposite-anchor handles share the Layer rectangle
controls and checked backend geometry; the first effective resize authors independent Cell
Size. A natural-size resize must match the current source extent before it can change anything.
Unknown sources retain a move handle at their actual position, with no invented size or resize
handles. Frame uses a presentation-only neighborhood around that position.

The backend resolves the Cell UUID and checks at most 64 ancestors before editing its real
typed Parameters; pointer samples never load the whole Matrix. It preserves Fit, opacity,
enabled state and media Resolution, and does not replace the source runtime epoch. Zero motion
adds no history. The shared latest-sample gesture owns one history group; held reads are fenced,
accepted settings update visible/off-page presentation, and completion refreshes the latest query.
Escape, selection/project replacement and disposal retire the owned gesture and pointer capture.

The shared keyboard input moves the selected Cell and resizes a focused handle through the
same typed geometry operations, including natural and unavailable-source movement. Held arrows
share one history group; separate taps capture fresh geometry after closure. The standard
Inspector edits the same Cell placement Parameters; see [shared geometry](mapgyver-spatial-geometry.md)
for lifecycle and Layer/Composition controls. [M12/7 acceptance](../progress/evidence/mapgyver-m12-spatial-acceptance-2026-11-05.json)
covers native spatial controls/pixels and large sparse documents; retained production qualification stays in the ledger.
