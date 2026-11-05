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

The first topology milestone is partial M12/4 delivery. Aggregate group application demand,
pre/post ranges, outgoing/final-use retention, shared executable ownership/recovery checks,
bounded group grid presentation and actual group Inspector/window checks remain required.
Group configuration persistence alone does not establish execution. Keep M12/4 Active until
the complete gate in the [implementation plan](../plan/MapGyver_Implementation_Plan.md) passes.
