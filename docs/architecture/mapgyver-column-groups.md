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

The viewport uses derived presentation slots: each Column occupies one slot, and an empty
leaf group occupies one selectable slot without a Column or Cell. `MatrixViewport` requests
`axis_start`/`axis_count`; `column_total` remains the real Column count and `axis_total`
includes empty placeholders. Column snapshots expose their `axis_index`. Group snapshots
carry depth and slot ranges separately from playback's flat Column ranges. The cached view
index returns only the intersecting slots and their ancestors, bounded by 64 slots and 32
levels; selected Column data is resolved independently of the visible window.

`ColumnGroupHeaders.svelte` renders the bounded ancestry bands. Deep headers scroll within
at most 8 rem or 30% of the viewport height, and Cell/native-preview geometry uses the same
header height and slot positions. Group selection uses Golden's shared selection and standard
Inspector; inline names and context actions send ordinary Matrix intents. `DropColumnItem`
passes root/inside/before/after presentation targets to the backend, which resolves complete
sibling order and rejects invalid cycles. Group layout adds no authored settings or Cells.

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

The group grid and actual standard Inspector are verified in the native WebView, including
Normal/Hidden parameter history and offscreen editing. Actual group-owned Normal/Hidden window
visibility also passes. M12/4 remains partial delivery until native source/output integration and
requested/submitted/stopping marker checks complete the
[implementation plan](../plan/MapGyver_Implementation_Plan.md) scope.

Requested group IDs are projected from committed Column-origin playback requests. Renderer
plan stamps carry their own group ancestry through installation and submission; a later
request cannot rewrite an older submitted stamp. The grid shows requested and submitted
content separately from stopping/retirement-unverified process observations. One shared
application runtime reader serves Matrix groups; rows do not poll independently.

The application runtime retains released consumer identities as observation-only `Retained`
entries while the owned process retires, and removes them after ownership is fully released.
These records never enter admission/demand and cannot authorize a launch. Actual Normal/Hidden
group windows pass, but the NVIDIA external-sender first-frame check remains unqualified;
see the [current marker acceptance evidence](../progress/evidence/mapgyver-m12-group-markers-2026-11-05.json).
