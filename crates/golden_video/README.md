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
- `native/`, enabled by `d3d11` on Windows, owns caller-selected adapter/device creation, the
  shader pipeline, persistent window/Spout output pools and device-scoped event completion.
  `D3d11Config` contains endpoint metadata; native interfaces stay on the render worker.

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
plan targets, compositing scratch and output conversion textures separately. Its owned texture
pool/constant-buffer charge is not measured driver residency, padding or native-object overhead.

`golden_io` supplies bounded workers, cancellation, publication fences, readiness and off-publisher
input disposal. The runtime retains one latest desired plan, one CPU preparation, one prepared
candidate, at most two submitted batches and two retiring generations. Request credits bound
accepted work plus undrained terminal outcomes; a stalled consumer cannot block resource service
or silently discard admitted results. Project/device/publication identities fence stale work.
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

The native pipeline executes the demanded DAG with clear, sample and ordered Normal/Add/Multiply/
Screen composition. It decodes sRGB and premultiplies coverage before filtering, then converts
to each explicit output color/alpha contract. Output pools persist across plan generations:
windows retain two fixed-size BGRA8 backbuffers; Spout senders retain one SDK shared texture and
an AddRef of the exact caller device. Window size changes use DXGI stretch, without reallocating
backing buffers. The host retains each borrowed HWND through successful runtime retirement.

Every queued command batch ends in a preallocated D3D11 event query, including output failures
and unwind. Output results are bounded observations, so a busy window does not abort other
consumers or abandon already queued GPU uses. Spout API acceptance does not prove delivery:
the pinned SDK can wait up to 67 ms on its access mutex and return success after skipping a copy
on timeout. This limitation remains part of native cadence qualification.

`ExternalTexture::source` carries an optional `SpoutSource`: exact selector, required storage/
color/alpha, polling interval and admitted maximum extent. Bindings travel inside the same
revisioned plan as their consumers. `D3d11Config::inputs` reserves `InputLimits` before native
allocation: at most 32 overlapping source workers and an explicit descriptor-storage capacity.
That capacity remains charged while unused; each subscription consumes five maximum-size
images (capture, three bridge slots and sender alias). Old/prepared/retiring subscriptions share
these limits. Each source owns a Golden IO worker and a separate
device on the selected adapter. Prepared resource generations hold demand leases, warming inputs
before activation; discarding the last demand stops receive polling. SDK work never runs in the
renderer or engine loop.

The public receive-into API uses admitted capture storage instead of an SDK-allocated receiving
copy. Three NT-handle/keyed-mutex bridge slots coalesce input; capacity is checked before receive.
Renderer imports stay local and retain reader leases through renderer completion. Source-device
completion separately protects SDK aliases and writer/capture work. Resize retires old readers
and cached aliases before replacing its pool within the same reservation. Source snapshots report
actual extents; a plan must adopt that descriptor before copying the resized input. Missing or
mismatched input has zero storage (black for opaque descriptors), with explicit source state.

Changing a retained binding requires a fresh logical texture ID; reusing an ID with a different
subscription is rejected without changing the live generation. Identical bindings are shared
across generations. Unused subscriptions cancel asynchronously; they consume capacity until
their worker has actually disconnected after native cleanup. Source/worker pressure returns an
explicit resource-pressure outcome without waiting for retirement on the renderer. Source
snapshots carry a subscription UUID in addition to device/texture/session identity.

App-owned channel demand/publication and source-observation fencing remain integration work.
The SDK can transiently open a changed foreign alias
before post-update validation; owned pools and published descriptors stay bounded, but this is
not a hard driver-residency guarantee.

Dynamic preview geometry, physical backing-buffer resize, native device-loss proof, full
source/output recovery, app media/channel integration, persistent assets and sustained
native qualification remain in the [M03 acceptance gates](../../docs/progress/mapgyver-status.md#m03-acceptance-items).
The [input/native evidence](../../docs/progress/mapgyver-m03-render-foundations.md#bounded-spout-inputs--2026-10-29)
closes the native execution mechanism gate; full native qualification stays open.

Run `cargo test --locked -p golden_video -p golden_time -p golden_io` for contract, worker/null
runtime and shared-time coverage. On Windows, add `--features golden_video/d3d11` for native
contracts and real shader compilation. The ignored hardware pixel test is a separate explicit
run; it uses finite readbacks outside the production frame path. The owned host consumer runs
with `cargo run --locked -p MapGyver --features native-video-smoke --bin native_video_smoke -- --adapter <index>`
and reports the observed adapter/LUID. Its API-acceptance and retirement checks do not establish
scanout, delivery, physical/DPI correctness or Q01/Q02 budgets.

For cross-process input verification, first build `native_source_fixture` with the same MapGyver
feature, then run `cargo test --locked -p golden_video --features d3d11 -- --ignored --nocapture --test-threads=1`.
The owned fixture accepts only finite test commands and uses GPU-generated colors; its control
pipe carries no video frames. The tests cover preparation/cancellation, bridge pressure, static
frame reuse, disconnect/reconnect, bounded resize and oversized-source rejection.
