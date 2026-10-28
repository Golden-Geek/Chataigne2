# MapGyver native qualification harness

This package owns M01 experiments only. It depends on public Windows/Spout bindings and has no
Golden engine, app node or production bootstrap dependency. Root launch still selects Chataigne.
The future production renderer and desktop presentation extension have separate owners in the
[MapGyver plan](../../../docs/plan/MapGyver_Implementation_Plan.md).

Inventory and source-smoke commands are noninteractive and open no windows. The desktop probe
below opens only its own windows, initially without requesting focus. All commands preserve
display settings. The executable itself sends no keyboard/mouse input; user-authorized
computer-use checks may exercise its own windows. Run from the workspace root:

```powershell
cargo build --locked -p mapgyver_native_probe
target/debug/mapgyver_native_probe.exe inventory
target/debug/mapgyver_native_probe.exe smoke 0
target/debug/mapgyver_native_probe.exe smoke 1
cargo test --locked -p mapgyver_native_probe --test native_smoke -- --ignored --nocapture
python tools/mapgyver/audit-spout-sdk.py
```

For the ordinary Tauri/D3D11 underlay and output probe, first install workspace npm dependencies
and build its isolated Svelte 5 fixture:

```powershell
npm run check --workspace mapgyver-native-probe-ui
npm run check:generated --workspace mapgyver-native-probe-ui
npm run build --workspace mapgyver-native-probe-ui
cargo build --locked -p mapgyver_native_probe --features desktop
target/debug/mapgyver_native_probe.exe underlay 0 8
```

Choose the current adapter index from inventory. `underlay ADAPTER SECONDS` accepts 8–3600
seconds and closes both windows after render-worker cleanup. Use a longer duration, such as
180 seconds, for manual observations. Closing either owned window also requests cleanup.
The default feature set retains the headless inventory/source-smoke path.

For longer interactive sessions, use the same qualification runner's explicit diagnostic mode:

```powershell
python tools/mapgyver/run-qualification.py 0 diagnostic --underlay --profile debug --seconds 900 --warmup 0 --samples 1
```

It captures the native stdout/stderr before compacting every resource row, records input hashes
and marks the result as interactive evidence. Early operator closure is normal for this mode;
fixed Q01/Q02 timed runs still reject early closure. Diagnostic mode cannot select qualification
or recreation and requires one session with zero warmup. Direct native pretty-printed reports
can exceed terminal output limits during long sessions; a truncated report is incomplete evidence.
Compaction reduces repetition but does not impose a transport-size bound: use a collector that
can retain the complete runner output for the requested duration. Never treat an omitted middle
as a complete native report or a performance pass.

Collectors with terminal size limits can request `--encoding gzip-base64`. Each output line is
a versioned JSON envelope containing the complete compressed result, decoded byte length and
SHA-256. Compression occurs after the native process exits, outside its measurement window.
It retains every row, integer QPC value, failure and unavailable counter; it does not change
the evidence schema or native workload. Default output remains plain JSON. Drain each result
as it arrives and keep stdout/stderr distinct: progress records use stderr, which a merged
terminal collector can interleave inside a large stdout packet. Compression reduces size but
does not guarantee a bound for arbitrary data or several accumulated results. A missing or
truncated envelope cannot qualify a sample. A collector that demultiplexes known progress
records must validate the restored envelope's complete length/hash before interpreting it.

```powershell
python tools/mapgyver/run-qualification.py 0 q01 --encoding gzip-base64
```

The runner's `decode_evidence(packet)` validates envelope version, base64/gzip, decoded length
and hash before parsing. Saved series artifacts retain these envelopes under `packets`; decode
each with that helper to inspect the original evidence. The digest detects damaged collection;
it does not prove that the native measurements satisfy a budget.

The desktop fixture has two transparent native preview regions with DOM drag handles, an opaque
sidebar/header, menu and modal dialog. Controls change docking width, preview visibility and
WebView zoom; the stage scrolls. CSS-client bounds and ancestor/viewport clipping travel as
metadata. The UI-thread host converts them using actual WebView2 rasterization scale × zoom.
Separate full sampling viewports and scissors preserve source coordinates during clipping.
The DOM viewport excludes scrollbars. Layout publications retain only the latest revision.
Menus stay outside canvas layout flow, and visibility changes schedule a geometry publication.
Native input testing caught the former sticky menu's collapsed-margin displacement; opening a
menu must neither move preview frames nor leave old native geometry behind.

One render worker receives both GPU sources, samples them side-by-side into a 1920×1080
texture, sends it through Spout, and presents it in a separate native window. The editor
underlay has its own nonblocking swapchain. No pixels travel through IPC, and no recurring
readback occurs in this path. A slow/occluded preview does not block on `Present`; actual
cadence, driver copies, color/alpha and memory budgets still need qualification.

Each underlay run also starts a separate `observe ADAPTER SENDER diagnostic` process on the selected GPU.
The render worker sends four finite checkpoint requests after submitting the initial, resized,
source-absent and recovered composites. The independent receiver samples each half of the
1920×1080 output: left red/green/dark/blue, right yellow throughout. Source reconnect and GPU
handoff can briefly expose the preceding composite, so each checkpoint permits at most four
diagnostic attempts; every attempt is retained. Wrong colors, changed output extent, a missing
sender, incomplete checkpoints or unclean consumer exit fail the run.

These diagnostic readbacks live only in the validator process. Each copies two isolated pixels
into a two-pixel staging row; it never reads a whole frame. Four checkpoints cap total readbacks
at sixteen. The worker never waits for a checkpoint acknowledgement during rendering. Bounded
JSON lines carry test controls/results over the child's owned pipes; they are not an app frame
transport. The report records separate render/observer process IDs, adapter LUID and samples.

The render worker reports CPU submission work, start intervals and GPU execution distributions.
A four-slot ring of timestamp/disjoint/event queries is polled without waiting or implicit
flushes. End markers are explicitly submitted before the CPU pacing sleep; otherwise the span
would include that sleep. A busy slot skips instrumentation and records the skip. Query reads preserve
`S_FALSE` readiness instead of treating a successful HRESULT as completed data. Each metric is
capped at 216,001 samples; invalid/pending/truncated counts remain explicit. Only shutdown
permits a 100ms query drain. These eight-second, un-warmed distributions include diagnostic
overhead and are not output-present cadence, end-to-end latency or Q01/Q02 qualification.
The GPU metric is elapsed GPU command-timeline span, including dependencies/submission gaps,
not shader occupancy or physical display scanout time.

```powershell
cargo test --locked -p mapgyver_native_probe --features desktop --test native_composite -- --ignored --nocapture
```

The test runs the independent consumer on every hardware adapter, checks complete GPU timing
accounting, and verifies that a missing output fails rather than producing a default success.

The host retains both HWNDs until the worker releases its SDK/GPU resources. Fixtures accept a
typed JSON stop over their owned stdin pipe. Normal completion drains bounded reports while
waiting for exit, with a five-second error deadline; abnormal owner drop retains the one-second
forced-stop fallback and cannot count as a clean qualification exit. The worker
checks that its three sender registrations disappear. Normal UI-thread window destruction
then ends Tauri's returning event loop; report delivery and error exits are retained.
Compiler and Windows SDK selection are captured by the same `cc` discovery used by the SDK.

Rust DTOs live in the tool's small library. `generate_probe_bindings` is an independent binary:

