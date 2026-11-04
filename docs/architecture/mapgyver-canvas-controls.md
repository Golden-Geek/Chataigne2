# Canvas controls

Golden graph UI owns the public `CanvasToolbar` and keyboard navigation command mapper.
GraphCanvas and Golden UI's `Viewport2D` compose them while retaining their own camera,
pointer and animation lifetimes. Native preview underlays remain owned by the desktop host.
MapGyver supplies contextual action snippets to the same toolbar; Composition, Screen
Warping and Surface Source no longer need duplicated navigation or internal title bars.

Matrix keeps bounded table scrolling. Home and Frame use cached global row/column indices
in the Rust selection snapshot, including when the selected object is outside the visible
window. Column positions are indexed when the cached view is built; selection lookup does
not scan the document. Toolbar clearance preserves access to the column headers.

Unselected Surfaces have a thin transparent outline and no handles or hover fill. Selection
adds a light fill, stronger outline and handles. Invalid mapping retains a red outline with
only a light selected fill; unrelated areas remain clear.

## Current acceptance

The shared controls are implemented. M11/6 remains **Active**. Software suites, native
anchored/reduced-motion navigation, Composition handles and browser Surface Source and
invalid-quad checks pass. The final native aggregate fails the Screen keyboard sequence
after Undo/Home; its later Matrix and narrow-peer checks therefore did not run. Resolve
that input/history failure and verify the corrected native Matrix toolbar placement and
narrow peer before accepting the gate. See the [receipt](../progress/evidence/mapgyver-m11-canvas-controls-2026-11-04.json).
