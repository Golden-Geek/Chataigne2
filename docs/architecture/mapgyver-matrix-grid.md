# Matrix scrolling grid

MapGyver owns Matrix hierarchy and insertion policy in `apps/mapgyver/src/matrix`.
`creation.rs` resolves `CreateAxisItem` against the committed document. Column/Layer Groups
receive children; selected Columns and Layers create siblings in their current parent.
Cell and owned child-media selection resolve both axes from the real Cell address. Unrelated
live selection uses the root; missing or excessively deep context rejects. Existing typed
creation operations own defaults, labels, validation and one-step history for every caller.

The app grid under `apps/mapgyver/ui/src/lib/matrix` is a scrolling table. It has no canvas
Frame/Home/Zoom toolbar. `GridCreationHeaders` supplies trailing Column/Column Group and
Layer/Layer Group menus, including when either axis is empty. The UI sends selected node
identity to the backend and selects the returned committed object.

`viewport.ts` owns viewport-dependent presentation geometry. Small tables expand Layers and
Columns into the available area; Groups retain single-line 2.1 rem headers. Large tables
retain minimum 7.5 rem Layer/Column spans and compress
scroll ranges below browser layout limits. Trailing creation headers occupy explicit space.
The same projection drives mounted cells, Column Group bands and native preview tiles.
Measured overlays do not contribute to the extent they measure. Stable scrollbar gutters
avoid a scrollbar/resize feedback loop. Read windows stay bounded to 64 rows and 32 slots;
empty addresses remain unauthored and selected Inspector data remains independently queried.

`GridRowHeader` owns row labels, selection, rename, Stop and Group expansion. Groups hide
inline Opacity/Blend Mode; those same typed Parameters remain in the generic Inspector.
`RowControls` projects each mounted Layer's existing `parameters/opacity` and
`parameters/blend` nodes through Golden's public Parameter editor registry. It resolves
UUIDs through the graph index and traverses only declaration children, never the Layer's
Cell collection. The Inspector and inline widgets therefore share identity, constraints,
control mode, edit sessions and history. A labelled group contains each widget; an HTML
label around the slider would activate its numeric input after dragging and could create
an extra rounded-value edit on blur. Inline gestures cannot initiate a row drag.

The cached backend view index supplies visible row ordinals and preceding Group counts.
Optional `MatrixViewport.row_layout` carries viewport-dependent row heights and scroll offset;
the backend finds the bounded window by binary search. `rowOffset` uses the same prefix counts
for DOM hit targets, native preview tiles and trailing creation headers. No complete row list
is transferred to determine mixed-height scrolling.
Panel-state normalization preserves and validates that physical query when Dockview reapplies
the panel's saved state; dropping it would replace the current mixed-height projection.

Group expansion edits Golden's existing `presentation.collapsed` metadata. A Group listens
only to its own metadata and invalidates the cached view when that flag changes; color edits
do not rebuild the index. Collapse removes descendant grid rows and preview demand while
independently selected objects remain inspectable. Authored Group settings, playback/render
configuration and source ownership remain independent of expansion.

Row and Column header backgrounds select their real document objects, including through
Enter and Space. Embedded rename, launch and Parameter controls retain their own input
handling. Layer and Group titles sit at the top left, with hierarchy indentation applied
to the title rather than reducing the controls' available width.

Grid selection paint reads Golden's explicit document selection by UUID, independently of
the retained row/Column edit address. A selected Cell does not select its headers; labels
have no additional selection outline. Golden's modifier policy handles additive/toggle
selection, shared across panels. Visible objects read `presentation.color` through the graph
index and Golden's color resolver; the grid never stores a separate authored color or scans
Cell collections. Layers, Columns and Groups declare color-edit permission; Cells inherit
the ordinary user-item permission. Inspector color edits, history, copies and project loading
therefore use the same metadata as the outliner. Playback markers remain separate.

`grid-appearance.css` scopes Matrix card corners, quiet borders and short active-state
transitions. Golden's DOM contour action reads optional `--gc-viewport-contour-idle`,
`-hover`, `-selected` and `-invalid` colors, retaining its established fallbacks elsewhere;
SVG/native contour paint is unchanged. Cell elevation follows the renderer's explicit
per-Layer submitted columns, never the whole-Column union. Reduced motion removes transitions.
Corners and shadows do not change Cell hit bounds, thumbnail tile geometry or preview ownership.

Layer and Group sibling order in the document and grid is front to back. The Matrix capture
boundary reverses each immediate sibling set for Golden's back-to-front compositing draw
order. Nested Groups apply the same rule locally and remain one composited input in their
parent. Reordering changes the authored hierarchy; the UI never reverses a separate copy,
and Layer-internal transition inputs keep their independent additive playback order.
The Matrix Layer canvas also paints SVG hit regions back to front, using the backend's
row ordinals for visible and independently selected offscreen Layers. Selection cannot
silently promote a back Layer above a front Layer in overlap hit testing.

Empty Cells show a subtle outline. Add and X removal actions appear on pointer hover or
keyboard focus, retaining accessible labels, tooltips and context-menu equivalents.
Golden's public `registerActionIcon` lets the app choose the same X for built-in node menus
without changing other applications' default removal icon. Composition and Matrix actions
use that supplied asset; paired Matrix objects still remove through app-owned intents.

Behavioral tests cover backend context, grouped history, project replay, all public callers,
compressed/adaptive geometry and shared native tile spans. Native acceptance exercises real
empty/small/overflow grids, keyboard actions, resize stability, group drag/rename/history,
offscreen Inspector editing and the sparse 1000×1000 preview lifetime.
