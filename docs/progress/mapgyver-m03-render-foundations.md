# M03 render/resource foundations

Golden Video owns executable render-plan/resource contracts. MapGyver owns authored media/channel
identity, defaults, automatic-size intent and editor policy. The shared desktop host owns native
windows and WebView UI-thread access. This delivery closes the first of the [nine fixed M03 gates](mapgyver-status.md#m03-acceptance-items).

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
is a validated preparation value; atomic frame-boundary installation remains the worker-runtime
gate. The footprint counts each demanded texture once, including external references. Actual
GPU admission must additionally account for allocation padding, bridge/backbuffer pools,
preparation/live/in-flight/retiring generations, and every retained lease/cache allocation.

Validation:

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

## Shared prerequisites and continuing scope

The reconciliation checked the [shared plan](../plan/Chataigne2_Codex_Implementation_Plan.md) and
[Chataigne ledger](implementation-status.md). P14 and P15 remain unaccepted:

- Audio's `RenderClockCoordinator`/`ClockSource`/`ClockBlock` in
  [clock/timeline.rs](../../crates/golden_audio/src/clock/timeline.rs) are audio-owned helpers,
  currently not a production-wide monotonic activation/show authority. Golden Video must not
  depend on Golden Audio just to obtain time. Shared typed time/epoch/activation contracts must
  be introduced at their reusable owner and exercised by scheduling.
- Audio source fingerprints/cache keys are runtime preparation identities, not persistent asset
  IDs or relocation/relink locators. No generic persistent asset boundary exists yet. Cache
  eviction also does not release allocations pinned by external leases/voices. Implement and
  exercise that shared boundary once, keeping each app's catalog policy in the app.
- [Golden IO](../../crates/golden_core/runtime/io/src/lib.rs) already provides bounded workers,
  cancellation, revision guards, payload admission and off-actor retirement. The next runtime
  batch must reuse these public contracts, while retaining actual GPU ownership on its worker.

No time/asset implementation or full P14/P15 acceptance is claimed by this reconciliation.
Worker/null runtime, D3D11/Spout, device recovery, app channel/history/protocol contracts,
shared time/assets, preview geometry and native qualification remain M03 work. The original
cadence, memory-growth and retirement failures retain their unchanged Q01/Q02 budgets and
[continuing owners](mapgyver-qualification-contract.md#acceptance-ownership-after-the-m01-scope-correction).
No native capture, display/overlay change or rendered-pixel correctness pass was performed here.
