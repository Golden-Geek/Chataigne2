# Matrix Cell drops

MapGyver's Matrix backend owns address changes. `MatrixEdit::RelocateCell` moves
the actual Cell subtree within one Matrix, including its typed Parameters, owned
media and App Control children. Golden's ordinary application intent owns the
atomic history boundary. UI code supplies identities and an explicit occupied
destination policy; it must not rebuild content or configure internal addresses.

`Reject` requires an unmaterialized destination. A prepared Cell without media
still owns authored state and is occupied. `Swap` exchanges the two actual Cells;
an unmaterialized destination simply receives the moved Cell. A same-address
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

`DuplicateCell` supports Copy and Reference within or between Matrices. Both
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

Shared owner chains are bounded and cycle-checked. Their cached projections use
one definition-revision guard, separate from runtime launch revisions. Source
application edits invalidate linked consumers, while launches reuse the authored
index. A Layer checks newly added Cells for configured/shared application
ownership, including history restoration. It requests the shared callback tree
snapshot only for direct child additions and reads the two bounded Parameter
folders; ordinary geometry/value events request no tree snapshot. Unconfigured
Cell authoring does not invalidate logical application captures. Structural
materialization snapshot cost remains part of large-project qualification.

Missing linked owners retain their authored identities for Undo/recovery.
Relocation/swapping still requires one Matrix. GUI dragging, the Alt menu,
context/keyboard paths, scrolling/cancellation and complete native/process/
copy-reference lifetime qualification remain M13/7 work. Backend milestones
do not accept that gate.
