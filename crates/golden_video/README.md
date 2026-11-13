# Golden Video

Golden Video owns app-neutral video execution contracts. MapGyver owns media/channel identity,
authored size policy, source selection, mapping policy and editor behavior. The desktop host owns native
window construction and WebView UI-thread access. Video plans depend on neither app nodes nor Tauri.

Copy-only D3D11 input workers use explicit GPU fences for source completion; the selected
device must expose `ID3D11Device5` / `ID3D11DeviceContext4`. This production path must progress
without GPU timing diagnostics. Renderer/output completion is a separate event-query owner.

The implemented foundations are:

- `identity.rs` names logical textures, passes and output bindings. These are plan identities,
  separate from persistent media/channel IDs and native resource ownership/generations.
- `resource/` declares nonempty texel extents, storage, RGB transfer encoding and alpha convention.
  Source metadata is explicit; renderer-internal targets use linear-light premultiplied alpha.
  These data-only descriptors and the sampling filter derive Serde/TypeScript contracts, so app
  authoring DTOs reuse their canonical definitions. Deserializing an empty extent is rejected.
- `color/` validates finite SDR premultiplied clear colors and unit-range opacity.
- `decode/` (optional `ffmpeg` feature) demuxes local files and owns software decoder/frame
  lifetimes on a caller's worker. Original timestamps, planes and colour metadata survive;
  exact stream-timestamp seeking flushes and reuses a reading codec/pool; drain/EOF seeks
  replace the codec to discard delayed drain state. Returned frame references remain valid
  across either path. Explicit admission limits and
  poisoned-error state prevent unbounded packet work or false clean completion. It does not
  own app transport, GPU readback or implicit colour conversion.
  `DecodeLimits::software_threads` explicitly selects one, two or four bounded software codec
  workers. Only advertised slice threading is enabled; opened contexts exceeding the admitted
  count or enabling frame threading are rejected. HAP keeps its compressed parser and native
  hardware decode stays serial. This preserves decoded-frame queue semantics; third-party
  scratch/stack memory remains separate from retained-frame admission and must be observed.
  Software colour conversion retains one worker-local RGBA scratch frame for the current
  input format/extent. Published tight images own separate immutable bytes, so retained readers
  survive scratch reuse and layout changes; this removes repeated scratch allocation without
  adding a frame queue or changing the source storage allowance.
  The native source budget can opt into lifetime software RGBA and compressed HAP upload
  totals before any producer starts. Actual payload calls count even if their completed images
  are never sampled; busy/rejected calls and resident GPU copies add no CPU payload. Coherent
  totals survive storage retirement and clones share a budget identity. Default playback leaves
  these counters unmeasured and adds no per-frame accounting lock. Totals describe all producers
  in that budget; they do not establish per-channel PTS uniqueness or phase-boundary throughput.
  [Pinned Windows dependency and checks](../../tools/mapgyver/ffmpeg/README.md) describe the
  current boundary and remaining hardware/worker/package integration.
- `plan/` validates the complete submitted dependency graph and caller-supplied limits, then
  compiles only demanded sources/passes in deterministic dependency order. Multiple output
  bindings can share one texture without duplicating its producer or logical storage charge.
- `plan/mapping.rs` prepares checked crop/projective geometry for ordered compositing layers.
  Coefficients are solved in f64 and admitted only after checking their f32 corner correspondence
  and positive denominator branch. Normalized `Point`, `Quad` and `UnitRect` are canonical
  Serde/TypeScript contracts; applications own their authored meaning and selection policy.
  Native mapped layers inverse-project target coordinates before sampling the source crop,
  preserving the accumulated backdrop outside the quad. No intermediate texture per layer is
  added; composition keeps its existing shared ping-pong storage. Unmapped draws retain the
  ordinary pixel entry point; the mapped pixel entry point is prepared with the pipeline.
- `plan/pattern.rs` provides checked grid and eight-bar procedural layer sources alongside
  texture sources. The mapped shader evaluates patterns directly at target resolution, with no
  source texture, intermediate calibration image, decoder or CPU pixel transport. Applications
  choose cell counts, palettes and activation policy. Procedural draws consume the same submitted
  draw/dependency admission units as sampled layers. Both shared shader constant buffers are
  charged by native resource accounting; the configured resource ceiling is unchanged.
