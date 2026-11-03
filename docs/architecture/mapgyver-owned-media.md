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

Admitted clip bindings retain bounded immutable Reference definitions through replacement,
clear and column removal. The capture path follows their targets without retaining old authored
indexes or decoder copies. Runtime dependency checks include retired alias targets; final-use
pruning and Unload clear those definitions. Physical/copied media retirement is still required:
in-use replacement/removal rejects atomically until Unload. Uncaptured References also retain
that guard. This temporary restriction must be removed by runtime-owner retirement under M04/7.

This delivers the Composition ownership foundation and a partial Matrix ownership slice of M04/7;
full mixed parity, missing/unconfigured-source drops, native copying/sharing/resampling and
end-to-end scale remain open. M06 also retains degree-based Rotation (Placement v1 currently
keeps the prior radian convention), new handles/anchors, bounded independent panels and its
complete native/editor acceptance. See the [status ledger](../progress/mapgyver-status.md).

Verification for the Matrix ownership checkpoint (2026-11-03):

- Library: **304 passed, one existing ignored**, including generated-contract drift, all 33
  command kinds, four caller paths, typed/copy/project/history, feedback and outgoing fades.
- Strict all-target Clippy with `native-qualification` passed; both workspace formatters ran.
- MapGyver Svelte: zero errors/warnings; the ordinary bundled executable built successfully.
- `tests/headless/media.mjs --matrix-owned-media-ui --owned-media-ui` passed: actual manager
  drags, default/Alt copies, generic Cell/Placement Inspectors, independent variants,
  replacement Undo/Redo and Save/New/Load. The isolated headless host exited normally.
- Automatic Reference aliases no longer consume render-pass admission credits; the retained
  five-pass bounded Matrix test still rejects the extra group scratch pass without raising budgets.

No native experiment ran. These software checks do not close M04/7 or qualify physical/copied
media retirement, unconfigured/missing drops, the revised Matrix editor or native resource use.
