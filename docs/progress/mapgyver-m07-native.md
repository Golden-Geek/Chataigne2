# Matrix native pixels and scale qualification

**M07/4 Active — 2026-11-03.** Matrix owns authored structure, playback and captures under
`apps/mapgyver/src/matrix`; Golden owns bounded rendering, preview leases and completion-safe
retirement. The native suite exercises public app intents, the production renderer/preview
compositor and actual D3D11/Spout readbacks. [Model](mapgyver-m07-model.md),
[playback](mapgyver-m07-playback.md) and [grid](mapgyver-m07-grid.md) acceptance remain separate.

## Current requirement audit

| Requirement                    | Verified result / exact remainder                                                                                                                                                                                                                                                      |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Revised typed scene            | Software preflight reuses the actual three-Layer/three-Column defaults, checks six distinct owned Cell media, explicit 32×24 media/Layer extents, Group order and exact sparse project roundtrip. No implicit source stretching or extra default rows/Columns.                         |
| Typed Cell behavior            | Position, live opacity/position, Undo, disabled destination holding and enabled destination launching pass native pixels on Intel and NVIDIA.                                                                                                                                          |
| Typed Layer behavior           | Position, canvas size, source-size preservation, resize and Undo pass native pixels on both adapters.                                                                                                                                                                                  |
| Layer/Group/Final and preview  | Intel passes Normal/Add/Multiply/Screen, post-composite Group opacity, ordering/Undo, shared-source image, source deletion/Undo and transparent empty Column. The corresponding complete NVIDIA main-scene run is still required.                                                      |
| Shared production              | Intel's shared-source image passes. The checkpoint additionally asserts distinct owned Reference identities and exactly one prepared source record; this strengthened assertion is software-compiled but not yet native-run.                                                           |
| Interrupted shared-time blends | Standalone fade test passes both adapters: three three-color observations per Layer/Group/Final, conserved weights, stable outgoing ratio, monotonic progress, final blue endpoint and outgoing-demand retirement.                                                                     |
| Sparse scale                   | Both attempts author the exact 1,000×1,000 document with six Cells, then fail the preview Layout assertion before its native scale capture. The 1,000 configured requests, admission/draining/recovery and subsequent project reload/recreation are **not verified** under this scene. |
| Lifetime and bounds            | Successful Cell/Layer/fade fixture drops assert zero live allocations, balanced creation/release and empty output/source inventories. Complete main-scene sparse/burst/reload/final retirement on both GPUs remains required.                                                          |

The optimized attempt produced **108 complete image captures**, maximum error outside legal
UNORM ranges **0/255**, and six interrupted-mixture observation groups. These partial results
do not close M07/4. No resource, watchdog, fade, quantization or pixel budget was changed.

## Failure and next product action

The 23:12–23:57 CET batch consumed its **two native attempts**. Baseline dev-profile invocation:
three tests passed, one failed in 198.47 seconds. Rebuilding with the documented optimized test
profile ruled out that profile difference as a sufficient repair: three passed, one failed in
173.89 seconds, at the same preview Layout assertion after bulk structure creation. Neither
run reached the complete sparse/burst/reload scene or its second adapter. There is no third
native attempt in this batch.

The rejection is consistent with the unchanged 60-second preview lease expiring during bulk
typed authoring; the original assertion did not print its response, so the exact rejection code
is not claimed. The checkpoint now prints that response and separates Layer and Column
authoring durations. It does not reconnect around the failure, extend the lease or bypass the
180-second complete-scene watchdog. Static inspection confirms Matrix already submits detached
trees through one `AddUserItemTrees` per operation; replacing it with sequential edits is not a fix.

**Next deliverable:** correct the bulk typed-authoring cost at its actual Matrix/Golden boundary,
preserving typed ownership, history, callbacks and UI events. Require software evidence of the
correction before another native qualification. Retain this same Active gate; no unchanged GPU
rerun or additional measurement framework is needed. Cumulative native investigation: two runs,
372.36 seconds of measured execution; this checkpoint does not reset that allowance.

## Reproduction and evidence

Use the [fixture instructions](../../apps/mapgyver/tests/matrix_pixels/README.md) after inspecting
terminal software and strict Clippy success. Run all four ignored tests serially in the optimized
test profile. Keep the original resource limits and every-pixel 2/255 residual tolerance.
[Receipt](evidence/mapgyver-m07-native-checkpoint-2026-11-03.json) identifies exact binaries,
logs, partial results and remaining requirements. Shared-fixture edits after those runs improve
failure reporting only; they do not establish a new native pass. Broader process memory,
CPU/GPU/cadence, physical and final qualification retain M03/M05/M09 ownership.
