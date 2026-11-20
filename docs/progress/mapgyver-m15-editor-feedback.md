# Live editor feedback (M15/5)

The shared viewport interpolates hover and selection paint over 120 ms. With native video,
the contour and image share one presentation frame. The contour keeps its paint history
by object identity when the Matrix Cell drawing order changes. Keyboard focus draws a ring
without a second fill over the native selection.

The November 20 capture found a double selection tint in Composition and Matrix Layer.
The highlighted red test image had a blue value of 78 there and 67 in Screen Warping and
Surface Source. Switching the preview between Matrix Cells also exposed a Svelte state-write
error when reordering a focused contour dispatched blur during a template update. Golden's
contour action now dispatches its hover and focus callbacks without inheriting that update's
reactive scope.

[Original delivery and earlier checks](evidence/mapgyver-m15-editor-feedback-2026-11-17.json)

## Verification on November 20

- Owned Windows app captures showed intermediate hover colors in Screen Warping, Surface
  Source, Composition, Matrix Layer and Matrix Cell at 125% scale on Intel Graphics and
  NVIDIA RTX 5080 Laptop. Screen, Source, Composition and Layer also showed intermediate
  selection fill, cleared after rapid retargeting and jumped directly to the final fill
  in reduced motion. The selected red image reached the same blue value of 67 in all four.
- The native overlay lease stayed active throughout each recording and the settled paint
  received a fresh installation acknowledgment. The Cell editor retained its selected image
  while a peer was hovered, then installed that peer's preview on keyboard selection.
  The app recorded zero page errors or rejected Matrix intents. Selection and paint left
  authored settings unchanged.
- The bundled Cargo build, MapGyver UI type check, 32 Golden viewport tests, 71 Golden video
  UI tests and three overlay contract tests passed. Captures used owned windows with physical
  output disabled.
- The geometry regression verified held Composition move, resize and Escape, Source crop
  actions and typed Matrix Layer/Cell edits in a 131-by-131 sparse Matrix. Captured guides were
  within one physical pixel of the moving Composition image. The Matrix navigation recheck
  kept image and outline within 2.5 pixels during Frame, Home and a dock resize.

Receipts in `target/`: `mapgyver-m155-focus-build-183235.log`,
`mapgyver-m155-final-matrix-nvidia-183458.log`, `mapgyver-m155-final-shared-nvidia-183513.log`,
`mapgyver-m155-final-matrix-intel-183529.log` and `mapgyver-m155-final-shared-intel-183545.log`.
Geometry and navigation receipts: `mapgyver-m155-geometry-183613/result.json` and
`mapgyver-m155-matrix-nav-183705.log`.