- `plan/envelope.rs` provides checked Linear/SmoothStep opacity factors evaluated from each
  submission's Golden Time observation. Layers multiply authored opacity by that factor on the
  native render worker, without frame-driven plan publication or heap allocation. Fades use the
  authority's monotonic coordinate; show transport changes do not restart them. Replacing the
  authority settles to the target instead of comparing unrelated origins. Applications own
  transition policy, interruption weights, source demand and completion cleanup; Golden retains
  the resources of replaced generations until their actual GPU uses retire.
- `runtime/` owns bounded publication, preparation, frame scheduling, generation installation
  and completion-based resource retirement. `VideoRuntime` starts either the pixel-free null
  qualification backend or the opt-in Windows D3D11 backend through the same control/core path.
  The native render worker opts into Golden IO's worker-owned high-resolution deadline wait.
  Its timer and notification event retire with that context; saved notifiers retain neither.
  Ordinary workers keep their existing waits. No busy spin or global timer-resolution change
  is used, and OS wake precision is not a hard real-time or display-deadline guarantee.
  `RuntimeConfig.additional_frame_rates` contributes independent absolute deadlines rather
  than quantizing slower consumers onto the fastest clock. Native `D3d11Config.output_rates`
  supplies per-consumer delivery clocks and automatically adds their runtime deadlines.
  Output conversion/admission observes those clocks even during cached presentation redraws;
  show transport does not reset their phase. Apps own display-following or explicit-rate policy.
- `preview/` owns metadata-only session negotiation, full-layout replacement, installation status,
  host geometry conversion and coalesced leased registration. Apps resolve opaque source keys;
  neither app-domain types nor native handles belong in these DTOs.
  Resource-routing generations fence stale source/view installation. Geometry-only updates use
  checked `RegionPlacement` bindings sampled by the render worker, without preparing another
  resource generation. Shared background/image passes sample one placement per submitted batch.
  Full metadata generations still require a matching sampled/accepted frame before acknowledgement.
  Separate per-view lifetimes retain valid pixels across source swaps and unrelated registrations;
  removal or overlapping painter-order changes withdraw affected regions.
  Host geometry, session replacement, lease expiry and close withdraw every destination. Region
  fences are checked by the render worker independently of source admission and other outputs.
  Live placement storage is fixed per admitted view and charged to retained plan metadata.
  Placement revisions wake the native renderer independently of the media deadline. Preparation
  builds fixed dependency masks; changed geometry redraws its affected presentation passes and
  outputs from admitted cached textures. Source reception and unrelated passes/outputs retain
  their content cadence. Downstream presentation work uses the last content time, and does not
  increment the media frame counter or replace its `FrameObservation`.
  Redraws use the same in-flight limits, completion tokens, resource claims and project/device
  fences as content submissions. Sampling alone cannot confirm delivery: pending state clears
  only after the affected endpoint accepts the corresponding use. Pending GPU/ready-output
  admission retries on the dedicated renderer worker after 1 ms; unavailable/recovering outputs
  retry after 100 ms. These deadlines stop when accepted, and unrelated endpoints do not impose
  their retry delay on the changed view. No engine/UI polling or new resource generation is
  needed for unchanged-capacity geometry. This contract permits high-refresh presentation;
  real-input scanout pacing and latency still need physical qualification.
