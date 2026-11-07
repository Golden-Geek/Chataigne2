# Matrix Cell drops

MapGyver's Matrix backend owns address changes. `MatrixEdit::TransferCell` moves
the actual Cell subtree within one Matrix, including its typed Parameters, owned
media and App Control children. Golden's ordinary application intent owns the
atomic history boundary. UI code supplies identities and the requested transfer
action; the backend chooses occupied handling. The UI must not rebuild content
or configure internal addresses.

Its `Drop` action exchanges two actual Cells or moves into an unmaterialized
address. A prepared Cell without media still owns authored state and is occupied.
Explicit `Move` requires an unmaterialized destination. A same-address
request is a no-op. The backend validates both addresses and the completed media
dependency graph before staging changes. It writes at most two Column Parameters
and reparents at most two Cells; it creates/removes no subtree. Existing document
and dependency preflight costs remain outside continuous pointer processing.

Existing launched clips keep their source, launch and application identities at
their current playback scope. Moving authoring does not relaunch or redirect a
running clip to the destination's previous content. A subsequent explicit launch
uses the new authored address. Retained outgoing sources participate in cycle
validation and existing completion-safe retirement.

Cell structural events invalidate application membership on reparenting, even
when the Column Parameter is unchanged. That boundary also handles Undo/Redo;
it is independent of the UI or the particular operation that moved the Cell.

The `Copy` and `Reference` actions work within or between Matrices. Both
require an unmaterialized destination, including when the source and destination
are the same address. Copy imports the complete dense Cell subtree through
Golden's public persistence API, with one UUID remap and a typed Column override.
Parameter values, constraints and metadata survive without an f32 round-trip.
External references retain their identities; owned producers gain fresh ones.

Reference imports the Cell settings and links its media through an actual
Reference node. Its typed **Shared app control** Parameter points to the source
Cell. With no local App File, it shares that Cell's application identity instead
of duplicating a producer process. A local App File takes precedence through the
same backend projection for Inspector, script and transport writes. Explicit
Cell application configuration or source replacement clears the shared link.
Column and Group preparation remain part of the destination's hierarchy;
Cell-local producer ownership is what the reference shares.

Shared owner chains are bounded to32 Cells and cycle-checked. Reference admission
reserves its new hop before any subtree is imported, so the operation cannot
create a document its owner resolver rejects. Cached projections use one
definition-revision guard, separate from runtime launch revisions. Source
application edits invalidate linked consumers, while launches reuse the authored
index. A Layer checks newly added Cells for configured/shared application
ownership, including history restoration. It requests the shared callback tree
snapshot only for direct child additions and reads the two bounded Parameter
folders; ordinary geometry/value events request no tree snapshot. Unconfigured
Cell authoring does not invalidate logical application captures. Structural
materialization snapshot cost remains part of large-project qualification.

The grid sends `Drop` for normal dragging. Alt-drop opens Golden's menu with
explicit Move/Copy/Reference actions; it sends no mutation until a choice is
made. Backend validation remains authoritative for occupied destinations.
Drag payloads contain bounded identities, the ephemeral project instance and
snapshot generation. Menu actions and late replies fence the session and target
Matrix, and completed operations adopt selection only while its captured owner
remains current. Closed menus clear their item closures.

The app-owned script API exposes
`local.transferMatrixCell(requestId, matrixId, cellId, layerId, columnId, action)`.
Its action is the same Rust-owned Drop/Move/Copy/Reference enum, and its committed
or rejected callback follows the ordinary media operation and history boundary.
Malformed identifiers or action names are rejected before queueing any work.

Ctrl/Cmd+C and X capture a Cell identity, V pastes Copy or Move, and Alt+V with
the same modifier pastes a Reference. Grid context menus offer equivalent actions.
The clipboard is scoped to the workbench session/project; it stores no authored
Cell settings. Rejected cuts remain available, and successful moves clear them.
The grid registers focused clipboard handlers through Golden's public prioritized
command registry, ahead of generic subtree editing. They are removed when the
grid unmounts and leave editable fields and other panels to their own handlers.
The playback context action is called **Trigger Cell (Cut)** to distinguish it
from clipboard Cut.

Missing linked owners retain their authored identities for Undo/recovery.
References keep the selected media identity when its originating Cell is cleared,
replaced or removed; they do not silently adopt a replacement producer. Restoring
the same source through history restores that binding. Independent copies keep
their owned content. A prepared empty Cell can also be copied or referenced:
its authored settings and application ownership survive without inventing media.
An already running clip retains its admitted owned producer graph through this
removal, following the existing Matrix lifetime contract. The authored Reference
can report missing while that clip finishes; a fresh launch resolves the current
missing target, and stopping the final consumer retires the retained graph.
Relocation/swapping requires one Matrix. The [M13/7 acceptance audit](../progress/evidence/mapgyver-m13-cell-drop-acceptance-2026-11-07.json)
covers all public callers, bundled UI operations, sparse structural updates and
both-GPU playback/source/process lifetime and retirement. Structural timings
do not establish manipulation cadence or waive retained production budgets.
