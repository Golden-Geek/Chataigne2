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

## Second-pass return action

M14/8 is **functionally delivered, Pending qualification**. No acceptance gate closes.
Return before M09 to the existing optimized `screen_antialiasing_costs` fixture, selecting
Smooth4x for measured sustained1080p/4K costs on both GPUs under unchanged budgets.
Complete 4× overload/live-retiring/scratch admission, supported-hardware rejection and
physical-output/recovery qualification. The older M12/8 Intel4K timing failure remains
recorded; the user's prior AA acceptance is unchanged. A reproduced ordinary-use blocker
can reopen this earlier. Do not add profiler or benchmark variants during feature delivery.

Next product deliverable: M14/2 smooth shared-editor navigation, then M15 feedback.
