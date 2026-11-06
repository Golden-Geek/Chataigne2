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
resolution and measured renderer FPS sit below it. M11/10's original contract remains accepted;
the larger scalable typography is qualified separately under M12/10 in the active ledger.

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
storage is charged once per render plan. The worker-owned indexed pattern buffer is 16,032
bytes, including its conservative 16×16 spatial candidate masks. Complete fixed Pipeline
buffers total 16,272 bytes and participate in native admission. GPU, producer and work limits
are unchanged. See [Screen anti-aliasing](mapgyver-screen-antialiasing.md) for the mask contract.

MapGyver embeds unmodified Noto Sans Mono Regular and its [SIL Open Font License](../../apps/mapgyver/assets/fonts/OFL.txt);
the [asset manifest](../../apps/mapgyver/assets/fonts/README.md) pins source and hashes. The
Tauri bundle includes the notice under `licenses/NotoSansMono-OFL.txt`. Font selection and
readout/label sizing belong to MapGyver. Golden's public `FontAtlas` uses
[fontdue 0.9.4](https://docs.rs/fontdue/0.9.4/fontdue/struct.Font.html) to rasterize 95 ASCII
glyphs at a requested density on the preparation worker. It retains pixels, not a font parser.
Two sizes keep small cell labels subordinate to the larger clock/resolution/FPS readouts.
Through 4K, readout glyph cells are rasterized at twice their displayed dimensions, so Raw
does not upscale an 8×8 font or require final Screen smoothing. Outline rasterization is
limited to density changes; dynamic values only select existing glyphs.

Golden admits printable ASCII lines of
at most 64 characters or at most 1,024 row-major labels of eight characters each. A complete
canvas has at most 8,192 character bytes, including reserved dynamic runs; tile counts and
finite glyph geometry are checked before submission. Text storage participates in CPU input
admission. One indexed label lookup per pixel avoids a draw or loop over every cell. The
generic built-in public-domain pixel font remains available for explicit pixel-text uses;
MapGyver calibration uses atlas coverage for all four text primitives. Atlas text samples
linear alpha and multiplies premultiplied paint, independent of bitmap RGB. Transparent
glyph guards prevent neighboring cells and empty texels from producing halos.

The native render worker samples `GetLocalTime` once per submitted frame. It formats live text
into fixed stack storage and measures successful renderer submissions over monotonic windows
of at least one second. FPS is unavailable until a window completes and resets after idle gaps
or clock replacement. This is renderer cadence, not configured refresh rate, source FPS or a
claim that every output presented every frame. Rainbow animation continues to use the shared
monotonic clock independently of wall-clock changes. Neither text nor telemetry starts an
additional timer or creates render demand after the final consumer closes.

The bitmap is a drawing asset, not a media producer, receiver or authored project object.
Canvas construction requires one image operand for image/atlas-text primitives, rejects
unused operands and rejects opaque image metadata for alpha coverage. Image primitives have
an explicit source rectangle; text names a 16-column/six-row glyph region of that same image.
`ImageAtlas` packs the logo and both glyph sizes into one guarded RGBA image, preserving the
single upload, drawing asset and native sharing/retirement mechanisms.
The graph charges distinct bitmap pixels against its existing texture-byte limit and CPU
submission storage; shared instances are counted once. Native generations share one uploaded
texture while holding the immutable CPU identity through completion-based resource retirement.
Sampling reuses Golden's declared color/alpha/filter contract, including premultiplication and
sRGB decoding before linear interpolation.

The app preparation owner memoizes only weak image references, with at most eight density
keys. Repeated warp edits reuse live pixels, while the last plan/native resource releases
them. PNG decoding keeps its 512×512 RGBA/2 MiB codec bounds. Font input is capped at 4 MiB,
raster scale at 256 pixels/em, and each generated glyph/combined atlas at 16 MiB. App density
caps are 192 pixels/em for readouts and 64 for labels; requested native aspects through 4K
stay within those bounds. Preparation temporarily owns font parsing/raster buffers and the
packed result; it runs outside the engine/render frame loop. Each plan and live/retiring
native generation charges the actual combined image bytes under existing admission limits.
Functional fixture capacities are not revisions to retained production timing, growth or
device qualification.

Deterministic tests cover supported aspects, palette rows, circle/square proportions, extreme
extents, admission, retained storage and shared-clock sampling. Native kernel qualification
checks geometry, palette and clock landmarks at landscape, portrait, square and 4K extents.
Current acceptance results and any remaining M12/10 requirements belong to the
[active ledger](../progress/mapgyver-status.md).

The actual-app cohort captures 1920x1080, 1080x1920, 1024x1024, 3840x2160 and a 1919x1079 resize,
then live frames, a perspective warp and calibration with an assigned source. Pixel analysis
decodes local time/resolution/FPS, checks palette rows and labels, and verifies thin diagonal
rainbow flow. Initial frames after topology recreation correctly show unavailable FPS; a later
stable window measures 60.0 FPS. Off restores the retained source/crop and the full output's red
pixels; disabling the output retires its sender. An independent Media Preview may continue
rendering its selected source, so output withdrawal does not assert global renderer idleness.
The full-image observer reserves 128 MiB for receiving/staging/CPU copies of one 4K frame; the
original small-image observer keeps 4 MiB. Neither changes production or retained qualification
budgets. The original M11/10 captures use NVIDIA; reusable kernels and resource leases pass
both NVIDIA and Intel. Those older pixel-font images do not accept M12/10's changed typography
or retained physical/device/timing gates.

[M12/10 typography acceptance](../progress/evidence/mapgyver-m12-typography-2026-11-06.json)
covers forty complete actual-app images across both GPUs and Raw/Smooth modes. Clock,
resolution and measured FPS decode through all named aspects, resize and perspective warp;
274,634 sampled readout pixels, including fractional edges, match independent linear-coverage
expectations within two RGB codes. Subordinate cell labels retain their own smaller atlas.
Their oracle propagates D3D11.3's documented 0.6 integer-ULP UNORM storage choices through
the actual intermediate/resolve targets, retaining the initial ideal-rounding failure and
both result sets. The remaining residual tolerance stays two codes. This qualification
accepts typography; the [M12/8 cost gate](../progress/mapgyver-status.md) remains separate and
returns immediately on this implementation. No sustained performance is inferred here.