- `RegionPlacement::with_tile_capacity` reserves up to 2,048 checked image rectangles sharing
  one `BlitRegion` texture binding. `update_tiles` atomically replaces the bounded set; empty
  tiled placements draw no image. Each tile has its own normalized destination and source crop.
  The render worker samples the set once per completion token, without resource preparation,
  another source upload or decoder. Tiles retain their declared painter order and the binding's
  visibility lifetime. Latest, sampled and output-accepted arrays are charged at the reserved
  capacity; source, pass, dependency and texture-memory limits retain their existing meanings.
  This is a generic native draw primitive; preview negotiation and app viewport policy own
  admission and projection into it.
  Preview protocol version 5 admits passive `tile_groups` inside one logical registration.
  Each group has an opaque source, CSS-relative rectangles and fixed repetition capacity;
  the parent source is an owner key and creates no image demand. Groups have no interactive
  overlays. Host clipping preserves each source crop. One layout admits at most 2,048 groups
  and rectangles and 4,096 reserved slots. The default remains 64 registrations; consumers
  may negotiate fewer (MapGyver uses eight). Geometry updates reuse each source binding;
  group removal and session expiry withdraw retained placements. Installation waits for the
  parent marker and every group to be sampled through the accepted output completion token.
  The parent paints no opaque aperture over its child images.
- `native/`, enabled by `d3d11` on Windows, owns caller-selected adapter/device creation, the
  shader pipeline, persistent window/Spout output pools and device-scoped event completion.
  `D3d11Config` contains endpoint metadata; native interfaces stay on the render worker.
  Its four fixed HLSL programs are compiled by this crate's `build.rs` and embedded as DXBC.
  Device initialization/recovery only creates device-owned shaders/state from those bytes;
  it does not recompile constant HLSL or retain a process-global compiler cache. Entry points,
  Shader Model 5 profiles, flags and source name are unchanged and regression-checked byte for
  byte against the former compiler path. Shader edits automatically rebuild the generated
  module. Native D3D11 builds currently require a Windows build host for the system compiler;
  portable plan/null builds skip shader generation. Qualification fixtures with their own
  marker programs retain their explicit test compiler path.
  Public `SpoutProbe` observes an exact sender on an IO worker without a renderer/window.
  Its explicit-adapter constructor preserves caller device policy. `new_for_sender` instead
  resolves a device by opening the current sender's shared handle before receiving-image
  allocation, for renderer-independent readiness checks. Candidate devices submit no image
  work; one bounded receiving image remains charged through completion. Endpoint replacement
  retires that image before selecting the replacement's device.
  Probe copies and qualification staging use the same explicit GPU timeline fences as source
  receivers, so a copy-only context can complete without renderer draws or timing queries.
  Read-only `adapters()` reports PCI identity, memory capacities and adapter-owned GDI outputs.
  `preferred_adapters()` preserves Windows' high-performance order independently of raw
  enumeration indices. `select_adapter(AdapterPreference)` resolves automatic hardware selection
  or an exact explicit LUID; an absent/software explicit choice never falls back. Hosts pin the
  selected identity through renderer recreation so shared textures cannot silently cross GPUs.
  `AdapterId::key` / `from_key` expose exact `dxgi:hhhhhhhh:llllllll` tokens for host preferences
  and observations. A LUID can change after OS/device re-enumeration; such a saved choice becomes
  unavailable and requires explicit re-selection rather than a label/index-based substitution.
  Display enumeration errors are explicit and do not prevent device creation or masquerade as
  an empty/headless output list. DXGI rectangles follow the caller's DPI awareness; the host's
  DisplayConfig catalog and actual window bounds own physical-placement verification. Reported
  adapter memory capacities do not substitute for current DXGI usage/budget observations.
  Sender-only checks allocate no graphics storage. Image checks reserve a caller-shared payload
  budget before one receiving texture, reuse the native receiver and require actual copy completion.
  Endpoint replacement withdraws evidence; cleanup retains storage until completion/device removal.
  Apps own startup deadlines, polling/backoff, project/launch fences and process policy.
