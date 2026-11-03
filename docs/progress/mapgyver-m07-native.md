# Matrix native pixels and scale acceptance

**M07/4 Accepted — 2026-11-03. M07 is complete at 4/4.** Matrix owns authored structure,
playback and captures under `apps/mapgyver/src/matrix`; Golden owns bounded rendering,
preview leases and completion-safe retirement. Public app intents, the production renderer
and preview compositor, and actual D3D11/Spout readbacks prove the current revised contract.
[Model](mapgyver-m07-model.md), [playback](mapgyver-m07-playback.md) and
[grid](mapgyver-m07-grid.md) retain their separately verified scopes. M11 owns later feedback changes.

## Original requirement audit

| Requirement                    | Inspected proof                                                                                                                                                                                                                                                                                        |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Revised typed scene            | Actual three-Layer/three-Column defaults, six distinct owned Cell media, explicit 32×24 media/Layer extents, Group order and exact sparse project roundtrip. No implicit source stretch or extra default rows/Columns.                                                                                 |
| Typed Cell and Layer behavior  | Native Cell position/live opacity/Undo/disabled hold/enabled launch, and Layer position/canvas/resize/Undo preserve child media size on both GPUs.                                                                                                                                                     |
| Layer/Group/Final and preview  | Independent complete rasters pass on both GPUs for Normal/Add/Multiply/Screen, alpha, post-composite Group opacity, ordering/Undo and transparent empty Columns.                                                                                                                                       |
| Source loss and recovery       | Actual source deletion gives the transparent expected contribution; Undo restores pixels, and sparse project reload recreates correct Layer/Group/Final/preview outputs on both adapters.                                                                                                              |
| Shared consumers               | Two distinct owned Cell References project the same source and exactly one prepared physical source record. Shared-source and restored-source rasters pass across Layer/Group/Final/preview on both adapters.                                                                                          |
| Interrupted shared-time blends | Three three-color observations per Layer/Group/Final conserve weights, preserve outgoing ratio and monotonic progress, reach the final blue endpoint, then retire outgoing demand. Both the full scene and standalone test pass on both GPUs.                                                          |
| Sparse scale and bursts        | Exact 1,000-Layer ×1,000-Column document, one Group and six populated Cells stays sparse. Each adapter receives 1,000 configured launch requests on the original 10 ms schedule; exactly eight are admitted under the existing credit cap, outcomes drain, pixels recover and history stays unchanged. |
| Bounds and owner lifetime      | Original plan/resource/admission assertions pass throughout. All twelve fixture owners complete retirement with zero live plan allocations, balanced creation/release and empty output/source inventories.                                                                                             |

## Production correction

Bulk Column creation used three whole-document lifecycle snapshots for each new embedded
App Control, including its empty init/ready callbacks. `AppControlNode` now declares the actual
stage requirements: fresh attachment binds through generated child events; attachment with
existing descendants keeps its snapshot for load/replacement rebinding; empty init/ready stages
need none. Inbox reconciliation retains its existing shared-snapshot contract. No engine
behavior, typed ownership, callback, history, protocol or persistence rule was bypassed.
[Boundary details](../architecture/mapgyver-parameters.md).

The regression authors 997 Columns into the actual three-Column default, checks all 1,000 typed
controls, stable order, one-step exact Undo/Redo and project reload. It records **three lifecycle
snapshots and 554 ms** for the bulk creation operation. The complete native scene authors
1,000 Layers/Columns in **960 ms on Intel** and **885 ms on NVIDIA**.

The [prior checkpoint](evidence/mapgyver-m07-native-checkpoint-2026-11-03.json) preserves both
failed attempts and their exact limits. That evidence led to this production correction.
Passing software ownership/history/project and snapshot-bound checks supplied new evidence
for the complete acceptance run; the investigation was not extended by an unchanged native retry.

## Current verification

[Acceptance receipt](evidence/mapgyver-m07-native-acceptance-2026-11-03.json):

- **393 library tests pass**, zero failures/two retained exclusions, in **14.83 seconds**.
  Twelve Matrix/Composition raster software tests also pass. Generated TypeScript equality
  passes in the library suite; no DTO changed. Strict all-target native/process Clippy passes.
- After terminal software/strict success, the optimized test-profile binary passes **all four
  native tests**, zero failures/exclusions, in **230.89 seconds**. Intel Graphics and NVIDIA
  GeForce RTX 5080 Laptop GPU both complete the original main scene and separate geometry/fades.
- **192 complete image captures**, every texel checked; maximum error outside propagated legal
  UNORM ranges is **0/255** under the unchanged **2/255** residual limit. Twelve interrupted
  observation groups cover 36 Layer/Group/Final mixtures. Twelve balanced retirements pass.
- Original 180-second main-scene and 90-second separate-test watchdogs, 60-second preview lease,
  64 MiB resource/texture caps, two in-flight/two retiring/one prepared generations, eight
  outstanding requests and sixteen prepared channels remain unchanged and pass.
- Actual launch spans are **10,054 ms Intel / 10,038 ms NVIDIA**, separately from draining and
  recovery. A 10 ms requested schedule does not prove sustained cadence or global CPU/GPU budgets.

[Fixture instructions](../../apps/mapgyver/tests/matrix_pixels/README.md) own reproduction.
M03/M05 physical/performance gates, M08 process qualification, M11 feedback and integrated M09
qualification remain required; these small native rasters do not qualify their broader workloads.
