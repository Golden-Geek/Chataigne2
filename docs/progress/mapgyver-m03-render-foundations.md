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

## Native D3D11 execution and owned host consumer — 2026-10-29

The Windows-only, opt-in `golden_video/d3d11` feature now executes the real
[`Backend` implementation](../../crates/golden_video/src/native/backend.rs) through the existing
worker and resource core. MapGyver's feature-gated native smoke consumes public Golden APIs;
the shared desktop host still owns window construction, WebView coordination and HWND lifetime.

- Explicit adapter LUID selection creates the caller-owned D3D11 device without automatic
  adapter/WARP fallback. Devices, contexts, views, SDK instances and textures remain worker-local.
- The resource core admits the fixed backend baseline before native pool initialization.
  That baseline reserves two window backbuffers per binding, one SDK shared texture per Spout
  sender and the shader constant buffer. Plan admission additionally charges demanded targets,
  compositing scratch and output conversion textures across prepared/live/retiring generations.
  These owned-storage charges do not claim measured driver residency, alignment or object overhead.
- The production shader executes clear/sample and ordered Normal, Add, Multiply and Screen
  composition. sRGB decode and coverage premultiplication precede filtering; BGRA8/RGBA8 storage
  and output transfer/alpha contracts remain explicit.
- Output pools persist per device across plan generations, avoiding duplicate swapchains for
  one HWND. Window presentation uses zero-time readiness and nonblocking Present. Spout senders
  retain the exact caller device, validate texture layout and reject silent sender renaming.
  Output failures remain bounded metadata rather than abandoning other consumers' queued work.
- Preallocated event queries fence every queued batch. The submission guard establishes its
  query on normal completion or unwind; resources retain their charge through actual last use.
  The outer native-presentation task closes the runtime and waits for its retirement proof before
  returning the HWND lease. A timeout alone never authorizes window destruction.

Verified checks for this milestone:

| Check | Result |
| --- | --- |
| Native-feature combined tests | IO: 58 unit tests; Time: 32 unit tests and 1 public-consumer test; Video: 65 unit tests and 2 public-consumer tests passed, with 1 hardware test initially ignored |
| Explicit hardware pixel test | The ignored test was then run explicitly and passed 9 finite cases on Intel adapter LUID `00000000:00016c2a` |
| Strict native-feature Clippy | All targets passed for IO, Time and Video |
| Owned host consumer | `native_video_smoke --adapter 0` passed window-only, Spout-only and shared-producer phases after a client resize; HWND stayed live through renderer retirement and was destroyed before host return |

The [finite hardware test](../../crates/golden_video/src/native/tests/pixels.rs) checks decode
before interpolation, coverage before interpolation, encoded-premultiplied input, four blends
with fractional alpha, opaque sRGB output and encoded-premultiplied output against numerical
oracles with a `2/255` component tolerance. Readback is test-only. These nine cases are bounded
pixel evidence, not the sustained scene/display matrix.

The [owned smoke](../../apps/mapgyver/tests/native_video/smoke.rs) uses an isolated app identity,
an explicit observed adapter and the same runtime controller/resource path. Separate single-output
phases establish API acceptance for both consumers before the combined shared-producer phase.
It also checks the UI remains responsive while the task retains its native window lease.
The final rebuild/rerun also passes explicit stopped/zero-reservation/native-initialization-false
assertions after retirement. Its [retained packet](evidence/mapgyver-m03-native-smoke-2026-10-29.json)
records exact executable hashes, pixel values/references and cumulative phase observations.
The normal suite passes 158 tests; its one initially ignored hardware test passes separately.
Default-feature Video tests (62 unit + 2 public), default/native strict Clippy, strict smoke
Clippy, both default app checks and required root/Core formatting also pass. No dependency
versions were upgraded. The initial sandbox smoke attempt could not create its isolated
preferences directory; the final owned run succeeded with normal desktop access.

Spout's pinned SDK can wait up to **67 ms** for its texture-access mutex and still return true
after a timeout without copying. The current shim cannot distinguish that skip from delivery.
`accepted_outputs` therefore means API acceptance, not receiver delivery or scanout; this bounded
SDK limitation remains visible in cadence/qualification work.