```powershell
cargo run --locked -p mapgyver_native_probe --bin generate_probe_bindings
cargo test --locked -p mapgyver_native_probe --lib
```

The first command prints canonical TypeScript. The UI's drift check compares that output with
`ui/src/generated.ts`; it does not rebuild or overwrite a running native probe. Geometry tests
cover fractional DPI edges, clipped sampling bounds, empty registrations and bounded rejection.
UI source uses relative layout units; Rust rectangles/texture extents deliberately use physical
pixels. The generated 16×16 test icon is a native resource, not a UI layout exception.

Tauri 2.11.5/runtime-wry 2.11.4/Wry 0.55.1 remain locked. Bare native `WindowBuilder` needs
Tauri's public `unstable` feature in this version; it is enabled only by this probe's `desktop`
feature. It is an experiment, not the future Golden native-presentation extension or a product
bootstrap. The procedural icon is generated in Cargo's output directory, with Tauri's supported
config overlay selecting it; no Chataigne branding/private resource is imported.

Select adapter indices from the current inventory; indices can change. Reports also include
adapter LUIDs. Software adapters fail the smoke explicitly. The integration test runs every
enumerated hardware adapter and tests software rejection. It is ignored by default because it
requires real local hardware; the command above explicitly enables it.

The smoke owns two separate source processes on the selected adapter. A sends 1920×1080 red,
resizes the live sender to 960×540 green at frame 90, unregisters at frame 180, and recreates the
same name at frame 210 with a blue 1920×1080 texture. B remains yellow at 1280×720. Both source
schedules end at frame 360 and target 60 sends/sec. Receiver SDK instances retain the selected
application device and automatically reconnect using explicit sender names. Generated names
include process/time suffixes; a name collision fails. The parent reaps only its owned children
and verifies that fixture registrations disappear before reporting success.

Four diagnostic staging readbacks verify A's three phases and B's color. All source generation
uses GPU clears; ongoing sends/receives use the SDK's texture API. Pixel readback is confined to
the smoke validator. Poll observations are **not distinct frame counts**: Spout's local settings
can disable its frame counter. This static-card lifecycle test does not qualify throughput,
frame age, alpha/blending, composition, full pixel accuracy or recurring-readback absence in a
future production path.

`d3d.rs` owns COM devices/textures and DXGI inventory. `spout.rs` fences the SDK's borrowed device
with Rust lifetimes, keeps each SDK instance on one worker and clones borrowed received texture
interfaces before another SDK receive. SDK destruction precedes device teardown. GPU waits and
final cleanup in this harness run on its dedicated process thread, never a Golden actor or WebView
thread. These internals are not a public production API; later reusable code must live at its
Golden owner rather than being path-imported from this tool.

DXGI output rectangles are observed under a temporary per-monitor-aware-v2 thread context.
Inventory memory queries precede device allocation. They are not measured scene peaks or a
complete DisplayConfig/DPI catalog. Preserve each report's actual values instead of assuming
CIM mode sizes or a discrete adapter's display ownership.

The pinned probe binding is `spout2-sys =0.1.1`, DX feature only. Its SDK/shim builds statically
with MSVC through `cc`; no downloaded runtime DLL is used. The published `spout2-rs` wrapper lacks
receiver device injection, so this harness exercises the public sys API directly. See the
[binding audit](../../../docs/progress/mapgyver-m01-native-feasibility.md) and retain
[Spout notices](THIRD_PARTY_NOTICES.txt) alongside any distributed probe executable.

## Fixed Q01/Q02 scene runner

`qualify ADAPTER q01|q02 SAMPLE_SECONDS WARMUP_SECONDS` selects fixed physical extents from Rust.
Q01 uses a 1920×1080 first source/composite/native output; Q02 uses 3840×2160. The second source
stays 1280×720. Both request unclipped 640×360 and 480×270 previews. The worker reports actual
output size and every sampled frame with mismatching geometry; a clamped/rescaled window does
not become a smaller passing workload. The output window moves before applying its physical
size, so a monitor DPI transition cannot silently rescale the requested extent. A 4K client
rectangle can extend beyond the available monitor; its allocation does not qualify 4K scanout.

Fixture shaders generate checker/grid regions, seven-segment channel/frame IDs, lifecycle colors
and four straight-alpha stripes. Only 32 bytes of uniform metadata are uploaded per source frame.
BGRA8 UNORM textures pass through the sampling shader without color conversion or alpha blending;
this tests raw transfer, not the future product's color/premultiplied compositing policy. Each
independent consumer checkpoint checks two opaque colors plus two interior 75%-alpha samples.
Bilinear-boundary pixels are deliberately excluded from the constant-stripe reference samples.

Timing collection starts after warmup; worker pacing and source recovery run throughout it.
One-second resource observations run on the render worker outside CPU/GPU submission intervals.
They record renderer/source/observer private bytes, working sets and handles, plus renderer DXGI
local/nonlocal current usage and budget. Before/after GPU COM-release process snapshots are
separate. Child reports now add source/observer logical texture and DXGI process observations.
WebView subprocess memory, per-texture physical residency and driver allocation retirement
remain unmeasured. Descriptor payload ownership is not a physical-residency measurement.

```powershell
cargo build --release --locked -p mapgyver_native_probe --features desktop
python tools/mapgyver/run-qualification.py 0 q01
python tools/mapgyver/run-qualification.py 0 q02
python -m unittest discover -s tools/mapgyver/tests -v
```

The runner defaults to three separate release processes per command, each with 30 seconds warmup
and a 120-second sample. It prints one JSON evidence object per run to stdout, captures current
inventory and hashes, and checks executable/source/assets remain unchanged. Every 1 Hz resource
observation is retained in rows with a shared column schema; missing counters stay null. Use
`--samples 1` for one explicitly incomplete series or `--profile debug --seconds 8 --warmup 2`
for a short diagnostic. It never writes files or changes display settings. Run one workload at
a time. Full native tests also include four short fixed-scene runs across the hardware adapters:

```powershell
cargo test --locked -p mapgyver_native_probe --features desktop --all-targets -- --include-ignored --test-threads=1
```

`tools/mapgyver/analyze-release.py ARTIFACT.json` reads a saved series with compressed `packets`,
validates every envelope and independently rederives the raw source-marker checksums, clocks,
counts and age distributions. It rejects mixed executable/source inputs, commands and adapter
identities; dynamic DXGI usage/budgets remain observations. A full series requires three release
results numbered 1/2/3 with the fixed 30+120-second durations. Failures remain failures.
The analysis reports the fixed CPU/GPU, logical-payload and consumer-age targets in their
measured scope. It retains call shortfall, lateness and busy/occluded counts without claiming
displayed FPS, combined physical memory or complete-process shutdown qualification.
GPU percentiles from incomplete query accounting remain observable statistics; they cannot
qualify the full GPU budget. The analysis preserves skipped/invalid/pending/truncated counters
and requires matching CPU/GPU sample counts before reporting that target met.

Schema v8 adds `generations[].operation_timing` and correlated GPU timeline outliers. Schema v9
adds GPU admission and a seventeenth operation, `gpu_admission`. The Rust registry in
`src/operation_timing.rs` owns the measured operation names and ordering. Each
frame records a local frame index, QPC stamp and scheduled-start delay. Operation offsets and
durations use monotonic nanoseconds; the QPC stamp is taken immediately before the Instant
origin, so cross-process alignment includes that small sampling skew. These are CPU wall
intervals, including any preemption inside a call, rather than exclusive CPU execution time.
Resource sampling is timed separately from the existing submission-work interval.

