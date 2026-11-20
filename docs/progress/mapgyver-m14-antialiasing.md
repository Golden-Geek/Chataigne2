# Screen 4× anti-aliasing (M14/8)

Screen's ordinary typed Anti-aliasing Parameter now offers Raw, Smooth (2×) and
Smooth (4×). The new choice evaluates sixteen subpixels per final pixel and resolves
through two 2:1 linear passes. Outputs and previews consume the same resolved texture.
Raw and 2× retain their existing graphs. The [architecture](../architecture/mapgyver-screen-antialiasing.md)
explains the Golden raster partition/viewport mechanism and MapGyver quality policy.
Tiling makes 4K representable within the original8192 dimension limit; it does not
establish sustained 4K performance or increase any production budget.

## Verified feature behavior — 2026-11-17

- MapGyver's complete native-feature library:732 passed,8 explicitly ignored.
  Golden Video's library:329 passed. Six independent raster-reference checks pass.
  This includes real Parameter admission, invalidation, history, script calls,
  duplication/reload, unchanged producers/output identities, integer odd partitions,
  chained resolves, and pass/dimension/byte admission. Generated Screen contracts match.
- Strict all-target Clippy on MapGyver and Golden Video, with native qualification and
  application-process fixtures, passes with warnings denied. It was repeated after the
  fixture correction below. UI check: zero errors/warnings. Both required formatters pass.
- The finite optimized native fixture passes on Intel Graphics (LUID87734) and NVIDIA
  RTX5080 Laptop (LUID88966):108 whole images/82,944 texels through two SDK outputs and
  the production preview. Raw/2×/4×, opaque/translucent warped edges, both ordinary and
  forced-tiled paths, Undo/Redo and reload pass. Maximum residual outside the existing
  UNORM conversion intervals is0 bytes; maximum nominal rounding difference is2 bytes.
  The oracle models the additional RGBA8 resolve write; older tolerances are unchanged.
  Native resource retirement passes. The unchanged120-second watchdog completes in15.76s.
- The bundled WebView Inspector passes Raw default, all three actual enum choices,
  repeated switching and unchanged endpoint/enabled metadata. Owned PID15500 exits0;
  there are no page errors. Overlay uses the user's standing Off instruction.

The first native attempt passed untiled Intel pixels, then rejected the tiled graph:
19 passes exceeded the older test fixture's16. The new explicit tiled fixture now uses
production's existing24-pass cap; original fixtures retain16. Its small64-pixel dimension
cap forces the same four-tile mechanism used by4K under8192. The corrected software
preflight passes before the one corrective native run. No app budget was raised.

Tested executable SHA-256:

- `MapGyver.exe`: `6EB04C124B0ED89548CC01EE6ABB65386F506F38C6CFD064F74BB058ED04AB89`
- `screen_antialiasing-941f95da43631156.exe`: `733BCF9BE95AF6F2048B41A359347425E0CF82CE229B58DF8C9DBA3B192728E4`

## 4K scratch allocation — 2026-11-20

- The optimized 4K NVIDIA calibration and Solid Color fixture initially rejected its two-Surface
  Screen: `1303272032` plan bytes plus `33193872` already reserved exceeded the 1 GiB cap.
  Sharing one compatible scratch texture across four scheduled composite passes saved
  `398131200` bytes (379.7 MiB). The same scene then started and sent Spout frames at
  `938334704` reserved bytes (894.9 MiB). [Local receipt](../../target/mapgyver-4x-cost-after-205834/gpu-1-3840-Smooth4x/renderer.jsonl)
- The 1080p and 4K memory fixtures passed on Intel and NVIDIA at the original 512 MiB and
  1 GiB caps. Raw → 4× → Raw delivered each generation. A second full 4× generation exceeded
  the available headroom, so the renderer rejected it before allocation and kept the active
  Spout output advancing. All four runs retired to zero live textures and no endpoints within
  37 ms. The retained Raw/2× resource fixture passed, with eight 1080p Screens now required to
  exceed the same cap. [4× receipt](../../target/mapgyver-4x-resources-205834/results-4x.json) ·
  [Raw/2× receipt](../../target/mapgyver-4x-resources-205834/results.json)
- Five planning tests, strict targeted Clippy and both formatters passed. The optimized native
  pixel test rechecked 108 images (82,944 texels) on both GPUs through two Spout outputs and
  the preview, including forced tiles, warped edges, alpha, history and reload. The largest
  error outside the reference conversion interval was zero. The tests used no physical outputs.
- The one-second NVIDIA diagnostic measured 12.98 ms GPU p95 against a 12 ms target and
  reported missed submission deadlines. It establishes startup admission and output, with
  sustained timing still open. [Analysis](../../target/mapgyver-4x-cost-after-205834/analysis.json)

## Remaining qualification

M14/8 is **functionally delivered and Active for qualification**. Next run the existing optimized
`screen_antialiasing_costs` fixture with Smooth4x at 1080p and 4K on both GPUs, using the full
warmup and three 120-second samples. Retain the supported-hardware rejection and physical
output/recovery cases. The earlier M12/8 Intel 4K timing result and user acceptance remain recorded.
