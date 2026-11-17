# Screen anti-aliasing

MapGyver owns the typed Screen choice and final composition projection. Golden owns generic
composite/sample operations, linear premultiplied filtering, texture validation and native
allocation/retirement admission. Source producers and media sampling are shared and unchanged.

The selected M12/8 approach is **Raw** by default, or **Smooth (2×)**. Raw retains the existing
single Screen composite pass exactly. Smooth composites the same normalized Surface mappings
and procedural calibration directly into a target twice the output width and height, then
uses one linear Sample pass to resolve to the exact authored output extent. At this integer
ratio, each final pixel averages its four intermediate texels. Golden decodes and premultiplies
before interpolation; both intermediate and final targets are linear premultiplied RGBA8.
This reuses the existing shader contract instead of adding an app-specific Golden operation.

M14 adds **Smooth (4×)**: four samples on each axis, or sixteen shader evaluations per final
pixel. Two linear 2:1 Sample passes resolve all sixteen contributors; a single 4:1 bilinear
sample would omit twelve of them. Both intermediate writes retain the same RGBA8 contract.
Raw and Smooth (2×) keep their original graph and admission behavior.

Golden's bounded integer raster partition and viewport homography support 4× at 4K without
raising the production 8192-pixel texture-dimension limit. A 3840×2160 Screen uses four logical
1920×1080 tiles, each rasterized at7680×4320, resolved through3840×2160 to1920×1080, then copied
into its exact final pixel rectangle. The assembly uses replacement blits, with no overlapping
alpha blending or seam interpolation. Odd extents partition integer pixels exactly. Sources and
logical calibration dimensions remain shared; tile-local coordinates evaluate the original
full-output mapping. At1080p one tile needs three passes; at4K four tiles plus assembly need
thirteen. Existing aggregate pass, dependency, byte, native residency and GPU retirement limits
can reject this more expensive choice; there is no hidden quality downgrade or budget increase.

For one Surface, untiled4× plan textures total21 times the final pixel count (16+4+1).
Tiled4K adds a separate final assembly target, totaling22 times the final count before producer,
conversion, scratch and retiring allocations. Sustained cost and retained overload/recovery
qualification stay in the second pass; implementing this option does not accept those gates.

Calibration layout, stroke widths and resolution readouts use the logical final extent;
the higher raster density changes sample coverage, not authored dimensions or displayed
resolution. Calibration is generated in the high-resolution pass rather than upscaling a
previously rasterized pattern. Physical outputs, Shared Texture outputs and editor views all
consume the same resolved Screen texture. Source textures are neither duplicated nor supersampled.

Compared with Raw's one target, Smooth describes five times its pixel storage (four-times-size
intermediate plus final target). A multi-Surface composite also needs Golden's existing ping-pong
scratch: two high-resolution working targets plus the final target, before output conversion,
source pools, caches and concurrently live/retiring generations. These are costs to admit and
measure, not new budgets. Checked dimension arithmetic, plan dimension/pass/dependency/byte
limits and native allocation admission remain authoritative. Rejection is visible through the
existing runtime outcome; no switch back to Raw or budget increase is permitted silently.

Per-Screen final and intermediate logical identities remain stable across parameter edits;
switching to Raw removes the intermediate pass/storage from the demanded graph. Every authored
choice lives on a real enum Parameter, projects through Rust-generated DTOs, and uses normal
history, duplication and persistence. There is no UI-owned setting or output-specific fork.

Golden's generic procedural payload uses one worker-owned indexed shader buffer. FXC lowered
constant-buffer aggregate selection into a full 32-entry mask/merge scan inside every pixel's
primitive loop; even a flat constant array retained that problem. A read-only raw buffer makes
each required row a direct GPU load. The 15,008-byte packed header/palette/primitive/text prefix,
single Canvas draw and raster math remain unchanged. A view
is created before submission, unbound after each draw, and retained with the render worker through
GPU retirement. Compile-time offsets and full-capacity native pixel checks guard the byte layout.

Canvas additionally prepares a fixed 16×16 spatial mask on its rendering worker. Each mask
contains the potentially covering primitive indices; the shader visits set bits in ascending
authored order. Bounds are conservative for rectangles, text runs, finite lines and ellipse
outlines, so this skips evaluations without replacing pixel coverage, filtering or blending.
Dynamic text uses the same render-boundary bytes/geometry as the packed frame. No actor work,
source producer or per-pixel CPU processing is introduced. The mask adds exactly 1,024 bytes
to that same raw buffer: 16,032 bytes total payload and 16,272 bytes of fixed Pipeline buffers.
Native admission charges the complete size, including concurrently retained resources, under
the existing limits. This implementation does not itself establish the retained timing gate;
the ledger must record measured acceptance separately.

Microsoft documents that MSAA shares shader results across samples and does not remove surface
aliasing. That is why this path supersamples shader evaluation as well as warp boundaries:
[Direct3D rasterization rules](https://learn.microsoft.com/en-us/windows/win32/direct3d11/d3d10-graphics-programming-guide-rasterizer-stage-rules).

This records the implementation decision, not M12/8 acceptance. Required remaining proof includes
Raw parity, edge/alpha/text/palette quality, live/history/projects, physical/Shared Texture parity,
both GPUs, measured 1080p/4K costs and overload/recovery under unchanged M03/M11 limits. The
[current ledger](../progress/mapgyver-status.md) owns gate state.