Only sampled operation durations contribute to their distributions, each capped at the existing
216,001-sample bound. Unexecuted draw/Present/resource operations remain absent, rather than
zero-duration observations. A CPU event is retained for a >=4 ms operation, >=16.666667 ms frame
or scheduled-start delay, an unavailable GPU-query slot, or a coalesced GPU-busy tick. Warmup and sample windows each
retain their first 128 events, count every omission, and retain the largest event separately;
the score is max(frame duration, scheduled delay). This is an instrumentation threshold, not a
new workload/acceptance budget. Recording and sorting happen on the render worker; the host UI
does not poll these records.

The existing GPU queries also retain up to 128 >=16.666667 ms timeline events plus the maximum,
with raw start/end ticks, frequency, originating frame/QPC and observed-completion QPC. No new
profiling queries, waits, pixel readbacks or uniform uploads are added by operation timing. A GPU timeline span includes
dependencies and CPU submission gaps; it does not establish GPU busy time or an internal driver
cause. A missing query remains missing. The Python analysis verifies bounds, frame counts,
operation ordering, raw ticks, maxima and omissions before interpreting the records. Original
captures retain their actual earlier schemas; absent operation data is never synthesized.

Schema v14 splits the former combined QueryEnd interval into `query_end` (profiling end
markers), `gpu_signal` (allocation serial bookkeeping and completion marker enqueue), and
`gpu_flush` (driver Flush and admission commit). These remain CPU wall intervals, including
preemption; they cannot establish exclusive driver execution time. The nineteen-operation
registry and stale-binary guard update together. Legacy reports keep their original meaning.

`FrameAdmission::signal` returns a must-use batch borrowing the admission gate until its
consuming `flush` call commits the reservation. The ordinary `submit` path performs both
steps in the same order. The renderer measures them separately without changing GPU work,
the two-batch ceiling, source selection or pacing. No wait, dummy command or extra flush is
introduced. The split is diagnostic instrumentation, not a claimed stall repair.

## Bounded renderer GPU submission

Schema v9 introduced `generations[].gpu_admission` with two persistent event queries. Schema v12
uses an owned D3D11 timeline fence, independently of the four profiling slots. `gpu_admission.rs` owns the tested
FIFO admission policy: at most two renderer frame batches are reserved or pending. Only a
completed oldest prefix can retire. `GetCompletedValue` polling neither waits nor flushes;
a driver call can still block internally. This bounds renderer submissions, not fixture
contexts, SDK internals, driver queues or displayed frames.

If both slots are pending, the tick coalesces before receive, composition, send or presentation.
All three cadence paths record Busy; observer draining, stop/deadline checks and normal pacing
continue. These misses remain counted against the original 60-Hz target. Operation traces and
cadence count scheduled attempts; CPU/GPU submission distributions count admitted work. A
coalesced tick has no profiling-query gap and no synthetic zero-duration GPU/SDK operations.

After every admitted batch, the runtime ends profiling markers, signals the next fence value,
then issues its final context `Flush`, even if profiling had no free slot. This moves the existing
final flush out of the profiler; SDK flushes remain unchanged. The headless timing isolation
case explicitly submits its markers too. Input SDK lifecycle calls now run on isolated source
workers as described below. Admission does not remove blocking SDK calls or driver stalls.

Sparse resource observations preserve `gpu_pipeline` counters and the fixed capacity. Missed
resource periods are skipped instead of causing a catch-up sampling burst. A successful final
context fence must leave no pending/reserved batches and equal submitted/completed counts.
The analyzer checks these invariants, send/timing counts and cadence accounting. The short
[release controls](../../../docs/progress/evidence/mapgyver-m01-gpu-admission-controls-2026-10-28.json)
exercise both workloads; historical evidence retains its original schema and fingerprints.
The later [three-sample Q02 series](../../../docs/progress/evidence/mapgyver-m01-gpu-admission-q02-release-2026-10-28.json)
and [independent analysis](../../../docs/progress/evidence/mapgyver-m01-gpu-admission-analysis-2026-10-28.json)
retain 7,200 accepted calls per path and complete timing coverage in each sample, with no native
GPU-busy coalescing observed. Earlier Q02 stalls remain retained; this cohort does not prove an
internal cause or a fix. The [native evidence](../../../docs/progress/mapgyver-m01-native-feasibility.md)
records measurements and remaining gates.

## Source lifecycle isolation

Schema v10 moves source SDK receive, resize, absence/reconnect and receiver destruction into
`source_worker`, with one caller-owned same-adapter D3D11 device/immediate context per source.
Endpoints and COM interfaces stay on their creating thread. Only immutable dimensions and
owned NT resource handles cross the latest-publication mailbox. This is an owned qualification
experiment, not a Golden production host or media protocol.

Each source has at most three live bridge slots. A slot with any mailbox/reader reference is
unavailable to the producer; published-but-discarded slots are reclaimed only after a successful
keyed-mutex acquisition. Resize replaces only a free, acquired slot. The producer copies its
SDK receiver image into the slot, flushes and releases key 1. The renderer imports on its own
device and acquires key 1 with timeout zero, keeping its previous image if the new slot is not
ready. Its retained frame releases key 0 after queued reads. Absent sources clear the publication;
the worker continues looking up the same explicit sender name for recovery.

Renderer metadata access uses `try_lock`; it never calls source SDK functions or waits for a
source worker during frame submission. `source_poll_a/b` replace the older `receive_a/b`
operation names. Native import/driver calls can still block internally; the architecture does
not promise a hard real-time driver boundary. Spout output send remains on the render worker.
The shutdown path fences renderer reads before releasing readers and joining source workers.
Workers then destroy their own SDK endpoints, drain allocations and release their devices.
Stop notification is published only after the renderer releases its current reader and clears
the mailbox. A worker can observe stop and enter final ownership checks immediately, without
another pacing sleep. The [shutdown-order regression](../../../docs/progress/evidence/mapgyver-m01-source-worker-stop-order-regression-2026-10-28.json)
holds publication ownership and verifies notification cannot race ahead of its release; it
fails with the previous order. Native stress and device reconstruction verify GPU teardown
separately. The earlier NVIDIA Q01 sample-3 failure remains retained with its original inputs.
Schema v11 also bounds each source context to two submitted batches independently of its
profiling ring. Its completion markers and timestamp queries use that source context only;
warmup/sample classification shares the renderer's absolute session window. Admission and
profiling use the same captured classification; no new receive starts after the session end.
The worker then awaits reader release before teardown. Every admitted
source tick submits a completion marker, even without a new image or an available profiling
slot. The copy handoff flush and final marker flush are explicit; SDK flushes remain unknown.

The bridge follows Microsoft's [shared NT-handle/keyed-mutex flags](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag)
and [AcquireSync contract](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgikeyedmutex-acquiresync).
The binding's generic success conversion loses positive `WAIT_TIMEOUT`/`WAIT_ABANDONED` results,
so the native wrapper checks the exact HRESULT. Only S_OK grants ownership; timeout is Busy and
abandonment fails the probe rather than using inconsistent pixels. NT handles are unnamed,
non-inheritable and closed through Rust ownership. No keyboard/mouse automation is involved.

