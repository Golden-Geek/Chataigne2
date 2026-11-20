# Smooth shared-editor navigation (M14/2)

Golden's `Viewport2D` now animates Frame and Home over 160ms with ease-in/ease-out,
using the same displayed camera as existing pointer-anchored smooth zoom. MapGyver's
Screen, Surface Source and spatial editors inherit it. Toolbar actions and F/H use
the same functions. There is no second CSS camera or authored Parameter change.
The [canvas architecture](../architecture/mapgyver-canvas-controls.md) owns the boundaries.

Repeated identical destinations preserve their deadline. Retargeting begins at the
displayed camera; a new wheel gesture anchors there instead of the old Frame target.
Editing/panning freezes motion, and identity replacement resets Home immediately.
Reduced motion applies the same target directly. Existing size/disposal cancellation,
selection framing and bounded backend lookup are preserved.

The actual-app check exposed a normal shortcut defect: Golden's global command capture
prevented F/H before the local viewport handler could see them. Each viewport now registers
those commands with the existing router, admitting only its own focused view and excluding
text editing/held pointer input. The browser fixture includes the real global router and a
second viewport, so the earlier standalone-only shortcut coverage cannot hide that defect.

## Verified feature scope — 2026-11-17

- Golden UI:392 tests across71 files pass, including exact easing, endpoints, repeated
  destinations, retargeting, pointer anchors, cancellation and reduced motion.
- The isolated real-input browser fixture passes intermediate Frame/Home presentation,
  final selection framing, keyboard/toolbar parity, text-field isolation, focused-peer
  command ownership, pan/edit cancellation, resize and identity replacement.
- MapGyver and Chataigne UI checks each report zero errors/warnings. The current bundled
  MapGyver builds successfully through ordinary Cargo. Touched frontend files are formatted;
  root and Golden Core formatting pass at checkpoint. No Rust or protocol changed.
- The actual WebView cohort passes on Intel Graphics (`dxgi:00000000:000156b6`, PID52676)
  and NVIDIA RTX5080 Laptop (`dxgi:00000000:00015b86`, PID38844). Screen Warping, Surface
  Source and Composition each present intermediate zoom/Frame/Home cameras, preserve the
  pointer anchor and keep native-underlay aperture/overlay DOM rectangles aligned. Home
  reaches exactly1; domain input cancels motion; camera navigation leaves authored Screen
  data unchanged. Same-UUID New/Load resets obsolete camera state. Both owned apps exit0,
  with no page errors/rejected intents and their requested native GPU observed through
  public runtime status. These are DOM/layout checks, not native-window pixel captures.

The older app fixture now uses the current Cargo observer, exact InstanceId, unique owned
Surface name, stable panel IDs and explicit crop selection. Its setup failures are not
accepted evidence; the successful cohorts run all three editor cases without skipping them.

Bundled app SHA-256:
`F7ED398449F6BC7CB53F87DED500FCEE36AA977700FC0A5C3855AF2950F69266`.

## Second-pass verification — 2026-11-20

M14/2 is **Accepted**. Next, qualify visible shared editor feedback and matching installed
paint frames under M15/5.

- The original Matrix Layer Frame capture found up to 32 physical pixels of separation
  between the image and outline. Composition and Matrix now submit their outlines, handles
  and guides through the same native overlay path as Screen Warping and Surface Source.
  The image and guides use the same presented camera. DOM hit regions, keyboard focus and
  the browser fallback remain available.
- The bundled app passed Frame and Home on NVIDIA RTX 5080 Laptop and Intel Graphics in
  Screen Warping, Surface Source, Composition, Matrix Layer layout and Matrix Cell content.
  Window capture measured intermediate image sizes and a maximum 2.5 physical pixels of
  separation at 125% Windows scale across those views.
- Both GPUs framed the selected Layer beyond the first 128-item page in a sparse 132-Layer,
  64-Column Matrix with one Cell. Moving the real dock divider during Frame cancelled the
  animation at its displayed camera. Navigation left the authored Matrix unchanged.
- The shared viewport checks preserved pointer-anchored zoom, edit interruption, reduced
  motion and same-UUID project replacement. The bundled Cargo build, MapGyver UI check,
  32 shared viewport tests and three overlay tests passed. Existing NVIDIA app checks also
  passed Composition rotation, keyboard and pointer geometry, Matrix Layer and Cell resize,
  grouped Undo/Redo and Escape.
- The browser fixtures suppressed fullscreen and rectangle windows. Separate latency,
  high-refresh synchronization and physical-output qualification remain in their own gates.

Receipts in `target/`: `mapgyver-m142-matrix-nvidia-175737.log` (original failure),
`mapgyver-m142-fixed-navigation-pixels-nvidia-181154.log`,
`mapgyver-m142-fixed-navigation-pixels-intel-181228.log`,
`mapgyver-m142-fixed-matrix-navigation-nvidia-181256.log`, and
`mapgyver-m142-fixed-matrix-navigation-intel-181213.log`.
Gesture receipts: `mapgyver-m142-fixed-composition-nvidia-181453.log`,
`mapgyver-m142-fixed-matrix-layer-geometry-nvidia-181310.log` and
`mapgyver-m142-fixed-matrix-cell-geometry-nvidia-181542.log`.
