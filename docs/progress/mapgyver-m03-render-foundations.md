# M03 render/resource foundations

Golden Video owns executable render-plan/resource contracts. MapGyver owns authored media/channel
identity, defaults, automatic-size intent and editor policy. The shared desktop host owns native
windows and WebView UI-thread access. This evidence covers the compiler and subsequent bounded
worker/shared-time deliveries; accepted counts belong to the [nine fixed M03 gates](mapgyver-status.md#m03-acceptance-items).

## Immutable render-plan compiler — 2026-10-28

The new [Golden Video crate](../../crates/golden_video/README.md) is independent of Tauri,
engine nodes, apps and wire serialization. It contains:

- Logical texture/pass/output IDs, nonempty texel extents and explicit BGRA8/RGBA8 storage,
  RGB transfer encoding and alpha convention. Source format does not infer alpha/color policy.
  Internal render targets require linear-light premultiplied alpha.
- Finite SDR clear colors and unit-range opacity, with explicit straight-to-premultiplied
  conversion and rejection of invalid values instead of silent clamping.
- Clear, full-target sample and ordered composite operation descriptions. Their source references
  are the dependency source of truth; geometry extensions arrive with their real consumers.
- Complete-graph validation before pruning: unique writers/identities, defined inputs/output
  sources, metadata/dimension constraints, iterative cycle detection and raw graph-work bounds.
- Immutable compiled ownership with deterministic pass-ID tie breaking, demand pruning and
  shared producers across output bindings. Inactive values/capacity are released during
  preparation, and retained layer buffers are compacted.
- Checked per-source/per-target logical footprints and an inclusive caller-selected texture
  byte ceiling. Resize changes descriptors/footprint without changing logical identities.

Both successful and rejected compilation create no native resources. A returned compiled plan
is a validated preparation value consumed by the worker runtime below. The footprint counts
each demanded texture once, including external references. Actual
GPU admission must additionally account for allocation padding, bridge/backbuffer pools,
preparation/live/in-flight/retiring generations, and every retained lease/cache allocation.

Earlier compiler-checkpoint validation:

- `cargo test --locked -p golden_video`: **27 unit tests and 2 public-consumer tests passed**.
  Cases include direct/indirect/inactive cycles, blocked-dependent diagnostics, repeated/shared
  inputs, missing references, all raw limits, dimension/byte overflow, exact byte ceilings,
  empty demand, metadata preservation, stable order under permutations and resize identity.
- A reversed **10,000-pass** chain compiles iteratively with the expected order and exact
  logical footprint. This is deterministic graph coverage, not a Q04 latency or native pass.
- Color/extent tests include NaN/infinity, adjacent out-of-range floating-point values,
  premultiplied coverage, both storage orders, 1080p/4K sizes and `u64` overflow boundaries.
- Strict all-target Clippy passes. Independent compiler review found no blockers in graph
  ordering, demand closure, checked accounting or compact immutable ownership. Root/Core
  formatters and whitespace checks pass.

## Bounded worker/null runtime and shared time — 2026-10-28

The [runtime](../../crates/golden_video/src/runtime/mod.rs) uses existing Golden IO worker,
cancellation, revision, payload and readiness contracts. Its control owner contains metadata
and CPU input leases; backend resources remain on the render worker, with `!Send` resources
exercised by the null backend.

- One latest desired publication coalesces superseded work. One CPU preparation and one
  resource-prepared candidate are bounded separately. Retained raw input bytes and all accepted
  requests have explicit admission limits.
- Each accepted request holds a credit until its exactly-once terminal outcome is drained.
  Completion claims are atomic, including publisher/fatal-finalization races. Reliable outcomes
  cannot grow beyond those credits or stall the frame/resource loop when the consumer stops.
- The resource core reserves the backend's complete charge before allocation. Partial failure
  releases newly created objects before releasing that charge and preserves the live plan.
  Prepared/live/retiring resources remain charged; at most two batches and two old generations
  await device-scoped last-use completion. Foreign, old and future completion tokens cannot
  release current resources.
- Installation and activation use frame boundaries and submission capacity. Cancellation,
  project replacement, supersession and scheduled lateness are explicit terminal outcomes.
  Project-change completion waits for old project resources and preparation to retire.
  Admission checks close/project/publication fences under a short metadata lock; native calls
  and resource destruction stay outside it. Already-admitted uses may finish after a fence,
  without republishing an old frame as the replacement project's latest observation.
- Close/drop never join on the publisher/actor thread. The worker drains actual uses and drops
  resources before its retirement proof; preparation and retained input disposal are also
  observed. A timeout cannot authorize native-window destruction. Fatal status clears stale
  resource/preparation/frame observations.
- The null backend creates no pixels or native interfaces. Manual/automatic completion,
  padding charges, partial allocation/submission failure and device loss exercise the same
  ownership path. Bounded cached rebuild metadata and capped retry preserve recovery intent;
  this simulation is not native device-recovery qualification.

The new [Golden Time owner](../../crates/golden_time/README.md) provides typed monotonic,
show and engine-sample coordinates, checked rational conversions, immutable mappings,
authority/discontinuity epochs and activation stamps. Video consumes it for absolute cadence,
scheduled installation, late policy, clock replacement and per-submission observations. Multiple
outputs consume one snapshot; callbacks do not advance show time. Apps retain transport policy.
Replacement publishes the mapping and its clock origin atomically, so readers cannot mix
authority lifetimes while a clock changes.

Verified test results for this delivery:

| Package | Passing coverage |
| --- | --- |
| `golden_io` | 58 unit tests, including payload-free wake/checkpoint/deadline service |
| `golden_time` | 32 unit tests and 1 public-consumer test |
| `golden_video` | 58 unit tests and 2 public-consumer tests, including the earlier compiler coverage |

The suites include bounded admission/outcomes, worker-side cleanup, full simulated resource
charges, atomic rejection, stale completion, cancellation and checked time/activation behavior.
Three final regressions cover old-frame status after project replacement, concurrent clock/mapping
publication and cancellation of an already-prepared future plan on close. All 151 tests pass;
none are ignored. Required root and Golden Core formatters and whitespace checks pass.
Combined strict all-target Clippy passes for IO/time/video; `cargo check` passes for both
MapGyver and Chataigne2. The installation API deliberately returns a rejected candidate inline,
with one scoped `result_large_err` allowance, avoiding a new allocation on a busy frame-boundary
retry. Final integration checks and accepted gates are recorded in the ledger.
No native GPU timing, memory/handle budget, rendered-pixel correctness, source/output health or
MapGyver media integration is qualified by the null backend.

## Shared prerequisites and continuing scope

The reconciliation checked the [shared plan](../plan/Chataigne2_Codex_Implementation_Plan.md) and
[Chataigne ledger](implementation-status.md). P14 and P15 remain unaccepted:

- Audio's `RenderClockCoordinator`/`ClockSource`/`ClockBlock` in
  [clock/timeline.rs](../../crates/golden_audio/src/clock/timeline.rs) are audio-owned helpers,
  currently not a production-wide monotonic activation/show authority. The shared Golden Time
  boundary now exists and is consumed by video scheduling, without a video-to-audio dependency.
  Chataigne's audio/DMX adoption, authority handoff and native synchronization remain P14 work.
- Audio source fingerprints/cache keys are runtime preparation identities, not persistent asset
  IDs or relocation/relink locators. No generic persistent asset boundary exists yet. Cache
  eviction also does not release allocations pinned by external leases/voices. Implement and
  exercise that shared boundary once, keeping each app's catalog policy in the app.
- [Golden IO](../../crates/golden_core/runtime/io/src/lib.rs) already provides bounded workers,
  cancellation, revision guards, payload admission and off-actor retirement. This runtime reuses
  those public contracts; native backends must preserve the exercised worker-only ownership.

This shared time delivery does not complete full P14; persistent assets and full P15 remain open.
D3D11/Spout, native device recovery, app channel/history/protocol contracts, persistent assets,
preview geometry and native qualification remain M03 work. The original
cadence, memory-growth and retirement failures retain their unchanged Q01/Q02 budgets and
[continuing owners](mapgyver-qualification-contract.md#acceptance-ownership-after-the-m01-scope-correction).
No native capture, display/overlay change or rendered-pixel correctness pass was performed here.
