# MapGyver qualification contract

Golden Video's native harness owns GPU measurements; MapGyver owns authored reference scenes
and end-to-end product qualification. The desktop host owns WebView/window/DPI observations.
This contract fixes workloads and engineering budgets for M00–M10 before measurements.
The 2026-11-02 plan merge revises Q03 and adds Q06/software-decode accounting below; existing
numeric budgets, native GPU-sharing requirements and historical failures remain unchanged.
The 2026-11-03 M11/2 revision supersedes the fixed **product allocation ceiling** for useful
concurrent 4K demand: production admission uses a measured adapter LOCAL budget with explicit
headroom. Existing workload texture-usage, timing, private-growth and retirement budgets remain
required; a larger product admission ceiling does not accept any retained qualification gate.
Bounded overload tests may intentionally use a stricter workload ceiling to verify real native
rejection/recovery without exhausting the user's GPU. Report that ceiling separately from
production policy, exact actual texture/pool costs and the measured adapter budget.
Scoped functional evidence and the approved M01 route decision live in the
[native report](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28);
no complete workload or original full native gate is qualified by that decision.
[M00 inventory](mapgyver-m00-baseline.md) identifies the available test machine.

## Acceptance ownership after the M01 scope correction

On 2026-10-28 the user approved M01 as a functional host/backend/binding decision permitting
the thin M02 shell. Its four items and evidence are defined in the [plan](../plan/MapGyver_Implementation_Plan.md#m01--decide-the-functional-native-rendering-route).
The old eight-item M01 dashboard is replaced by four scoped functional items, not eight newly
passed qualification items. All workload definitions, numerical budgets and reporting rules
below are unchanged; historical failures and unavailable observations remain authoritative.

| Required evidence                                                                                                           | Acceptance owner                                                     |
| --------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| Reusable renderer Q01/Q02 CPU/GPU/cadence/frame-age, queue/admission, memory/handles, retirement and recovery               | M03; endpoint/source supervision in M04                              |
| Physical output/preview/reference-card accuracy, DPI/input matrix, alignment/visible latency and editor/output independence | M05                                                                  |
| Integrated Q01/Q02 release series, Q03 soak and full product Q05 across implemented features                                | M09; each feature owner verifies its own recovery before integration |

M02 proceeds from the functional decision without treating these continuing requirements as
passed. Targeted diagnostics remain evidence; explaining SDK/driver internals is not an extra
acceptance item. Qualify one named adapter/route first; dual-vendor success is not mandatory.

## NVIDIA overlay prerequisite

For NVIDIA GPU qualification, **NVIDIA overlay must be Off before the probe/test processes
start**. The user approved this documented prerequisite on 2026-10-27 after the controlled
[on/off/on test](evidence/mapgyver-m01-nvidia-overlay-control-2026-10-27.json): handle growth
reappears in both enabled conditions and disappears in the disabled condition. The capture DLL
is loaded throughout, so module presence cannot verify whether the feature is active.

Record the observed setting and method with the run, verify it through the native UI or an
authoritative supported query, and restore the user's original setting after temporary tests.
Overlay-on captures remain diagnostic evidence of a known failing environment; preserve them.
This precondition leaves every workload, timing/memory/cleanup budget and other hardware check
unchanged. It does not repair NVIDIA internals or establish a full-scene/Q05 pass. The user's
earlier control restored On. The latest recorded card-accuracy observations found an already-Off
setting and left it unchanged; future NVIDIA runs must observe Off again before startup.

## Workloads

Use deterministic color/alpha/checker/numbered-grid cards with visible channel and frame IDs.
Archive the exact scene/fixture configuration and hashes with each result. Use BGRA8/RGBA8
sources with declared alpha/color conversion; record actual intermediate formats and bytes.

| ID                      | Fixed workload                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | Owner / required phase                                                                                                                                                 |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q01 Native feasibility  | Two fixture Spout senders at 1920×1080 and 1280×720, each 60 fps, on one named adapter. Composite both into one 1920×1080 native rectangle and one 1920×1080 Spout output. Two independently sized editor previews, initially 640×360 and 480×270 physical pixels, with DOM overlays                                                                                                                                                                                                                                 | M01 functional evidence; M03 renderer budgets, M05 presentation, M09 integrated series; independent producer/consumer processes                                        |
| Q02 4K feasibility      | Q01 with first sender and both outputs at 3840×2160, second sender still 1280×720; keep two previews                                                                                                                                                                                                                                                                                                                                                                                                                 | Same owners as Q01; same adapter and explicit format/copy accounting                                                                                                   |
| Q03 Complete product    | Solid Color, Picture and Shared Texture each have two variants. Composition has one output and owns Reference children for two source media. Matrix has two layers, one group and eight columns/16 populated Cells with owned Reference media, exporting layer/group/final outputs. Video has two file channels on one transport. Three Screens, one per target mode, two ordered Surfaces each; exercise source, Reference, Composition, Matrix and Video routing. Media Preview and all four editors are available | M09; first at 1920×1080, then first source/Video channel and every Screen at 3840×2160, others 1280×720; record actual logical/mirrored/unique rendered channel counts |
| Q04 Authored scale      | 10k, 50k and 100k authored media/channel/surface/Matrix entities, exact node counts also reported. Bound actual demand to 16 active channels and four 1080p outputs; remainder inactive. Single batched detached-tree creation                                                                                                                                                                                                                                                                                       | M03 deterministic admission fixtures, M09 actual-app measurements; inactive authored size is separate from GPU throughput                                              |
| Q05 Recovery/retirement | Q01/Q03: 100 source resize/restart cycles, 100 output remove/recreate cycles, 100 project replacements, 1000 superseding plan edits, 1000 Matrix launches including interrupted transitions. Two seconds between lifecycle cycles; launch/edit burst at 100 intents/sec                                                                                                                                                                                                                                              | Owning milestone as feature arrives; M09 complete-app run                                                                                                              |
| Q06 Video               | Frame-numbered common/performance/extended-codec fixtures, mixed frame rates, exact/superseding seeks, concurrent 1080p/4K files and shared Reference consumers as specified below                                                                                                                                                                                                                                                                                                                                   | M10 per tier; M09 actual bundled integration, Q01/Q02-style sustained sampling for performance claims and inclusion in the Q03 soak/Q05 lifecycle                      |

Q03 explicitly replaces the obsolete two-channel Composition and reference-only instance/cell
fixture. Its owned Reference children mirror existing sources without feedback; they do not add
independent decoders or source renders. Record actual authored node counts, mirrored channels
and unique demanded passes rather than retaining the old “12 channels” total. References to the
single-output Composition must also be exercised. No previous Q03 result qualifies this fixture.

For Q03 and Q06, use frame-numbered Video files with mixed 24/30/60 fps channels and transparent
fixtures where the codec supports alpha. Q06's fixed concurrency is a 1080p60 plus 4K30 file,
then two 4K60 files for the performance tier, with independent shared consumers and 60 Hz outputs.
Qualify H.264/H.265/ProRes, HAP/HAP Alpha/HAP Q and DXV/WebP (including animated WebP) separately;
record exact container, codec/profile/pixel format, decode route and unsupported variants.
Test paused-zero/rate/loop, exact timestamp seeks and selected-channel stepping, final-frame hold,
cold/warm seeks, superseding seeks, file loss/relink, device/project replacement and final-use
retirement. Define fixture-specific seek correctness/latency criteria before the first run;
report measured latency distributions without silently inventing a pass threshold.

Source assignment fixtures must include each blend mode, cropped/perspective surfaces
and transparent edges. Generate pictures once on the fixture/preparation worker. A fixture using
CPU pixel send/receive can test recovery but cannot qualify Q01/Q02's GPU-sharing route.
Q04 has distinct generation/admission and steady-state interaction measurements; setup is never
silently omitted from a claimed creation timing.

## Run and reporting method

- Record commit plus dirty-file hashes, profile, executable/assets hashes, Cargo/npm lock hashes,
  selected MSVC/Windows SDK, Spout wrapper/sys/upstream revision, OS/driver/WebView versions,
  adapter LUID, memory budget, display identity/origin/mode/DPI and power mode. Pin each result
  to its actual inputs. A changed driver/adapter/workload starts a new result series.
- Build the x64 native harness against repository-locked Tauri/Wry. Use 30 seconds warmup,
  then three independent 120-second samples for Q01/Q02. M09 adds a 30-minute Q03 soak and
  Q05 lifecycle run. Report each sample's count, median, p95, p99, maximum and failures;
  never replace failed distributions with a favorable average.
- Measure CPU submission work separately from presentation waits; GPU timestamp queries must
  be nonblocking. Report source-to-output frame age, missed deadlines, present intervals,
  plan age, queued/retiring resources and per-output cadence. A minimized editor does not
  disable output demand; test 60 seconds minimized and 60 seconds disconnected/busy.
- The published-frame ceiling may be proved by a verified native storage/admission upper bound
  covering every accepted output publication. Record actual window descriptor/latency/readiness
  and Spout shared-image identities; stale versions must coalesce. Report exact occupancy as
  unknown when it is not measured. PresentMon pending events or GPU completion alone are not
  publication-capacity evidence; this does not replace GPU critical-path/time measurements.
- A complete machine-QPC enclosure from both exact retained app source-worker submissions
  through source/renderer query collection can prove a GPU-path upper bound without comparing
  independent GPU clock origins. Include endpoint quantization and every frame/dependency.
  A loose upper bound remains inconclusive; correlated context-span sums do not replace the
  original critical-path requirement. Any narrower GPU-work acceptance needs user approval.
- Record logical texture allocation bytes plus DXGI current usage/budget, process private bytes,
  working set and handle counts after warmup, at peaks and after retirement. Include SDK/driver
  allocations where observable; unknown counters remain unknown. Do not equate shared GPU
  memory with dedicated VRAM or double-count one physical allocation across consumers.
- Input/geometry observations require the native app: 100%, 125%, 150%, 200% scaling, mixed-DPI
  moves and negative origins, resize/maximize/restore, docking, scroll, zoom and tab hide/show.
  Capture at least 30 observed native gestures per available configuration, including outside-region
  release and menus/dialogs above previews. Manual interaction or user-authorized desktop
  automation may produce this evidence; record the method. Keep unavailable configurations open.
- Test device recreation using an explicit harness injection first; hardware/driver reset,
  unplug and genuine removal are separately named evidence. Do not reset the user's display
  driver or change their monitor arrangement automatically.

## Fixed initial budgets

These are engineering acceptance targets on the recorded adapter, not observed performance or
user-specified product promises. Revision requires a documented rationale before a new run;
preserve earlier failures. Reject unsupported allocations/plans atomically instead of reducing
scene resolution or switching adapters silently.

| Metric                                                                                                  | Q01 / 1080p                                                                      | Q02 / 4K and Q03                 | Q04 scale                                                     |
| ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------- | ------------------------------------------------------------- |
| Cadence                                                                                                 | 60 fps/output; missed 16.667 ms deadlines ≤0.1% after warmup                     | Same; independent output metrics | Same for four demanded 1080p outputs                          |
| GPU critical path                                                                                       | p95 ≤12 ms, p99 ≤16.667 ms                                                       | Same                             | Same                                                          |
| CPU render submission work                                                                              | p95 ≤4 ms, p99 ≤8 ms, excluding waits                                            | Same                             | Same; work proportional to demand                             |
| App-owned live + retiring texture bytes                                                                 | ≤512 MiB                                                                         | ≤1 GiB                           | ≤2 GiB                                                        |
| GPU admission ceiling                                                                                   | App allocation ceiling and ≤50% measured DXGI local budget, whichever is smaller | Same rule                        | Same rule                                                     |
| Process private bytes, including owned workers, excluding separate source fixtures/WebView subprocesses | ≤1 GiB                                                                           | ≤2 GiB                           | ≤4 GiB at 100k authored entities                              |
| Published-but-not-consumed frames                                                                       | ≤2 per output; coalesce stale preview frames                                     | Same                             | Same                                                          |
| Input/geometry to visible response                                                                      | p95 ≤50 ms; settled edge alignment ≤1 physical pixel                             | Same                             | p95 ≤100 ms, plus no UI task ≥50 ms during steady interaction |

Also report the WebView/fixture process memory separately. For integrated adapters record shared
segment usage; if a local budget is absent/zero, budget admission stays unqualified until M03
defines and tests an explicit shared-memory policy. Never treat missing counters as zero usage.

Source-to-output latency target is p95 ≤3 frame periods at 60 Hz, excluding intentional
transitions. Each renderer keeps at most one preparation in flight, one latest desired plan
and two retiring plan generations; pressure must coalesce or reject without blocking the
actor/UI thread. Superseded work still releases its resources after GPU completion.

Window latency qualification retains the entire accepted-image cohort. A trusted known drop
is an accounted non-display and remains a deadline miss; its unmeasured age contributes an
unbounded endpoint to conservative whole-cohort percentile bounds. Qualify p95 only when its
original-cohort rank has a finite upper bound within the existing target. Do not compute a
passing percentile by excluding drops, invent their display/consumption timestamps, or treat
missing/untrusted rows or source/plan identities as complete coverage. Finite observed-only
distributions are reported separately, and exact pending occupancy remains unknown when its
consumption witnesses are absent. This clarifies percentile accounting, not a new zero-drop
requirement or a change to the existing deadline/latency budgets.

Q05 must return handles to within 10 of its warmed stable baseline and texture allocations to
the current scene's exact live set after a 10-second drain. Private-byte growth after the first
10 cycles must be ≤32 MiB over the remaining cycles; retained caches must have explicit measured
ceilings. Shutdown target is ≤5 seconds with all owned workers/processes reaped. Failed
endpoints retry with capped backoff; record recovery delay and independence of the other channel.

Q04 backend receipt target for a fixed-size edit is p95 ≤50 ms and must not scale linearly with
inactive authored count. Report batched creation elapsed time and peak memory separately; no
creation-time pass is inferred from receipt/steady-state results. Every undoable gesture
produces one history entry. Spout/native sharing and hardware Video routes permit no recurring
CPU texture readback/upload. One-time Picture uploads, GPU copies/conversions and explicitly
software-decoded Video uploads are reported separately under the allowance below.

## Software-decoded Video upload allowance

This allowance applies only to formats/channels actually decoded in software; it does not
relax Spout/GPU-sharing or hardware-frame residency. Admit at most one upload of each newly
sampled decoded frame per source channel, shared by all consumers/References. Holding the same
frame adds no repeated upload. Bound queues by the declared admitted frame-pool capacity and
charge CPU decode buffers plus GPU upload/live/retiring storage within the existing memory
ceilings. Declare the concrete pool/queue limits before validation.

For each channel report sampled/uploaded frames, actual plane/stride bytes, pixel conversion,
bytes per second and peak in-flight bytes. The upload ceiling is the sum of each channel's
admitted sampled frame rate multiplied by its declared uploaded frame bytes; a higher rate,
repeated upload to another consumer, unaccounted copy or unbounded queue fails this allowance.
Compressed HAP payload uploads must likewise report their actual bytes/conversion separately;
they do not claim hardware-decoder zero-upload behavior. All existing CPU/GPU/frame-age/cadence,
memory-growth/handles and shutdown targets remain operative. Never hide Video cost by lowering
the retained Q01/Q02 workload or silently enlarging budgets.

## Gate outcomes

Use `PASSED_NATIVE`, `FAILED_NATIVE`, `PASSED_DETERMINISTIC`, `SOURCE_ONLY` or `UNAVAILABLE`
per criterion, each with the exact command/configuration and evidence artifact. Deterministic
tests qualify policy and lifecycle, not desktop layering, Spout compatibility or mixed DPI.
M01 closes on the approved functional route decision and its scoped native evidence. The
[continuing native matrix](../architecture/mapgyver-native-rendering.md#native-feasibility-gate)
remains acceptance work for M03/M05/M09 and the feature owners above. If ordinary hosting fails
the implemented presentation requirements, resolve the explicit DirectComposition alternative
before claiming editor quality. Outstanding native checks do not turn into source-backed passes.
