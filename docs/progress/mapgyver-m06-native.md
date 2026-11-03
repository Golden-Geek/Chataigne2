# M06/3 native Composition acceptance

MapGyver owns captured child-media transforms and shared channel plans. Golden owns D3D11
execution, native preview composition, output publication and completion-safe retirement.
**Accepted 2026-11-03** on Intel Graphics and the RTX 5080 Laptop GPU. See the
[native receipt](evidence/mapgyver-composition-native-2026-11-03.json).

The existing fixed `composition_pixels::native_nested_composition_pixels_cover_transforms_blends_resize_missing_undo_and_reload`
test drives the production `ChannelRenderer` and native `PreviewController`. Three 1×1 alpha
sources feed nested 8×4 and 32×24 Compositions, with actual owned Reference children. Two
independent native outputs share the root; a third readback observes the real preview compositor.

| Original M06/3 requirement                                  | Observed result on both hardware adapters                                                                                                                                                                                                                                                |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Nested/shared output and one production per demanded output | Five distinct produced targets remain shared across six Reference aliases and both root outputs. The native assertion preserves five, rather than counting 11 logical records as separate render production. M06/2 independently proves exactly five producing passes for those targets. |
| Alpha and blends                                            | All complete output images pass the independent premultiplied linear-light raster/UNORM oracle for Normal, Add, Multiply and Screen, with source alpha 0.5/0.75/0.25 and independent instance opacity.                                                                                   |
| Pivots, signed/nonuniform scale and degrees                 | The original asymmetric pivot and mirrored 90° case passes, followed by an independent −90°/nonuniform-scale/pivot change. The degree-based production renderer agrees with the independent inverse-affine oracle.                                                                       |
| Canvas resize                                               | The authored canvas changes to 64×48 while consumers retain 32×24 output size; exact resampled output and preview pixels pass.                                                                                                                                                           |
| Missing source and Undo                                     | Removing a shared source omits its contribution without losing healthy sources; Undo restores the complete image. The output, preview and production-sharing assertions remain active.                                                                                                   |
| Project/recreation and retirement                           | Sparse load into a fresh native renderer reproduces the final image. Every fixture closes preview and renderer, waits for completion, and asserts zero live plan allocations, created=released, and empty native source/output inventories.                                              |

Final result: **1 passed in 44.42 seconds**, **54 complete images / 41,472 texels**, nine stages ×
three readbacks × two GPUs. Maximum nominal error and maximum error outside the oracle's
legal quantization interval were both **1/255**, within the original **2/255 residual tolerance**.
The owned test exited zero; no test process remained.

## Oracle reconciliation and cumulative attempts

M06/1's two earlier attempts stopped at stale source-binding and metadata-count assumptions,
without pixel acceptance. M06/2's new compiled five-producer/six-alias proof supplied the recorded
return trigger and resolved the producer-count decision before this hardware batch resumed.

This batch's baseline passed both native Composition outputs in the first scene, then failed
the preview oracle at texel 0: actual `[0,0,137,255]`, old nominal `[13,13,138,255]`. Inspection
of `preview/layout.rs` and its existing
`native_active_areas_are_black_even_without_sources_while_the_workbench_keeps_its_theme`
test established the current product contract: active apertures are black; the 0.0075 linear
workbench theme is outside them. This fixture's aperture covers its entire preview output.

The independent oracle's preview backing now uses black. Its source images, geometry, blending,
quantization propagation, residual tolerance and runtime budgets are unchanged. A dedicated
software assertion checks black backing without theme bleed; a separate half-step assertion
retains the precise 0.6-ULP conversion rule. This corrects an obsolete expected backdrop rather
than changing production behavior or enlarging the error allowance.

After software/lint success, the single corrective full native run passed all cases. Cumulative
attempts are **four**: the two recorded M06/1 failures, this backdrop baseline, and the final pass.
No additional probe, reduced scene, budget revision or partial native gate is substituted.

Checks and reproduction:

- Retained prerequisite at `0a3f0b3c`: **346 app library tests**, generated contracts and strict
  all-test native-qualification Clippy passed. Production app code is unchanged in this milestone.
- Corrected `cargo test --locked -p MapGyver --features native-qualification --test composition_rendering`:
  **6 software tests passed**, one explicit native case ignored, with current target compilation.
- Strict `--lib --tests --features native-qualification` Clippy passed after the oracle correction.
  The shared Matrix oracle consumer's software suite also passed; no Matrix native gate is awarded.
- Run the named native case after prerequisites with `--exact --ignored --nocapture --test-threads=1`.
  The final executable hash and exact commands/results are in the linked receipt.
- Both Cargo formatters and diff checks passed. No runtime/protocol/UI source changed, so prior
  generated-contract and bundled UI results remain valid.

Local receipts: `target/mapgyver-composition-native-{prerequisite,prerequisite-final,clippy,shared-oracle,qualification,qualification-final}.log`.
The baseline failure and final pass are retained separately. No large image archive is needed:
the checked-in fixed fixture compares every readback texel and retains its original assertions.

M06 is **3/5 accepted**. Next is M06/4's actual revised corner/edge resize, outside-corner rotation,
pointer/zoom and grouped-history acceptance. Independent panels remain M06/5. These finite
32×24 readbacks do not qualify whole-app throughput, GPU timing/VRAM, physical window geometry,
input latency, device removal or release readiness; their original gates remain open.
