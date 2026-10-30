# Golden Video

Golden Video owns app-neutral video execution contracts. MapGyver owns media/channel identity,
authored size policy, source selection, mapping and editor behavior. The desktop host owns native
window construction and WebView UI-thread access. Video plans depend on neither app nodes nor Tauri.

The implemented foundations are:

- `identity.rs` names logical textures, passes and output bindings. These are plan identities,
  separate from persistent media/channel IDs and native resource ownership/generations.
- `resource/` declares nonempty texel extents, storage, RGB transfer encoding and alpha convention.
  Source metadata is explicit; renderer-internal targets use linear-light premultiplied alpha.
  These data-only descriptors and the sampling filter derive Serde/TypeScript contracts, so app
  authoring DTOs reuse their canonical definitions. Deserializing an empty extent is rejected.
- `color/` validates finite SDR premultiplied clear colors and unit-range opacity.
- `plan/` validates the complete submitted dependency graph and caller-supplied limits, then
  compiles only demanded sources/passes in deterministic dependency order. Multiple output
  bindings can share one texture without duplicating its producer or logical storage charge.
- `runtime/` owns bounded publication, preparation, frame scheduling, generation installation
  and completion-based resource retirement. `VideoRuntime` starts either the pixel-free null
  qualification backend or the opt-in Windows D3D11 backend through the same control/core path.
- `preview/` owns metadata-only session negotiation, full-layout replacement, installation status,
  host geometry conversion and coalesced leased registration. Apps resolve opaque source keys;
  neither app-domain types nor native handles belong in these DTOs.
- `native/`, enabled by `d3d11` on Windows, owns caller-selected adapter/device creation, the
  shader pipeline, persistent window/Spout output pools and device-scoped event completion.
  `D3d11Config` contains endpoint metadata; native interfaces stay on the render worker.
- `qualification/` owns Rust-generated metadata for the held native-preview test fixture.
  The opt-in Windows `qualification` feature also exposes GPU source/consumer marker fixtures;
  app-owned runners select scenarios and own processes, without putting product policy in Golden.

