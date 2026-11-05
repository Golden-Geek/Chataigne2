# Matrix Cell placement

MapGyver owns Cell placement policy in `apps/mapgyver/src/matrix`. Golden supplies typed
Parameters, document edits/history, checked render geometry and GPU execution. Media Resolution
continues to describe the source texture; Cell Size describes its destination in Layer pixels.

`CellNode` declares optional Size (`Vec2`, whole pixels in 1..16384 per axis) and Fit Mode
(`Fit`, `Fill`, `Stretch`) alongside Position, Opacity and Enabled. Size is disabled on new Cells:
their placement follows the current source extent. Enabling Size authors a fixed destination;
disabling it restores natural sizing without discarding the stored dimensions or Fit Mode.
The standard Golden Inspector edits these same Parameters. There is no UI-owned sizing state.

The Rust `CellSettings` projection contains `size: Option<Extent2D>` and the existing app-owned
`MediaFitMode`; Rust generates its TypeScript contract. ConfigureCell validates first, then
updates values and Size metadata in one app-operation history entry. An absent sparse address
is inserted as one detached configured subtree. The constructor's Size seed only initializes
Parameter metadata; persisted values and enabled state belong to the Parameter nodes.

Render capture retains placement with the source binding. Position, Size, Fit Mode and Size
enable/disable invalidate the authored projection without advancing the source epoch. Live
edits follow the same source; deleted/replaced outgoing clips keep their retained placement
until retirement. Copy, save/reload and Undo/Redo use ordinary typed document state.

The media renderer shares one aspect-policy helper between output Resolution and Cell
placement. Fit centers content inside the destination, Fill crops the source centrally, and
Stretch fills the destination directly. Cell placement applies crop and quad in the existing
Layer input mapping, adding no intermediate pass. Natural size resolves against the current
prepared source extent. Layer logical-canvas placement remains Stretch, independently of its
output Resolution.

This supersedes the historical M07 no-Cell-size contract. It provides the backend/Inspector
part of M12/7. [Shared spatial geometry](mapgyver-spatial-geometry.md) adds Layer/Cell canvas
move/resize and grouped pointer/keyboard history. [Per-Layer Cell layout](mapgyver-cell-layout.md)
adds bounded navigation and inactive preview. The [complete M12/7 audit](../progress/evidence/mapgyver-m12-spatial-acceptance-2026-11-05.json)
records native input, qualified both-GPU pixels and sparse/copy/project behavior;
separate production qualification remains in the [current ledger](../progress/mapgyver-status.md).