- `qualification/` owns Rust-generated metadata for the held native-preview test fixture.
  The opt-in Windows `qualification` feature also exposes GPU source/consumer marker fixtures;
  app-owned runners select scenarios and own processes, without putting product policy in Golden.
  Its bounded owned-desktop capture authenticates foreground HWND/PID, client bounds and DPI,
  and reports pre-input/readback QPC values. DXGI's desktop-wide `ProtectedContentMaskedOut`
  notice is retained as metadata, rather than rejecting an unrelated owned region. Windows'
  already-masked pixels stay masked; consumers must establish the actual expected image and
  overlay from the exported region. The notice never substitutes for pixel evidence. See the
  [DXGI frame contract](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_outdupl_frame_info).
  Readback timing bounds do not establish physical scanout latency or high-refresh pacing.

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
`GpuFrameBudget::snapshot` reports one coherent source-admission total and peak, separated into
decoder arrays, hardware conversion producers, HAP producers and software-upload producers.
Each allocation remains charged once through the last retained frame, shared image or reader;
cloned consumers do not duplicate its bytes. Decoder arrays include their aligned NV12/P010
plane storage and fixed pool count. Producer groups include conversion input/constants and
their three output slots. These are logical admitted payloads, not driver allocation sizes or
CPU decoder scratch/stack memory. All classes share the original source-storage ceiling.

Snapshots are sparse observations; exact written/drained/queued accounting requires quiescent
producers. Disabled runtimes allocate no timing queries. Drops, invalid/skipped/uncollected timing
and missing OS counters remain explicit.
`DiagnosticCapture` optionally drains these same events to a caller-owned writer on a separate
I/O thread. Its fixed4096-row batch, configured byte/duration ceilings and explicit final health
bound capture independently of rendering. Sparse snapshots include native allocation ownership,
process counters and process-local DXGI observations. Opaque decoder/driver overhead remains
unknown. `DiagnosticCapture::spawn` accepts an optional shared source budget and writes
its `source_gpu` snapshot on the capture I/O worker. A missing budget is explicit null; retaining
the budget observer retains no allocation. MapGyver supplies its existing budget across source/
device generations. Stop producers before `finish`, inspect its I/O result and require a terminal
`capture_end` with no queued/dropped rows or duration cutoff. A footer alone cannot certify a
successful final flush. Capture owns metadata only; finish/drop it on an outer I/O host, never
on a rendering/UI worker. It shares the existing `kind:diagnostic,event` JSONL envelope.
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
Image/GPU inputs additionally require the exact stream lifetime. Their copied sequence and
pixel metadata belong to the allocation, so a temporarily unavailable sample during a compatible
rebuild retains the prior pixels and tags together. A different stream starts cold; explicit
withdrawals still clear. All writes remain ordered on the same immediate context. Immutable
images share storage by exact prepared payload and descriptor, across logical input IDs. Each
uploaded texture pins that payload lease, preventing address reuse while a cache entry can be
upgraded; equal app asset IDs alone do not establish equal content. Expired weak entries are
pruned at preparation. The index owns neither GPU storage nor asset leases. Core reservations
still charge the full declared generation footprint, preserving the existing conservative bounds.

`native/backend/cache.rs` retains unchanged render-pass contents using the actual last-write
completion token on each allocation. A changed source invalidates its dependents; another
compatible generation writing a shared target invalidates its former contents. New resources
start cold. The cache adds bounded dependency metadata, without GPU storage or input leases.
Live region placement/visibility, opacity envelopes, clock-dependent Canvas patterns and
untracked external inputs still render. Output conversion, presentation cadence and completion
fences remain independent of pass reuse. `FrameCpuTrace` reports rendered/reused logical pass
counts; these counts do not establish a frame-rate or source-age improvement.

Persistent Spout conversion targets use that same source/target-write invalidation rule.
An unchanged converted image still goes through ordinary output admission and every due SDK
delivery, retaining real frame counters, endpoint recovery and completion ownership. Window
back buffers redraw for each admitted Present; direct untracked external inputs never reuse.
`OutputTrace::conversion_reused` explicitly distinguishes a reused conversion from missing
conversion timing. A reused conversion has no draw interval, while its delivery/API observation
still records the actual attempt. The cache owns metadata only, with no extra texture or lease.

Qualification consumers can select `BinaryFrameCodeSpec` to sample an explicitly declared
black/white code and separate sentinel. The existing append-only twelve-word GPU log retains
the code and sampled intensities; one final post-window staging readback independently checks
their agreement. `ConsumerFrame<M>` flattens typed marker payloads, preserving ordinary
CardStamp JSON fields without interpreting Video codes as cards. Invalid rows remain errors.
Prepared `OutputSourceTrace` facts declare each output's exact external dependencies and count,
while `SourceUseTrace` retains actual RGB descriptors and Image/GPU publication identities.
These opt-in observations support exact SDK-version/retained-PTS joins, including equal-sized
files and resident frames; incomplete tables or ambiguous versions do not prove provenance.

