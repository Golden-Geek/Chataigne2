# Composition and Matrix-owned media

MapGyver's media layer owns containment and source identity. Composition owns placement and
canvas operations; Golden owns typed Parameters, subtree import, identity remapping and history.
`media/composition/children.rs` projects the document, `drops.rs` implements source insertion,
and capture/dependency validation consume those same public media identities.

Composition media contain their single output channel and ordered actual media children.
An authored item is a normal `MediaNode` or `ReferenceNode`, with exactly one
`mapgyver.composition.placement.v2` child. That child declares Output, Position, Scale, Rotation,
Pivot, Opacity, Blend and Enabled Parameters. Output is an editable typed channel reference:
a value pins that channel identity; empty follows the first available output. No separate Instance document node or source
media Parameter duplicates the parent identity. Canvas `InstanceId`/snapshots are presentation
projections: their ID is the real media child's UUID and their source routes that child's output.

Rotation is authored in degrees as the real `parameters/rotation_degrees` Parameter and the
Rust-owned/generated `Transform2D.rotation_degrees` field. Negative angles and multiple turns
retain their authored values. Geometry replies remain in degrees; only transient trigonometry
for geometry, canvas presentation and renderer mapping converts to radians. No radian-valued
snapshot, cache or second authored model crosses those boundaries.

Default `CompositionEdit::DropSource` inserts a Reference with Placement in one detached tree.
Its target rendering/decoder remains shared. `copy: true` captures the complete source media
subtree through Golden's public import API. Internal UUID references remap together;
Reference targets outside the copied subtree remain external. Copied Picture Files become independent owned Parameters,
with internal decode sharing by locator. Every authored channel variant, constraint and control
subtree survives the copy. New copied source media own independent settings and identities.
Replacing an existing Reference edits its typed Target/Output atomically. Replacing owned
content retains the exact Placement subtree and returns the new media identity. Undo/Redo
groups the complete operation, including imported media and replacement cleanup.

`AddInstances`, duplication, geometry, reorder/removal and inspection operate on actual media
children. Batched creation uses a detached forest. Media Manager follows the real hierarchy;
the generic Inspector displays each child's media Parameters and Placement. The canvas sends
the drag's Alt state as `copy` and viewport-derived coordinates, with defaults/validation owned
by the backend. Missing/unconfigured Reference media can be dropped with an empty automatic
Output. A default drop retains the original Reference as Target; Alt-copy preserves its full
independent Target/settings subtree, even when no channel is currently available.

`CompositionEdit::CreateMedia` builds a complete owned source or unconfigured Reference and
Placement as one detached tree, without a source object in the manager. The backend allocates
the label/identity, validates finite canvas coordinates and capacity, and applies normal media
defaults. New empty media have no dependency edges; subsequent Reference/drop edits retain
the existing cycle validation. The bounded editor page supplies the same creation catalog as
other app creation surfaces. Canvas right-click actions send only the chosen type and canvas
coordinates, then select the returned real child. `createCompositionMedia` and the typed
**Create Composition Media** command share this operation and `InstancesCreated` callback.
The app script template includes the named method; generic `editMedia` exposes all remaining
copy/reorder/delete operations. Menu actions are fenced to their original project and channel.
The canvas declares Golden's `data-local-context-menu` boundary so the generic node menu's
window capture handler leaves these domain actions to the Composition editor.

The manager's named `dropCompositionMedia` method and typed **Drop Composition Media** command
use the same intent and committed callback. The command's Position, Copy media and Replace media
Parameters use schema `mapgyver.media.command.v6`, whose Fit Mode command replaces Sampling. Older command records reject.
Placement v2 is versioned separately; radian Placement v1 and old reference-only
`mapgyver.composition.instance` records reject explicitly before recoverable node decoding.
Legacy `rotation` operation fields also reject rather than silently defaulting an angle to zero.
Source media v7 and Reference v3 own Fit Mode for variable-size sampled sources and fix sampling to Linear;
the [source-sizing contract](mapgyver.md) owns its behavior. Their source-setting formats are unchanged.
No persisted-data migration or alternate compatibility model is introduced.

