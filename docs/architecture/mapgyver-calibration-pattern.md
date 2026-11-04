# Surface calibration pattern

MapGyver owns the calibration layout and palette policy in
`apps/mapgyver/src/screens/calibration.rs`. Golden Video owns checked, app-neutral procedural
geometry in `plan/canvas.rs` and its D3D11 implementation. App names, branding and calibration
choices stay outside Golden.

The current geometry slice replaces the Grid calibration image with alternating grey square
cells, second-row brightness/color palettes, a fitted circle, both diagonals and a diagonal
rainbow hue flow along thin grid boundaries. `ColorBars` remains the independent plain palette.
The complete official pattern still requires its centered supplied logo, row/column labels,
resolution/FPS and system-time labels, followed by full native app qualification.

Layout uses the actual Screen render-pass extent. The complete logical image is projected
through the Surface homography, so changing a warp does not silently change the source pattern's
logical aspect. Resizing the output rebuilds its layout. Calibration retains the authored
source/crop; Off returns to that routing. It requires no source or receiver while active.

A canvas contains at most 32 checked rectangle, checkerboard, grid, ellipse, segment and palette
primitives. Golden evaluates them in order within one procedural compositing draw, avoiding
one full-image ping-pong pass per shape. Geometry is prepared outside the engine actor. Each
render submission samples the existing shared monotonic time; rainbow phase wraps before
conversion to floating point. It never advances a clock, installs another timer or polls the
engine. Static display lists share immutable storage across frame samples.

Admission charges every canvas primitive against the existing dependency/work limit. App
preparation reserves the same count before selecting derived sources. Shared display-list
storage is charged once per render plan. The native pattern constant buffer grows from 160 to
6,816 bytes (6,656 additional fixed bytes per renderer owner), remains bounded, and is included
in the existing pipeline resource charge. No existing budget is increased.

Deterministic tests cover supported aspects, palette rows, circle/square proportions, extreme
extents, admission, retained storage and shared-clock sampling. Native kernel qualification
checks geometry, palette and clock landmarks at landscape, portrait, square and 4K extents.
Current results and remaining official-pattern acceptance belong to the
[active ledger](../progress/mapgyver-status.md).