`D3d11Control::allocations_snapshot` separately observes actual committed native owners. Each
production plan texture carries one metadata release guard after its native texture/views;
Rc sharing, reused generations and weak cache entries add no charge. Source inventories report
the committed three-slot bridge pool, its actual descriptor and retained renderer import/view
owners/readers. Imported aliases reference source storage and add no owned texel payload. Resize
retains its existing real GPU/alias retirement handshake; source zero-count publication follows
actual bridge object/handle release. Output inventories retain checked DXGI back buffers or the
accepted SDK shared-image identity through endpoint recovery/retirement. Partial SDK owners
remain unknown until a successful send establishes their layout, rather than reporting zero.

The observer retains only counts/layouts/identities; it adds no native resource opens, per-frame
GPU query, pixel readback or persistent COM ownership. Plan create/release counters update at
their existing object lifetimes; source/output metadata remains worker-owned. Sources publish
independently, so construction transients are not an atomic measured peak. The known total is
validated descriptor texel payload, not physical driver allocation size, row padding or residency.
Those remain explicitly unknown and separate from process-local DXGI usage/budget observations.
Native qualification captures the public inventory at sampled snapshots and after retirement;
the renderer lifecycle fixture also records each checkpoint and baseline/final inventories.

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

Each collected GPU query also retains machine-QPC bounds: immediately before submitting its
first marker and after all nonblocking query reads succeed. Offline analysis joins the exact
retained app source-worker batches to each renderer/output batch and encloses all three scopes
from the earliest submission to latest collection, with one CPU-counter tick for quantization.
This can prove the unchanged full-path GPU budget when the complete conservative upper bound
fits; it never compares independent GPU timestamp origins. The enclosure includes queueing,
CPU command-feed gaps, retained-frame age and collection delay. A bound above budget is
inconclusive, not a GPU failure. Correlated context span sums remain descriptive and cannot
replace that bound or a calibrated critical-path measurement. Missing collection or clocks
stay unknown; diagnostic query failures do not change rendering, admission or retirement.
Disabled diagnostics create no timing object and add no QPC calls. No additional GPU query,
flush, native-resource open, wait or global clock/power-state change is introduced.

Optional window diagnostics retain the actual endpoint incarnation and the three DXGI
swapchain interface aliases with each Present API bracket. Aliases are captured during cold
endpoint creation; disabled diagnostics perform no additional interface queries. These are
non-owning diagnostic numbers, never project/control handles and never opened by analysis.
The production runner can opt into a pinned PID-filtered PresentMon collector. Offline joins
require exact PID, alias and native API interval before associating a software display event
with a renderer token, source image or authored plan. Missing/dropped/ambiguous records stay
explicit. Software-display backlog is separate from native buffer occupancy, GPU queue time
and physical scanout; it cannot certify those remaining budgets.

Accepted output traces also expose worker-owned `OutputQueueBound` metadata. Window creation
reads back the actual two-buffer flip-discard descriptor and latency-one setting; each accepted
Present records its retained readiness permit and the same zero sync interval used by the call.
The complete native back-buffer count is a conservative app-publication capacity bound, rather
than an estimate made from pending software display events. Spout records the actual accepted
shared image/version: the pinned SDK's single-image send path overwrites that one image under
its ownership lock, coalescing older versions without an app-owned receiver FIFO. This uses
existing send metadata and adds no per-frame resource opens or CPU pixel reads. In particular,
the SDK's `GetSenderTexture` is a receiver-side, potentially allocating helper and is not used
to inspect an owned sender.

