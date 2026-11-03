# Composition and Matrix-owned media

MapGyver's media layer owns containment and source identity. Composition owns placement and
canvas operations; Golden owns typed Parameters, subtree import, identity remapping and history.
`media/composition/children.rs` projects the document, `drops.rs` implements source insertion,
and capture/dependency validation consume those same public media identities.

Composition media contain their single output channel and ordered actual media children.
An authored item is a normal `MediaNode` or `ReferenceNode`, with exactly one
`mapgyver.composition.placement.v1` child. That child declares Output, Position, Scale, Rotation,
Pivot, Opacity, Blend and Enabled Parameters. Output is a producer-owned stable channel choice;
ordinary channel retargeting remains deferred. No separate Instance document node or source
media Parameter duplicates the parent identity. Canvas `InstanceId`/snapshots are presentation
projections: their ID is the real media child's UUID and their source routes that child's output.

Default `CompositionEdit::DropSource` inserts a Reference with Placement in one detached tree.
Its target rendering/decoder remains shared. `copy: true` captures the complete source media
subtree through Golden's public import API. Internal UUID references remap together; external
asset/target references remain external. Every authored channel variant, constraint and control
subtree survives the copy. New copied source media own independent settings and identities.
Replacing an existing Reference edits its typed Target/Output atomically. Replacing owned
content retains the exact Placement subtree and returns the new media identity. Undo/Redo
groups the complete operation, including imported media and replacement cleanup.

`AddInstances`, duplication, geometry, reorder/removal and inspection operate on actual media
children. Batched creation uses a detached forest. Media Manager follows the real hierarchy;
the generic Inspector displays each child's media Parameters and Placement. The canvas sends
the drag's Alt state as `copy` and viewport-derived coordinates, with defaults/validation owned
by the backend. An unconfigured or missing Reference source currently has no selectable output
and cannot yet be dropped; that parity case remains open under M04/7.

The manager's named `dropCompositionMedia` method and typed **Drop Composition Media** command
use the same intent and committed callback. The command's Position, Copy media and Replace media
Parameters advance its schema to `mapgyver.media.command.v4`. Older command records reject.
Placement v1 is versioned separately; old reference-only `mapgyver.composition.instance` records
reject explicitly. Source media v5 and Reference v1 retain their existing source-setting formats.
No persisted-data migration or alternate compatibility model is introduced.

Matrix Cells are real `mapgyver.matrix.cell.v1` nodes under their Layer, with typed Column
and Output Parameters and at most one actual Media/Reference child. Cell snapshots project
that child's identity; the selected output belongs to the child. Default grid drops create a
Reference; Alt-drops copy the entire media subtree. A copied Composition item's root Placement
is excluded because Matrix owns its layout. Group/Column copies remap all internal references,
including Cell addresses and paired exports, in one history operation. Matrix row/column
settings still use the earlier opaque model; their typed redesign remains M07/M08 work.

The virtualized grid selects the actual Cell for Golden's Inspector. Media Manager traverses
Matrix hierarchy and owned sources. `dropMatrixMedia`, the typed **Drop Matrix Media** command,
and `MatrixEdit::DropSource` share the backend implementation; command schema v4 has 33 kinds.
Old Cell records reject explicitly instead of restoring the removed opaque source model.

The Media Manager owns one runtime-only producer registry in `media/retention.rs`.
Admitted Matrix bindings retain reachable channel definitions and derived inputs there; shared
producers occupy one record across Cells and Matrices. Replacing or removing playing owned
media needs no Unload. Capture reads a deleted Cell-owned source from this immutable graph,
while external producers remain live and external target deletion still reports missing.
The registry retains runtime projections; authored trees, obsolete indexes and decoder copies
stay out of it.

Retired Compositions keep their captured placements. Retired nested Matrices keep their last
admitted launch/envelopes and settle their outgoing inputs under shared time; cached records
drop their live PlaybackControl handles to avoid ownership cycles. Nested application references
are conservatively held through the owning clip; the revised app-control contract remains M08.
Feedback validation follows retained owned graphs through their external producer frontier.

The registry separately admits at most 4096 channel records, placement/input elements and
Matrix consumers. Capacity rejection preserves the previous admitted graph. Consumer registration
uses media UUIDs; capture/settlement/Unload prune weak consumers and unreachable metadata.
A registry lifetime guard fences later plan installation. Picture preparation and Golden Video
retain decoded/GPU leases through final frame completion, with their existing byte/flight limits.
Automatic Reference pass accounting is shared by rendering and Picture preparation.

A launch records its authoring epoch alongside its edit-completion receipt. If content changes before
the first capture, that launch cannot silently admit the later replacement; explicit relaunch
activates it. Cell authoring binds invalidation before structural edits so this also works for
previously uncaptured Cells. These runtime records never persist; all authored settings stay
in their typed Parameter children.

This delivers the Composition and Matrix ownership/retirement foundation of M04/7;
full mixed parity, missing/unconfigured-source drops, native copying/sharing/resampling and
end-to-end scale remain open. M06 also retains degree-based Rotation (Placement v1 currently
keeps the prior radian convention), new handles/anchors, bounded independent panels and its
complete native/editor acceptance. See the [status ledger](../progress/mapgyver-status.md).

Verification for the Matrix ownership/retirement foundation (2026-11-03):

- Library: **311 passed, one existing ignored**, including generated-contract drift, all 33
  command kinds, four caller paths, typed/copy/project/history, feedback and outgoing fades.
  Physical-copy replacement/removal, uncaptured launch epochs,
  derived retirement, atomic channel/element capacity rejection and Picture leases are covered.
  All 56 affected rendering tests passed after the final consumer-registration deduplication.
- Strict all-target Clippy with `native-qualification` passed; both workspace formatters ran.
- UI sources are unchanged; the prior zero-error/warning Svelte check remains applicable.
  The ordinary bundled executable built successfully.
- `tests/headless/media.mjs --matrix-owned-media-ui --owned-media-ui` passed: actual manager
  drags, default/Alt copies, generic Cell/Placement Inspectors, independent variants,
  launched-cell replacement Undo/Redo and Save/New/Load. The isolated headless host exited normally.
- Automatic Reference aliases no longer consume render-pass admission credits; the retained
  five-pass bounded Matrix test still rejects the extra group scratch pass without raising budgets.

No native experiment ran. These software checks do not close M04/7 or qualify native media
retirement, unconfigured/missing drops, the revised Matrix editor or native resource use.
