# Golden Video

Golden Video owns app-neutral video execution contracts. MapGyver owns media/channel identity,
authored size policy, source selection, mapping and editor behavior. The desktop host owns native
window construction and WebView UI-thread access. Video plans depend on neither app nodes nor Tauri.

The implemented foundations are:

- `identity.rs` names logical textures, passes and output bindings. These are plan identities,
  separate from persistent media/channel IDs and native resource ownership/generations.
- `resource/` declares nonempty texel extents, storage, RGB transfer encoding and alpha convention.
  Source metadata is explicit; renderer-internal targets use linear-light premultiplied alpha.
- `color/` validates finite SDR premultiplied clear colors and unit-range opacity.
- `plan/` validates the complete submitted dependency graph and caller-supplied limits, then
  compiles only demanded sources/passes in deterministic dependency order. Multiple output
  bindings can share one texture without duplicating its producer or logical storage charge.
- `runtime/` owns bounded publication, preparation, frame scheduling, generation installation
  and completion-based resource retirement. Its public `VideoRuntime` currently starts the
  pixel-free null qualification backend through that same control/ownership path.

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

D3D11/Spout execution, native recovery/qualification, source/output health, app media/channel
integration, persistent assets and preview bindings remain in the [M03 acceptance gates](../../docs/progress/mapgyver-status.md#m03-acceptance-items).

Run `cargo test --locked -p golden_video -p golden_time -p golden_io` for contract, worker/null
runtime and shared-time coverage. Native GPU performance and rendered-pixel correctness require
their separate qualification evidence.