`generations[].source_workers` retains each worker's all-window receive-wall-time distribution,
calls, publications, pool pressure, three-slot bound and sparse/final allocation observations.
Every publication adds one explicitly counted full-image GPU copy on that source context;
the renderer adds no bridge copy, uniform upload or pixel readback. SDK-internal copies remain
unknown. The renderer process's OS/DXGI observations include these worker contexts, while each
logical ledger counts only its own allocations. Analysis checks all ledgers and compares the
sum of their independent peaks as a conservative payload upper bound, not a simultaneous peak
or physical residency measurement. Worker resources undergo the same lossless compaction as
source/consumer reports. Historical schemas keep their actual ownership and measurements.
Each context's warmup-aware CPU/GPU timeline remains separately observable. Marginal context
percentiles cannot establish the asynchronous pipeline's critical path: analysis preserves the
original full GPU budget as unqualified, even when each measured context is individually within
target. Consumer frame age still spans fixture creation through independent output observation.

The explicit Intel boundary test holds all three reader slots across 100 producer rejection
checks, releases one, verifies safe reuse with a changed diagnostic color and drains both
contexts. Its finite pixel checks are test instrumentation, outside renderer workloads:

```powershell
cargo test --locked -p mapgyver_native_probe --features desktop --bin mapgyver_native_probe native_bridge_holds_three_readers -- --ignored --nocapture
```

The [saved source-isolation analysis](../../../docs/progress/evidence/mapgyver-m01-source-worker-analysis-2026-10-28.json)
retains short controls, three sustained Q02 runs and warmup admission pressure. The
[ten-rebuild capture](../../../docs/progress/evidence/mapgyver-m01-source-worker-recreation-2026-10-28.json)
also exercises retirement of the source workers across device generations. Full GPU critical
path, NVIDIA/new-path Q01 series and the other M01 gates remain open.

## Device generation reconstruction

`recreate ADAPTER q01|q02 CYCLES` explicitly retires and rebuilds the probe's D3D11 device,
two swapchains, shaders, received/composite textures and Spout endpoints. It accepts 1–100
recreations, targets two seconds of operation between rebuilds, and keeps the two source
processes, independent receiver, output sender name and both host-owned HWNDs alive. The first
rebuild waits for all four source lifecycle checks. The last generation runs for at least two
seconds and its acknowledgement, then continues rendering for a ten-second settling period.
A separate watchdog includes that period, operating time and bounded retirement overhead;
early closure or an incomplete sequence fails qualification.

Each replacement generation draws a unique opaque 16×16 marker into the composite on the GPU.
The independent receiver must observe that marker, blue/yellow source colors and the alpha
reference before another rebuild. Five isolated pixels per attempt distinguish the new
generation from the old shared image. There is at most one outstanding generation request;
the observer's final bounded line carries a count while individual results travel separately.

`render_worker.rs` supervises the persistent session/processes. Its `generation` module owns
each device scope. After swapchains/SDK/textures drop, the worker clears retained context state
and waits on a submitted event fence for at most two seconds. The function then returns and
the device/context COM references drop before the next generation begins. Fixtures explicitly
confirm clean exit at final retirement; emergency Drop cleanup remains best effort.

Reports keep timing and geometry under `generations[]`, with per-generation setup time,
consumer-ready acknowledgement time, retirement fence duration and a process snapshot after
device COM release. Setup and destruction remain included in the generation's total lifetime.
`steady_scene` captures each device with its scene live after the minimum operating interval
and acknowledgement. The final `settling` checkpoint keeps the same windows, source/consumer
processes and scene live for ten more seconds. Compare warmed generation 10 with later live
checkpoints and the settled final scene; retain post-release snapshots as a distinct ownership
set. Logical live/retiring counts remain separate from physical residency and external handles.
These clocks describe the test stages, not physical display latency or a hardware reset.
Each generation also records `end`: `deadline_reached`, `recreated`,
`final_generation_complete` or `window_closed`. Rendering captures that reason before cleanup.
Timed Q01/Q02 runs require the complete requested warmup/sample interval; closing a window
after all four lifecycle checks still cancels the run. Recreation must finish the last
generation's two-second operating interval, acknowledgement and ten-second settling period
before its watchdog expires. The header displays backend-owned generation and phase state.
Cancellation retires fixtures, endpoints and GPU resources before returning an error; teardown
time cannot complete an interrupted measurement. Interactive `underlay` permits an ordinary
close once its lifecycle checks are complete, recording `window_closed` as its reason.
Source fixtures confirm their first submitted image via a bounded readiness pipe before the
session starts. Diagnostic retries span producer periods; all four attempts remain retained.

```powershell
python tools/mapgyver/run-qualification.py 0 q02 --recreate 2 --samples 1
python tools/mapgyver/run-qualification.py 0 q02 --recreate 100 --samples 1
# After verifying Off in NVIDIA's UI, qualify the selected NVIDIA adapter explicitly:
python tools/mapgyver/run-qualification.py 1 q02 --recreate 100 --samples 1 --nvidia-overlay off
```

The evidence runner's v2 records include the baseline collector hash and observed power scheme/
battery state. It does not monitor power continuously. Historical v1 reports retain their
original layout and fingerprints. Schema v4 adds generation/settling columns and the caller's
declared NVIDIA overlay state. `--nvidia-overlay off` records a declaration; separate native UI
observations verify the prerequisite. Do not infer active state from a loaded DLL.
Schema v5 preserves every independent-consumer frame-age record as integer QPC stamps and
marker words in `observer.frame_age.sample_columns` / `sample_rows`.
Schema v6 adds compact child-resource rows, retaining every phase/periodic snapshot and null
GPU segments under `sources[].resources` and `observer.resources`.
Schema v7 retains native swapchain latency/admission observations in each generation and the
existing sparse presentation records.
`shutdown_seconds` measures from final GPU retirement through fixture/observer reaping and
native event-loop exit/worker join. It excludes report serialization, final process exit and
WebView subprocess termination, so it is not a complete-process shutdown claim.
Full driver removal/unplug/reset, exact driver allocation
retirement and the complete Q05 product lifecycle workload remain separate requirements.

## Logical resources and submission cadence

`allocation.rs` owns a worker-local logical ledger; `texture.rs` couples its leases to native
texture references. Clones share one lease. Supported resources are single-level,
single-sample BGRA8, counted as width × height × 4, with two buffers per swapchain. Unsupported
descriptors fail instead of receiving guessed byte counts. The owner order in each report is
application textures, swapchain buffers, SDK receiver private copies, and SDK shared sender
textures. Imported source shared handles refer to fixture-owned allocations and are not
counted again as renderer allocations.

SDK receiver copies follow the actual texture descriptor/pointer; sender allocations follow
the audited SDK's creation contract after a successful send. These are logical payload bytes,
not allocation alignment, physical residency or driver-private memory. Receiver allocations
are observed independently of new-frame delivery. Leases remain live while either the SDK
endpoint's wrapper or a retained render frame owns them. Dropped allocations stay in retiring
totals until a context completion marker covers this worker's last possible use. Other
processes may still retain shared resources, and DXGI/DWM may retain backbuffers; these counters
do not prove those external lifetimes have ended. The existing bounded
four-slot query ring now emits completion markers during warmup too; timestamp measurement
still starts after warmup. No per-frame CPU wait or texture readback is added.

Every generation records logical peaks, snapshots after resource drop and after the final
GPU fence, plus clear/draw/uniform-byte/readback-pixel/SDK-call counters. SDK call success is
not an exact GPU-copy counter: its sender may return success when texture access skips the
copy. The audited receive path copies the shared source into a receiver-private texture;
it is not a zero-copy import. Internal SDK copy counts and driver/display-queue retirement
remain separately unqualified; child logical ownership is exposed below.