Offline qualification requires every accepted publication in each selected output cohort to
carry the matching API/endpoint or shared-image identity. Unknown metadata stays unavailable;
capacity above the target is insufficient evidence, not a measured occupancy failure. Verified
capacity at most two is sufficient for the publication-queue ceiling; exact instantaneous
occupancy and consumption timestamps remain unknown. This does not qualify GPU queue time,
critical path or scanout. The DXGI contracts are
[GetMaximumFrameLatency](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getmaximumframelatency)
and [frame readiness](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getframelatencywaitableobject);
the SDK source/provenance is retained in the binding's ownership notes linked below.

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

When replaced generations still hold GPU reservations, one compiled replacement may wait for
retirement instead of rejecting a valid edit. No candidate resources are allocated while it waits;
admission retries only after reserved bytes or device identity change. New publications supersede
that metadata, and source/project/time/shutdown fences still settle each request exactly once.
`RuntimeSnapshot::waiting_for_resources` distinguishes this stage from CPU preparation and GPU
installation. The active frame stays installed. A replacement that cannot coexist with the active
generation even after all retirement completes still rejects with `ResourcePressure`; ceilings
and completion requirements remain unchanged.

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

`native/outputs/scheduling.rs` first polls admission and delivers every immediately ready output.
Initially busy windows are retried afterward in plan order, before the same submission's GPU
completion marker. Only the first deferred window may use a one-millisecond kernel timeout;
the others poll, so requested waiting does not scale with output count. This runs on the native
render worker, never the engine/UI thread, and retains preallocated retry storage. OS scheduling
and opaque native APIs can exceed a requested timeout; the existing timing qualification still
applies. Hidden windows do not wait or consume permits, and Spout never waits on a foreign
consumer. An already-issued Present is never retried. Each output records one final outcome,
with actual API brackets kept separate from admission waiting; no frame/fence is split.

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
latest desired layout, and keeps opaque covers outside an established preview's retained overlap
until the latest native installation is confirmed. Navigation does not withdraw an installed frame
or report capability failure while the replacement geometry is pending.
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

`ExternalTexture::source` selects an immutable image or `ExternalSource::Spout`: exact selector,
metadata policy, polling interval and admitted maximum extent. `SpoutMetadata::Declared` fixes
storage/transfer/alpha. `Discover` admits native BGRA/RGBA byte and RGBA16F/RGBA32F storage.
It interprets byte RGB as sRGB and floating RGB as linear, both with straight alpha, because
classic Spout carries no independent transfer/alpha metadata. Float imports retain their native
8/16-byte texels through the receiving bridge; their storage charge includes that full precision.
Unsupported native formats are rejected rather than guessed. Bindings travel inside the same
revisioned plan as their consumers. `D3d11Config::inputs` bounds at most 32 overlapping workers
and one aggregate source-storage budget. Declared subscriptions reserve their maximum before
worker creation; discovered subscriptions reserve actual layouts before bridge allocation.
Each reservation charges three bridge images and one sender alias. The public checked
`InputLimits::source_storage_bytes` uses the same slot count as allocation and import caches;
apps do not duplicate pool-layout arithmetic. Capacity remains held through actual retirement.
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
and cached aliases before replacing its pool within the same aggregate budget. Source snapshots report
actual extents; a plan must adopt that descriptor before copying the resized input. Missing or
mismatched input samples zero source pixels (black for opaque descriptors), with explicit source state.

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
Discovered allocation pressure reports its native out-of-memory cause and retries through
the source worker's existing bounded recovery backoff, independently of healthy peers.

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
Qualification-only unit tests add a per-runtime observation gate after actual D3D11 query
completion. The held-observation test verifies retained ownership, deferred project acknowledgement,
prompt close and independent healthy output; it does not simulate or qualify a physical GPU hang.
The source recovery fixture has a separate non-removal preflight. Genuine removal uses an exact,
ignored test guarded by `GOLDEN_VIDEO_EXTERNAL_DEVICE_REMOVAL=1`, records an actual
`GetDeviceRemovedReason` error and waits for an externally triggered event. It never resets a GPU
itself. Such a reset can affect other applications and requires an explicitly approved test window;
software compilation and a passing preflight alone cannot close the recovery gate. See the owning
ledger for exact test names, results and outstanding qualification.
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
