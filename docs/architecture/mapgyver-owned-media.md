# Composition-owned media

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
Parameters advance its schema to `mapgyver.media.command.v3`. Older command records reject.
Placement v1 is versioned separately; old reference-only `mapgyver.composition.instance` records
reject explicitly. Source media v5 and Reference v1 retain their existing source-setting formats.
No persisted-data migration or alternate compatibility model is introduced.

This delivers the Composition ownership part of M04/7. Matrix still needs actual child media;
full mixed parity, missing/unconfigured-source drops, native copying/sharing/resampling and
end-to-end scale remain open. M06 also retains degree-based Rotation (Placement v1 currently
keeps the prior radian convention), new handles/anchors, bounded independent panels and its
complete native/editor acceptance. See the [status ledger](../progress/mapgyver-status.md).
