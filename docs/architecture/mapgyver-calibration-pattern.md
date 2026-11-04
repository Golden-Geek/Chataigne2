# Surface calibration pattern

MapGyver owns the calibration layout and palette policy in
`apps/mapgyver/src/screens/calibration.rs`. Golden Video owns checked, app-neutral procedural
geometry in `plan/canvas.rs` and its D3D11 implementation. App names, branding and calibration
choices stay outside Golden.

The current geometry slice replaces the Grid calibration image with alternating grey square
cells, second-row brightness/color palettes, a fitted circle, both diagonals and a diagonal
rainbow hue flow along thin grid boundaries. `ColorBars` remains the independent plain palette.
The centered supplied 512x512 logo is an immutable Canvas bitmap. Subtle A1/B2 cell labels
follow the grid, local system time (`HH:MM:SS.mmm`) sits above the logo, and the logical output
resolution and measured renderer FPS sit below it. Complete native app qualification remains
required before the official-pattern gate closes.

Layout uses the actual Screen render-pass extent. The complete logical image is projected
through the Surface homography, so changing a warp does not silently change the source pattern's
logical aspect. Resizing the output rebuilds its layout. Calibration retains the authored
source/crop; Off returns to that routing. It requires no source or receiver while active.

A canvas contains at most 32 checked rectangle, checkerboard, grid, ellipse, segment, palette, image and text
primitives. Golden evaluates them in order within one procedural compositing draw, avoiding
one full-image ping-pong pass per shape. Geometry is prepared outside the engine actor. Each
render submission samples the existing shared monotonic time; rainbow phase wraps before
conversion to floating point. It never advances a clock, installs another timer or polls the
engine. Static display lists share immutable storage across frame samples.

Admission charges every canvas primitive against the existing dependency/work limit. App
preparation reserves the same count before selecting derived sources. Shared display-list
storage is charged once per render plan. The native pattern constant buffer grows from 160 to
15,008 bytes (14,848 additional fixed bytes per renderer owner), remains bounded, and is included
in the existing pipeline resource charge. GPU, producer and work limits are unchanged.

Text uses a compiled Basic Latin bitmap font from the public-domain
[font8x8 crate](https://docs.rs/font8x8/0.3.1/font8x8/). Golden admits printable ASCII lines of
at most 64 characters or at most 1,024 row-major labels of eight characters each. A complete
canvas has at most 8,192 character bytes, including reserved dynamic runs; tile counts and
finite glyph geometry are checked before submission. Text storage participates in CPU input
admission. One indexed label lookup per pixel avoids a draw or loop over every cell. The font
requires no media producer, runtime font object or per-frame image upload.

The native render worker samples `GetLocalTime` once per submitted frame. It formats live text
into fixed stack storage and measures successful renderer submissions over monotonic windows
of at least one second. FPS is unavailable until a window completes and resets after idle gaps
or clock replacement. This is renderer cadence, not configured refresh rate, source FPS or a
claim that every output presented every frame. Rainbow animation continues to use the shared
monotonic clock independently of wall-clock changes. Neither text nor telemetry starts an
additional timer or creates render demand after the final consumer closes.

The bitmap is a drawing asset, not a media producer, receiver or authored project object.
Canvas construction requires an image operand for image primitives and rejects unused operands.
The graph charges distinct bitmap pixels against its existing texture-byte limit and CPU
submission storage; shared instances are counted once. Native generations share one uploaded
texture while holding the immutable CPU identity through completion-based resource retirement.
Sampling reuses Golden's declared color/alpha/filter contract, including premultiplication and
sRGB decoding before linear interpolation.

The app preparation owner memoizes only a weak image reference. Repeated warp edits reuse live
pixels, while the last plan/native resource releases them. PNG preparation is limited to a
512x512 RGBA result (1 MiB) and 2 MiB codec workspace; it runs at preparation, never on the actor.
The generic 64 KiB CPU submission fixture remains unchanged and explicitly rejects the logo's
larger retained input. The image-bearing calibration routing test declares 2 MiB per submission
and 8 MiB aggregate retention; its GPU/work/producer limits are unchanged. These are functional
fixture capacities, not revisions to retained production timing, growth or device qualification.

Deterministic tests cover supported aspects, palette rows, circle/square proportions, extreme
extents, admission, retained storage and shared-clock sampling. Native kernel qualification
checks geometry, palette and clock landmarks at landscape, portrait, square and 4K extents.
Current results and remaining official-pattern acceptance belong to the
[active ledger](../progress/mapgyver-status.md).