`submission_editor_output_spout` records sampled attempts, accepted calls, busy/occluded/no-target
outcomes, intervals between accepted calls, and lateness against each nominal frame deadline.
It excludes warmup. An accepted Spout call does not prove a new source frame was copied, and
an accepted Present does not prove scanout. Raw `GetLastPresentCount` / `GetFrameStatistics`
results are sampled once per second for each swapchain, including HRESULT errors. No DwmFlush
or display wait is introduced. Microsoft documents multiple-monitor reliability limits for
[frame statistics](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getframestatistics);
displayed cadence, queues, frame age and physical latency still require stronger evidence.

Each native swapchain now uses `FRAME_LATENCY_WAITABLE_OBJECT`, sets and verifies maximum frame
latency **one**, and preserves that creation flag on resize. Its caller-owned waitable handle
closes before the swapchain COM reference. Before drawing into a backbuffer, the worker checks
readiness with a **zero-timeout** wait. A not-ready editor skips that editor attempt; native output
and Spout still run independently. `Present(0, DO_NOT_WAIT)` remains nonblocking. A ready
reservation is retained for non-accepted Present retries and released on acceptance, allowing
the latest frame to replace a busy attempt without repolling an already consumed signal.

`admission_editor_output` records whole-generation ready polls/signals, not-ready observations,
reservation retries and accepted/busy/occluded Presents. Sparse records also retain the actual
`GetMaximumFrameLatency` result. Sample-window cadence remains separate from those cumulative
counts. `rendered_revision` advances only on an accepted editor Present. Neither a ready signal,
queue setting nor accepted Present is a displayed-frame count, DWM queue-depth measurement or
physical input-to-visible timestamp. These observations do not close display qualification.
See Microsoft's [queue-limit contract](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-setmaximumframelatency)
and [waitable-handle ownership contract](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getframelatencywaitableobject).

`trace-capability` checks public real-time ETW session-creation access using one fixed,
descriptive MapGyver session name, no enabled providers and no log file. On success it stops
only the session whose handle it just created; collisions are returned without touching the
existing session. It never elevates, edits permissions or changes a security setting. This
is a preflight, not a display capture; successful creation does not prove provider access
or usable display events. The 2026-10-28 recheck passes outside the agent sandbox with the
administrator token (`start_status: 0`, `stop_status: 0`); the sandbox account still returns
Win32 **5 / access denied**. See [paired access evidence](../../../docs/progress/evidence/mapgyver-m01-etw-access-2026-10-28.json).

```powershell
target/debug/mapgyver_native_probe.exe trace-capability
```

For ordinary ETW sessions without administrator elevation, the development account must have
Performance Log Users membership (`S-1-5-32-559`) in its effective login token. This machine's
development account is already a member; sign out of Windows and back in before testing from
a normal PowerShell/VS Code process. Non-elevated access remains unverified, and kernel tracing
can still require elevation. Use host execution outside the restricted sandbox for the collector;
development-account membership does not grant rights to the separate sandbox account.