Source-local sRGB textures with straight or opaque alpha use typeless storage with separate
raw UNORM and decoded SRGB views. The sampler selects the decoded view and skips shader gamma
decoding, then premultiplies each texel before interpolation. Encoded premultiplied sources
retain shader unpremultiplication before gamma decoding. Shared input/output formats and raw
copy/marker views remain UNORM; no extra texture or per-frame view allocation is introduced.
See the [sampling checkpoint](../../docs/progress/mapgyver-m03-qualification-runner.md#hardware-srgb-source-decoding--2026-10-29)
for finite color tests and the bounded cost comparison.

Opaque output transfer uses an explicit draw-uniform shader branch. Linear shader targets,
including hardware-encoded window views, skip unused sRGB transfer math. Raw sRGB Spout
targets retain shader encoding. Native tests cover finite color/alpha cases and a paired
isolated draw-cost control; see the
[output branch checkpoint](../../docs/progress/mapgyver-m03-qualification-runner.md#opaque-output-shader-branch--2026-10-30).

Native `D3d11Config::diagnostics` optionally enables a bounded metadata queue, CPU API brackets
and two GPU timestamp/disjoint slots per context. The queue uses pinned
[`concurrent-queue` 2.5.0](https://docs.rs/concurrent-queue/2.5.0/concurrent_queue/struct.ConcurrentQueue.html)
bounded storage: concurrent source/render producers and the reporting consumer share no metadata
mutex. Capacity pressure still drops explicitly; recording never grows storage to avoid a drop.
Snapshots are sparse observations; exact written/drained/queued accounting requires quiescent
producers. Disabled runtimes allocate no timing queries. Drops, invalid/skipped/uncollected timing
and missing OS counters remain explicit.
Renderer slots also prepare two intermediate timestamps on the same disjoint scope, after
input copying and after render passes. Completed rows partition that exact batch into inputs,
rendering and outputs; incomplete/invalid marker order cannot certify partitions, and slot
reuse clears marker state. Source/fixture contexts allocate no intermediate queries. These
elapsed intervals include GPU scheduling, dependency waits and command-feed gaps. They do not
isolate active shader work, measure native buffer occupancy or align clocks across contexts.
Opt-in native scheduling uses an explicitly captured QPC origin/frequency to map absolute
rational cadence slots to machine-counter boundaries with checked integer rounding. Each
consumed slot records admission and an exact submitted token when present; final fences bound
observed coverage. Clock replacement invalidates the mapping, while show-time discontinuities
within the same monotonic authority retain it. Failed initialization falls back to SystemClock
with unavailable mapping. Diagnostics disabled retain SystemClock and create no cadence UUIDs.
These are software submission observations, not GPU completion or displayed-frame deadlines.
Context-local GPU spans and inclusive CPU wall intervals are observations, not a complete GPU
critical path or active CPU estimate. Process-local DXGI budget/usage and sparse process resource
APIs run outside engine/UI ticks. See the
[production qualification runner](../../docs/progress/mapgyver-m03-qualification-runner.md)
for fixture ownership, reproduction and remaining measurement limitations.

Run compilation on a preparation worker when desired state changes, outside engine ticks,
UI callbacks and per-frame rendering. Compiled storage prunes inactive metadata and compacts
retained buffers; the executable plan does not keep the submitted inactive vector capacity.

The execution DAG describes texture dependencies and operations. It is distinct from Golden
Graph's authored graph document/ports/history/canvas, and is neither persisted nor transported
as JSON. Operation input references are the dependency source of truth. Composite layers keep
their authored order; independent passes use stable pass-ID ordering.

The footprint counts tight logical source/target bytes once per demanded texture. It is a
preparation bound, not actual GPU admission or an assertion about native allocation usage.
The worker core reserves the backend's full charge before allocation and keeps it through
prepared/live/retiring ownership and actual resource destruction. Backend charges must cover
padding, bridge/presentation pools and retained cache leases. Null charges simulate these bounds;
they are not native memory measurements. Missing DXGI counters do not establish a zero-usage policy.
Native accounting reserves the fixed backend baseline before creating its pools, then charges
plan targets, compositing scratch and Spout output conversion textures separately. Its owned texture
pool/constant-buffer charge is not measured driver residency, padding or native-object overhead.

Native generations retain compatible texture/view sets through a device-local weak index in
`native/backend/resources.rs`. Mutable storage is keyed by logical role and complete descriptor;
all its writes remain ordered on the same immediate context. Source sequence/availability state
is revalidated by each generation before drawing, including clears for missing inputs. Immutable
images share storage by exact prepared payload and descriptor, across logical input IDs. Each
uploaded texture pins that payload lease, preventing address reuse while a cache entry can be
upgraded; equal app asset IDs alone do not establish equal content. Expired weak entries are
pruned at preparation. The index owns neither GPU storage nor asset leases. Core reservations
still charge the full declared generation footprint, preserving the existing conservative bounds.

Preparation diagnostics join proposed resource/plan identity, native creation/reuse counts and
CPU wall time to each submitted frame's exact resource generation. Offline analysis can measure
native preparation and prepared-to-first-submit delay. Optional plan lifecycle records come
from the runtime's admitted mailbox publication, first worker receipt and atomic resource
installation. Superseded/rejected/cancelled input retains its own terminal observation; device
recreation retains the authored identity and records a distinct installation. The bounded
diagnostic recorder exists before the render worker starts so its first publication cannot be
lost to startup. First receipt is recorded once across preparation retries. Offline analysis
joins the exact resource generation to first use and per-output accepted CPU API age. Retained
plan age naturally grows between edits and is not edit latency. Missing clocks/joins stay
unavailable. GPU queue and physical presentation age remain separate measurements. Artifact
schema version is Rust-owned and generated with the fixture contracts as `capture-schema.json`.

Optional window diagnostics retain the actual endpoint incarnation and the three DXGI
swapchain interface aliases with each Present API bracket. Aliases are captured during cold
endpoint creation; disabled diagnostics perform no additional interface queries. These are
non-owning diagnostic numbers, never project/control handles and never opened by analysis.
The production runner can opt into a pinned PID-filtered PresentMon collector. Offline joins
require exact PID, alias and native API interval before associating a software display event
with a renderer token, source image or authored plan. Missing/dropped/ambiguous records stay
explicit. Software-display backlog is separate from native buffer occupancy, GPU queue time
and physical scanout; it cannot certify those remaining budgets.

Opt-in native diagnostics retain each actual copied Spout image's 32-bit SDK share-handle/counter
pair through source publication and output delivery. These ephemeral values are diagnostic
identities only, absent from project files and control protocols; they retain/open no native
resource. Unknown/unversioned peers have no claimed image version. Qualification fixtures
record the same metadata with producer markers and each independent GPU capture. Offline
Q01/Q02 analysis joins accepted output images to their first completed capture and independently
checks both decoded markers against the exact renderer source-use images. Repeated captures
cannot replace missing outputs; recycled/duplicate version identities remain ambiguous. Source
publication/receive-to-output API age and sender-API-to-capture delay bounds use CPU QPC only.
They do not establish window presentation age, calibrated GPU queue time or a GPU critical path.
The SDK sender-info contract stores `uint32_t shareHandle`; diagnostic identity accepts its
zero/sign-extended pointer representations and rejects unrelated 64-bit NT handle values.
Actual native access always retains its original validated handle.
SDK counter agreement alone cannot certify pixel identity. The
[GPU ownership checkpoint](../../docs/progress/mapgyver-m03-qualification-runner.md#gpu-keyed-spout-image-handoff--2026-10-29)
isolates observed counter/pixel disagreements after renderer conversion at the outgoing SDK
handoff. Owned Spout outputs now request GPU keyed shared textures before initialization;
creation failure never downgrades synchronization. Detailed sends acquire CPU metadata then GPU
ownership with an explicit finite timeout per lock. Renderer outputs pass zero; isolated fixture
source workers request at most 2 ms and record that policy in their snapshots. Neither request
bounds opaque SDK/driver work in the whole call. Busy copies no shared pixels and advances no
counter. Unchanged
counter hints take only the CPU metadata lock. Existing completion fences still cover work
submitted before a rejected send. A late Spout Busy can follow conversion; the window preflight
alone rejects before conversion. Legacy incoming peers retain their own synchronization contract.
The pinned SDK fork's [ownership/provenance notes](../../vendor/spout2-sys-0.1.1/GOLDEN_PATCH.md)
document the public extension and DX11 peer requirement. Test-only GPU taps and a final readback
check input, conversion and outgoing pixels; ordinary rendering adds no pixel readback. Short
controls now have exact Q01 lineage and no observed Q02 pixel mismatch, but Q02 capture coverage,
cadence/age and complete native qualification remain open. Earlier failed captures are preserved.

Native output conversion/publication retains compiled plan order. The
[rejected scheduling controls](../../docs/progress/mapgyver-m03-qualification-runner.md#rejected-native-output-order-experiments--2026-10-30)
preserve staged-conversion and shared-first experiments whose Q02 controls regressed. The
app-owned offline CPU-order audit describes those experiments; it adds no runtime acceptance
requirement and establishes no GPU readiness, queue depth or critical path.

Baseline admission precedes backend initialization. The render core distinguishes pending first
initialization, ready service and confirmed device loss. Initial `AllocationFailed` results retry
on the worker with capped, cancellable backoff, retaining the original baseline reservation and
first device identity. Invalid limits, budget rejection and unsupported capabilities are terminal;
an initialization failure does not manufacture device loss or advance the device generation.
While initialization is pending, the worker continues serving time/project intents and retaining
the latest desired plan. Plan preparation and GPU completion service begin after initialization.

`ResourceSnapshot.initialized` records the first successful initialization and remains true through
later loss/reconstruction; `device_lost` identifies that separate stage. Native
`D3d11Snapshot.initialized` and qualification `NullSnapshot.initialized` report whether their backend
currently has initialized resources. Cancellation wakes retry waits between attempts; it cannot
guarantee interruption of a blocked driver call. Cleanup and baseline release still require actual
owner retirement, including partial allocations from a failed attempt.

`golden_io` supplies bounded workers, cancellation, publication fences, readiness and off-publisher
input disposal. The runtime retains one latest desired plan, one CPU preparation, one prepared
candidate, at most two submitted batches and two retiring generations. Request credits bound
accepted work plus undrained terminal outcomes; a stalled consumer cannot block resource service
or silently discard admitted results. Project/device/publication identities fence stale work.
`PlanSubmission` adds bounded app-neutral `golden_io::RevisionGuard` fences and source-only demand.
Guards are rechecked before preparation and installation and retained for device reconstruction;
an invalidated source cannot be resurrected by automatic rebuilding. `submit_plan` remains the
ordinary graph-only entrypoint. `RenderPlan::compile_with_source_demands` validates external roots
and retains their resources without requiring a presentation dependency, allowing receiver warmup
before an Automatic source has an extent. Guard and source-root storage count toward input limits.
Admission linearizes at the final current-generation check; a use already admitted before a
concurrent project/publication fence may finish afterward. Project-change completion waits for
those uses, and old frames never become the replacement project's latest frame observation.

Resources stay worker-local, including `!Send` backend objects. Installation occurs at a frame
boundary; last-use completion gates release. `close` and handle destruction do not join. An outer
presentation/shutdown worker must observe `RuntimeRetirement::wait` succeeding before destroying
a native window. A timeout is not retirement. Null controls expose only bounded metadata and
completion/failure injection, never pixel buffers or resource ownership.

The shared [Golden Time](../golden_time/README.md) authority supplies show/monotonic observations,
discontinuity epochs and activation stamps. Scheduling uses absolute rational deadlines, explicit
late policy and one authority snapshot per submission, rather than advancing time per output.
Time transport policy remains app-owned; video does not depend on audio to obtain time.
Clock replacement publishes the new mapping and its clock origin together for reader consistency.

[Golden Assets](../golden_assets/README.md) supplies persistent source references and bounded CPU
preparation separately from render identities. `ExternalSource::Image` retains a
`PreparedAsset<ImageData>` with immutable tight RGBA8/BGRA8 texels and explicit color/alpha metadata.
Video input admission charges that lease's retained reservation; GPU admission charges each native
texture through prepared/live/retiring use. Submit automatically captures image revision guards,
and core preparation/final installation/reconstruction recheck them without turning stale source
rejection into a device fault. Apps own image codecs, asset catalogs and relink/retry policy.

The D3D11 backend supplies immutable image bytes as
[`CreateTexture2D` initial data](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11device-createtexture2d)
during preparation. It performs no recurring CPU upload/readback. An image consumes no Spout input
worker or bridge capacity. `uploaded_images`/`uploaded_image_bytes` count native creations, not
frames, GPU residency or delivery. Current retained assets can recreate resources on a fresh
device; invalidated assets cannot install or rebuild. Native finite tests separately verify alpha
filtering, unchanged upload count across frames and controlled completed teardown/new-device
recreation. Actual hardware removal and sustained memory/timing budgets remain unqualified.

The native pipeline executes the demanded DAG with clear, sample and ordered Normal/Add/Multiply/
Screen composition. It decodes sRGB and premultiplies coverage before filtering, then converts
to each explicit output color/alpha contract. Output pools persist across plan generations:
windows retain two fixed-size BGRA8 backbuffers; Spout senders retain one SDK shared texture and
an AddRef of the exact caller device. Window size changes use DXGI stretch, without reallocating
backing buffers. The host retains each borrowed HWND through successful runtime retirement.
An admitted window renders directly through a cold-prepared SRGB backbuffer view, with opaque
linear shader output encoded by the view. It allocates no generation conversion texture and
performs no conversion-to-backbuffer copy. A borrowed `pipeline::RenderTarget` separates the
destination view/encoding from texture ownership. Admission still precedes drawing; the endpoint
retains that view through completion-fenced failure/recreation. Spout retains its UNORM texture
and existing shader color/alpha conversion, including encoded premultiplied output semantics.
Equal-extent output conversion uses one texel load; resizing retains linear filtering. Both
paths keep the same explicit color/alpha conversion rather than copying incompatible outputs.

Each configured output has one fixed endpoint slot. An endpoint's creation or delivery failure
enters `Recovering` independently of healthy outputs, with retries starting at 250 ms and backing
off to two seconds. The baseline remains reserved while an endpoint is absent. Failed native
resources remain owned until the renderer completes their last attempted batch; the old endpoint
is then destroyed before its replacement is created. A retry cannot overlap two endpoint pools.
Busy or occluded windows retain their endpoint, and retry backoff resets only after API acceptance.
Quiescing stops retries without joining from the UI or control engine.

Output snapshots expose lifecycle state, endpoint generation, creation attempts and the last
HRESULT. Recovery withdraws the prior accepted completion token, so acceptance from an earlier
endpoint cannot establish readiness for its replacement. Output state notifications wake consumers
on transitions rather than requiring UI/engine polling. An output error alone does not prove device
removal; only the retained device's removal authority permits device-wide discard and reconstruction.
Runtime clock intents update time and command outcomes; they do not clear an unrelated backend fault.
Recovery state reflects resumed service rather than receipt of a clock change.

`RenderOperation::Regions` draws ordered normalized `BlitRegion`s onto a cleared target.
Destination clipping retains the matching source crop; the native pipeline uses viewport/scissor
geometry and the same source color/alpha conversion. An empty region set produces a clear, so
omitted, hidden or expired preview registrations need no stale image reuse. A supplied visibility
guard also clears an already active target after invalidation without reallocating its resources;
independent outputs continue while replacement preparation is pending. Region dependencies and
retained storage use the existing graph/input limits. The shader constant buffer now reserves
64 bytes, including the source rectangle; native resource accounting includes that baseline.

The local preview owner accepts one current frontend session and bounded full layouts. New layout
revisions replace desired geometry; unchanged heartbeat geometry renews the lease without
invalidating an installed generation. Session replacement, invalid layout, explicit disconnect,
expiry and close invalidate geometry guards. Drive `PreviewReceiver` on notifications or its next
lease deadline from a host/preparation worker, not an engine/UI polling loop.

The shared desktop host's optional `NativePresentation::with_control` routes the bounded local
`native_presentation_control` command to this metadata owner. UI-thread callbacks sample actual
native client and WebView bounds, BoundsMode, rasterization and zoom; CSS-to-physical scale is not
provided by the frontend. Clipped geometry becomes normalized client coordinates, preserving
projection onto the admitted backing when DXGI stretches a fixed-size window output.

[`golden-video-ui`](../../packages/golden-video-ui/README.md) supplies the reusable Svelte component,
session and transport adapter. It coalesces DOM layout changes, retains one in-flight IPC and the
latest desired layout, and keeps opaque covers until the latest native installation is confirmed.
Removed views retain bounded opaque retirement masks until an installed clear is acknowledged;
lease expiry is the fallback after frontend interruption. CSS-pixel placement of measured masks
is a deliberate geometry exception; product layout remains in relative units. Browser sessions
report native preview unavailable while editing continues. These bindings own no app mutation or
source policy and carry no pixel frames.

`Applied.installed` is distinct from metadata acceptance. The native owner must observe a reliable
Video Installed outcome and a submitted frame of that exact identity before `mark_installed`.
The owned consumer also verifies that window output's accepted completion token through
`D3d11Control::output_snapshots`; acceptance by another output cannot establish preview readiness.
Neither acknowledgement proves scanout, delivery, physical alignment or input correctness.
The shared-host/Svelte consumer lives under `apps/mapgyver/tests/native_preview/`; specialized
editors and full display/DPI/input qualification retain their app milestone owners.

Generate its canonical TypeScript contracts with:

```text
cargo run --locked -p golden_video --bin generate_preview_protocol -- packages/golden-video-ui/generated
```

Every queued command batch ends in a preallocated D3D11 event query, including output failures
and unwind. Output results are bounded observations, so a busy window does not abort other
consumers or abandon already queued GPU uses. The pinned SDK's detailed transfer status
distinguishes an actual Spout copy from an access timeout; API acceptance still does not prove
consumer delivery or scanout. Opaque API waits remain part of native cadence qualification.

`ExternalTexture::source` selects an immutable image or `ExternalSource::Spout`: exact selector, required storage/
color/alpha, polling interval and admitted maximum extent. Bindings travel inside the same
revisioned plan as their consumers. `D3d11Config::inputs` reserves `InputLimits` before native
allocation: at most 32 overlapping source workers and an explicit descriptor-storage capacity.
That capacity remains charged while unused; each subscription consumes four maximum-size
image charges (three bridge slots and sender alias). The public checked
`InputLimits::source_storage_bytes` derives this charge from the same slot count used by
allocation and renderer import caches; apps do not duplicate pool-layout arithmetic.
Old/prepared/retiring subscriptions share
these limits. Each source owns a Golden IO worker and a separate
device on the selected adapter. Prepared resource generations hold demand leases, warming inputs
before activation; discarding the last demand stops receive polling. SDK work never runs in the
renderer or engine loop.

The public receive-into API copies directly into a free, write-locked bridge slot, without an
SDK-allocated receiving image or a private capture-to-bridge copy. Three NT-handle/keyed-mutex
bridge slots coalesce input; capacity is checked before receive.
Renderer imports stay local and retain reader leases through renderer completion. Source-device
completion separately protects SDK aliases and bridge-writer work. Resize retires old readers
and cached aliases before replacing its pool within the same reservation. Source snapshots report
actual extents; a plan must adopt that descriptor before copying the resized input. Missing or
mismatched input has zero storage (black for opaque descriptors), with explicit source state.

Each input owns one reader lease per admitted bridge slot. Multiple prepared targets can copy
the same immutable publication using that device's existing ownership, extending retirement
to the latest submitted use. Keyed ownership is never acquired recursively. Exact allocation
identity fences reuse; a replacement cannot displace an unfinished reader. This fixed-size
index needs no per-frame reader allocation or cross-source search. Source recovery publishes
the actual import/ownership error before signaling its worker; real device status is queried
separately rather than manufacturing a device-removal HRESULT. The
[plan-transition regression](../../docs/progress/mapgyver-m03-qualification-runner.md#shared-reader-ownership-across-plan-generations--2026-10-30)
checks two targets' pixels, latest-fence retirement and truthful logical-fault recovery.

Changing a retained binding requires a fresh logical texture ID; reusing an ID with a different
subscription is rejected without changing the live generation. Identical bindings are shared
across generations. Unused subscriptions cancel asynchronously; they consume capacity until
their worker has actually disconnected after native cleanup. Source/worker pressure returns an
explicit resource-pressure outcome without waiting for retirement on the renderer. Source
snapshots carry a subscription UUID in addition to device/texture/session identity.

A retiring source session withdraws its latest publication, enters `Recovering` and invalidates
its observation guard before draining source-device completion. Reader, bridge and SDK
ownership remain retained through their respective completion fences. Query failures remain
visible in source metadata; cancellation and a renderer release acknowledgment do not establish
source-device completion. A persistently failing or blocked drain can still retain ownership
indefinitely; bounded stuck-drain recovery remains unqualified.

`D3d11Control::set_source_notifier` wakes a preparation owner on source state/layout/session/pool
or registry changes, excluding per-frame counters. `source_observations` captures metadata and its
layout revision guard under one metadata lock. Descriptor/session/pool changes, retirement withdrawal
and final stop invalidate the guard; availability alone does not, avoiding warmup starvation. An app
rechecks availability before publication; native missing-source handling supplies zero storage after loss.
MapGyver's app-owned channel publisher consumes these APIs; Golden imports no app model types.
The SDK can transiently open a changed foreign alias
before post-update validation; owned pools and published descriptors stay bounded, but this is
not a hard driver-residency guarantee.

The M03 recovery gate remains open for genuine device-removal qualification and bounded recovery
from stuck source/driver drains.
Physical backing-buffer resize, production output/editor integration and sustained native
qualification retain their [M03 acceptance gates](../../docs/progress/mapgyver-status.md#m03-acceptance-items).
The [input/native evidence](../../docs/progress/mapgyver-m03-render-foundations.md#bounded-spout-inputs--2026-10-29)
closes the native execution mechanism gate; full native qualification stays open.

Run `cargo test --locked -p golden_video -p golden_time -p golden_io` for contract, worker/null
runtime and shared-time coverage. On Windows, add `--features golden_video/d3d11` for native
contracts and real shader compilation. Hardware cases remain separate explicit runs; their finite
pixel oracles use readbacks outside the production frame path. The owned host consumer runs
with `cargo run --locked -p MapGyver --features native-video-smoke --bin native_video_smoke -- --adapter <index>`
and reports the observed adapter/LUID. Its API-acceptance and retirement checks do not establish
scanout, delivery, physical/DPI correctness or Q01/Q02 budgets.

For cross-process input verification, first build `native_source_fixture` with the same MapGyver
feature, then run `cargo test --locked -p golden_video --features d3d11 -- --ignored --nocapture --test-threads=1`.
The owned fixture accepts only finite test commands and uses GPU-generated colors; its control
pipe carries no video frames. The tests cover preparation/cancellation, bridge pressure, static
frame reuse, disconnect/reconnect, bounded resize and oversized-source rejection.
