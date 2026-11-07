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

This first delivery supports same-Matrix relocation/swapping through the public
operation. Foreign Cells/addresses reject before mutation. Copy/Reference,
supported cross-Matrix drops, grid dragging, the Alt menu, context/keyboard paths
and complete native/process qualification remain M13/7 work; this API milestone
does not accept that gate.