External inputs, including Spout receivers, are explicitly rejected until bounded source workers
are integrated. Window backing extents remain fixed: a client resize exercises **DXGI stretch**,
not dynamic preview registration, physical backing-buffer resize or DPI qualification. Native
device-loss/recreation has no hardware proof yet. Sustained native CPU/GPU/cadence/frame-age,
admission/memory/handle/shutdown budgets and full recovery remain open.

Gate 3 remains **partial** and M03 remains **3/9** accepted gates. No gate was awarded for this
checkpoint; gates 4, 8 and 9 retain their unverified native scope. The original Q01/Q02 budgets
and failures remain with their [continuing owners](mapgyver-qualification-contract.md#acceptance-ownership-after-the-m01-scope-correction).

## Bounded Spout inputs — 2026-10-29

This delivery extends the native backend above with independent Spout source workers. The
public `SpoutInput` binding supplies a texture identity, exact sender selector, maximum admitted
extent, required storage/color/alpha and polling interval. At most 32 live bindings are admitted;
this is a native worker limit, not a media-catalog limit. Bindings are immutable at startup;
app-owned live binding publication remains gate 5 work.

Each Golden IO worker owns its source SDK instance, exact-LUID D3D11 device, capture texture,
three NT-handle/keyed-mutex bridge slots and event-query completion. Preparation acquires demand
before frame submission; prepared, live and retiring generations retain it. Last-demand release
stops SDK receive polling. Baseline admission reserves five maximum-size images per configured
input: capture, three bridge slots and the foreign sender-storage alias. Renderer generation
targets remain charged separately. This accounts for owned descriptor storage, not physical
driver residency, alignment or object overhead.

The pinned SDK's public receive-into operation avoids an SDK-private receiving image. Bridge
capacity is admitted before receive/copy. Publications coalesce; renderer imports cache by weak
allocation identity, with at most three per source. Strong reader leases and keyed-mutex access
survive until matching renderer completion. Source GPU work drains independently before its
pool or SDK alias is released. Shutdown quiesces producers before renderer drain and waits for
source retirement off the actor/UI thread.

Actual extent is observed separately from the admitted maximum. An accepted resize retires
renderer aliases/readers and source work before replacing that source's pool; the renderer
device is retained. An unavailable or incompatible source resolves to zero storage (black for
opaque sampling). A new generation can reuse the last image of a paused sender without waiting
for another publication. Source absence/reconnect/backoff is independent per worker.

The [hardware input test](../../crates/golden_video/src/native/backend/tests/inputs.rs) runs two
owned independent producer processes using public Golden Video APIs. It verifies:

- Public runtime reception and close/retirement with stopped sources and zero reserved bytes.
- Prepared-demand warmup before rendering, cancellation to idle and stable SDK-call counts.
- Three-slot bridge pressure stopping SDK receive work while renderer reader leases are held.
- Two-source composition, paused-sender frame reuse and continued healthy-source content when
  the other sender stops or exceeds its admitted maximum.
- Stop/restart, an admitted 2×2 → 3×2 resize, rejected 8×8 input against a 4×4 maximum, and
  return to 2×2. Only the affected pool changes; both source devices and renderer stay intact.
- Six finite pixel observations within `2/255` component tolerance, with test-only readback.

The [retained packet](evidence/mapgyver-m03-spout-inputs-2026-10-29.json) records the exact
executables, selected Intel LUID, six observations and final source counters. Both explicit
hardware tests pass; the existing nine finite color/alpha/filter/blend cases remain green.
The regular native-feature IO/Time/Video suites pass 161 tests, with those two hardware tests
selected separately. Default Video passes 62 unit + 2 public tests. Strict native/default
Clippy, strict fixture/smoke Clippy, both default app checks and root/Core formatting pass.
The rebuilt public shared-host smoke also passes all three output phases and retirement
assertions. No dependency versions or app defaults changed.

This closes fixed M03 gate 3, bringing M03 to **4/9 = 44%**; it does not close M03. Native
forced device-loss/recreation is not hardware-qualified. Dynamic app binding publication,
media/channel identity/history/generated contracts, persistent assets, preview contracts and
Q01/Q02 native measurements retain gates 4/5/7/8/9. Output HWND backing remains fixed DXGI
stretch. No physical display, DPI, scanout, sustained timing or memory/handle pass is inferred.

The SDK may wait 67 ms for its mutex and report success without a new copy. Publications count
SDK-reported handoffs, not proved fresh delivery. During a sender change the SDK can transiently
open a changed foreign alias before post-update descriptor validation; owned pools and accepted
publications stay bounded, but this does not establish a physical foreign-residency hard cap.
Mismatch handling drains source work and disconnects before retry. These limits remain visible
for production recovery and qualification; original budgets and failures remain unchanged.

## Media/channel authoring contracts — 2026-10-29

The app now owns [`src/media/`](../../apps/mapgyver/src/media/mod.rs), separate from Golden's
renderer. `MediaId` and `ChannelId` wrap persistent Golden node UUIDs; a typed pair resolves
only within its exact owner. A missing/deleted channel never redirects to another channel.
Media and channels are separate nodes, keeping edits/history local to the affected authored
leaf. Initial media/channel creation queues one detached subtree. Detached project validation
walks nodes/children linearly and performs no native work or frame polling.

The app lifecycle delegates `mapgyver.media.edit` to backend operations for creation, adding
channels, configuration, rename, reorder, removal and inspection. Golden's existing atomic
app-operation/history route serves transport/UI, headless callers and QuickJS. Backend defaults
select labels and initial settings. Removing the last channel is rejected; removing its media
makes references explicitly missing, and undo restores the authored UUIDs. Source kind and
owner validation reject mismatched pairs without mutation or a history step.

New projects include the standard Dashboard and a Media Manager authoring container. The initial
definitions cover Solid Color and Shared Texture. Automatic size uses source metadata or the
backend Solid Color canvas; absent sources stay unresolved. Fixed size retains its output
extent separately from unavailable/unconfigured/incompatible status. Dimensions are nonzero
and bounded; selector, explicit storage/color/alpha and linear color components are validated.
The first reload run exposed sparse defaults being omitted by the shared codec. The authored
contract now supplies those backend defaults during decode; final codec/history tests pass.
This is normal sparse persistence, with no legacy-format migration or runtime resources in JSON.

[`protocol.rs`](../../apps/mapgyver/src/media/protocol.rs) is the intent/result source of truth.
Golden Video's data-only descriptor and filter declarations now derive Serde/TypeScript, avoiding
an app-maintained copy. Generate the app's 20 TypeScript files with:

```text
cargo run --locked -p MapGyver --bin generate_media_protocol -- apps/mapgyver/ui/src/lib/media/generated
```

The byte-comparison drift test verifies all generated files. The UI adapter consumes the
generated intent through the existing session interface; dedicated editor controls remain
planned. Generated declarations use the generator's formatting with trailing spaces removed,
and are excluded from Prettier along with Svelte build output. The thin executable delegates to the app library so
tests and code generation use supported public modules rather than path-imported source files.

Verified checks:

- `cargo test --locked -p MapGyver -p golden_video --features golden_video/d3d11`: **11 app +
  70 Video tests pass** (68 Video unit + 2 public). Two hardware tests remain explicitly ignored;
  no GPU implementation changed or new hardware qualification is claimed here.
- App tests cover backend defaults, different per-channel sizes, real configure undo/redo,
  rename/reorder/remove/undo/reload identity, source selector roundtrip, atomic invalid-operation
  rejection, production actor save/reload, QuickJS committed result/history, size/availability
  resolution, invalid extent/color/selector rejection and generated-contract drift.
- Strict all-target native-feature Clippy passes for app/Video and for both optional smoke
  feature groups. Default `cargo check --locked` passes for MapGyver and Chataigne2. UI check
  reports zero errors/warnings, UI lint passes, and both required Rust formatters pass.
- Cargo.lock adds only dependency edges to already locked Serde, ts-rs and test-tempfile
  packages; no dependency version or Chataigne startup default changes.

Gate 5 remains **partial** and M03 stays **4/9 = 44%**. Actual app demand/configuration must still
publish fenced runtime bindings, consume source observations and preserve these IDs through
native resource recreation. Startup native bindings are still immutable. Full Media Manager
controls, command nodes and app-owned script templates belong to M04; the generic QuickJS app
operation/callback path is already exercised. Picture/assets, Composition and Matrix remain
with their planned deliveries. No renderer, physical-output or full-app budget is relaxed.

## Plan-owned source subscriptions — 2026-10-29

The channel bridge needed a reusable boundary repair first: Spout inputs were fixed at runtime
startup, so app channel changes could not publish new source configurations with a plan.
`ExternalTexture::source` now carries immutable `SpoutSource` metadata through the existing
project/revision/device preparation and installation fences. The compiler validates selectors,
polling intervals and actual/maximum image contracts even in inactive branches. Retained input
admission includes selector capacity. No endpoint update can race separately from its plan.

`D3d11Config::inputs` now reserves `InputLimits`, rather than a startup subscription list.
Its global baseline charges the explicit storage capacity, including unused capacity. Each
subscription consumes five maximum-size images; worker count (at most 32) and storage capacity
bound all prepared/live/retiring overlap. Plan targets are charged separately as before.
An unchanged subscription shares its worker across generations. A changed contract needs a
fresh logical texture ID while the old binding is retained; an identity conflict is rejected
without changing the live plan. Fixed native output bindings/HWND ownership are unchanged.

Unused subscriptions cancel asynchronously. Their metadata/worker/storage slots remain occupied
until the worker's event channel disconnects after native cleanup. The renderer never waits
for this retirement during preparation. Overload reports resource pressure and preserves the
current scene; callers retry after capacity becomes available. Source snapshots include an
exact subscription UUID as well as device, texture and source-session identity. These are
runtime observations, distinct from persistent MapGyver media/channel IDs.

The expanded owned hardware harness verifies the original reception/resize/reconnect/static
frame/retirement cases, then prepares a new blue subscription while the old red subscription
is retained. Reusing the old binding with a different selector is rejected and leaves purple
pixels intact; a fresh binding produces blue without device replacement. A fifth overlapping
subscription exceeds four-worker capacity without changing that live blue image. Releasing
unused generations permits asynchronous capacity reuse. A separate two-source storage ceiling
rejects a third subscription despite four worker slots, preserving the purple image.
The public runtime also verifies a resource-pressure outcome with unchanged live generation,
continued frames, `Running` state and no runtime fault. Review corrected source-capacity
rejection so ordinary admission pressure does not enter the backend-fault path.

The [retained packet](evidence/mapgyver-m03-plan-sources-2026-10-29.json) records executable
hashes, eleven input pixel observations and the host regression. The existing nine finite
color/alpha/filter/blend cases pass. During the final regression, the pixel helper initially
received `DXGI_ERROR_WAS_STILL_DRAWING` (`0x887A000A`) from nonblocking `Map` after its query
completed. The test-only helper now polls that specific readiness result within the same total
five-second deadline; every other error still fails. This follows the documented
[nonblocking Map contract](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-map).
The failed run remains recorded; this helper repair changes no pixel oracle, tolerance,
production path or budget.

Verified checks: **82 regular tests** (11 app, 69 Video unit + 2 public), plus both explicitly
selected hardware tests on Intel LUID `00000000:00016c2a`; strict native all-target app/Video
Clippy including smoke targets; strict default Video Clippy; default checks for both apps;
required root/Core formatters and whitespace checks. The rebuilt shared-host smoke passes its
window-only, Spout-only and combined resized-client phases with retirement before HWND release.
No dependency version, generated control contract, UI or display/overlay setting changes.

This is the native prerequisite for gate 5's app-owned channel bridge, not completion of that
bridge. M03 stays **4/9 = 44%**. App demand capture, channel-to-plan publication, fenced source
observations and identity through resource recreation remain next. Gates 4/7/8/9 and all
Q01/Q02 recovery, timing, physical residency, display/DPI and shutdown qualification remain
open with their original budgets and failure evidence.

## App channel publication bridge — 2026-10-29

Gate 5 now connects the existing authoring contracts to actual renderer demand. The app owns
[`capture.rs`](../../apps/mapgyver/src/media/capture.rs) and
[`rendering/`](../../apps/mapgyver/src/media/rendering/mod.rs); Golden imports no app model.
The engine owner captures only demanded leaves after transaction settlement, with no device
access, plan construction, wait or whole-tree snapshot. A host/preparation worker drives
`ChannelRenderer`, which publishes immutable plans through public `VideoRuntime` APIs. GPU work
starts only with consumer demand; the default workbench does not create a renderer by itself.

The typed Media Manager owns a runtime-only instance and revision lifetime. Manager membership,
media membership and channel revisions fence captured leaves across configure, enable changes,
removal, undo and reload. Destruction explicitly invalidates guards even when history retains
the removed node payload. These clocks and all source observations stay out of persistence.
Multiple output routes share one target/pass for a channel. Size/filter changes retain its
runtime identity; changing a Spout selector/storage contract allocates a fresh binding while
the persistent `(MediaId, ChannelId)` remains unchanged.

Golden's `PlanSubmission` carries bounded revision guards and optional external source roots.
The compiler validates and charges those roots independently of presentation, permitting
Automatic-size receiver warmup before metadata exists. An unresolved channel reports no extent;
its consumer gets a transparent 1×1 GPU placeholder. That texture is never an authored or observed
channel size. Missing and Disabled join the Rust-generated availability contract. Fixed extents
remain distinct from unavailable sources; no missing reference redirects to another channel.

Native source metadata changes wake a preparation owner, excluding frame counters. Metadata and
layout guards are captured coherently; descriptor/session/pool changes or final stop invalidate
the guard. Availability alone does not invalidate layout, preventing warmup starvation. The app
rechecks availability before publication, and native missing-source handling supplies zero storage
after loss. Project/device/source stamps reject old observations; retained guards are rechecked
before preparation, frame-boundary installation and automatic reconstruction. A removed authored
source cannot be resurrected by device rebuilding. `channels()` reports admitted desired metadata;
reliable Installed outcomes identify the actual live generation.

The [evidence packet](evidence/mapgyver-m03-channel-bridge-2026-10-29.json) records the final
executable hashes and check scope. The explicit
[`channel_rendering` test](../../apps/mapgyver/tests/channel_rendering.rs) uses two independent,
owned GPU-only Spout producers on Intel LUID `00000000:00016c2a`. Two authored channels resolve
at 2×2 and 3×2, then selector replacement, resize to 4×3, disconnect/reconnect, undo and codec
reload preserve their references. Each ready checkpoint requires an Installed outcome for the
latest request and a submitted frame from that exact generation. Old project/device/layout
observations are rejected. Full controlled close/retirement followed by a fresh runtime/device
on the same adapter also preserves the loaded references. Owned windows/processes retire before
release, with a finite fixture watchdog. This tests metadata, bindings and installation; it is
not an app-output pixel oracle or actual hardware device-removal test.

Verified checks:

- **92 regular tests pass:** 17 app, 73 Video unit and 2 public Video tests. Two Video hardware
  tests remain ignored in that regular run. New coverage includes capture invalidation even with
  history retention, source-only roots/limits, guard invalidation between prepare/install,
  stale-device/project observations, shared targets and null-backend device reconstruction.
- The explicit app hardware test and both explicit Golden hardware tests pass; the latter retain
  their independent finite color/alpha/filter/blend and source/admission pixel assertions. The
  rebuilt shared-host smoke passes window-only, Spout-only and resized combined phases.
- Strict all-target Clippy passes for app/Video both by default and with both native smoke feature
  groups. Default checks pass for MapGyver and Chataigne2. UI check reports zero errors/warnings;
  UI lint, generated-contract drift, required root/Core formatters and whitespace checks pass.
- Cargo.lock adds only app edges to the already locked Golden IO and UUID packages; no version,
  Chataigne startup default, display or overlay setting changes.

This closes fixed **gate 5**, bringing M03 to **5/9 = 56%**; overall stays **3/10 = 30%**. No
denominator or qualification budget changed. Dedicated Media Manager controls, command nodes,
callbacks and app-owned snippets remain M04; production Screen/preview demand belongs to M05.
Gates 4/7/8/9 remain open, including genuine device-removal recovery and sustained Q01/Q02
qualification. Next is the shared persistent asset/preparation boundary needed by Picture media.