Matrix Cells are real `mapgyver.matrix.cell.v1` nodes under their Layer, with typed Column
and Output Parameters and at most one actual Media/Reference child. Cell snapshots project
that child's identity; the selected output belongs to the child. `CellSnapshot.media` preserves
occupied-slot identity independently of its optional resolved `definition.source`. Default grid drops create a
Reference; Alt-drops copy the entire media subtree. A copied Composition item's root Placement
is excluded because Matrix owns its layout. Group/Column copies remap all internal references,
including Cell addresses and paired exports, in one history operation. Matrix row/column
settings still use the earlier opaque model; their typed redesign remains M07/M08 work.

The virtualized grid selects the actual Cell for Golden's Inspector. Media Manager traverses
Matrix hierarchy and owned sources. `dropMatrixMedia`, the typed **Drop Matrix Media** command,
and `MatrixEdit::DropSource` share the backend implementation; command schema v4 has 33 kinds.
Old Cell records reject explicitly instead of restoring the removed opaque source model.

Media identity and output choice are separate in `media/source_node.rs`. Available media-node
drops pin the first ordered channel, while unavailable References retain an automatic choice.
Changing Target or restoring a missing producer resolves that choice without materializing
channel nodes, polling or changing authored Parameters. Explicit output choices stay pinned
through channel reorder. Generic Inspector edits use the same typed Output Parameters; canvas
geometry/settings edits preserve the choice instead of silently pinning a resolved projection.

Matrix clips admit a media/output selection before any concrete channel exists. Their bounded
bindings resolve it during demand capture, retain the last admitted concrete output for
retirement, and fence unresolved Reference chains so target recovery wakes capture. Empty Cells
remain distinct from occupied unavailable media and do not acquire later content without relaunch.
Parameter-cycle validation resolves automatic outputs against the proposed atomic values.

Golden's parameter codec explicitly writes a baseline value when its dynamic default differs;
clearing an Output to automatic (or an integer to zero) therefore survives project reload.
The fix belongs in Golden persistence, with direct codec tests, rather than in app-specific
serialization or an authored-state migration.

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

The Composition and Matrix ownership/retirement foundation now passes
[M04/7 functional Reference acceptance](../progress/evidence/mapgyver-reference-native-2026-11-03.json),
including both-adapter mixed/native copying, sharing, resampling and final-use retirement.
Whole-app scale remains required under Q04. [M06/1's model contract](../progress/mapgyver-m06-model.md)
is accepted; degree-based Rotation now uses Placement v2.
New handles/anchors, bounded independent panels and complete native/editor acceptance retain
their separate gates. See the [status ledger](../progress/mapgyver-status.md).

Verification for the Matrix ownership/retirement foundation (2026-11-03):

- Library: **315 passed, one existing ignored**, including generated-contract drift, all 33
  command kinds, four caller paths, typed/copy/project/history, feedback and outgoing fades.
  Physical-copy replacement/removal, uncaptured launch epochs,
  derived retirement, atomic channel/element capacity rejection and Picture leases are covered.
  Unavailable default/Alt drops, live automatic/pinned outputs, nil-output feedback, four
  caller paths and typed commands are covered. Golden's parameter suite passed 27 tests and
  its persistence suite passed 15, including the dynamic-default clearing regression.
- Strict all-target MapGyver Clippy with `native-qualification` and Golden engine lib/tests
  Clippy passed; both workspace formatters ran.
- MapGyver Svelte reported zero errors/warnings; the ordinary bundled executable built.
- `tests/headless/media.mjs --matrix-owned-media-ui --owned-media-ui` passed: actual manager
  drags, default/Alt copies, generic Cell/Placement Inspectors, independent variants,
  launched-cell replacement Undo/Redo and Save/New/Load. `--unavailable-reference-ui` also passed
  real missing/unconfigured drops, independent copies, generic Target/Output widgets, automatic
  recovery, pinning/reorder and project roundtrip. The isolated headless host exited normally.
- Automatic Reference aliases no longer consume render-pass admission credits; the retained
  five-pass bounded Matrix test still rejects the extra group scratch pass without raising budgets.

These foundation software checks are supplemented by
[native Reference acceptance](../progress/evidence/mapgyver-reference-native-2026-11-03.json):
all three source kinds pass the complete matrix on Intel Graphics and NVIDIA RTX 5080,
including removed-copy playback and Unload/final-use retirement. M06/M07 redesigned editors
and whole-app resource/performance qualification retain their separate gates.
