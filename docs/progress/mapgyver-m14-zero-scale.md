# M14/4 zero-scale Composition

Updated 2026-11-16. This is MapGyver-owned placement and render-plan behavior.
M14/4 is functionally verified and awaits native pixel qualification. Overall acceptance
remains 88/106 (83%); this checkpoint does not accept the complete gate.

## Product behavior

Zero on either or both Scale axes is valid authored state. Placement Parameters and typed
Composition operations retain their finite-value checks while permitting zero, including
signed zero. Derived-layer preparation skips only the collapsed child's draw before attempting
the projective mapping. Healthy siblings continue rendering; an entirely collapsed Composition
still produces its valid empty output. A later nonzero scale recovers through ordinary
revision/capture invalidation. Selection, independent Inspector bindings, source ownership,
copy and persisted typed Parameters retain their existing paths.
Live resize presentation projects preserved source fractions around the fixed opposite anchor,
so recovering a collapsed line/point never divides by its former zero width or height.

This change belongs entirely to MapGyver. Golden's nondegenerate mapping contract is unchanged;
no Curve, Spatializer, Chataigne or generic editor feature is included.

## Acceptance scope and current checks

Required: typed operation, ordinary Inspector/direct Parameter, transport/command/script callers;
either/both-axis and signed-zero crossings; healthy siblings and all-collapsed plans; selection
and off-page Inspector access; nonfinite rejection; recovery; Undo/Redo, copy and project reload;
independent native output/preview pixels on both available GPUs.

The selected software cohort is Composition unit/caller/render-plan tests with native fixture
features, strict app/test lint, native fixture software oracle tests, scoped browser formatting,
and the ordinary app build. The existing Composition browser test now edits zero through actual
Inspector fields, retains the selected off-page item, restores signed scale and verifies exact
Undo/sibling state. The existing pixel fixture adds eight zero/crossing/recovery states to its
original nine states, including all-collapsed clearing/restoration, using independent output
consumers and the production preview compositor.
Software/build terminal success is required before running it. NVIDIA additionally requires
the original observed Overlay Off precondition; no old observation is reused for a new run.

Passing checks:

- `cargo test --locked -p MapGyver --features native-qualification --lib composition`:
  **63 passed** (`target/m14-zero-scale-composition-tests-complete-2026-11-16.log`).
- Native fixture software checks: **7 passed, 1 explicit hardware test ignored**
  (`target/m14-zero-scale-pixel-software-complete-2026-11-16.log`).
- Strict `cargo clippy --locked -p MapGyver --features native-qualification --lib --tests -- -D warnings`
  and ordinary default app build passed. Root/Core formatting and scoped Prettier passed.
- Composition presentation tests: **4 passed**; MapGyver UI typecheck: **0 errors/warnings**.
- `node apps/mapgyver/tests/headless/media.mjs --composition-review --composition-ui`:
  actual compiled-product browser/Inspector checks passed, with zero page errors/rejections
  and normal host exit 0. This includes zero on either/both axes, signed recovery, retained
  off-page selection/Inspector, exact Undo and unchanged siblings, plus the original Composition
  copy/reload/reconnect/independent-panel scopes. These are software UI checks, not physical pixels.

Default MapGyver binary SHA256:
`8640604a4f6f87f2291aacd45dfd1f1d64adc3cd95affc608f3a959bab3120f0`.
Product browser log `target/m14-zero-scale-composition-product-ui-final-2026-11-16.log`, SHA256
`fe84718ff886f8dc27fcd33a4c7ab3c85288784b1b7f799e710cad64df3254cd`.

Earlier failed logs remain preserved. Corrections were new-test readiness/identity setup,
the former zero-rejection expectation, raw engine edits sharing a UI-history fixture, and
legacy Reference ownership assertions in native/headless fixtures. Those assertions now
verify the child's own channel and its actual Reference target rather than requiring reuse
of the original source channel. No product budget or source-identity guarantee was relaxed.

**Remaining:** independent output/preview pixels on both GPUs. The fixture is built and its
software prerequisites pass. The supported desktop reader still reports native pipe error 2;
the requested current NVIDIA Overlay Off observation is pending. Return when that observation
arrives (or supported desktop access can establish it), then run the existing explicit ignored
pixel test with the unchanged 120-second watchdog and record adapter identities/pixels/retirement.
No unchanged desktop-helper repair/profiler chain is queued, and an old Overlay observation
will not be relabeled as current qualification. Other MapGyver product features can proceed.
