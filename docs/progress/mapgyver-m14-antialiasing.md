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

## Sustained Shared Texture timing — 2026-11-20

The optimized fixture completed all four serial cohorts through Spout. Each used the same two
Surfaces (official calibration grid and translucent Solid Color), a 30-second warmup and three
120-second samples at 60 FPS. All processes exited 0, with complete CPU and GPU event coverage.
The table reports the worst percentile and the range of per-sample deadline counts.

| GPU | Output | CPU p95 / p99 (ms) | GPU p95 / p99 (ms) | On time per 7,200 slots | Verdict |
| --- | --- | ---: | ---: | ---: | --- |
| NVIDIA RTX 5080 Laptop | 1080p | 0.35 / 0.40 | 4.34 / 4.63 | 7,200 | Pass |
| NVIDIA RTX 5080 Laptop | 4K | 0.42 / 0.49 | 14.18 / 14.50 | 7,200 | GPU p95 over budget |
| Intel Graphics | 1080p | 0.53 / 0.84 | 38.67 / 39.86 | 3,308–3,318 | GPU and deadlines over budget |
| Intel Graphics | 4K | 0.70 / 1.17 | 151.12 / 153.45 | 827–829 | GPU and deadlines over budget |

Limits remain CPU p95/p99 4/8 ms and GPU p95/p99 12/16.667 ms. Deadline counts measure Spout API
submission. All four stayed within the 512 MiB or 1 GiB resource cap and retired to zero tracked
textures and endpoints within 368 ms. On Intel 4K the render stage was largest in every sampled
batch, with up to 147.01 ms p95 measured in that stage. Stage spans include GPU waits and
scheduling. Physical outputs were disabled. [Local analysis](../../target/mapgyver-4x-sustained-210115/analysis.json)

The fixture executable SHA-256 was `10666BA588489DC8EC795F1A5E51955A0588FFEF62C54C35153D45313549A2F2`.

## Direct source-over compositing — 2026-11-25

Golden Video uses D3D11 source-over blending for bounded Normal layers on linear
premultiplied RGBA8 targets. The same source eligibility rule controls allocation and
admission. Other blend modes and inputs that can exceed UNORM bounds retain shader blending.

- The optimized build, 381 Golden Video tests and 126 focused MapGyver Screen and planning
  tests passed. The MapGyver UI check reported zero errors and warnings. Clippy passed with
  the existing `type_complexity` finding in `screens/catalog_runtime.rs` left at warning level.
- The independent MapGyver raster fixture matched all 108 Shared Texture and preview images
  on Intel Graphics and the GeForce RTX 5080 Laptop GPU within its existing conversion bounds.
  It covered warping, alpha, history, reload and forced tiling with physical outputs disabled.
- The opt-in Golden Video suite passed 30 tests, including exact native source-over pixels
  through a partially covered translucent draw. Two additional tests failed. The external HAP
  test required `MAPGYVER_HAP_EXTERNAL`. The initial allocation recovery test timed out before
  its first draw while waiting for the deliberately unavailable adapter to enter `Faulted`.
- The two-Surface 4× resource fixture measured 182,477,056 generation bytes at 1080p and
  763,085,056 at 4K on both GPUs. This saves one 126.6 MiB scratch texture per generation
  in each case. Raw/2× and 4× tests rejected over-budget plans before allocation, kept the
  active output advancing and retired tracked textures and endpoints to zero. The longest
  4× retirement was 55.01 ms. [4× results](../../target/mapgyver-4x-direct-resources-20261125-2135/results-4x.json)
  · [Raw/2× results](../../target/mapgyver-4x-direct-resources-20261125-2135/results.json)

## Sustained comparison — 2026-11-25

The same Shared Texture fixture completed the four cohorts with a 30-second warmup and three
120-second samples per cohort. CPU and GPU coverage was complete. CPU p95/p99 stayed within
4/8 ms, memory stayed within the original caps, and tracked textures and endpoints retired to
zero within 272 ms. The GPU limits remain 12 ms p95 and 16.667 ms p99.

| GPU | Output | Prior GPU p95 (ms) | Direct GPU p95 / p99 (ms) | On time per 7,200 slots |
| --- | --- | ---: | ---: | ---: |
| NVIDIA RTX 5080 Laptop | 1080p | 4.34 | 8.82 / 8.96 | 7,200 |
| NVIDIA RTX 5080 Laptop | 4K | 14.18 | 13.20 / 14.14 | 7,200 |
| Intel Graphics | 1080p | 38.67 | 32.93 / 34.26 | 3,877–3,971 |
| Intel Graphics | 4K | 151.12 | 123.49 / 124.68 | 989–992 |

NVIDIA 1080p still passes with a higher GPU p95. NVIDIA 4K improved and still exceeds GPU
p95. Both Intel cases improved and still exceed the GPU and submission limits.
[Local analysis](../../target/mapgyver-4x-direct-sustained-20261125-2138/analysis.json)

## Remaining qualification

M14/8 remains **Active**. Return with a change that reduces work in the supersampled render
stage for NVIDIA 4K and both Intel sizes. Recheck independent pixels, preserve NVIDIA 1080p's
timing acceptance and compare the remaining cohorts to the same limits. Unsupported-hardware
rejection and physical output/recovery remain open. The earlier M12/8 Intel 4K result and user
acceptance remain recorded.
