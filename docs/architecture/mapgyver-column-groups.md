# Matrix Column Groups

MapGyver owns the Column axis in `apps/mapgyver/src/matrix`. A Column Group is a real
`mapgyver.matrix.column_group.v1` node under Matrix Final or another Column Group.
Its ordered domain children are Columns and nested Column Groups. Layer Groups remain
the separate compositing hierarchy; neither axis can be inserted into the other.

`column_group_node.rs` owns intrinsic typed App Control children. The existing shared
application definition, conditional parameters and normalized runtime remain the only
application configuration model. The group UUID is its application consumer identity.
Fresh controls default to Normal; explicit Hidden is authored in the same typed parameter.

`column_axis.rs` walks the actual tree once to derive stable flattened Column order and
each group's half-open Column range. These ranges and sibling ordinals are read projections,
never parallel authored lists. Empty groups retain their real sibling position. Ownership
resolution follows at most 32 groups, independently of Matrix size. The existing 100,000
document-element limit includes groups, Columns, Layers and materialized Cells.

`column_groups.rs` implements creation, rename, reparent/reorder, ungroup and removal.
Cell addresses remain stable Column IDs when topology changes. Ungroup moves existing
children in one transaction; it does not recreate Columns or Cells. Removal also removes
Cells addressed to descendants. `copying.rs` duplicates the group forest and sparse Cells
together, then remaps typed references through the existing bulk duplication boundary.
All operations use the public Matrix intent contract, including scripts and transport.
Rust DTOs generate the TypeScript identities, edits and snapshots.

`application_axis.rs` caches one immutable application projection shared by logical demand
and render capture. Each configured group contributes one half-open range over flat Columns;
it does not copy a binding into every child. Empty groups contribute no demand. Pre/post
distances extend the range by flat Column count, clamp at axis ends and never wrap. A Column
cursor inside the range preserves its preparation/active/outgoing reason; outside neighbors
contribute preload demand without being displayed. Group, Column and Cell associations share
the existing 4,096-binding admission bound.

Column-origin Layer requests retain their group ancestors, including when a disabled
destination preserves a previous request. Independent Cell triggers activate their Cell
controls without activating Column or group controls. This preserves the existing independent
trigger contract. Retained render clips pin their prior application references across edits,
regrouping and deletion; captured outgoing plans keep their final-use references. New captures
release those references after transition settlement. The normalized application runtime owns
process sharing and stop/re-entry; the Matrix does not launch or supervise a second process path.
[Demand/lifecycle evidence](../progress/evidence/mapgyver-m12-group-demand-2026-11-05.json)
covers real owned nested-group sharing, 20 unload/reload cycles, five stopping re-entries,
reload/delete/Undo and shutdown, alongside software range and retained-plan checks.

M12/4 remains partial delivery until bounded group grid/contextual authoring, actual group
Inspector/default-window and requested/submitted/stopping marker checks, and the complete
native ownership/recovery scope in the [implementation plan](../plan/MapGyver_Implementation_Plan.md)
pass. Backend topology and demand checks do not alone establish full actual-app acceptance.