PresentMon was not found on PATH in the earlier inventory; WPR/xperf were present. The later
[pinned portable route](#pid-filtered-presentmon-route-control) now verifies actual provider/event
capture through the administrator host path. Preserve the earlier denied-token observation as
unavailable evidence, not a presentation failure or a display pass. See Microsoft's
[StartTrace permissions](https://learn.microsoft.com/en-us/windows/win32/api/evntrace/nf-evntrace-starttracew)
and the [PresentMon console contract](https://github.com/GameTechDev/PresentMon/blob/main/README-ConsoleApplication.md).

Evidence runner schema v3 retains nested allocation and presentation observations in
`resources.gpu_observations`, with the same index as the existing compact resource rows.
Unknown/error results survive compaction. Historical v1/v2 records retain their original schema.

All owned fixture startup paths now use the same first-submitted-image readiness fence,
including standalone smoke. This repairs its observed initial-black-frame race without
increasing pixel-readback counts or weakening its color oracle.

## Source and consumer resource ownership

`child_resources.rs` records each child's own ledger and DXGI local/nonlocal process usage at
one-second intervals; sources also record every resize/absence/recovery phase. Four reusable
event queries are polled without waiting on those background workers. Sparse completion markers
allow retired logical leases to drain during long runs. The final report retains snapshots
before resource drop, after drop and after a context fence, plus peak payload bytes and work
counters. Queries, buffers, shader objects, driver-private allocations and physical residency
are outside texture-payload totals.

Each fixture owns its authored texture and SDK shared sender texture. The consumer owns its
SDK private receiver copy, frame-age GPU log, finite lifecycle staging rows and final log staging
texture. The final staging texture now shares the same ownership wrapper as other textures.
Imported shared allocations are counted by the allocating source/sender, rather than again as
receiver-owned allocations. Do not sum process DXGI figures as physical VRAM, or sum independent
process peaks as a measured simultaneous peak. Fixture, renderer and diagnostic-consumer totals
remain separate, including for budget comparisons.

`explicit_gpu_copy_calls` / `explicit_gpu_copy_bytes` count only copies issued by this harness.
They cover isolated diagnostic pixels and the final log copy; shader draws and SDK calls are
separate. SDK-internal copies/conversions remain unknown. Renderer counters remain free of
diagnostic copies, while fixture uniform uploads stay attributable to each source process.

`wire.rs` and `process/reported.rs` own the shared 4096-byte JSON-line decoder and eight-message
reader channel. Sources now use typed Ready/resource/Finished messages as well as the observer.
Four snapshots fit a resource chunk, with at most 3,620 observations per child (one-hour workload
plus bounded startup/phase observations). Parent owners drain while children finish. Chunk
sequence/count/order, identities, capacities and zero tracked ownership after the final fence
must validate before accepting a report. A dropped owner disconnects its reader before stopping
the child and joining, so a full bounded channel cannot strand teardown. This is harness IPC;
no product/Golden control protocol or UI bootstrap changes.

## Source frame age in the independent consumer

`card.rs` uses the existing 32-byte uniform upload to encode each fixture frame's ID, channel
identity and QPC stamp into five wide image cells, with a checksum. Their centers survive
the fixed composite's bilinear scaling. The existing color/alpha and device-generation oracles
sample different regions. There is no new renderer upload or image readback.

`observer/frame_age.rs` samples ten pixels per observation into a bounded GPU-only texture log.
Four event-query slots are polled without waiting during normal operation. The completion
stamp is taken only after the corresponding GPU copy query completes; CPU enqueue time could
precede a newer source image reaching the GPU. Polling targets 60 Hz, preserves phase through
ordinary wakeup jitter and skips whole overdue periods after a stall. No-frame, ring-full and
overdue-period counts remain visible. Lifecycle color checks may delay observation.

At finish the consumer copies/maps the log once, then sends raw QPC/marker records in chunks
of twelve. The existing 4096-byte line and eight-message channel bounds remain enforced.
The parent drains while the child exits, validates chunk order/count/capacity and recomputes
the summary from raw samples. Error paths disconnect the receiver before joining its reader.
The probe enables `serde_json`'s `float_roundtrip` feature: exact summary verification must
survive the JSON boundary without rounding a derived mean to a neighboring floating value.
The deterministic regression retains the reproduced 19-sample case; comparisons stay exact.
Maximum duration is one hour / 216,001 records; log dimensions remain within D3D11 limits.
`marker_copy_pixels`, `final_log_copy_bytes` and `log_readbacks` describe this instrumentation's
copies separately from existing finite lifecycle diagnostics and unknown SDK-internal copies.

Reports distinguish valid, absent, corrupt/future/regressing, repeated and unique source frames.
Only channel A's known absent-background pattern is treated as absence. Sample stamps must fall
after warmup and before the admitted maximum deadline; completion order is validated. Missing
observations and invalid markers do not silently become zero latency. Skipped source frame IDs
include deliberate source absence and observer gaps; they are not displayed-frame drops.

This measures **fixture CPU stamp to observed consumer GPU-copy completion**. It includes source
GPU work, sharing/composition, consumer copy and observer scheduling/query-poll delay. It is
an upper-bound observation of that path, not a native-window/preview scanout timestamp or
physical input-to-visible latency. Raw outliers and sample coverage remain part of the evidence.
Full repeated release qualification and displayed cadence/queue observations remain required.
See Microsoft's [QPC guidance](https://learn.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps),
[GPU region-copy contract](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copysubresourceregion)
and [query-completion contract](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-getdata).

Large raw-trace series can exceed a terminal's 1 MiB per-result ceiling even after compression.
Use the paced collector and continuously drain its output while it emits chunks:

```powershell
python tools/mapgyver/collect-evidence.py 0 q02 --seconds 120 --warmup 30 --samples 3 --encoding gzip-base64 --nvidia-overlay on
```

The collector runs the unchanged qualification command, captures stdout, then emits indexed
65,536-character JSON chunks at half-second intervals after native measurement ends. The final
record preserves runner exit status, exact stream byte count and SHA-256. Reconstruct with
`collect-evidence.py`'s `reconstruct(records)`, then validate each normal runner envelope with
`run-qualification.py`'s `decode_evidence(packet)`. Missing, reordered or modified chunks must
fail validation. Pacing changes collection transport only; no native sample is slowed or truncated.

## Finite authored-card accuracy

The accuracy wrapper emits checksummed gzip/base64 packets by default through the shared
qualification codec. Keep packets opaque when passing through JavaScript: NVIDIA GPU timestamp
integers can exceed its exact-number range. Decode with the Python runner helper for analysis.
`--encoding json` is available for consumers that preserve unsigned integers; do not parse and
reprint decoded reports through JavaScript's `number` type.

```powershell
python tools/mapgyver/run-color-accuracy.py 0 q01 --nvidia-overlay on
python tools/mapgyver/run-color-accuracy.py 0 q02 --nvidia-overlay on
```

Choose the current hardware adapter index from `inventory`. NVIDIA still requires native
overlay-Off verification before startup and restoration afterward; the wrapper rejects an
On/unknown declaration. The two examples select Intel only when inventory identifies it at 0.

`color_accuracy` owns a separate headless diagnostic: two one-time CPU-authored BGRA image
uploads on separate D3D11 devices, GPU Spout input sharing, the actual `Quad` composite on a
third device, and GPU Spout output to a fourth consumer device in the same process. It maps
one full consumer image after GPU completion. The ordinary rendering/timing modes never
enable this readback or these pixel uploads. All upload/copy/readback costs are explicit.

`color_reference` owns authored bytes and the independent CPU bilinear/clamp oracle. Cards
include asymmetric corner identities, color ramps, checker edges and straight-alpha stripes,
including nonzero color at alpha zero. Every output pixel/component is compared, including
edges and the channel seam. The fixed tolerance is 2/255 per component for UNORM linear-filter
rounding; no boundary mask or pixel exclusion is allowed. Reports retain mismatch totals,
maximum error per component and the first sixteen differences. The wrapper preserves failed
native stdout/stderr and exact input fingerprints instead of dropping a failed report.

This verifies finite authored-image transfer/sampling. It does not qualify the procedural-card
shader/frame labels, the source-worker bridge, cross-process full-image accuracy, native
preview/scanout geometry, sustained timing, driver-internal copies or physical residency.
No default workload, GPU-only passing route or performance budget changes.

Add `--processes` to use two independent producer children, the existing source-worker
NT-handle/keyed-mutex bridge, the renderer process and an independent consumer child:

```powershell
python tools/mapgyver/run-color-accuracy.py 0 q01 --processes --nvidia-overlay on
python tools/mapgyver/run-color-accuracy.py 0 q02 --processes --nvidia-overlay on
```

`color_accuracy/processes` owns this diagnostic's orchestration and bounded child lifecycle.
The normal `source_worker` implementation owns both ingest devices, three-slot handoffs and
retirement; no bridge implementation is copied into the diagnostic. Each producer uploads its
immutable authored image once and republishes its GPU texture. Only the consumer performs the
single full-image map and CPU comparison. Private child pipes carry bounded typed control and
reports, never full-image payloads or native handles; failures contain at most sixteen diagnostic
pixel differences. Distinct child identities, selected adapter,
full-image coverage, per-publication GPU-copy costs, reader release and successful child exits
are verified before success. A native pixel mismatch remains a failed report with its counts
and first sixteen differences; it cannot disappear as an empty successful child result.

The wrapper reports the process route separately from the single-process control. Both retain
the same authored inputs, extents and fixed tolerance. Producer watchdogs, capture/startup
deadlines and owned-process guards bound cancellation and cleanup. Cross-process authored-image
accuracy still does not qualify procedural-card generation, previews, scanout, sustained timing
or physical residency.

## Procedural-card shader accuracy

```powershell
python tools/mapgyver/run-color-accuracy.py 0 q01 --processes --procedural --nvidia-overlay on
python tools/mapgyver/run-color-accuracy.py 0 q02 --processes --procedural --nvidia-overlay on
```

This mode uses the same bounded producer/consumer process route and real source-worker bridge,
but each producer draws the existing `Card` shader once. It uploads only its 32-byte uniform;
there are no CPU image uploads. Ordinary animated fixture drawing and this finite exercise
share `Card::draw_input`, with the same shader source and constant-buffer layout. The normal
draw entry supplies the live clock stamp; this diagnostic supplies fixed inputs.

`color_reference/procedural` interprets the card specification on the CPU, independently of
shader execution. It covers seven-segment digits, grid/checker/color regions, four alpha stripes
and all five marker words. Frame IDs 123456/789012 cover all decimal digits alongside channel
IDs 0/1. Fixed marker stamps are `0123456789abcdef` / `fedcba9876543210`; these are image payloads
for accuracy, not clock/latency observations. Source colors are RGBA `[1, .25, 0, 1]` and
`[0, .5, 1, 1]`. The consumer creates and quantizes each reference source at its actual extent,
then applies the existing independent bilinear/clamp oracle to every composite pixel.

The fixture kind is bound in child readiness and terminal reports, preventing an authored-image
control from satisfying the procedural request. The wrapper verifies zero pixel-upload cost,
one producer draw/32-byte uniform per source, unchanged 2/255 tolerance, complete pixel coverage,
bridge copying, cleanup and child identity. A native mismatch is preserved as a failed capture.
The fixed-image check does not qualify animated/lifecycle accuracy, native previews/scanout,
physical alignment, sustained timing, driver-internal copies or physical residency.

## Editor JS/document isolation

```powershell
python tools/mapgyver/collect-evidence.py 0 q01 --editor-isolation --seconds 185 --warmup 0 --samples 1 --encoding gzip-base64 --nvidia-overlay on
```

This dedicated lifecycle mode deliberately blocks only the owned probe WebView's JavaScript
thread for 65 seconds. It observes more than 60 seconds inside that interval, navigates the
same WebView to `about:blank`, observes another >60 seconds, then reloads its original bundled
URL and requires two freshly rendered preview registrations. The native output and Spout
sender retain demand throughout. The 185-second value is a watchdog; only explicit reconnect
completion can pass. Cancellation, stage timeout or early close cannot qualify the exercise.
This is not a three-sample fixed-workload performance run, browser-process crash recovery or
proof of displayed cadence. No desktop input injection or display changes are needed.

Rust-owned document sessions scope UI layout revisions. The host page-navigation callback
invalidates them on every document start, including normal reloads; the test controller does
not mint sessions. Replacement clears registrations and resets revision acknowledgement; old
queued layouts and old render acknowledgements cannot own a replacement's views. Each frontend
document binds its first positive host session once.
The layout/session and busy-command/activity DTOs generate their TypeScript declarations.

`analyze-editor-isolation.py` validates raw independent consumer markers for every one-second
observation interval, accepted native/Spout progress, no frontend status polls, cleared blank
document registrations, fresh reconnect acknowledgement and post-fence cleanup. It preserves
startup invalid/absent markers separately from the interruption windows, and does not turn
positive interval progress into a 60-fps claim. Source and renderer query/admission counters
remain available. The finite busy loop exists only as this explicit qualification stimulus.

Build the UI bundle before desktop Rust builds/checks; Vite replaces `dist` while building,
and Tauri embeds those assets at compile time. Do not run those dependent operations in parallel.

## PID-filtered PresentMon route control

`capture-presentmon.py` owns a separate bounded capture. Defaults are an eight-second sample
after the contract's 30-second warmup; `--sample-seconds` and `--warmup-seconds` record explicit
overrides, with sample at least eight seconds and total at most one hour. Earlier two-second
warmup controls remain historical, including their resize/absence/reconnect phases. The helper
uses the pinned official portable executable under the ignored
`target/qualification-tools/presentmon-2.6.0/` directory; it installs no service. Obtain it from
the [official release](https://github.com/GameTechDev/PresentMon/releases/expanded_assets/v2.6.0)
and verify SHA256 `b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af` before use.
The tested asset also has a valid Intel Corporation Authenticode signature. Recreate this
ignored dependency after cleaning `target`; it is not bundled with an app artifact.

```powershell
python tools/mapgyver/capture-presentmon.py 0 q01
python tools/mapgyver/capture-presentmon.py 0 q02 --output target/presentmon-q02-run01.json
```

Run through the verified administrator host path. The helper creates a fresh UUID session,
filters output to the spawned renderer PID, requests legacy CSV with raw Present-start QPC,
keeps dropped rows and disables input tracking. It drains native/CSV pipes concurrently and
captures two extra seconds around the nominal native duration. Raw rows outside the
exact native sample window remain available. Timeout cleanup is scoped
only to that invocation's generated session name; existing services/default/foreign sessions
are never selected. No elevation/group/security setting is changed. The installed FrameView
fork exited silently on this command and remains preserved as failed evidence; it is not the
default capture tool. An explicit `--presentmon PATH` allows a separately fingerprinted tool.

For large captures, `--output` exclusively creates a generated packet under the existing
`target` directory and prints a small byte-count/SHA256 receipt. Choose a fresh filename each
time. Existing files, paths outside `target` and missing parents are rejected before startup;
an exclusive open also prevents a later race from overwriting a file. The packet retains every
raw stream and exact native integer. Verified packets are promoted to repository evidence
separately; stdout remains available when no output path is requested.

After the native process exits, while the collector drains its tail, `trace-status SESSION`
uses read-only `ControlTraceW(QUERY)` on that invocation's generated UUID name. It returns
bounded QPC, the returned identity and separate `EventsLost`, `LogBuffersLost` and
`RealTimeBuffersLost` counters. Failure or invalid identity keeps statistics unavailable.
The helper requires a successful query after every recorded native Present interval and
preserves raw collector output even if query startup fails. The two-second tail is an
opportunity to query a live session; slow native teardown can leave the query unavailable.
`live_etw_query_verified` and zero counters describe that one live snapshot. Final shutdown,
later delivery and collector analysis loss remain unverified. See Microsoft's
[query contract](https://learn.microsoft.com/en-us/windows/win32/api/evntrace/nf-evntrace-controltracew)
and [counter definitions](https://learn.microsoft.com/en-us/windows/win32/api/evntrace/ns-evntrace-event_trace_properties).

`presentmon_data.py` validates the observed v1 schema and exact PID/QPC values. It separates
Present-window drop counts from Display-window intervals, retaining pre-window predecessors
and missing/zero timing rather than inventing zero cost. Its original analysis remains unlabeled.
`presentmon_roles.py` separately requires exact renderer PID, queried DXGI interface address and
Present-QPC lifetime matches for every CSV row and every declared presenter. Native generation
reports own the typed editor/output identities and half-open bounds; queried aliases are not
assumed equal. Missing/unknown/overlapping mappings fail explicitly; disjoint pointer reuse is
allowed. A Display timestamp beyond the recorded Present interval remains observable because identity is
matched using Present QPC. No identities enter UI DTOs or persisted product documents.
Each native presenter also records `window_at_end`: physical window/client geometry, its
actual monitor/device, DPI and separately fallible current mode. The shared scoped thread-DPI
authority serves both adapter inventory and these observations. Nominal integer refresh from
`DEVMODEW` is distinct from rational refresh, vblank/latch timing and a continuous monitor history.
All raw CSV/native stdout and input/tool hashes are kept
in checksummed compressed evidence. Absence of loss warnings means only **no loss reported**; explicit
zero loss is not independently verified. Shared compositor timestamps are not independent
physical-output cycles and must not be pooled into a higher display rate. This short control
establishes the software capture route, not full cadence, scanout, optical/input latency,
alignment, GPU critical path or M01 acceptance. `role_correlation_verified` describes only exact
mapping; it does not grant a performance pass. The short Intel Q02 control retains 85 editor
and 20 output drops per 480 Presents despite all native calls being accepted. Preserve this
failure and investigate its cause before claiming cadence. See the
[role evidence](../../../docs/progress/mapgyver-m01-native-feasibility.md#exact-nativeetw-role-correlation-and-4k-displayed-drops--2026-10-28)
and [upstream CLI semantics](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/README-ConsoleApplication.md).

## Correlated source/renderer timing

Schema 13 adds `dependency_trace` to every renderer generation and `gpu_trace` to each of its
three context timing reports. The bridge publication carries the source receive-batch ID
separately from the reusable texture allocation. Every admitted sampled composite records the
two reader batch IDs it actually draws, including repeated readers and absent sources. It does
not read pixels, wait for instrumentation, change source selection or introduce frame IPC.

Traced contexts capture timestamp/disjoint results during warmup too, because the first sampled
composite can retain a warmup publication. Warmup spans remain excluded from sampled context
distributions. Each raw span preserves batch ID, CPU submission/completion-observation QPC,
context-local start/end ticks, frequency and sample-window flag. Missing, invalid, skipped,
pending and truncated results remain explicit. Span/dependency logs cap at 216,001 records each;
the runner losslessly compacts them into column/row arrays before compressing output. Admission
continues to use independent timeline fences. Headless event-only negative controls keep their
explicit untraced profiler behavior.

```powershell
python tools/mapgyver/analyze-pipeline.py docs/progress/evidence/SAVED_CAPTURE.json
```

The analyzer joins exact dependencies after capture and recomputes context distributions from
raw query results. It reports the per-composite sum of correlated source/render spans and the
CPU-QPC envelope from the earliest consumed ingest batch's start to observed renderer completion.
GPU start/end subtraction is confined to each span's own disjoint scope; different devices'
GPU clocks are never directly compared. The serial span sum omits gaps and does not establish
the pipeline critical path. Spans can include CPU command-submission gaps and source bookkeeping
after publication; their sum is not utilization or elapsed pipeline time. Receive-batch IDs
identify publications, not unique fixture pixels. The CPU envelope conservatively includes scheduling, queueing,
completion polling and retained-source age; an envelope above target is inconclusive about
the GPU budget. Missing dependencies cannot turn into zero-cost stages or a qualification pass.
The endpoint is renderer GPU completion, not native scanout, independent-consumer delivery or
physical input response. Full critical-path/scanout qualification remains separate.

## Headless query-progress isolation

`gpu-progress ADAPTER MODE` runs a separate eight-second completion-progress control without
windows, fixtures or a consumer process. Each case begins after the same completed 16×16 GPU
clear, then attempts 480 ticks with the existing two-batch admission policy. Modes are `event`
(admission markers only), `warmup` (the usual profiler's event-only warmup path), `timestamp`
(timestamp commands enabled), `receiver_warmup`, `receiver_timestamp` (the same modes plus an
explicitly missing Spout receiver), and `clear_warmup` (a small real GPU clear every admitted
tick). The receiver cases reject an unexpected sender rather than silently connecting elsewhere.

Reports preserve every busy tick's frame/QPC, all-window CPU work, optional profiling results,
admission before the final drain, final post-fence ownership and work counters including the
seed/drain clears. GPU polling never waits in the loop; the existing explicit Flush remains unchanged.
After recording the observation window and pending counters, a final 16×16 clear supplies real
GPU work for the diagnostic drain. It cannot erase observed misses or improve measured timing.
No pixel readback
or CPU image upload is introduced. These controls isolate marker/profiler/empty-receive behavior;
they do not reproduce resize/absence of a previously connected sender or prove Q01/Q02 throughput.
NVIDIA still requires native overlay-Off verification before startup and restoration afterward.
`fence` and `fence_warmup` use timeline admission with no timestamps, without/with the usual
event-only profiler. `fence_no_drain` also omits the final clear and checks empty-work retirement
through the runtime's timeline completion. Explicit event-query admission remains diagnostic
only, never a silent fallback. The probe now requires timeline-capable D3D11 device/context
interfaces; unsupported hardware/OS combinations fail explicitly.

```powershell
target/release/mapgyver_native_probe.exe gpu-progress 0 warmup
```

See Microsoft's [asynchronous Flush contract](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-flush)
and [DONOTFLUSH query-polling caveat](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_async_getdata_flag).
Marker progress must be measured before attributing a missing-source stall to an internal driver
or before changing submission behavior merely to make the timing look favorable.

The [retained controls](../../../docs/progress/mapgyver-m01-native-feasibility.md#timeline-completion-and-isolated-progress-controls--2026-10-28)
reproduce NVIDIA event-warmup admission at 4/480 ticks, versus timeline-warmup 480/480 on the
same final binary. Intel and NVIDIA also complete timeline retirement without a final clear.
Runtime admission/retirement therefore uses the existing modern completion fence; profiling
remains diagnostic. Short native controls and 100 rebuilds validate the repair's scope, while
full repeated series, context-spanning GPU timing and memory-retention explanation remain open.

## Headless ownership isolation

`retirement ADAPTER CASE COUNT` uses the desktop build's native dependencies without starting
Tauri or creating windows. It accepts 1–1000 scopes, snapshots the process after each scope's
COM release, then records eleven observations across a ten-second idle drain with no live
device. Run one case at a time; this is a diagnostic control, not the full Q05 scene workload.

```powershell
cargo build --locked -p mapgyver_native_probe --features desktop
target/debug/mapgyver_native_probe.exe retirement 1 texture_only 100
```

Choose the current hardware adapter index from `inventory`; do not assume `1` is NVIDIA.
The cases isolate these operations:

| Case | Work in each scope |
| --- | --- |
| `inventory` | Enumerate the selected adapter and memory budgets, without creating a device |
| `device_empty` | Create and release a device/context, without textures or completion queries |
| `query_only` | Empty device plus the existing event-query retirement fence |
| `texture_only` | Create/release a 4K BGRA texture, ClearState/Flush, without drawing or polling |
| `clear_only` | Texture plus GPU clear, ClearState/Flush, without completion polling |
| `device` | Texture plus clear and the existing retirement fence |
| `device_no_memory` | `device`, excluding memory-budget queries during setup |
| `context_first` | `device`, explicitly release context before the final owned device reference |
| `final_flush` | `device`, with another Flush after the retirement query drops |
| `thread` | `device` on a fresh, joined thread for every scope |
| `fence` | Fence the clear before texture drop, compared with the runtime's post-drop timeline retirement |
| `spout` | `device` plus one sender and two receivers; verify sender registration cleanup |
| `timing` | `device` plus the scene's bounded timestamp-query ring |

Reference observations use a balanced AddRef/Release on an owned temporary IUnknown interface.
They include that temporary reference and never decide lifetime or release foreign references.
`context_first` additionally records the result of releasing its final owned observation
reference after both wrappers have dropped. Counts are diagnostic, not a residency measure.
Runtime admission and retirement now use timeline fences. `query_only` retains its explicit
event-query implementation for isolation; older captures retain their original completion paths.

The [captured isolation matrix](../../../docs/progress/evidence/mapgyver-m01-ownership-isolation-2026-10-27.json)
preserves all cycle/drain observations and exact input fingerprints. In the current environment,
NVIDIA handle growth reproduces in repeated device scopes with texture creation, including
after a zero final device Release result; Intel handles stay flat. The later approved
[overlay on/off/on control](../../../docs/progress/evidence/mapgyver-m01-nvidia-overlay-control-2026-10-27.json)
reproduces growth only in enabled conditions, with the capture module loaded throughout.
The original overlay setting is restored to On. The user approved an
[overlay-off NVIDIA qualification prerequisite](../../../docs/progress/mapgyver-qualification-contract.md#nvidia-overlay-prerequisite).
Verify and record Off before future qualifying processes; restore the original setting after
temporary tests. The loaded DLL cannot verify active state. Workloads/budgets remain unchanged;
an internal teardown fix and full-scene retirement pass are not claimed.

The later [NVIDIA release scene capture](../../../docs/progress/evidence/mapgyver-m01-scene-retirement-2026-10-28.json)
completes 100 controlled rebuilds and ten seconds of live settling with verified overlay Off.
Relative to generation 10, handles remain within ten and maximum positive private growth is
8.133 MiB; all scopes drain tracked logical allocations. Original overlay On is restored.
This retirement-only archive preserves every process/allocation row and generation oracle;
it does not qualify full Q05, physical residency, displayed cadence or complete-process shutdown.

M01 remains open. Continue the full [qualification contract](../../../docs/progress/mapgyver-qualification-contract.md),
including sustained pacing/resources, copies and latency.
Manual visual/input evidence is required for layering, clipping and mixed DPI. A failed underlay
must trigger the documented DirectComposition decision before production editor rollout.
