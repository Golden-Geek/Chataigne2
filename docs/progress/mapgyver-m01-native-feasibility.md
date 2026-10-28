# M01 native feasibility evidence

The qualification harness owns experiments; Golden's future GPU/host mechanisms remain separate
from MapGyver's product systems. M01 is **in progress**, not a presentation-architecture approval.
See the [ledger](mapgyver-status.md), [qualification contract](mapgyver-qualification-contract.md)
and [harness commands/ownership](../../tools/mapgyver/native-probe/README.md).

## Exact native/ETW role correlation and 4K displayed drops — 2026-10-28

The diagnostic report now records each presenter's typed role, queried `IDXGISwapChain`/`1`/`2`
addresses as hex strings, and QPC bounds enclosing its Present calls. Constructor wiring owns
the role; no address order or frame-count heuristic assigns it. Temporary queried COM references
retire during setup. Only CLI generation reports contain these ephemeral identities; the UI,
generated DTOs, product protocol and persistence do not receive them. No per-frame native work,
pixel readback or rendering policy changes were added.

`presentmon_roles.py` matches every raw row by exact renderer PID, normalized interface address
and half-open **Present-QPC** lifetime. It rejects missing/unknown/ambiguous identities and
overlapping aliases, permits disjoint generation reuse, and keeps role/generation/address
summaries separate. Display timestamps can extend past a presenter's interval; both actual
captures retain one such event per role. Tests cover those tails, pointer reuse and integer QPC
above 2^53. The original unlabeled analysis remains alongside the separate correlation result.

The short Intel [1080p](evidence/mapgyver-m01-presentmon-role-intel-q01-2026-10-28.json) and
[4K](evidence/mapgyver-m01-presentmon-role-intel-q02-2026-10-28.json) captures use the same native
release SHA256 `d0cc1ce5b996eb67edf1d2cc5b971618e3412faa5e3d02c43101b60ec64b4a5c`
and identical source/assets/helper/tool inputs. Both map all raw rows exactly; all three queried
aliases happen to equal the actual ETW address for each object. That equality was measured,
not assumed as a general COM guarantee. Earlier accuracy/trace captures retain their old binary.

| Short Intel control | Raw rows editor / output | Sample Presents per role | Dropped editor / output | In-window display events editor / output | Display-interval p99 editor / output |
| --- | --- | --- | --- | --- | --- |
| Q01, 2 s warmup + 8 s sample | 593 / 594 | 480 | 0 / 0 | 480 / 480 | 20.8326 / 20.8374 ms |
| Q02, 2 s warmup + 8 s sample | 590 / 591 | 480 | 85 / 20 | 394 / 459 | 49.9900 / 33.3405 ms |

Q02 therefore observes drops of 85/480 (17.71%) for the editor and 20/480 (4.17%) for output,
despite native sampled admission accepting 480 calls on each path with no busy/occluded/late
calls. Maximum displayed intervals are 50.0200/33.3575 ms. Each role has one display arrival
from before the sample and two tails beyond it; Present and Display denominators differ.
These are retained cadence failures in this control, not a role-correlation failure or an
explanation of the cause. Do not substitute accepted calls, repeat until passing or relax the
fixed budgets. The larger-than-monitor 4K window still does not qualify physical 4K scanout.

[Independent analysis](evidence/mapgyver-m01-presentmon-role-analysis-2026-10-28.json) recomputes
all 2,368 rows, native stdout/compaction, hashes, summaries and exact windows. Each role has
one fewer all-window CSV row than native accepted Presents. The pinned collector
[seeds its first observed present](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/PresentMon/OutputThread.cpp#L325-L329)
without emitting that CSV row; the aggregate deficit is consistent with that behavior and does
not independently prove zero loss or identify the omitted call. Both traces report no loss
warnings; independent event/buffer loss verification remains open. Correlated software roles
are not independent physical display cycles.

Native and collector processes exit 0 with empty stderr, current inputs remain unchanged,
both owned UUID sessions are absent after exit, and the pre-existing collector service stays
at PID 5864. No desktop setting is changed. Fifty-two deterministic Rust tests, strict desktop
Clippy and 54 Python tests pass. These short controls establish role mapping and expose a
timing failure; they do not replace repeated 30+120-second samples, physical alignment/input
latency, full GPU critical path, lifecycle/retirement or any complete M01 acceptance item.

## NVIDIA procedural/authored accuracy and exact evidence transport — 2026-10-28

NVIDIA adapter 1, RTX 5080 Laptop GPU, LUID `00000000:000170d3`, now has the same fixed-card
process/bridge checks as Intel. All cases compare every component of every output pixel with
no exclusions, fixed tolerance 2/255 and maximum observed BGRA error 1/255.

| NVIDIA case | Pixels compared | Outside tolerance | Producer CPU pixel uploads |
| --- | --- | --- | --- |
| [Procedural 1080p](evidence/mapgyver-m01-procedural-card-nvidia-q01-2026-10-28.json) | 2,073,600 | 0 | 0 |
| [Procedural 4K](evidence/mapgyver-m01-procedural-card-nvidia-q02-2026-10-28.json) | 8,294,400 | 0 | 0 |
| [Authored 1080p](evidence/mapgyver-m01-procedural-authored-control-nvidia-q01-2026-10-28.json) | 2,073,600 | 0 | 2 initial images |
| [Authored 4K](evidence/mapgyver-m01-procedural-authored-control-nvidia-q02-2026-10-28.json) | 8,294,400 | 0 | 2 initial images |

Procedural producers draw once with 32 uniform bytes each; only the independent consumer maps
one full image. The real source-worker pools stay within three surfaces, GPU-copy totals match
publications, renderer SDK receives/readbacks are zero, children exit/reap and all six logical
texture ledgers drain. [Cross-adapter analysis](evidence/mapgyver-m01-card-accuracy-nvidia-analysis-2026-10-28.json)
verifies all eight Intel/NVIDIA captures against identical native source/assets/executable
`ad04cf11b7778a52a9fbcc3a224266af700326f9a601d7252154a17131eb1e72`.

[Native settings observations](evidence/mapgyver-m01-card-accuracy-nvidia-conditions-2026-10-28.json)
record an already-Off overlay at 09:19:55 UTC and Off at 09:22:32. It remains Off;
no overlay setting was changed or restored to an older value. No desktop capture occurs during
measurement. This supersedes the older On-restoration observation for current state.

The audit caught rounded large NVIDIA GPU timestamp integers in the initial JavaScript-saved
parsed reports. [Those initial copies remain preserved](evidence/mapgyver-m01-nvidia-card-initial-serialization-2026-10-28.json).
The canonical captures above were restored from exact original runner stdout, with no native
rerun or measurement change; parsed reports now equal their original native JSON exactly.
The wrapper now defaults to checksummed gzip/base64 so large integers remain opaque to
JavaScript. Its earlier bytes are pinned to commit `73548569`; analysis records the old/new
wrapper hashes and verifies that this transport change is the only native-input-map difference.
Tests preserve a GPU timestamp above 2^53 and its adjacent integer exactly.

Both adapters now pass fixed procedural/authored-card accuracy through this process/bridge
route. Animated/lifecycle cases, native previews/physical outputs, sustained timing/GPU critical
path and physical retirement remain open. No complete M01 acceptance item closes.

## First PresentMon provider/event route — 2026-10-28

The [official portable 2.6.0 asset](https://github.com/GameTechDev/PresentMon/releases/expanded_assets/v2.6.0)
matches published SHA256 `b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af`
and has a valid Intel Corporation Authenticode signature. It is an ignored workspace tool,
not an installed service. The [capture](evidence/mapgyver-m01-presentmon-intel-official-route-2026-10-28.json)
records renderer PID 46748, exact-PID filtering, a fresh owned UUID session, twelve seconds
around a nominal 8+2-second Intel Q01 control, legacy CSV/QPC and disabled input tracking.
Native and collector exit 0; both output pipes are drained concurrently and all raw rows remain.

There are 1,192 CSV rows: 596 each for two **unlabeled** `DXGI Composed: Flip` swapchains.
The exact eight-second Present window `[1801629597025, 1801709597025)` at 10,000,000 ticks/s
has 480 Present records per chain, no reported drops and no unavailable display timing.
Each Display window has 480 events/intervals, including its retained pre-window predecessor.
Derived intervals exactly match `msBetweenDisplayChange`: p95 16.9752 ms, p99 20.8141 ms,
maximum 20.8759 ms. These tails remain recorded; no budget is relaxed or qualification inferred.
All 596 paired display timestamps are identical across the two chains. They are shared
compositor observations, not 960 independent display cycles or identified physical outputs.

No event/buffer loss is reported; zero loss is not independently verified. The unique session
is absent after normal exit, and the pre-existing Intel collector service remains running.
No default/foreign session or security/group setting is changed. Exact sample clipping,
integer QPC, missing/zero timing, dropped rows and unlabeled chains have focused parser tests.
This establishes the software provider/event capture route, not full displayed-cadence series,
optical/input latency, alignment, native role attribution or the source-to-output GPU critical path.

The [installed FrameView fork attempt](evidence/mapgyver-m01-presentmon-intel-initial-2026-10-28.json)
remains a failed record: native control succeeds, but that collector exits 1 with empty streams.
The host token is high integrity, native session creation/stop passes, and no failed owned
session remains; the fork's failure cause is unproven. The earlier
[help preflight failure](evidence/mapgyver-m01-presentmon-help-preparation-2026-10-28.json)
also remains: valid help exits 1 on stderr; the helper now validates advertised flags instead
of assuming help must exit 0. No probe/session was launched by that preparation failure.

The first official capture's outer scope text still says "vendor" from the initial helper;
its executable hash/path identify the official asset. The label/default were corrected for
future use. [Exact historical helper bytes](evidence/mapgyver-m01-presentmon-first-route-helper-source-2026-10-28.json)
pin the first capture's implementation. See [capture ownership/commands](../../tools/mapgyver/native-probe/README.md#pid-filtered-presentmon-route-control).

## ETW access after administrator relaunch — 2026-10-28

The user relaunched VS Code as administrator and reported a successful preflight. A native
recheck of the current release probe confirms `start_status: 0` and `stop_status: 0` outside
the agent sandbox as `BENPORTABLE\bkupe`, with high integrity (`S-1-16-12288`). The same binary
inside the sandbox as `BENPORTABLE\codexsandboxoffline` still returns status 5/access denied.
The earlier denied-token evidence below remains historical, not a host-wide capture blocker.
See the [paired access evidence](evidence/mapgyver-m01-etw-access-2026-10-28.json).

The development account is already a direct member of Performance Log Users (`S-1-5-32-559`),
although that group is absent from the current administrator token. No group, privilege,
policy or sandbox setting was changed during this recheck. Sign out of Windows and back in
before testing a fresh non-elevated development process; membership configuration alone does
not verify its effective rights. Do not grant capture rights to the separate sandbox account.

Use the working host execution path for capture, verify the providers/events needed for
displayed cadence, and retain the token used with the capture evidence. This empty-session
test enables no providers and writes no trace file; it does not qualify actual display events,
scanout or physical latency. M01 acceptance remains open.

## Fixed procedural-card shader accuracy across processes — 2026-10-28

The existing `Card` shader now has an independent CPU reference for its digits, grid/checker,
color/alpha regions and five marker words. Normal drawing and this finite exercise share
`Card::draw_input`; shader source and 32-byte constant-buffer layout are unchanged. Producers
draw once without CPU pixel uploads, then publish their GPU textures through Spout, the real
source-worker bridge and `Quad` composite to an independent consumer. That consumer compares
every output pixel/component after one full-image map, with no masks/exclusions. Tolerance
remains the predeclared 2/255. Fixed frame IDs 123456/789012 cover all decimal digits alongside
channel IDs 0/1; [commands and exact inputs](../../tools/mapgyver/native-probe/README.md#procedural-card-shader-accuracy)
include the fixed colors and marker payloads, which are not latency/clock observations.

| Intel procedural diagnostic | Process IDs A / B / renderer / consumer | Pixels compared | Maximum BGRA error | Outside tolerance | Bridge publications A / B |
| --- | --- | --- | --- | --- | --- |
| [1080p](evidence/mapgyver-m01-procedural-card-intel-q01-2026-10-28.json) | 46124 / 31032 / 7008 / 22372 | 2,073,600 | 1 / 1 / 1 / 1 | 0 | 15 / 18 |
| [4K](evidence/mapgyver-m01-procedural-card-intel-q02-2026-10-28.json) | 29820 / 36288 / 12716 / 19256 | 8,294,400 | 1 / 1 / 1 / 1 | 0 | 34 / 37 |

Each producer records exactly one draw/32 uniform bytes and zero CPU pixel uploads/readbacks.
Renderer/ingest readbacks remain zero; only the consumer maps one complete image. All PIDs are
distinct, children exit successfully and are reaped, senders unregister, bridge pools stay
within three surfaces and all six texture ledgers drain. Child readiness/terminal reports bind
the fixture kind; an authored result cannot satisfy the procedural request. Failed comparisons
retain full-image totals and the first sixteen differences through the bounded report protocol.

[Authored 1080p](evidence/mapgyver-m01-procedural-authored-control-intel-q01-2026-10-28.json)
and [authored 4K](evidence/mapgyver-m01-procedural-authored-control-intel-q02-2026-10-28.json)
controls pass on identical release executable/source/asset/wrapper inputs. Executable SHA-256:
`ad04cf11b7778a52a9fbcc3a224266af700326f9a601d7252154a17131eb1e72`.
Earlier captures retain their historical fingerprints. No desktop input/capture or overlay/
display settings change is used; NVIDIA remains last observed restored On at 08:17:51 UTC.

This adds `PASSED_NATIVE` for fixed procedural-card accuracy through the Intel process/bridge
route. Animated/lifecycle cases beyond these inputs, NVIDIA, native previews/output scanout/DPI,
sustained cadence/latency/GPU critical path, physical residency and full Q05 remain open.
M01's complete acceptance items remain 1/8 closed; partial evidence earns no item increment.

## Cross-process authored-card accuracy through the source-worker bridge — 2026-10-28

The finite accuracy diagnostic now also runs two independent producer children and an
independent consumer child around the renderer process. Each producer uploads its immutable
authored card once, then sends GPU Spout frames. The renderer uses the existing `source_worker`
implementation: separate ingest devices, bounded NT-handle/keyed-mutex surfaces and retained
readers. The actual `Quad` compositor writes the fixed output, which reaches the consumer
through GPU Spout. Only the consumer maps one complete image and runs the CPU reference.
Private child pipes carry bounded typed control/reports; no frame pixels or native handles
cross them. Parent admission verifies PID/role/adapter, full coverage and terminal sequencing.

| Intel process diagnostic | Process IDs A / B / renderer / consumer | Pixels compared | Maximum BGRA error | Outside tolerance | Bridge publications A / B |
| --- | --- | --- | --- | --- | --- |
| [1080p](evidence/mapgyver-m01-authored-card-processes-intel-q01-2026-10-28.json) | 33772 / 17556 / 7132 / 16908 | 2,073,600 | 1 / 1 / 1 / 1 | 0 | 13 / 16 |
| [4K](evidence/mapgyver-m01-authored-card-processes-intel-q02-2026-10-28.json) | 36252 / 37996 / 10932 / 41224 | 8,294,400 | 1 / 1 / 1 / 1 | 0 | 28 / 31 |

Both cases preserve the predeclared 2/255 component tolerance and compare every pixel, including
edges, the source seam and straight-alpha stripes; there are no masks/exclusions. Bridge GPU
copy totals are 107,827,200 / 58,982,400 bytes at 1080p and 928,972,800 / 114,278,400 bytes at 4K,
exactly one source-sized copy per publication. Both pools peak at their existing three-surface
cap. Source ingestion remains on its owning workers; renderer SDK receive calls are zero.
Renderer and ingest CPU pixel uploads/readbacks remain zero. The only uploads are the two
initial producer images; the only full-image readback is on the consumer. Reports retain all
bridge operation/admission/timing observations without treating this finite exercise as timing
qualification. Reader ownership drops before source-worker shutdown; all six texture ledgers
drain, all three senders unregister and all three child processes exit successfully and are reaped.

[Local 1080p](evidence/mapgyver-m01-authored-card-local-process-control-intel-q01-2026-10-28.json)
and [local 4K](evidence/mapgyver-m01-authored-card-local-process-control-intel-q02-2026-10-28.json)
controls verify the original one-process route on the same binary and every source/asset/tool
input. All four runs use release executable SHA-256
`a67469f610ad96617d0b28c5b20b84e4f83983656fa1d1ad35912c5344dac788`.
Earlier local-only and lifecycle evidence below retain their original fingerprints.

This adds `PASSED_NATIVE` for Intel finite authored-image accuracy across processes and the
real ingest bridge. It does not qualify procedural-card/frame-label generation, NVIDIA,
native previews/scanout/DPI alignment, sustained cadence/latency/GPU critical path, physical
residency or complete Q05 retirement. No desktop automation/capture or settings changes are
used. NVIDIA overlay remains last verified restored On at 08:17:51 UTC; no NVIDIA run occurs.

## Finite authored-card transfer and sampling accuracy — 2026-10-28

The qualification harness now owns a separate headless `color-accuracy` command and
[capture/verification wrapper](../../tools/mapgyver/run-color-accuracy.py). It uploads two
immutable CPU-authored BGRA images once on separate D3D11 devices, receives both through GPU
Spout on the renderer device, draws the actual `Quad` shader into the fixed composite and
sends that through GPU Spout to a fourth consumer device. All four devices are in one process.
Only this finite diagnostic maps the full output; ordinary rendering/timing modes are unchanged.

The independent CPU reference samples authored source bytes using texel centers, bilinear
filtering and clamp addressing. Cards contain asymmetric corner identities, horizontal/vertical
ramps, checker edges and five straight-alpha stripes, including nonzero color at alpha zero.
Every pixel and BGRA component is compared, including boundaries and the channel seam. The
tolerance was fixed at 2/255 per component before the native runs. No pixel mask or exclusion
is used. Mismatch counts cover the entire image; only the first sixteen diagnostic differences
are bounded. Tests reject flips, channel swaps, BGRA swaps, premultiplication and errors above
tolerance, and independently verify unequal bilinear weights and corner/seam identities.

| Intel diagnostic | Composite pixels compared | Maximum BGRA component error | Pixels outside tolerance | Initial CPU pixel upload | Consumer readback |
| --- | --- | --- | --- | --- | --- |
| [1080p](evidence/mapgyver-m01-authored-card-intel-q01-2026-10-28.json) | 2,073,600 | 1 / 1 / 1 / 1 | 0 | 2 calls, 11,980,800 bytes | 1 image, 8,294,400 bytes |
| [4K](evidence/mapgyver-m01-authored-card-intel-q02-2026-10-28.json) | 8,294,400 | 1 / 1 / 1 / 1 | 0 | 2 calls, 36,864,000 bytes | 1 image, 33,177,600 bytes |

Both runs use Intel adapter 0, LUID `00000000:00016c2a`, with source B fixed at 1280×720.
The release executable SHA-256 is `fbc8201a40fa2488ad98f98c299a3fc2bdca99947cd7fc2221ba6ff08d62076d`.
All native inputs and the wrapper's own hash remain unchanged through both runs. The wrapper
revalidates exact image coverage, upload/readback costs, two renderer draws, one renderer clear,
GPU endpoint observations, cleanup and adapter identity. Renderer CPU pixel uploads/readbacks
are zero. All test senders unregister and all four logical texture ledgers drain after fences.
No desktop input/capture, overlay, display or other setting changes are used for these Intel runs.

This is `PASSED_NATIVE` for finite authored-image transfer/sampling on Intel. It does **not**
close full Q01/Q02 accuracy: procedural-card/frame-label generation, the source-worker bridge,
cross-process full-image transfer, native preview/scanout geometry and NVIDIA remain to test.
Nor does this diagnostic's elapsed time qualify sustained cadence, latency, GPU critical path,
driver-internal copies, physical residency or full Q05 retirement. The two one-time CPU uploads
and one full-frame diagnostic readback cannot become a passing GPU-only timing route.

The lifecycle and sustained performance captures below retain their exact earlier fingerprints.
They are not relabeled as runs of this diagnostic-enabled executable. Golden/app runtime and
UI/protocol code are untouched; all new mechanisms remain owned by the qualification harness.

## NVIDIA JS/document interruption and cross-adapter reconciliation — 2026-10-28

The same navigation-owned release executable and every recorded source/asset input now have
an NVIDIA counterpart. The [capture](evidence/mapgyver-m01-editor-isolation-nvidia-2026-10-28.json)
and [independent analysis](evidence/mapgyver-m01-editor-isolation-nvidia-analysis-2026-10-28.json)
record adapter 1, RTX 5080 Laptop GPU, LUID `00000000:000170d3`. Lifecycle completion takes
135.991 seconds against the unchanged 185-second watchdog; scoped teardown is 0.484 seconds.

| Observation window | Duration | Native / Spout accepted calls | Unique consumer fixture frames A / B | Editor status polls |
| --- | --- | --- | --- | --- |
| JavaScript busy | 61.0433693 s | 3,663 / 3,663 | 3,662 / 3,662 | 0 |
| Blank document | 61.0237627 s | 3,662 / 3,662 | 3,661 / 3,661 | 0 |

All 61 one-second intervals in each window have output progress and multiple checksum-valid
frames from both sources at the independent consumer. No invalid markers occur in either
interruption window. Three startup-invalid B markers and 29 A-absence observations remain
in the whole-session accounting. Session 2 has no preview registrations/layout/render
acknowledgements; session 3 restores both previews at revision 2. All 8,126 admitted renderer
batches join exact source dependencies; renderer and source admission have zero busy misses.
Tracked texture ownership drains after fences. The [normal-launch control](evidence/mapgyver-m01-editor-session-nvidia-control-2026-10-28.json)
also accepts all 480 sampled calls/path, with fixed geometry and complete dependency coverage.

[Native overlay observations](evidence/mapgyver-m01-editor-isolation-nvidia-conditions-2026-10-28.json)
verify original On at 07:50:04 UTC, Off before startup at 07:50:21, Off after both measurements
at 08:17:33, and original On restored at 08:17:51. The NVIDIA settings window was minimized
during measurement; no desktop capture/input, compilation or other owned GPU test ran then.
Only the explicitly authorized overlay setting was changed. Screenshot observations are
recorded as text; screenshot payloads are not retained. The initial minimize action succeeded;
its immediate state refresh reported a minimized window, so no screenshot was taken then.

Both adapters now prove this scoped busy-JS/blank-document/reconnect behavior on identical
final inputs. The analysis verifies all four final control/lifecycle captures and preserves
the earlier Intel intermediate capture separately. This **does not qualify 60-fps scanout**,
full-card pixel accuracy, the asynchronous GPU critical path, physical residency or full Q05.
The pre-lifecycle performance cohorts retain their historical fingerprints and scope.

## Intel JS/document interruption and session ownership — 2026-10-28

The qualification harness now has an explicit `editor-isolation` exercise. Its host supervisor
requests a finite 65-second block of the owned WebView's JavaScript thread, measures an interior
window over 60 seconds, navigates that WebView to `about:blank`, measures another >60 seconds,
then reconnects its original bundled document. This changes the real frontend lifecycle;
the native output, Spout sender, ingest workers and independent consumer continue operating.
No desktop input/capture, display changes or NVIDIA tests occur in this Intel capture.

Document sessions now scope layout revisions. The host page-navigation callback invalidates
the old session at document start, clears its registrations and requires a fresh low revision
from the new page. This also covers ordinary reloads without test-controller bookkeeping. The frontend
binds its first host session once; late old-document layouts and render acknowledgements are
ignored/rejected at the host/renderer boundary. Rust DTOs generate all updated TypeScript.
This is exercised harness behavior for a future public host seam, not a copied production host.

[Final native capture](evidence/mapgyver-m01-editor-isolation-final-intel-2026-10-28.json) and
[independent analysis](evidence/mapgyver-m01-editor-isolation-final-intel-analysis-2026-10-28.json)
record Intel adapter 0 with a 185-second watchdog, explicit lifecycle completion at 135.690
seconds, and 0.419-second scoped teardown. The release executable SHA-256 is
`65a4d0ca925f9166c530dceb96cf9281db88f34b2fbee3bbbea778d65e5ee439`:

| Observation window | Duration | Native / Spout accepted calls | Unique consumer fixture frames A / B | Editor status polls |
| --- | --- | --- | --- | --- |
| JavaScript busy | 61.0284793 s | 3,661 / 3,661 | 3,662 / 3,662 | 0 |
| Blank document | 61.0143981 s | 3,660 / 3,660 | 3,661 / 3,655 | 0 |

All 61 one-second intervals in each window have native/Spout progress and multiple distinct,
checksum-valid source frames observed by the independent GPU consumer. Blank-document snapshots
have session 2, zero preview registrations and zero layout/render acknowledgements. Reconnect
has session 3, two regions and layout/render revision 2. The host records a 65,000-ms frontend
busy interval, with the observation window wholly inside its start/finish receipts.

This proves the scoped output-continuity/document-reconnect behavior, **not 60-fps qualification**.
The final run has no renderer/source admission misses; all 8,123 admitted renderer batches
join exact source dependencies. Three channel-B invalid markers occur during initial startup
before either window, alongside 31 channel-A absence observations during the known source
lifecycle. Their raw rows and whole-run
counts remain; the first analyzer's overly broad whole-session invalid-marker rejection was
corrected to verify every interruption-window marker independently. No data is discarded.
All tracked renderer/source/child texture ownership drains after fences. Full GPU critical
path, displayed cadence, pixel alignment and physical residency remain unqualified.

A separate [final normal-launch control](evidence/mapgyver-m01-editor-session-final-intel-control-2026-10-28.json)
retains 480 accepted calls/path, expected fixed geometry, complete context query/dependency
coverage and zero post-fence ownership. Both captures validate against the new runtime/asset
fingerprints. The earlier twelve sustained performance samples below retain their exact
pre-lifecycle-fixture fingerprints; they are not relabeled as captures of this binary.
The NVIDIA counterpart is recorded above; full-card accuracy remains to test.

The [initial lifecycle capture](evidence/mapgyver-m01-editor-isolation-intel-2026-10-28.json),
[initial analysis](evidence/mapgyver-m01-editor-isolation-intel-analysis-2026-10-28.json) and
[initial normal control](evidence/mapgyver-m01-editor-session-intel-control-2026-10-28.json)
retain the intermediate implementation, whose supervisor explicitly assigned sessions.
It recorded 12 renderer, 8 source-A and 1 source-B admission misses, a blank-window call
shortfall and five startup-invalid B markers. These observations remain historical; they
are not relabeled as results from the final navigation-owned implementation. Analyzer/tool
hashes in that initial analysis identify its earlier verification, not current tools.

## Profiled NVIDIA series and cross-adapter reconciliation — 2026-10-28

[NVIDIA analysis](evidence/mapgyver-m01-submission-stages-nvidia-analysis-2026-10-28.json) now
retains three independent 30+120-second release samples each for Q01 and Q02 on RTX 5080 Laptop,
LUID `00000000:000170d3`. The exact executable, source, lock and asset fingerprints match the
current Intel cohort below. This turn changes no runtime source, protocol, app or Golden code.

[Native overlay conditions](evidence/mapgyver-m01-submission-stages-nvidia-conditions-2026-10-28.json)
record original On at 06:30:13 UTC, verified Off before startup at 06:30:27, still Off after all
owned probes exit at 06:51:28, and restored original On at 06:51:44. Verification uses NVIDIA
App Features screenshots and accessibility state through the computer-use skill. Capture is
inactive during measurement; no display, driver, security or other setting changes.

| NVIDIA workload / sample | Accepted calls per editor / native output / Spout | Signal / Flush stage maximum | Consumer age p95 A / B |
| --- | --- | --- | --- |
| Q01 / 1 | 7,200 / 7,200 / 7,200 | 0.605 / 0.591 ms | 36.887 / 48.685 ms |
| Q01 / 2 | 7,200 / 7,200 / 7,200 | 0.736 / 0.445 ms | 25.504 / 31.919 ms |
| Q01 / 3 | 7,200 / 7,200 / 7,200 | 0.551 / 0.677 ms | 19.838 / 30.773 ms |
| Q02 / 1 | 7,200 / 7,200 / 7,200 | 0.621 / 0.251 ms | 15.784 / 15.175 ms |
| Q02 / 2 | 7,200 / 7,200 / 7,200 | 0.743 / 0.455 ms | 43.390 / 31.834 ms |
| Q02 / 3 | 7,200 / 7,200 / 7,200 | 0.586 / 0.281 ms | 40.930 / 46.617 ms |

All six runs preserve 7,200 renderer and source-context samples, zero warmup/sampled admission
misses, complete query accounting, valid independent consumer markers and zero tracked
post-fence ownership. All 43,200 admitted renderer batches have exact source dependency joins;
161,768 raw context spans validate. Q02 samples 2/3 retain 675/354 repeated source-B bridge
references respectively; these do not become unique fixture-frame claims. Their observed
consumer-age p95 values still meet 50 ms. Recorded CPU and per-context GPU percentiles meet
their scoped targets. Query-end, signal and flush maxima are 0.2386, 0.7430 and 0.6768 ms.

Correlated serial GPU-span p95 ranges from 2.012 to 4.786 ms across the Q02 samples. CPU-envelope
p95 ranges from 21.141 to 33.543 ms. Neither metric is substituted for the full asynchronous
GPU critical path; that budget remains unqualified. The earlier Intel 207.798-ms combined
QueryEnd delay is not reproduced or attributed, and its failing cadence cohort stays retained.
No claim about physical residency, displayed cadence, scanout or full retirement follows from
these successful submission/query/consumer observations. NVIDIA still has no attached DXGI
outputs; the existing physical 4K presentation limitation is unchanged.

The short NVIDIA Q02 control accepts all 480 calls/path and preserves 477/480 source receives,
with 26 absent source-A references and complete admitted timing/dependency coverage. The
timestamp-free control completes 480 ticks and retires without a final clear. Both controls
share native inputs with the six release runs. Lossless collection preserves 3,810,251 and
3,832,439 bytes across 59 chunks per sustained series, with all stream/envelope hashes verified.

Together the current Intel/NVIDIA cohorts now provide twelve full release samples with the
same native inputs: 86,395 admitted renderer dependencies and 323,619 raw context spans. Intel
admission misses and GPU outliers remain recorded separately. The remaining M01 work is full
GPU-path measurement, busy/disconnected editor behavior, full-card accuracy, memory-retention
explanation and the unavailable physical/display/retirement checks, rather than another
unchanged performance cohort. M02 remains gated by those acceptance criteria.

## Split submission stages and current Intel series — 2026-10-28

The harness now separates the previous combined QueryEnd interval into profiling end markers,
completion-signal enqueue and context Flush/admission commit. A must-use signaled batch borrows
the admission gate until its consuming flush commits the reservation. The default source-worker
submission still performs both steps together. Command order, frame work, the two-batch bound
and explicit flush count are unchanged; no wait or GPU padding is introduced. Schema 14 and its
analyzer guard require the nineteen-operation registry. Stage times remain CPU wall intervals,
including bookkeeping/preemption; they do not identify exclusive driver execution time.

[Current Intel analysis](evidence/mapgyver-m01-submission-stages-intel-analysis-2026-10-28.json)
retains three independent 30+120-second release samples for each of Q01 and Q02. Both series,
the short control and the timestamp-free control have identical current native inputs. All
envelope and collector stream byte counts/SHA-256 values validate. No desktop capture, build,
other owned native probe or setting change occurs during these runs. Overlay state is declared
On; this turn uses Intel only, and makes no fresh NVIDIA qualification or overlay observation.

| Workload / sample | Accepted editor / output / Spout calls, target 7,200 | Signal / Flush stage maximum | Consumer age p95 A / B |
| --- | --- | --- | --- |
| Q01 / 1 | 7,193 / 7,193 / 7,195 | 0.876 / 0.163 ms | 31.336 / 31.324 ms |
| Q01 / 2 | 7,200 / 7,200 / 7,200 | 2.392 / 0.451 ms | 28.331 / 49.9949 ms |
| Q01 / 3 | 7,200 / 7,200 / 7,200 | 1.090 / 0.289 ms | 45.460 / 33.365 ms |
| Q02 / 1 | 7,200 / 7,200 / 7,200 | 1.344 / 0.173 ms | 33.704 / 33.726 ms |
| Q02 / 2 | 7,200 / 7,200 / 7,200 | 2.641 / 0.664 ms | 37.659 / 49.249 ms |
| Q02 / 3 | 7,200 / 7,200 / 7,200 | 3.693 / 0.197 ms | 42.498 / 44.663 ms |

Q01 sample 1 retains five sampled renderer/source-A admission misses and one source-B miss;
its editor/output shortfall is 0.097222%, within the unchanged 0.1% target. Its correlated
serial GPU-span maximum reaches 203.290 ms, preserving a GPU-progress outlier despite short
CPU submission stages. Q02 sample 2 separately retains warmup misses of 5 renderer / 4 source A /
1 source B. These do not become missing profiling samples or fabricated zero-cost work.

All 43,195 admitted sampled renderer batches have exact dependency coverage; 161,851 raw GPU
spans validate across all eighteen context scopes. Query accounting is complete and all tracked
renderer/source/child ownership drains. Recorded per-context GPU, CPU submission and consumer-age
p95 targets meet their scoped budgets; full asynchronous GPU critical path stays unqualified.
The CPU envelopes still cannot establish that target. Receive-batch identity is distinct from
unique pixel-frame identity, which remains independently checked by consumer markers.

The prior 207.798-ms combined QueryEnd delay does not reproduce in these six samples. End-marker
closure reaches at most 0.040 ms, signal 3.6933 ms and flush 0.6641 ms. This is **no stall fix or
internal-cause attribution**: the earlier failed cadence cohort remains retained. Use these
separate stages if the failure recurs; do not average it away or repeat until favorable.

The [timestamp-free control](evidence/mapgyver-m01-submission-stages-intel-headless-2026-10-28.json)
admits all 480 ticks, drains all ownership and retires without a final diagnostic clear. The
[short Q02 control](evidence/mapgyver-m01-submission-stages-intel-control-2026-10-28.json) likewise
accepts all 480 renderer calls and preserves 477/480 source receives with complete admitted
query coverage. Native source-worker interfaces and production Golden/app code remain unchanged.
Current-variant NVIDIA series still require fresh native overlay-Off verification and restoration;
physical/display, retirement and full GPU-path criteria remain open.

## Correlated source/renderer evidence — 2026-10-28

The harness now records exact source receive-batch IDs on bridge publications, independently of
the reused texture allocation. Every sampled composite records the retained reader IDs it draws.
All three contexts retain bounded raw query spans, including warmup, with local GPU ticks and
CPU-QPC submission/completion-observation stamps. Warmup remains outside sampled distributions.
The runner's schema 13 losslessly compacts these records; source/resource counters remain intact.
This is harness instrumentation, with no production Golden/app/UI protocol change.

[Pipeline analyzer](../../tools/mapgyver/analyze-pipeline.py) joins these exact dependencies and
recomputes context distributions. It never subtracts timestamps belonging to different devices
or different disjoint scopes. Two derived observations have deliberately distinct meanings:

| Observation | Scope and limitation |
| --- | --- |
| Per-composite serial sum of correlated source/render spans | Charges the actual consumed batch spans, including reused sources; omits inter-stage gaps and is not the asynchronous critical path |
| Earliest consumed ingest CPU stamp to observed renderer GPU completion | Conservative wall envelope including scheduling, queueing, polling and retained-frame age; an upper bound above target does not prove GPU-budget failure |

Absent sources remain explicit. Missing, invalid, skipped, pending or truncated context results
cannot become zero-cost stages or successful dependency coverage. Each span/dependency log caps
at 216,001 records. No pixel readback, upload, GPU synchronization wait or source-selection policy
is added. Traced warmup now includes timestamp commands; older fingerprints and explicit headless
event-only negative controls retain their distinct instrumentation behavior.

The first [Intel Q02 control](evidence/mapgyver-m01-pipeline-intel-controls-2026-10-28.json) uses
two-second warmup and an eight-second sample. All 480 renderer batches have exact dependency
coverage, including one warmup reference per source, 26 source-A absence references and four
repeated source-A readers. Renderer/source context query accounting is complete, raw consumer
markers validate and all post-fence tracked ownership is zero. Each renderer path accepts 480
calls without busy/late outcomes. This does not establish displayed cadence.

The serial span sum has p95/p99 17.747/20.773 ms; the CPU envelope has p95/p99 32.985/33.188 ms.
Neither qualifies the unchanged full GPU critical-path target. Consumer age p95 is 56.480 ms
for A and 47.465 ms for B, preserving the A failure against 50 ms. A short lifecycle-heavy
control cannot replace the prescribed 30+120-second repeated series or erase prior failures.

An [invalid validation attempt](evidence/mapgyver-m01-pipeline-stale-binary-2026-10-28.json)
started the old executable before its release rebuild finished; file locking made that build
fail. Its runner schema 13 cannot substitute for missing native dependency/raw-span fields.
The schema guard rejects the retained capture; rebuilding completed before the valid control.

The [sustained Intel Q02 analysis](evidence/mapgyver-m01-pipeline-intel-analysis-2026-10-28.json)
now retains three independent 30+120-second samples with identical current runtime inputs.
All 21,583 admitted renderer batches have complete dependency coverage, including their warmup
references; all nine context traces have zero skipped, invalid, pending or truncated results.
Raw consumer markers validate and every tracked renderer/worker/child ledger drains. Receive
batch IDs identify bridge publications, not unique fixture pixels; independent consumer markers
remain the authority for source-frame age/repetition.

| Sample | Accepted editor / native output / Spout calls, target 7,200 | Serial span sum p95 / p99 | CPU envelope p95 / p99 | Consumer age p95 A / B |
| --- | --- | --- | --- | --- |
| 1 | 7,194 / 7,194 / 7,195 | 14.049 / 16.944 ms | 28.455 / 28.561 ms | 48.168 / 46.165 ms |
| 2 | 7,200 / 7,200 / 7,200 | 16.044 / 18.418 ms | 31.222 / 31.335 ms | 46.745 / 33.829 ms |
| 3 | 7,186 / 7,187 / 7,188 | 11.768 / 13.912 ms | 32.438 / 32.557 ms | 28.501 / 46.237 ms |

Sample 3 fails the unchanged 0.1% missed-call target: shortfalls are 0.194/0.181/0.167%.
Its maximum render tick is 209.139 ms, with 207.798 ms in QueryEnd. Other measured operations
in that tick stay below one millisecond. QueryEnd includes timing end markers and timeline
signal/flush, so its internal blocking or preemption cause remains unproven. Sample 1 retains
five renderer admission misses; sample 3 retains one renderer/twelve source-A/one source-B
misses. Complete query accounting describes admitted work, not absence of missed work.

All sustained consumer-age p95 values meet 50 ms, without replacing the short control's A
failure. No CPU envelope establishes the GPU target, and the serial sum is not used to claim
it. GPU spans can include CPU submission gaps, and source spans can finish after bridge
publication; their sum must not be interpreted as GPU utilization or elapsed pipeline time.
Next, isolate QueryEnd's end-marker/signal/flush intervals before choosing a runtime repair;
retain this failing cadence cohort. Final-variant Q01 and NVIDIA series remain open.

The initial sustained collection exceeded the terminal's 1 MiB per-result ceiling in both
completed reports; their middle bytes are [unavailable](evidence/mapgyver-m01-pipeline-terminal-limit-2026-10-28.json).
The owned third run was stopped and none of that incomplete series counts as evidence.
The replacement uses the paced collector, preserving all three normal envelopes across 56
indexed chunks. Its 3,615,951-byte stream and SHA-256, all envelope lengths/hashes and exact
native inputs validate. Pacing occurs after all native windows end; it changes only transport.
No desktop capture, build or other native probe runs during the measured replacement series.
NVIDIA overlay is unchanged On; only Intel devices run in this series.

## Timeline completion and isolated progress controls — 2026-10-28

The harness owns this change. Runtime frame admission and GPU retirement now use a monotonic
D3D11 timeline fence on their creating context; profiling queries remain separate. Admission
polling does not wait, the two-batch bound is unchanged, and teardown has a two-second completion
deadline. Required `ID3D11Device5`/`ID3D11DeviceContext4` interfaces fail explicitly when absent;
event-query admission is retained only as an explicit diagnostic backend. No production Golden,
app shell, protocol or editor implementation changes here.

[Analysis and artifact hashes](evidence/mapgyver-m01-timeline-completion-analysis-2026-10-28.json)
retain eight-second controls without windows, fixtures or a consumer. Every case begins after
the same completed 16×16 seed clear and attempts 480 ticks. Busy frame/QPC observations are
preserved before any final diagnostic clear or retirement; successful draining cannot erase a
stalled observation window.

| Headless control | Intel admitted / busy | NVIDIA admitted / busy, overlay Off |
| --- | --- | --- |
| Event admission, no profiler | 480 / 0 | 4 / 476 |
| Event admission, event-only warmup profiler | 480 / 0 | 4 / 476 |
| Event admission, timestamp profiler | 480 / 0 | 480 / 0 |
| Event admission, missing receiver + warmup profiler | 480 / 0 | 4 / 476 |
| Event admission, missing receiver + timestamp profiler | 480 / 0 | 480 / 0 |
| Event admission, per-tick small clear + warmup profiler | 480 / 0 | 4 / 476 |
| Final timeline admission + warmup profiler | 480 / 0 | 480 / 0 |
| Final timeline admission, no final diagnostic clear | 480 / 0 | 480 / 0 |

The final NVIDIA binary also repeats the event-warmup 4/476 negative control alongside timeline
480/0. This reproduces progress failure without Spout lifecycle or sharing, and demonstrates
that adding timestamp commands changes the observation. It does not identify an internal driver
cause or establish that every clear call executes physical GPU work. The repair adds no dummy
draws, timestamps, pixel readback or CPU image upload to the runtime. Initial Intel event controls
overlapped a brief CPU test build and are diagnostic progress observations, not CPU performance
qualification. See the [asynchronous Flush contract](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-flush),
[Signal ordering](https://learn.microsoft.com/en-us/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11devicecontext4-signal)
and [CPU-visible fence completion](https://learn.microsoft.com/en-us/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11fence-getcompletedvalue).

Native controls preserve their exact implementation fingerprints:

- NVIDIA Q01, 30-second warmup + eight-second sample, tests the intermediate admission repair.
  NVIDIA Q02 uses the final timeline admission/retirement implementation at the same durations.
  Both accept 480 calls per renderer path and 480 measured receives per source, with zero warmup
  or sampled admission misses and complete timestamp accounting on all three contexts.
- Final Intel Q01 uses two-second warmup + eight-second sample. It accepts all 480 renderer calls;
  source contexts record 477/480 sampled receives, preserving the shorter source coverage rather
  than fabricating missing samples. All recorded context queries complete, with no admission
  misses. Consumer raw markers and zero post-fence tracked ownership validate in all three runs.
- These controls are not three full-duration release samples. The prior six NVIDIA 30+120-second
  runs and earlier Intel failures retain their historical fingerprints. Full asynchronous GPU
  critical path, repeated final-variant Q01/Q02 series, displayed cadence and physical latency
  remain open; separate context percentiles cannot qualify the combined pipeline.

The final [NVIDIA reconstruction](evidence/mapgyver-m01-timeline-completion-nvidia-recreation-2026-10-28.json)
completes 100 rebuilds, verifies generations 1–100 independently and drains 101 renderer/202 source
scopes. Against the unchanged generation-10 live baseline of 595.520 MiB and 1062 handles,
maximum positive private growth is 21.184 MiB. Stable pre-retirement checkpoints plus the final
live drain have 1055–1067 handles; sparse partial-setup observations are retained separately.
Final live settling lasts 10.016 seconds and worker/window teardown takes 0.515 seconds.

This cohort meets the scoped relative-growth target but peaks at 616.703 MiB, above the earlier
failing cohort's 609.410 MiB absolute peak. Its higher baseline prevents attributing a memory fix
to timeline completion. Preserve the earlier 47.926-MiB growth failure and investigate retention
with controlled comparisons. Logical zero ownership and these partial handle/memory observations
do not close full Q05, physical residency, device removal or complete-process shutdown.

[Native conditions](evidence/mapgyver-m01-query-progress-conditions-2026-10-28.json) record verified
overlay Off before NVIDIA startup and after its final reconstruction, then restoration of the
original On at 05:17:26 UTC. Final Intel controls follow restoration. No desktop snapshots occur
during native measurements and no other setting changes. Thirty-five deterministic Rust tests,
sixteen Python tests and the Intel held-reader bridge test pass; earlier failures remain evidence.

## Published binding audit

Audited the crates.io packages on 2026-10-27, not just their README examples:

| Package | Package SHA-256 | Finding |
| --- | --- | --- |
| `spout2-rs 0.1.1` | `6e3d5a4fafc4e72ae9a57e78d8a4b84e7580a7671b9729c26894fe2434c73bb4` | RAII and unsafe raw GPU methods; sender accepts caller device, receiver lacks device injection |
| `spout2-sys 0.1.1` | `670dd837cb0132982b6113a6c553e99453b0f8ba3014bd697bd7236e1a9d88aa` | Public flat C shim exposes caller-device initialization and GPU texture receive/send; selected for this probe only |
| `spout-rs 0.2.0` | `75ba9b1769e162537a2de87ef76926dd815e58ac8c3f3c03954243a38fef7677` | Receiver accepts caller device, but safe `usize` methods pass unchecked device/texture pointers into C++; not chosen |

Primary package sources: [spout2-rs](https://docs.rs/crate/spout2-rs/0.1.1/source/),
[spout2-sys](https://docs.rs/crate/spout2-sys/0.1.1/source/),
[spout-rs](https://docs.rs/crate/spout-rs/0.2.0/source/).
The `spout2` packages record VCS commit `04db4225b261b8dacc8da0eeae9734bd75320557`;
the alternative records `46dc8444f37400ee267b937facabc8e3ebbfdac7`. The former omit a repository
URL in package metadata, so their VCS stamp alone does not prove the vendored SDK commit.

The durable [SDK audit report](evidence/mapgyver-m01-spout-sdk-audit-2026-10-27.json) compares all
29 packaged upstream SDK files against Spout tag `2.007.017`, commit
[`f49e2f469f8cb25f559a6eaa61a3f5b8173fc100`](https://github.com/leadedge/Spout2/tree/f49e2f469f8cb25f559a6eaa61a3f5b8173fc100).
All 29 match after CRLF-to-LF conversion, with no other source changes. Raw bytes differ solely
by line endings; both raw SHA-256 and Git blob identities are retained. The audit tool exits
unsuccessfully for any other difference, missing upstream file or truncated upstream tree.

Reviewed the shim and upstream `SpoutDX.cpp` lifecycle:

- `OpenDirectX11` borrows the application device without AddRef and separately retains its
  immediate context. `CloseDirectX11` releases the context; an app device must outlive the SDK
  instance. The harness enforces that with a Rust borrow and no Send/Sync SDK handle.
- The no-argument `ReceiveTexture` copies the shared source into an SDK-owned texture using
  `CopyResource` and `Flush`. Its returned pointer is borrowed. The harness AddRefs its texture
  interface before later SDK calls and rejects a receiver device differing from the requested one.
- `IsUpdated` consumption is necessary after receive; leaving the flag set stalls SDK updates.
  Size/handle changes and reconnect use the same named receiver instance. Caller-owned devices
  disable automatic adapter switching; failed interop is not permission to select another GPU.
- `SendTexture` uses SDK-managed sharing and size changes. This reuses upstream discovery/access
  synchronization rather than reimplementing the Spout protocol. The shim catches C++ exceptions
  into null/false outcomes; production status/retry semantics still require M03/M04 work.
- SDK frame counting can be disabled in local settings. A successful receive or `IsFrameNew`
  indication cannot alone establish unique delivered frames or 60 fps. The smoke reports poll
  observations and verifies content/lifecycle; cadence remains open.

`spout2-sys` compiles 11 SpoutGL support sources plus SpoutDX and its C shim using C++17/MSVC,
matches Rust's CRT through `cc`, and links statically. DX-only still links Windows OpenGL/D3D
system libraries. Its GNU/ARM/native-doc paths do not qualify the x64 target. The actual x64
compile/link succeeded locally. The preliminary runs did not capture cc's selected compiler/SDK;
full Q01/Q02 evidence must add those exact selections.

Both candidates declare BSD-2-Clause. The chosen SDK package includes upstream root/SDK notices;
the wrapper package includes Jaden Hills' shim/wrapper notice. The harness carries all three in
[THIRD_PARTY_NOTICES.txt](../../tools/mapgyver/native-probe/THIRD_PARTY_NOTICES.txt).
Keep notices beside a distributed probe; this is not a complete app dependency-license closure.
No dependency was added to Chataigne's graph, and root default members still select Chataigne.

## Native smoke results

[Evidence](evidence/mapgyver-m01-spout-smoke-2026-10-27.json) records source/manifest/lock/binary
hashes, commands, adapter LUIDs and output. The explicit hardware integration test passed on
both hardware adapters and rejected the software adapter (one test, 12.82 seconds). Strict
probe/all-target Clippy and root/Core formatting checks passed.

Two independent fixture processes generate GPU textures. Channel A resizes in place, disappears,
and restarts under its original name. B remains available throughout. Four bounded diagnostic
readbacks per run confirm A's red/green/blue phases and B's yellow marker. Child exit and sender
registration cleanup are checked. The same Rust receiver instance survives A's recovery.

| Adapter | Native evidence | Limits |
| --- | --- | --- |
| Intel Graphics, LUID `00000000:00016c2a` | Concurrent 1080p/720p GPU sharing, live resize and restart/content checks pass; B has 80 observations during A recovery | This is not 80 unique source frames; no presentation or timing qualification |
| RTX 5080 Laptop, LUID `00000000:000170d3` | Same checks pass; B has 81 observations during A recovery | Same limitations; no display output reported on this DXGI adapter |
| Microsoft Basic Render Driver | Explicitly rejected before fixtures launch | Software fallback cannot count as hardware success |

DXGI reports both attached outputs under Intel and none under NVIDIA, unlike the earlier CIM
controller/mode hints. Record actual presentation routing in the next probe. Output rectangles
are collected under a temporary per-monitor-aware-v2 thread context; this alone does not prove
mixed-DPI alignment or monitor identity. Inventory memory values are pre-allocation observations,
not peak texture/driver usage. The native test created no windows and changed no display settings.

## Ordinary-host underlay/output plumbing — 2026-10-27

The harness now has an optional `desktop` feature with locked Tauri 2.11.5/runtime-wry 2.11.4/
Wry 0.55.1, plus an isolated Svelte 5 fixture. It creates its own transparent WebView window
and separate native output window. Bare native window construction exercises Tauri's public
`unstable` feature at this pinned version; no Golden host or product bootstrap is copied.

The UI-thread host reads actual WebView2 rasterization/zoom metrics and converts CSS bounds
to typed physical viewports/scissors. It retains only the newest layout revision, bounds two
registrations, and keeps clipped source sampling distinct from the visible region. The Svelte
fixture supplies drag handles, scroll/dock/hide/zoom controls and opaque menu/dialog overlays.
The render worker receives both sources, composites them side-by-side into a 1920×1080 GPU
texture, presents native output, and submits it to a Spout sender. Editor presentation uses
a separate nonblocking swapchain; frame pixels never enter IPC.

Both HWNDs survive until worker-side SDK/GPU retirement. Fixtures receive a graceful stdin stop
request, with a one-second forced-stop fallback. Their sender registrations and the composite
sender are checked after cleanup. The UI thread then destroys its windows normally and exits
through Tauri's returning event loop, preserving the final report and worker errors.

[Recorded evidence](evidence/mapgyver-m01-underlay-plumbing-2026-10-27.json) includes exact
source/lock/embedded-asset/executable hashes, hardware/runtime registration, compiler discovery,
commands and full reports. Both eight-second runs exit successfully with empty stderr:

| Adapter | Preview presents | Native output presents / Spout sends | Host / browser scale |
| --- | --- | --- | --- |
| Intel `00000000:00016c2a` | 473 | 474 / 474 | 1.25 / 1.25 |
| NVIDIA `00000000:000170d3` | 473 | 474 / 474 | 1.25 / 1.25 |

These are API submission counts and geometry acknowledgements, **not visually qualified frames,
unique source-frame counts, alignment measurements or frame-time distributions**. The editor
regions are 683×300 and 537×240 physical pixels on this run; native output is an independently
sized window sampling the 1920×1080 composite. This is intentionally recorded as plumbing,
not Q01's exact workload or its sustained acceptance. No manual desktop interaction was automated.

The final native source-recovery integration test also passes on both hardware adapters and
rejects software (one test, 12.88 seconds). Three deterministic geometry tests pass: clipped
sampling at four scales, shared fractional edges, and atomic admission of bounded/empty layouts.
Default and desktop/all-target strict Clippy pass; Svelte checks report zero errors/warnings,
bundling and Rust/TS drift checks pass.

During verification, running the original DTO command concurrently with native tests attempted
to overwrite the live probe executable. DTOs now live in a small Rust library with a dedicated
`generate_probe_bindings` binary. The drift check passed while the final Intel probe ran and
the probe executable hash remained unchanged. An initial forced Tauri exit also lost the report
and emitted a Chromium class-unregistration diagnostic. Returning event-loop execution and
normal window destruction after worker completion preserve the report and eliminate that
diagnostic in both final runs. Neither earlier attempt counts as a qualified run.

## Independent composite receiver and bounded timings — 2026-10-27

The native probe now supervises a separate `observe` process on the same explicitly selected
adapter. It receives the actual composite Spout sender and validates two pixel locations after
four submitted checkpoints: red/yellow initially, green/yellow after source resize, dark/yellow
while source A is absent, and blue/yellow after recovery. Thus channel B's content is checked
through A's failure/recovery, beyond the earlier submission and metadata observations.

The validator copies only two isolated pixels into one staging row per attempt. GPU handoff
may expose the preceding composite briefly, so each checkpoint permits at most four attempts,
all retained. The final runs use five diagnostic readbacks each. Rendering never waits for an
acknowledgement; four bounded control messages and an eight-message report queue keep the
consumer outside frame cadence. A missing output fails after a two-second receive deadline.
The report records distinct parent/consumer process IDs and matching adapter LUIDs.

CPU work/start intervals and GPU timestamp spans now produce bounded distributions. A four-slot
query ring never waits during rendering; pending, invalid, skipped and truncated counts remain
explicit. Readiness preserves GetData's `S_FALSE` through the public generated Windows vtable,
because the ordinary `Result<()>` wrapper loses that distinction. End markers are explicitly
submitted before pacing sleep. A first diagnostic run omitted that submission, measuring the
sleep as part of the GPU span; its complete reference reports/source hashes are retained in
the [new evidence](evidence/mapgyver-m01-composite-consumer-2026-10-27.json), separate from corrected data.

| Corrected eight-second run | CPU work p95 / maximum | GPU span p95 / maximum | Query samples / omitted |
| --- | --- | --- | --- |
| Intel | 0.655 / 68.847 ms | 1.408 / 5.204 ms | 474 / 0 |
| NVIDIA | 0.662 / 68.466 ms | 0.704 / 2.431 ms | 474 / 0 |

These runs include startup and have no warmup. CPU work excludes pacing sleep. GPU span measures
elapsed command-timeline time, including dependencies/submission gaps; it is not physical
scanout or active shader occupancy. Worker start intervals are not per-output presentation
cadence. The observer's checkpoint duration begins after its control request, so it is not
source-to-display latency. No fixed Q01/Q02 performance acceptance is claimed from these values.

Seven deterministic tests and both explicit native integration tests pass. Native coverage
includes both hardware adapters, software rejection, four content checkpoints, missing-output
rejection and complete timing-query accounting. Default/desktop strict Clippy, Svelte checks
and DTO drift checks pass. Both final standalone runs exit successfully with empty stderr;
source/asset/executable hashes and all diagnostic attempts are retained.

## Fixed scenes, warmup and resource observations — 2026-10-27

The Rust-owned `qualify` modes now select Q01/Q02 source, composite and native client extents.
Both retain the 720p second source and request unclipped 640×360 / 480×270 physical previews.
GPU shaders generate channel/frame numerals, checker/grid regions, phase colors and alpha
stripes, uploading only 32 bytes of uniform metadata per source frame. BGRA8 UNORM values and
straight alpha pass through without conversion/blending. The independent receiver checks
opaque color and interior 75%-alpha samples at all four recovery checkpoints; this is a narrow
transfer check, not the production color/compositing policy or full-card visual qualification.

Short fixed-scene tests pass for both Q01/Q02 on Intel and NVIDIA. They check exact client and
unclipped preview extents, warmup exclusion, alpha/lifecycle results and sparse resource data.
An initial DPI move reduced the output to 1534×863 because size preceded monitor selection;
the host now moves its owned window before applying physical size. A second validator attempt
sampled a bilinear alpha boundary at 4K; constant-stripe samples now use interior coordinates.
These failed attempts are recorded in the [new evidence](evidence/mapgyver-m01-fixed-workloads-2026-10-27.json).

Two separate **release** runs use 30 seconds warmup followed by 120 seconds measurement on
Intel LUID `00000000:00016c2a`. Each records 7,200 CPU/GPU samples, zero geometry mismatches,
zero skipped/invalid/pending/truncated queries, 150 one-second resource observations and empty
stderr. All executable/source/embedded-asset fingerprints remain unchanged through each run.

| First sample only | CPU work p95 / p99 / max | GPU span p95 / p99 / max | Renderer private peak | DXGI process local-segment peak |
| --- | --- | --- | --- | --- |
| Q01 | 0.567 / 0.660 / 1.092 ms | 1.257 / 2.642 / 3.403 ms | 72.0 MiB | 79.4 MiB |
| Q02 | 0.639 / 0.764 / 2.502 ms | 4.807 / 5.442 / 9.680 ms | 127.2 MiB | 234.7 MiB |

Each distribution is within the fixed CPU/GPU submission budgets for that single sample.
The full three-sample series and warmed NVIDIA runs remain open. Worker start interval maxima
are 17.789 / 30.193 ms; these are worker observations, not displayed-frame cadence or latency.
The 4K native client allocation extends beyond the available monitor and does not qualify
4K scanout. No complete Q01/Q02 performance or memory acceptance is claimed.

Resource observations retain renderer, two source and independent receiver private bytes,
working sets and handles, plus renderer DXGI current usage/budget. Before/after GPU COM-release
snapshots are separate; UI startup overlaps the initial baseline, so their difference is not
a leak test. Exact texture live/retiring bytes, source/receiver GPU residency, WebView process
data, copy counts and driver retirement remain unmeasured. Integrated DXGI local usage is not
dedicated VRAM. All sparse rows and missing counters are retained in the evidence schema.

Eight deterministic Rust tests, three native integration tests and two Python evidence tests
pass. Default/desktop strict Clippy, Svelte checks, bundling and Rust/TS drift checks pass.
The [runner](../../tools/mapgyver/run-qualification.py) captures current environment and hashes
and emits one evidence object per process without editing files or display settings. Its
default remains three independent 30+120-second samples; these observations explicitly used
`--samples 1` and cannot close that requirement.

## Device generations and retirement — 2026-10-27

The probe now reconstructs its complete application D3D11 device/resource scope on the render
worker. `render_worker` owns the persistent session, source processes, independent receiver and
host-supplied HWNDs; its `generation` module owns each device, swapchains, shaders, imported/
composite textures and Spout endpoints. Rust borrows keep endpoints within their device scope.
After their destruction, context bindings are cleared and a submitted GPU event is drained
under a two-second worker deadline. Device/context COM release precedes the next generation.

`recreate ADAPTER q01|q02 CYCLES` admits 1–100 reconstructions. It waits for the original four
source lifecycle checkpoints before the first rebuild, then runs each replacement for at least
two seconds and requires a successful independent generation observation before continuing.
The source processes, receiver process, sender selector and native HWNDs stay alive. Each
replacement GPU composite carries a distinct 16×16 marker. The receiver checks that marker,
blue/yellow pixels and alpha stripes using five isolated pixels, at most four attempts per
generation. One pending request and individual bounded result lines avoid an unbounded final
observer message. Reports now keep timing/geometry under `generations[]`, with setup, consumer
acknowledgement, retirement and post-COM-release process observations kept separate.

Short reconstruction tests pass for Q01/Q02 on both Intel and NVIDIA: two rebuilds/three
generations per run, unchanged source/receiver process IDs, both presentations and independent
marker checks after each rebuild. A generation request before source recovery is rejected.
Nine deterministic Rust tests and five explicit native tests pass, alongside strict Clippy,
UI/DTO checks and the two Python evidence-integrity tests.

A repeated fixed-workload run exposed a startup race: newly registered source textures could
be opened before their first valid image. The native regression reported black/zero-alpha
pixels and failed. Fixtures now confirm their first submitted GPU image through a bounded
readiness pipe before the session starts; no CPU texture readback is introduced. Diagnostic
attempts span producer periods instead of repeatedly sampling one in-flight handoff. All
attempts remain recorded, and the exact color/alpha oracle and four-attempt cap remain intact.
The full native suite passes with this correction. Final fixture retirement also checks
successful exit and rejects the forced-stop fallback rather than inferring clean shutdown
from disappeared sender registrations. Losing the parent's control pipe also cancels fixtures;
the hardware EOF test checks that path independently of the normal stop command.

Timed qualification and bounded recreation are now distinct typed exercises. An initial
100-cycle run used an operating-time-only deadline and ended before completing the sequence;
its failed command, binary/source fingerprints and error are retained separately. Recreation
now has an explicit watchdog that budgets operation plus bounded retirement and ends after
the last acknowledged generation has operated for two seconds. Device setup/destruction
remain measured. This fixes test orchestration without changing Q01/Q02's 30+120-second
sampling method or any CPU/GPU/memory acceptance budget.

The [100-cycle release captures](evidence/mapgyver-m01-device-recreation-2026-10-27.json)
complete all 101 device scopes and 100 independently observed replacement markers on each GPU.
Source/receiver process identities remain stable and both runs exit with empty stderr.

| Q02 reconstruction | Duration | Private growth after generation 10 | Intermediate post-release handles | Maximum consumer-ready / retirement fence |
| --- | --- | --- | --- | --- |
| Intel | 224.543 s | 9.875 MiB | 346–354 | 159.209 / 3.081 ms |
| NVIDIA | 225.710 s | 52.180 MiB | 439–831 | 252.417 / 4.566 ms |

Comparisons use generations 10–99 while sources/receiver remain alive. The final snapshot
closes sources and is a different ownership set. NVIDIA accumulates resources and remains
**unresolved**; do not call its retirement qualified. A 10-second drain and isolated device,
Spout and swapchain ownership tests must distinguish delayed driver retirement from retained
resources. No 10-second drain or exact texture residency observation was captured here.

This is controlled application-device reconstruction. It does not inject DXGI device removal,
reset the display driver, change monitor settings or qualify hardware reset/unplug recovery.
Consumer acknowledgement time describes a test stage, not displayed-frame latency. Exact
driver/texture retirement and the full Q05 product lifecycle workload remain open.

## Native visual/input automation and menu repair — 2026-10-27

The user authorized desktop automation and the AGENTS.md update, then explicitly resumed it
after an accidental physical Escape. Checks use the computer-use skill against the probe's
own windows. No display settings, monitor arrangement or other app content was changed.

Native capture reproduced an actual fixture bug: the sticky menu's normal-flow margins moved
the preview DOM frames by about 4rem while native images kept their previous rectangles.
The menu is now an absolutely positioned overlay with explicit relative-unit offsets, and
menu visibility participates in geometry publication. The rebuilt release probe passes the
same menu check at baseline and fractional zoom. This repair belongs to UI presentation hints;
it introduces no backend/domain policy in the UI.

The [native UI evidence](evidence/mapgyver-m01-native-ui-2026-10-27.json) records 31 completed
native input actions on the 125% primary display: menu/modal open/close, preview hide/show,
docking, scrolling, captured drags beyond original preview regions, zoom and maximize/restore.
Settled native captures show opaque DOM chrome/overlays above the blue/yellow images, matching
preview outlines and clipping at viewport edges. WebView zoom at 100%, 125%, 150% and 200%
produces matching host/browser scales of 1.25, 1.5625, 1.875 and 2.5. These are zoom checks,
not substitutes for 150%/200% OS display scaling. A restore input without a confirmed transition
is excluded from the count; its successful coordinate retry is recorded separately.

Previews stayed hidden for over a minute. Their native pixels cleared in the settled capture;
the foreground native output still displayed the blue/yellow composite and output/Spout
submission counters advanced. An occluded output capture showed the foreground editor and
is explicitly discarded. Unique frame cadence and latency are not inferred from static colors
or submission counts. Immediate captures sometimes showed transitional geometry/content;
quantitative input-to-visible latency remains open.

Bounded title-bar drags crossed to the existing second monitor. At unchanged 125% WebView
zoom, host/browser scale changed from 1.5625 to 1.25, consistent with 125%→100% display scale;
the visible native fills stayed aligned. The second display did not receive its full 30-input
sequence before the 900-second timer ended. The probe then exited successfully with empty
stderr, matching layout/rendered revision 60 and unchanged binary/source/assets fingerprints.
All 900 sparse resource rows are retained. This interactive run is not fixed-workload timing
qualification; unavailable configurations and physical-pixel alignment measurements stay open.

## Headless ownership isolation — 2026-10-27

The harness now has a windowless `retirement` command at the native diagnostic boundary.
Each scope returns after device/context release, then the process records ten seconds of
idle drain without a device. No Golden or app production ownership path was changed.
The [17 retained control runs](evidence/mapgyver-m01-ownership-isolation-2026-10-27.json)
use the optimized debug build and preserve 2,600 per-scope observations, all drain snapshots,
binary/source/asset fingerprints and unchanged-input checks. These are separate from the
earlier release scene captures and do not close Q05.

| Control | Handles after scope 10 → final scope → ten-second drain |
| --- | --- |
| Intel device / Spout / timestamp-ring, 100 scopes each | 177 → 177 → 177 in all three runs |
| NVIDIA device / Spout / timestamp-ring, 100 each | 242 → 422 → 422 in all three runs |
| NVIDIA adapter inventory only, 100 | 151 → 150 → 150 |
| NVIDIA empty device / event query only, 100 each | 221 → 221 → 221 in both runs |
| NVIDIA texture creation only, 100 | 242 → 422 → 422 |
| NVIDIA texture creation only, 1,000 | 242 → 2,223 → 2,223 |

The texture-only case creates/releases one 4K BGRA texture per device, without drawing or
completion polling. Additional NVIDIA controls reproduce growth after skipping adapter
memory queries, explicitly releasing the context first, flushing after query destruction,
using a fresh joined thread, or substituting a modern D3D11 fence. All 100 explicit final
device Release observations return zero. No speculative extra-release or teardown workaround
is adopted. Process handles remain retained beyond the drain; their owner/type is unmeasured.

Private bytes behave differently: after 100 scopes, Intel's three runs drain to 19.9–27.4 MiB
and NVIDIA's corresponding runs to 15.6–20.1 MiB. Thus transient private growth is not evidence
of permanently retained allocations. NVIDIA's 1,000-scope texture-only process still has
80.4 MiB private memory after its drain; no stable long-run bound is established.

Read-only module inspection of that owned 1,000-scope process found `nvspcap64.dll`, NVIDIA
App's Game Proxy version 11.0.9.251; its hash is retained. The NVIDIA overlay setting is enabled.
The initial overlay UI inspection did not expose recording/replay state and left the enabled
setting unchanged pending approval. The later approved comparison below supersedes the
loaded-module-only hypothesis; the original version/hash evidence remains historical.

### Validation and capture-session control

The [validation evidence](evidence/mapgyver-m01-isolation-validation-2026-10-27.json) preserves
two failed Intel Q01 sample-count checks (285 and 330 against the unchanged 350–500 range),
a failed bounded resize observation and a failed initial source-color read. The content oracles,
retry cap and timing thresholds have not been weakened. The settings window was closed before
the second Q01 failure; NVIDIA overlay remained enabled throughout.

After resetting the computer-use JS session, a fresh, fingerprinted Intel Q01 debug control
records 477 CPU/GPU samples, zero skipped/pending/invalid queries, matching geometry and empty
stderr. CPU/GPU p95 are 0.605 / 2.425 ms. This is a short 2+8-second diagnostic, not a new full
qualification sample. A single before/after observation does not establish the slowdown's
cause. Keep desktop capture inactive during timing qualification and retain failures alongside
subsequent validation rather than replacing their evidence.

With desktop capture inactive, the subsequent full suite passes nine deterministic and five
native tests, including all fixed scenes, reconstruction and source pipe-loss cleanup across
both GPUs. Default/desktop strict Clippy, both Python evidence tests and root/Golden Core format
checks pass. The isolated ownership evidence also passes row/column/count/drain and local-link
integrity checks. NVIDIA handle retention is still a separate unresolved result.

## Qualification termination repair — 2026-10-27

The [native before/after control](evidence/mapgyver-m01-generation-termination-2026-10-27.json)
reproduced an incorrect success: closing a Q01 window returned exit 0 after 28.273 seconds
despite requesting 2 seconds warmup plus a 120-second sample. All four content lifecycle checks
had passed, so the old completion test accepted the shortened measurement. No historical full
sample is invalidated by that fact alone; the failure concerns explicitly interrupted runs.

The native worker now records a typed termination reason before resource retirement. Timed
qualification accepts only its full rendering deadline. Device reconstruction accepts only
explicit intermediate rebuilds and the final acknowledged two-second operating interval;
watchdog expiry cannot masquerade as completion even if all device scopes/markers exist.
Cancellation follows owned source/SDK/swapchain/GPU retirement before returning its error.
This is qualification policy in the disposable harness, with no app/Core behavior change.

The rebuilt Q01 run rejects the same native Alt+F4 intervention with exit 1, a cancellation
message and no successful report. The interactive underlay control still exits cleanly after
19.870 seconds of its requested 120, with `window_closed`, all four content checks and empty
stderr. All three controls retain binary/source/asset fingerprints and unchanged-input checks.
A separate failed startup screenshot attempt is listed separately rather than counted as a
cancellation pass. Unit regressions cover early close and last-generation watchdog expiry;
native scene/reconstruction tests assert their successful termination reasons. The full suite
passes eleven deterministic and five native tests; reconstruction passes again on both GPUs
after removing the redundant report boolean. Two Python tests, strict default/desktop Clippy,
format/whitespace checks and evidence/link integrity pass. No owned probe process remains.

## Logical allocation and cadence observability — 2026-10-27

The probe's worker-local ledger now follows shared ownership leases for application textures,
swapchain buffer sets, SDK receiver-private textures and SDK shared sender textures. Clones
alias one allocation. BGRA8 payload bytes are descriptor-based, with unsupported formats/mips/
multisampling rejected. Dropped leases remain in logical retiring totals until a context
completion marker covers this worker's possible use. The bounded four-slot query ring runs
completion markers through warmup while retaining timestamp warmup exclusion. Generation
reports expose post-drop/post-fence snapshots, aggregate peaks and operation counters.

The [four native controls](evidence/mapgyver-m01-allocation-cadence-2026-10-27.json) use one
immutable optimized-debug binary, 2-second warmup and 8-second samples. All four exit cleanly
with unchanged inputs; all 40 sparse allocation/DXGI observations survive runner-v3 compaction.

| Adapter / workload | Peak tracked logical bytes | Sampled accepted editor / native / Spout calls | After final context fence |
| --- | --- | --- | --- |
| Intel Q01 | 55.010 MiB | 476 / 477 / 477 | Zero tracked live/retiring allocations |
| Intel Q02 | 179.595 MiB | 476 / 477 / 477 | Zero tracked live/retiring allocations |
| NVIDIA Q01 | 55.010 MiB | 475 / 476 / 476 | Zero tracked live/retiring allocations |
| NVIDIA Q02 | 179.595 MiB | 476 / 477 / 477 | Zero tracked live/retiring allocations |

Each control also records one sampled busy editor Present and one late iteration across the
three output paths. These short runs are not the 30+120-second repeated release series and
do not establish a cadence-budget pass. Accepted calls, intervals and deadline lateness are
reported independently for the editor, native output and Spout. Renderer-owned readback-pixel
and uniform-upload counters are zero; source-card uniforms and diagnostic observer reads stay
separate. SDK send/receive/texture-query/observation counts do not become exact GPU-copy counts:
the audited sender can return success without copying, and receivers use private texture copies.

Raw DXGI statistics retain typed success/error results. The Intel Q02 final sparse observation
contains last-present counter 530 and frame present counter 412. Displayed cadence and queue
length remain unqualified: [Microsoft documents multi-monitor reliability limits](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getframestatistics).
No DwmFlush or recurring renderer pixel readback was introduced. Logical payload counts do
not measure driver alignment/residency, external shared-handle lifetimes or DWM retention.
Fixture-owned imported source allocations are not counted again as renderer allocations;
source/observer logical totals and exact internal SDK copies remain open.

The initial full suite exposed the prior standalone smoke's initial black B texture on NVIDIA.
That path did not wait for source readiness. It now uses the same first-submitted-image fence
as the windowed probe; its color oracle and four readbacks remain unchanged. The failure log is
retained alongside the subsequent passing suite: 14 deterministic Rust tests, five explicit
native tests across both GPUs, two Python tests and default/desktop strict Clippy. Native tests
reconcile operation/cadence counts, exact app texture bytes and zero tracked post-fence ownership
through Q01/Q02 and device reconstruction. Physical retirement stays a separate requirement.

## Approved NVIDIA overlay control — 2026-10-27

The user approved temporary disable/test/restore after the possible replay-buffer loss was
explained. The [controlled on/off/on captures](evidence/mapgyver-m01-nvidia-overlay-control-2026-10-27.json)
run 100 fresh device/4K-texture scopes and a ten-second idle drain in separate headless
processes, with identical executable/source inputs. The settings window is closed for each run.

| Overlay state | Handles at scope 10 / scope 100 / after drain |
| --- | --- |
| On, original | 242 / 422 / 422 |
| Off, temporary | 221 / 221 / 221 |
| On, restored | 242 / 422 / 422 |

The overlay's green switch and explicit `Toggle switch On` announcement verify restoration;
the settings window is closed afterward. No driver reset, privacy setting or persistent
workaround was used. `nvspcap64.dll` version 11.0.9.565 remains loaded in all three probe
processes, and its installed file hash is unchanged before/after. Earlier version 11.0.9.251
evidence is a different cohort. Loaded DLL presence alone does not diagnose the active behavior.

This comparison isolates the observed handle retention to the enabled-overlay environment;
internal handle ownership/type remains unmeasured. Each scope recreates a device, so it does
not establish a leak for each texture on one persistent device. Full-scene/Q05 qualification
remains open. The user explicitly chose to document overlay-off as the NVIDIA qualification
prerequisite. The [contract](mapgyver-qualification-contract.md#nvidia-overlay-prerequisite)
records that decision; all workload and numeric budgets remain unchanged and overlay-on failures
stay retained. Future qualifying processes must start with Off verified, restoring the original
setting after temporary tests. M01 still needs full-scene retirement and its other native gates.

## Live-scene settling and retirement — 2026-10-28

The reconstruction worker now records one live checkpoint after each acknowledged two-second
operating interval. After the final interval it keeps rendering the same scene for ten more
seconds, with both windows, sources and the independent observer still active. Sparse resource
rows identify generation and settling phase. The watchdog grows from 408 to 418 seconds for
100 rebuilds to accommodate that fixed settling period; no workload or budget is reduced.
The UI displays the backend's generation/settling phase, using regenerated Rust-owned DTOs.

The [NVIDIA Q02 release capture](evidence/mapgyver-m01-scene-retirement-2026-10-28.json) starts
after native verification of overlay Off and completes 100 rebuilds / 101 device scopes in
236.139 seconds, with unchanged input fingerprints, exit 0 and empty stderr. All 100 independent
generation markers and color/alpha oracles pass; every scope retains the fixed geometry and
drains tracked logical allocations after its final context fence. All 236 periodic process/
allocation rows are retained, including ten during the final 10.000-second live settling phase.
The archive explicitly omits timing/cadence/copy distributions and raw presentation statistics;
it is a retirement capture, not a new Q01/Q02 performance series.

Compare generation 10's warmed live checkpoint with all later live checkpoints and the final
settled scene, preserving the active ownership set:

| Scoped observation | Measured result |
| --- | --- |
| Renderer handles | Baseline 765; later range 758–768; settled 759 (−6) |
| Maximum positive renderer private-byte growth | 8.133 MiB; baseline 604,262,400 bytes, settled 346,341,376 bytes |
| Final scene logical allocations | Nine live, zero retiring; exact owner totals retained |
| Final GPU/worker/native-window teardown | 0.371 seconds |

These observations meet the unchanged handle/private-growth targets for this controlled
reconstruction subtest. They do not close full Q05: source/output/project replacement and
superseding plan/Matrix lifecycles belong to their later owning milestones. Logical counts
do not prove physical driver retirement or source/observer allocation totals. The teardown
timer covers final GPU retirement, fixture/observer reaping, event-loop return and worker join;
it excludes report serialization, final process exit and WebView subprocess termination.

Overlay Off is verified again after capture, then original On is restored and verified through
the green switch and explicit `Toggle switch On` announcement. The settings window is closed
and desktop-capture session reset. Installed capture DLL version/hash are unchanged; that is
a cohort observation, not an active-state query. Overlay-on failures remain preserved.

The [native settling controls](evidence/mapgyver-m01-settling-controls-2026-10-28.json) retain two
completed short runs, including attempts where observation arrived too late to issue close.
A separate close during an observed settling phase returns exit 1, an explicit cancellation
and no successful report. No close was sent in the two completed controls. Four explicit native
reconstruction subruns across both GPUs/Q01/Q02 pass. Fourteen deterministic Rust tests, two
Python tests, strict default/desktop Clippy and UI/generated checks pass. M01 remains open.
Root/Golden Core format checks, saved table/count integrity, restoration records and 66 local
documentation links pass. Post-capture source edits only clarify comments and format Python;
each artifact retains its actual run's fingerprints. No owned native probe remains.

## Independent-consumer source frame age — 2026-10-28

The disposable fixture now encodes frame ID, channel identity, QPC words and a checksum into
five image cells using its existing 32-byte constant upload. The independent consumer copies
ten isolated pixels per observation into a bounded GPU log, with four nonblocking event-query
slots. Completion QPC is observed after the matching GPU copy fence. A CPU enqueue stamp alone
could precede newer source data reaching the GPU; a deterministic test exercises that case.
The consumer maps the log once at finish. The renderer adds no pixel readback or uniform upload.
This measures fixture CPU stamp through observed consumer-copy completion, including observer
scheduling/query-poll delay. Native output/preview scanout and physical input latency remain open.

Raw integer QPC/marker records survive runner schema v5 compaction. Reports retain absent,
invalid, repeated/unique frames, skipped source IDs, no-new-frame/ring-full observations and
overdue sampling periods, alongside complete distributions and maxima. Source absence and
observer gaps do not silently become displayed-frame drops or zero latency. Twelve-record
chunks fit the existing 4096-byte line cap, including maximum-width integers. The eight-message
channel remains bounded; parent finish drains it while waiting for child exit, and error paths
disconnect it before joining the reader. Sampling preserves phase through ordinary wakeup
jitter and skips whole overdue periods instead of accumulating drift or catching up in bursts.

An initial reconstruction test fails exact summary verification. The deterministic reproduction
shows JSON parsing changing a 19-sample mean from `12.373600000000001` to `12.3736`. Enabling
the probe's `serde_json` `float_roundtrip` feature repairs the boundary; exact comparisons and
the raw-sample oracle remain unchanged. The initial 456-sample Intel Q01 control is historical
evidence for the earlier polling schedule, with no invalid markers and a 92.754 ms maximum;
it does not substitute for final-input or repeated release qualification.

See the [harness measurement contract](../../tools/mapgyver/native-probe/README.md#source-frame-age-in-the-independent-consumer)
for ownership, timing scope and primary API references. The
[retained frame-age evidence](evidence/mapgyver-m01-frame-age-2026-10-28.json) includes that
initial control, the precision failure and the complete passing final native suite.

Four final-input optimized-debug controls use 2-second warmup plus 8-second observation windows.
All share identical binary/source/assets, retain unchanged-input checks, exit 0 and empty stderr:

| Adapter / workload | Captures | Source A p95 / maximum | Source B p95 / maximum |
| --- | --- | --- | --- |
| Intel Q01 | 479 | 16.901 / 91.370 ms | 25.167 / 83.294 ms |
| Intel Q02 | 480 | 28.326 / 93.379 ms | 30.931 / 96.968 ms |
| NVIDIA Q01 | 480 | 26.968 / 91.577 ms | 18.402 / 82.680 ms |
| NVIDIA Q02 | 479 | 28.964 / 88.858 ms | 21.612 / 81.294 ms |

All 1,918 raw observations survive compaction. No invalid markers, no-new-frame observations
or full query rings occur. The two 479-sample controls each record one overdue sampling period;
known A absence and repeated/skipped source IDs remain separate counters. Each observer maps
one 20 KiB log at finish, in addition to its existing finite lifecycle diagnostics. Renderer
readback-pixel and uniform-upload counters stay zero. These controls establish instrumentation
and scoped observed ages, not a repeated release, native-display or physical-latency pass.

The final full suite passes 19 deterministic and five explicit native tests, including four
reconstruction subruns across both GPUs/Q01/Q02 and source pipe-loss cleanup. Three Python tests,
strict default/desktop Clippy and generated UI DTO drift checks pass. Overlay Off is verified
before and after native runs, original On restored and verified, settings closed and capture
reset. M01 remains open.

## Source and consumer resource ownership — 2026-10-28

The harness now exposes each source/consumer's existing logical ledger, sparse DXGI process
usage and operations. Four bounded event slots allow old leases to retire during steady runs;
sampling and polling stay on their background workers. Sources retain every resize/absence/
recovery phase in addition to periodic observations. Final snapshots distinguish live resources
before drop, logical retirement after drop and zero tracked ownership after a context fence.
The consumer's final staging texture now shares the descriptor/lease wrapper; the GPU log,
private receiver copy and finite diagnostic staging rows are accounted too.

Shared imported allocations remain attributed to their allocating source/sender. SDK receiver
private copies are distinct allocations. Descriptor payloads exclude alignment, compression,
driver internals, buffers, shaders and query objects; DXGI process usage remains separate.
Neither process DXGI figures nor independent logical peaks are a measured combined physical
or simultaneous total. External source fixtures and diagnostic consumers remain identified
separately for budget comparisons.

The [child-resource evidence](evidence/mapgyver-m01-child-resources-2026-10-28.json) retains an
initial Intel run and four final immutable-input debug controls at 2+8 seconds, with unchanged
inputs, exit 0 and empty stderr. The final controls preserve 160 child snapshots, all source
phase extents/bytes and all existing resource/frame-age evidence. Both GPUs record the same
independent logical peaks:

| Workload | Source A | Source B | Independent consumer |
| --- | --- | --- | --- |
| Q01 | 19.775 MiB | 7.031 MiB | 7.949 MiB |
| Q02 | 79.102 MiB | 7.031 MiB | 31.680 MiB |

All twelve child reports finish with zero tracked live/retiring allocations. Source uniforms
equal 32 bytes per actual source draw, with no source readback. Harness-issued copy counters
reconcile each finite lifecycle pixel and frame-age log copy; SDK-internal copies/conversions
remain unknown. The renderer adds no diagnostic copy or upload. Worker/native-window teardown
is 0.315–0.377 seconds in these controls, with the existing complete-process/WebView exclusions.
This is short native instrumentation/lifecycle evidence, not full Q01/Q02 or Q05 qualification.

`wire.rs` and `process/reported.rs` now own the shared bounded child codec/reader. Sources use
typed Ready/resource/Finished messages, replacing the one-line readiness-only pipe. Four
snapshots fit each 4096-byte message, with an eight-message channel and 3,620-observation cap.
Parents drain results while children exit; dropped owners disconnect their reader before
stopping/joining. Missing/reordered/oversized chunks, bad counts/times and retained post-fence
allocations cannot complete the stream. These changes stay in the disposable harness.

The final complete suite passes 24 deterministic and five native tests, including all four
reconstruction subruns and source control-pipe loss. Tests check source phase bytes, source
uploads, explicit consumer copy bytes and post-fence zero ownership. Four Python tests, strict
default/desktop Clippy and UI binding drift checks pass. The resource-log capacity and maximum
width message tests preserve the fixed bounds. Native overlay Off is verified before/after
runs, original On restored and verified, settings closed and desktop capture reset.

## Independent swapchain admission and trace preflight — 2026-10-28

The disposable host presenter now uses waitable flip-model swapchains with maximum frame
latency 1, verified on creation and in every sparse observation. Each output has its own
zero-timeout readiness poll on the render worker. An unavailable editor slot skips that editor
without waiting on the native output or Spout path. Busy/occluded Present keeps the local
reservation for a latest-frame retry. Resize preserves the waitable flag; the caller-owned
handle closes before the swapchain reference. The editor layout revision advances only on an
accepted editor Present. See the [harness contract](../../tools/mapgyver/native-probe/README.md)
for API references and counter scope. This bounds queued rendering; it does not measure DWM
queue depth, displayed frames, physical visibility or Spout consumption.

The [presentation-admission evidence](evidence/mapgyver-m01-present-admission-2026-10-28.json)
retains an initial Intel control and four final immutable-input Q01/Q02 controls across both
GPUs. Each final run uses an optimized-debug binary, 2-second warmup and 8-second sample window,
exits 0 with empty stderr and unchanged inputs. All 40 sparse resource rows retain both chains'
maximum-frame-latency result: 80 observations of `Ok(1)`, including after resize.

| Adapter / workload | Whole-generation polls / ready / not ready, per chain | Sampled accepted editor / native / Spout | Worker/window teardown |
| --- | --- | --- | --- |
| Intel Q01 | 590 / 588 / 2 | 476 / 476 / 477 | 0.308 s |
| Intel Q02 | 590 / 588 / 2 | 476 / 476 / 477 | 0.327 s |
| NVIDIA Q01 | 584 / 582 / 2 | 476 / 476 / 477 | 0.351 s |
| NVIDIA Q02 | 583 / 580 / 3 | 476 / 476 / 477 | 0.361 s |

Each sampled path has 477 attempts and one late iteration. Each swapchain records one
readiness-busy attempt (1/477, about 0.21%); actual Present calls record no busy/occluded return.
Whole-generation counts include warmup and remain separate from sample-window cadence. No
0.1% cadence-budget or repeated release pass is claimed. All twelve source/consumer reports
and all four renderer generations end with zero tracked allocations after their context fences.
Renderer diagnostic-copy/readback and uniform-upload counters remain zero. Existing shutdown
scope exclusions and unknown SDK-internal copies/physical residency remain unchanged.

The new `trace-capability` command directly tests public real-time ETW session-creation rights
without providers or files. It uses a fixed descriptive name, stops only a successfully created
owned handle, and leaves name collisions alone. Native StartTrace returns status **5**, correctly
formatted as `Access is denied. (0x80070005)`: no owned session was created. The token is neither
elevated nor a Performance Log Users member; WPR and xperf are present, while PresentMon is not
on PATH. No permissions or security settings were changed. Displayed-frame capture remains
unavailable under this token; raw multi-monitor DXGI statistics remain uninterpreted observations.

The complete suite passes 27 deterministic and five native tests, including independent
admission/retry/error invariants, both GPUs' fixed workloads and all four reconstruction subruns.
Four Python tests, strict default/desktop Clippy and generated UI binding checks pass. NVIDIA
overlay Off is verified before/after native runs, original On restored and verified, settings
closed and desktop capture reset. M01 remains in progress.

## Secondary-display native input and minimized editor — 2026-10-28

The [secondary-display evidence](evidence/mapgyver-m01-secondary-native-ui-2026-10-28.json)
records 32 native keyboard/mouse inputs plus one supported activation restore on the available
1920×1080 secondary display at 100% OS scale, beyond three setup actions. The owned editor moves
from the primary 125% monitor to desktop origin `(2560, 0)` and settles at matching host/browser
scale `1.000 / 1.000`. The computer-use skill's supported APIs operate only the probe windows;
display settings and NVIDIA overlay are unchanged. The renderer uses Intel's named adapter.

Observed actions cover outside-region drag/release for both handles, subsequent drags, dock
width changes, stage scroll, tab hide/show, minimize/restore, maximize/restore, menus, modal and
native select input. A dragged A handle is clipped off the left viewport, then recovered through
its partially visible portion after window restore. Settled menus and modal cover native pixels
and accept input. WebView zoom 100/125/150/200% produces matching host/browser scale telemetry;
these zoom changes do not qualify OS 150/200% scaling. Immediate move/resize/zoom captures
occasionally include compositor transitions or a preceding native position; later settled
captures support the visual observations, without a transition-latency claim.

The tab remains hidden for **65.458 seconds** between observations: the settled stage has no
stale native color, native output accepted-call count advances 5,126→5,996 and Spout sends
12,580→16,340. The editor is separately minimized with previews visible; native output remains
visible blue/yellow at the start and after **79.263 seconds**. Output/send counters advance after
restoration, but include time before/after that minimized interval. These are bounded visual
and submission-independence observations, not displayed cadence, frame freshness or a 60-fps pass.
Disconnected/busy editor cases remain open.

The interactive probe exits 0 through normal window closure after 608.936 seconds, with final
layout/rendered revision `63 / 63`, matching scale 1.0 and 0.486-second scoped worker/window
teardown. Both owned windows close and desktop capture is reset. The terminal result omits
2,452,710 bytes from the middle of the large pretty-printed JSON. The
[retained truncated capture](evidence/mapgyver-m01-secondary-terminal-truncation-2026-10-28.txt)
is invalid as a whole JSON report. Intact header, complete observer report (610 resource rows)
and footer decode separately; missing source/renderer-generation/resource sections are explicitly
unavailable. No full long-run resource or timing pass is inferred from its clean exit.

The existing qualification runner now accepts explicit `diagnostic --underlay --warmup 0
--samples 1`, captures native pipes before compacting every row and labels interactive output
separately. Invalid combinations cannot silently turn diagnostics into Q01/Q02 qualification.
Five Python tests pass. A separate eight-second native control exits 0 with empty native stderr,
unchanged inputs, eight retained renderer observations and complete child reports. It verifies
the repaired collection route; it neither repeats the gestures nor replaces missing long-run
rows. Compaction is lossless but does not guarantee a transport-size bound for arbitrary durations.
The original interactive fingerprints and later runner-control fingerprints remain distinct.

The available secondary gesture sequence is now observed. Physical-pixel alignment, quantitative
input/geometry latency, OS 150/200%, negative origins, full-card color/alpha and repeated release
qualification remain open. No production host/backend decision is made in this step.

## Repeated Intel release series — 2026-10-28

The [Intel Q01](evidence/mapgyver-m01-intel-q01-release-series-2026-10-28.json) and
[Intel Q02](evidence/mapgyver-m01-intel-q02-release-series-2026-10-28.json) artifacts each retain
three independent release processes with 30 seconds warmup and 120 seconds observation.
All six share identical executable/source/assets and unchanged-input checks, exit 0 with empty
native stderr and keep desktop capture inactive. No display or NVIDIA setting changes occur;
the NVIDIA prerequisite does not apply to these Intel runs. Full NVIDIA release series remain
open. These are repeated native measurements of the fixed workloads, not full gate acceptance.

The runner now supports a lossless gzip/base64 envelope with decoded length and SHA-256.
Several decoded reports exceed the terminal's one-MiB result limit; all survive compressed
collection. One Q02 packet has a known runner stderr progress line interleaved into stdout by
the terminal collector. Removing that exact record reconstructs an envelope that passes complete
gzip, length and hash verification; the artifact records the demultiplexing. Future collectors
should retain stdout/stderr separately. No native payload, sample or failure is discarded.

The read-only [analysis tool](../../tools/mapgyver/analyze-release.py) independently rederives
43,165 raw consumer observations' integer markers, checksums, clocks, channel counts and age
distributions. It validates all envelopes, immutable inputs/commands/adapter identities, fixed
geometry, query accounting, queue observations and post-fence ownership. Dynamic GPU budgets
remain observations. The [saved analysis](evidence/mapgyver-m01-intel-release-analysis-2026-10-28.json)
retains every sample's scoped targets, distributions and maxima with analyzer/artifact hashes.

| Intel sample | CPU p95 / p99 | GPU p95 / p99 | A / B consumer-age p95 | Accepted editor / native / Spout calls |
| --- | --- | --- | --- | --- |
| Q01 1 | 0.577 / 0.674 ms | 1.985 / 2.430 ms | 28.362 / 32.187 ms | 7,200 / 7,200 / 7,200 |
| Q01 2 | 0.568 / 0.697 ms | 1.468 / 2.579 ms | 12.687 / 20.293 ms | 7,200 / 7,200 / 7,200 |
| Q01 3 | 0.592 / 0.743 ms | 2.725 / 2.913 ms | 17.202 / 21.372 ms | 7,200 / 7,200 / 7,200 |
| Q02 1 | 0.604 / 0.784 ms | 5.643 / 6.351 ms | 23.171 / 26.842 ms | 7,131 / 7,132 / 7,155 |
| Q02 2 | 0.611 / 0.762 ms | 7.149 / 7.959 ms | 30.884 / 33.123 ms | 7,200 / 7,200 / 7,200 |
| Q02 3 | 0.596 / 0.725 ms | 7.307 / 7.909 ms | 30.180 / 33.338 ms | 7,200 / 7,200 / 7,200 |

All six meet the scoped CPU and observed consumer-age p95 targets. All recorded GPU percentiles
are within target, but Q02 sample 1 has **16 skipped queries**: 7,139 GPU samples against 7,155
CPU samples. Its complete GPU-budget result is unqualified; the other five have complete query
accounting and meet the scoped GPU target. The analyzer rejects a full GPU pass when queries
are missing even if the surviving percentiles look favorable. Q01 records
7,200 attempts/accepted calls per path and zero recorded late/busy/occluded calls in all samples.
Q02 sample 1 retains **7,155 attempts**, 24/23 readiness misses and seven recorded late calls
per path. Relative to 7,200 target calls, accepted shortfall is **69/68/45** (about
0.958/0.944/0.625%). No 0.1% cadence-budget pass is claimed. Q02 samples 2/3 have regular call
cadence, but do not replace the first sample's failure.

Q02 sample 1 also retains **149.751 ms CPU** and **141.689 ms GPU** maxima, with consumer-age
maxima **760.070 / 280.684 ms**. Passing percentiles do not remove those stalls. Source-ID
repetitions remain explicit: Q01 sample 1 has 591 per channel; Q02 has 27/25, 0/1,487 and
0/1,436 across its three samples. Independently sampled source IDs, accepted calls and displayed
frames remain distinct observations. No marker is invalid. The next action is bounded
per-operation timing/outlier correlation around SDK receive/send, presentation and resource
sampling. The audited SDK contains a named-texture-mutex path with a 67-ms wait; that is an
investigation candidate, not attribution of these stalls or proof of the active mutex branch.
SDK-internal copy/conversion counts and physical residency remain unknown where unobservable.

The artifacts retain 900 renderer resource rows, all child resource rows and both chains'
maximum-frame-latency observations (all 1). Exact output/preview geometry has zero sampled
mismatches; 4K physical scanout remains unqualified. Renderer logical peaks are 55.010 MiB
at Q01 and 179.595 MiB at Q02, below their descriptor-payload ceilings. All six renderer scopes
and eighteen source/consumer reports drain tracked allocations after fences. Renderer uniform
uploads/readback stay zero. Scoped worker/window teardown is 0.419–0.441 seconds; complete
process/WebView shutdown and physical retirement remain separate.

The release build, 27 deterministic Rust tests and ten Python tests pass. Python regressions
cover output larger than the terminal limit, exact high integer clocks, corruption/version
rejection, incomplete GPU query accounting and preservation of failed/incomplete samples.
Production Rust/UI sources are unchanged.
M01 stays in progress, with NVIDIA, Q02 stall investigation and the other native gates open.

## Bounded operation attribution and Q02 reproduction — 2026-10-28

The owned qualification harness now measures 16 CPU operation intervals, spanning resource
snapshots, query setup, layout, each input receive, composition, Spout send, observer control,
target/draw/Present for each window, status and query completion. Rust owns the registry and
report DTOs. Warmup and sampled windows each cap retained CPU events at 128, count omissions
and preserve a separate largest event. Per-operation distributions exclude warmup and absent
operations. Frame/QPC stamps correlate these events with the existing bounded GPU query ring,
whose long spans now retain raw ticks, frequency and completion stamps. The
[measurement contract](../../tools/mapgyver/native-probe/README.md) describes thresholds,
sampling skew and capacity. No new GPU queries/waits, renderer readback or uniform upload is added.

The [two short release controls](evidence/mapgyver-m01-operation-timing-controls-2026-10-28.json)
use Intel Q01/Q02 at 2+8 seconds, with unchanged inputs, exit 0 and empty stderr. Each has one
sampled event: frame work takes 68.975 / 68.462 ms, dominated by input A receive at
**67.940 / 68.082 ms**. The pinned SDK's `ReleaseReceiver` contains an explicit `Sleep(67)`;
that cleanup path is separate from its 67-ms named-mutex timeout. The controlled lifecycle
timings and source inspection identify a blocking cleanup candidate, without attributing every
steady-state stall to either branch. The attribution artifact below retains the SDK source hash.

The [new Q02 release series](evidence/mapgyver-m01-operation-timing-q02-release-2026-10-28.json)
repeats three independent 30+120-second runs on Intel. All five controls/series runs share
identical instrumentation, binary/source/assets and unchanged-input checks, exit 0 with empty
native stderr, keep exact geometry and drain tracked renderer/child ownership after fences.
Desktop capture is inactive; no display or NVIDIA setting changes occur. The old failing
series stays retained for its exact earlier inputs. The
[attribution analysis](evidence/mapgyver-m01-operation-timing-analysis-2026-10-28.json)
contains the complete capped events, operation distributions, raw-marker verification and hashes.

| New Q02 sample | CPU / GPU maximum | Sampled CPU / GPU outlier events | Skipped GPU queries | Accepted editor / native / Spout calls |
| --- | --- | --- | --- | --- |
| 1 | 150.230 / 140.561 ms | 18 / 6 | 12 | 7,132 / 7,132 / 7,152 |
| 2 | 1.181 / 16.387 ms | 0 / 0 | 0 | 7,199 / 7,200 / 7,200 |
| 3 | 2.081 / 11.242 ms | 0 / 0 | 0 | 7,200 / 7,200 / 7,200 |

Sample 1 reproduces the failure. The first unavailable GPU-query slots appear at session
seconds **136.376–136.443**, with sub-1.3 ms CPU frame work. Long call intervals follow from
136.460 through about 137.504 seconds. Input A/B receive maxima are **111.582 / 103.566 ms**,
Spout send reaches **52.063 ms**, and query completion reaches **106.093 ms**. The latter
includes end markers, context `Flush` and accounting. Resource sampling stays below 0.04 ms
and each Present below 0.5 ms in this sample. No CPU or GPU events are omitted. GPU-query
exhaustion precedes the long CPU intervals, narrowing the next investigation to queue progress
and SDK/flush boundaries; wall intervals alone cannot prove an internal driver or scheduling cause.

There are 7,152 attempts, 20 readiness misses on each swapchain and eleven recorded late calls
per path. Accepted-call shortfall is 68/68/48 against 7,200. Consumer-age maxima remain
**772.239 / 294.210 ms**. The twelve missing GPU samples leave its full GPU budget unqualified.
Sample 2 retains one editor readiness miss; later samples do not replace the failure. Output
scanout, physical latency, unknown SDK/driver copies and the other M01 gates stay open.

The release build, 30 deterministic Rust tests, twelve Python tests and strict default/desktop
Clippy pass. Tests cover separate warmup, absent operations, overflow/maximum retention,
query gaps/late wakes, exact high QPC values, corrupted spans and stale schema-8 binaries missing
operation data. The next implementation should
bound in-flight GPU work and isolate source lifecycle operations that can block, with controls
for explicit flush/instrumentation effects before attributing a hardware/driver root cause.
No Golden production runtime, app protocol or UI contract changes in this step.

## Independent two-slot GPU admission — 2026-10-28

The qualification renderer now admits at most two frame batches through two persistent event
queries, independently of its four profiling slots. The policy retires only the completed FIFO
prefix using non-flushing polls. With both slots pending, a tick records Busy on editor,
native output and Spout paths before any receive/draw/send work. Observer draining, stop checks
and pacing continue. This bounds renderer submissions; source contexts, SDK/driver queues and
scanout are outside that bound. Blocking SDK lifecycle calls still require separate isolation.

Runtime submission now owns the final context flush after both profiling and admission markers,
including frames without a profiling slot. SDK flushes are unchanged. Resource sampling skips
missed periods instead of issuing catch-up reads. Schema 9 preserves sparse pipeline counters,
final post-fence completion and the new admission operation. Scheduled attempts and coalesced
ticks stay separate from admitted CPU/GPU timing samples. Original 60-Hz budgets are unchanged.

The [short release controls](evidence/mapgyver-m01-gpu-admission-controls-2026-10-28.json) retain
Intel Q01/Q02 at 2+8 seconds, unchanged inputs, exit 0 and empty native stderr. Each peaks at
two batches, admits 477 sampled ticks and drains all batches after its final context fence.
Neither control coalesces GPU-busy ticks. Each retains one swapchain readiness miss per window
and a lifecycle stall: CPU maxima are 68.238 / 68.336 ms. These controls validate accounting and
cleanup in their measured scope; they do not establish sustained pacing or remove the earlier
Q02 failures. The bounded policy's deterministic test also holds two slots incomplete across
100 rejected attempts and verifies prefix-only reuse and final draining.

The [new Q02 series](evidence/mapgyver-m01-gpu-admission-q02-release-2026-10-28.json) retains
three independent release processes with 30-second warmup and 120-second sampling. All five
controls/series runs share unchanged identical inputs, exit 0 with empty native stderr, preserve
fixed geometry and drain tracked renderer/child ownership. No desktop capture, display changes
or NVIDIA setting changes occur. The
[saved analysis](evidence/mapgyver-m01-gpu-admission-analysis-2026-10-28.json) validates every
envelope, current release fingerprints, raw markers, query/pipeline/cadence accounting and hashes.

| Q02 sample | CPU p95 / p99 / maximum | GPU p95 / p99 / maximum | A / B consumer-age p95 | Accepted editor / native / Spout calls |
| --- | --- | --- | --- | --- |
| 1 | 0.637 / 0.791 / 1.864 ms | 4.661 / 5.544 / 8.802 ms | 19.903 / 20.165 ms | 7,200 / 7,200 / 7,200 |
| 2 | 0.588 / 0.705 / 1.386 ms | 7.382 / 8.274 / 10.928 ms | 30.027 / 20.409 ms | 7,200 / 7,200 / 7,200 |
| 3 | 0.600 / 0.731 / 3.260 ms | 7.795 / 8.481 / 11.086 ms | 29.756 / 33.472 ms | 7,200 / 7,200 / 7,200 |

All three meet scoped CPU/GPU and consumer-age targets with complete query accounting,
zero sampled CPU/GPU outlier events and zero late/busy/occluded cadence outcomes. Each peaks
at two batches and completes all 8,990 warmup-plus-sampled submissions. No native GPU-busy
coalescing occurs in this cohort; forced backlog behavior is covered by the deterministic
policy/accounting tests, not an observed native overload pass. Source-ID repetitions remain
explicit (A/B: 9/32, 0/0, 0/0), and are separate from displayed cadence.

The intermittent steady-state stall does not reproduce here. This is evidence for the exact
new inputs, not proof of a driver/SDK root cause or a cure: admission markers and flush ownership
changed together, old failures remain retained, and short controls still contain lifecycle
stalls. Isolate blocking source lifecycle work and explicit instrumentation effects next.
The five runs retain 470 renderer rows and 22,560 raw consumer observations, with zero invalid
markers and zero tracked post-fence ownership in five renderer scopes and fifteen child reports.
The release build, 33 deterministic Rust tests, thirteen Python tests and strict default/desktop
Clippy pass. M01 remains open; NVIDIA sustained series, scanout/physical latency and the other
native gates are unchanged.

## Isolated source ingest — 2026-10-28

The owned harness now moves each input SDK receiver, resize/absence/reconnect work and receiver
destruction onto its own thread, D3D11 device and immediate context on the selected adapter.
The renderer polls a latest-publication mailbox and zero-timeout keyed mutex, importing an
owned NT handle on its own device. It retains the previous image when no new bridge image is
ready. Source absence clears the publication; the receiver continues recovering the same
explicit sender name. Endpoint borrows and SDK instances never cross threads.

Each worker has at most three live bridge textures and an independent two-batch GPU admission
bound outside its four profiling slots. Referenced mailbox/reader slots cannot be reused; resize
replaces only an acquired free slot. Each publication adds one measured full-image GPU copy
on the source context. There is no new renderer copy/readback/upload. The renderer fences its
reads before releasing readers and joining workers, which then retire their own resources.
Schema 11 retains worker timing, bounded submission, sparse ownership and known copy costs.
Admission and profiling share one captured warmup classification and the absolute session end;
no new receive starts during teardown after that deadline.

The [measurement contract](../../tools/mapgyver/native-probe/README.md#source-lifecycle-isolation)
describes NT-handle/key ownership and HRESULT handling. Only exact S_OK grants ownership;
positive timeout/abandoned results must not pass through the binding's generic success check.
The explicit Intel boundary test holds all three reader slots through 100 reuse rejection
checks, then releases one and verifies changed pixel content after safe reuse. Both contexts
drain tracked ownership. This test's finite diagnostic pixels are outside renderer workloads.

The [final short release controls](evidence/mapgyver-m01-source-worker-controls-2026-10-28.json)
retain Intel Q01/Q02 at 2+8 seconds with unchanged identical inputs, exit 0 and empty native
stderr. Each accepts all 480 sampled calls per path. Source A retains **68.318 / 68.152 ms**
receive maxima on its worker, while renderer submission maxima are **1.333 / 2.272 ms**.
The render context makes zero SDK receive calls. All source/render batches and logical ledgers
drain after their fences. Worker pools peak at three/two textures for A/B; independent-context
payload peak sums are conservative **81.817 / 265.728 MiB** upper bounds, not simultaneous or
physical peaks. The separate
[debug plumbing control](evidence/mapgyver-m01-source-worker-debug-2026-10-28.json) ran alongside
Clippy and has earlier instrumentation. The
[initial release control](evidence/mapgyver-m01-source-worker-initial-release-2026-10-28.json)
precedes the shared classification/deadline repair. Both retain their actual older fingerprints;
they are not substituted for the final controls or sustained evidence.

Per-context GPU timestamp distributions now describe different scopes than the older renderer
that also executed input SDK copies. Their marginal percentiles cannot prove the asynchronous
pipeline's critical path. Analysis keeps that original full GPU-budget result unqualified while
reporting individual context coverage/targets. Consumer frame age still covers the fixture to
independent output observation. The new bridge's extra copies and GPU admission do not remove
unknown SDK/driver copies, prove physical scanout, or establish a hardware/driver root cause.

The [sustained Intel Q02 series](evidence/mapgyver-m01-source-worker-q02-release-2026-10-28.json)
adds three independent 30+120-second release runs. All five final controls/series runs share
identical unchanged inputs, exit 0 with empty stderr and preserve fixed geometry. The
[saved analysis](evidence/mapgyver-m01-source-worker-analysis-2026-10-28.json) validates raw
markers, every context's query/admission accounting, logical ownership, copies and fingerprints.

| Q02 sample | Renderer CPU p95 / p99 / maximum | Renderer GPU p95 / p99 / maximum | A / B consumer-age p95 | Accepted editor / native / Spout calls |
| --- | --- | --- | --- | --- |
| 1 | 0.932 / 1.200 / 4.546 ms | 4.347 / 5.000 / 8.371 ms | 41.483 / 37.319 ms | 7,200 / 7,200 / 7,200 |
| 2 | 1.014 / 1.269 / 3.483 ms | 11.527 / 12.063 / 16.862 ms | 28.876 / 33.423 ms | 7,200 / 7,200 / 7,200 |
| 3 | 1.064 / 1.295 / 5.753 ms | 4.340 / 5.348 / 10.377 ms | 34.334 / 33.602 ms | 7,200 / 7,200 / 7,200 |

All three sampled windows have zero late/busy/occluded calls and complete renderer/ingest GPU
query coverage. Scoped render CPU, individual-context GPU percentiles and consumer-age p95 meet
targets; **the full GPU critical-path budget remains unqualified**. Sample 2 retains one
16.862-ms renderer GPU event. Sample 3 retains one CPU event with a 4.876-ms query-end/flush
operation. No outliers are omitted. Source-ID repetitions remain explicit (A/B: 900/159, 5/26,
0/0); neither accepted calls nor repeated IDs establish displayed cadence.

Sample 3 also preserves a **warmup GPU-progress stall**: the renderer coalesces 57 ticks, while
source A/B admission counters retain 50/46 busy ticks. The renderer events span 24.141–25.191
seconds after the consumer frame-age clock origin, with at most 0.012 ms CPU work per coalesced
tick. They issue no source poll, draw, send or Present and remain bounded at two batches.
All 57 events survive the capped log with no omissions; source counters stay separately scoped
and do not provide individual busy-event timestamps. This is native backpressure evidence and
an unresolved GPU-progress observation, not a proof of an internal driver/scheduling cause.
The later steady-state success does not discard this warmup behavior or earlier failed cohorts.

The five runs retain 470 renderer rows, 924 ingest rows and 22,558 raw consumer observations.
All five renderer scopes, ten ingest reports and fifteen external child reports drain tracked
ownership. Conservative context-peak payload sums are 265.728–285.064 MiB in sustained Q02.
There is no native pool exhaustion in these runs; the explicit held-reader test covers that
bound separately. GPU admission pressure is actually observed during the warmup stall.

The same release inputs also complete a
[ten-rebuild control](evidence/mapgyver-m01-source-worker-recreation-2026-10-28.json): eleven
renderer scopes and 22 ingest workers retire, while persistent fixtures and the independent
consumer verify generations 1–10 through exact markers and final alpha samples. Observed
generation readiness is 157–191 ms, final live settling is 10.017 seconds, and scoped worker/
window teardown is 0.335 seconds. It is a reconstruction/ownership control, not full Q05,
physical retirement or complete-process/WebView shutdown. NVIDIA and full Q01 release series
must be collected for the new source path. M01 stays open.

## Source shutdown notification ordering — 2026-10-28

The first NVIDIA Q01 release cohort uses the isolated-source path with verified overlay Off.
Its first two 30+120-second samples retain 7,200 calls per path, complete timing coverage and
zero tracked post-fence ownership. Both retain 254 source-A warmup admission misses. The third
sample **fails during shutdown** with `source worker still has a bridge reader at shutdown`;
no final native report is available for that sample. The
[failed cohort](evidence/mapgyver-m01-source-worker-nvidia-q01-release-2026-10-28.json) preserves
exit 1 and the child pipe-close errors rather than substituting either passing sample.
The [earlier short controls](evidence/mapgyver-m01-source-worker-nvidia-controls-2026-10-28.json)
retain the same pre-repair inputs. They are not evidence for the repaired version.

Inspection finds a concrete ordering race: the renderer publishes the stop flag before dropping
its current bridge reader. A worker can immediately acquire that flag and check surface ownership
while the renderer still retains the reader. The repair drops the reader and clears the latest
publication before the Release-store stop notification. The worker's Acquire-load then observes
that prior ownership release. It still clears its own publication at exit, covering a final
publication racing with the stop request. No SDK thread/device ownership is moved.

The [regression evidence](evidence/mapgyver-m01-source-worker-stop-order-regression-2026-10-28.json)
records the held-publication test failing with the old order and passing after the repair.
The full deterministic suite now has 35 passing Rust tests; fourteen Python tests and strict
desktop Clippy remain clean. Native controls, reconstruction stress and repeated release runs
verify the repaired binary separately below. The warmup GPU-progress observations and full
asynchronous critical-path qualification remain separate from this shutdown fix.

The [repaired NVIDIA Q01](evidence/mapgyver-m01-source-worker-stop-order-nvidia-q01-release-2026-10-28.json)
and [Q02](evidence/mapgyver-m01-source-worker-stop-order-nvidia-q02-release-2026-10-28.json)
cohorts each retain three independent 30+120-second release samples. All six accept 7,200
calls per editor/native/Spout path, with zero sampled busy/late/occluded outcomes and complete
GPU timing coverage on the renderer and both source contexts. Each recorded context meets its
scoped percentile target; the full asynchronous GPU critical path remains unqualified.

| NVIDIA sample | Renderer CPU p95 / p99 | Renderer GPU p95 / p99 | A / B consumer-age p95 |
| --- | --- | --- | --- |
| Q01 1 | 0.768 / 0.874 ms | 1.309 / 1.626 ms | 23.940 / 24.561 ms |
| Q01 2 | 0.741 / 0.881 ms | 1.215 / 1.529 ms | 34.783 / 24.559 ms |
| Q01 3 | 0.786 / 0.887 ms | 2.594 / 4.647 ms | 20.255 / 24.850 ms |
| Q02 1 | 0.782 / 0.856 ms | 1.731 / 1.810 ms | 31.964 / 36.536 ms |
| Q02 2 | 0.759 / 0.839 ms | 1.736 / 1.849 ms | 30.701 / 31.829 ms |
| Q02 3 | 0.803 / 0.934 ms | 4.057 / 6.208 ms | 43.070 / 36.686 ms |

All six preserve **254–255 source-A admission misses during warmup**, with none on source B
or the renderer. The source counters have no individual busy-event timestamps, so these data
do not establish the responsible operation or driver behavior. Investigate lifecycle and
instrumentation/flush effects without treating sampled success as a resolution. Source-ID
repetitions, maxima and all scoped targets remain in the
[saved analysis](evidence/mapgyver-m01-source-worker-stop-order-nvidia-analysis-2026-10-28.json).

The [repaired 100-rebuild control](evidence/mapgyver-m01-source-worker-stop-order-nvidia-recreation-2026-10-28.json)
retires 101 renderer scopes and 202 ingest workers, verifies generations 1–100 and drains
all tracked ledgers. Against generation 10's warmed live baseline, handles range 1054–1067
from baseline 1062, within ten. **Maximum positive live private growth is 47.926 MiB**, exceeding
the unchanged 32-MiB target. The final 10.017-second live settling interval falls below the
private baseline; that later drop does not erase the intermediate failure. Post-release
observations separately reach +69.176 MiB and are not substituted for live checkpoints.
Scoped final worker/window teardown is 0.531 seconds. No physical-retirement/full-Q05 pass or
internal memory-retention cause is claimed.

Nine repaired release runs share identical current inputs: two short controls, the two
three-sample series and the reconstruction control. The eight performance runs retain 920
renderer rows, 1,812 ingest rows and 44,159 raw consumer observations. The reconstruction adds
240 renderer rows and 12,175 observations. Analysis checks every marker, envelope, context
query/admission count, post-fence ledger and input hash, and preserves the old failed cohort.
The [native conditions record](evidence/mapgyver-m01-source-worker-nvidia-conditions-2026-10-28.json)
verifies overlay Off before startup and again after all tests, then restoration of original On.
No desktop snapshots occur during measurement; displays and other settings are unchanged.
NVIDIA has no attached physical outputs on this machine, so these measurements do not qualify
physical scanout or unknown driver/desktop copies. Production/runtime/protocol code is unchanged.

## Remaining gate

| Criterion | Current state / next evidence |
| --- | --- |
| D3D11/Spout caller-device ownership, two sources, resize/restart | PASSED_NATIVE for the limited smoke above; production supervision and full Q01/Q02 remain open |
| Ordinary Tauri/Wry underlay, DOM layering/clipping/input | 31 observed native inputs on the 125% primary and 32 on the 100% secondary display, plus secondary activation restore; settled overlay/clipping/drag observations pass in that scope. Other configurations and quantitative latency remain open |
| Native rectangle output and composited Spout send | Exact Q01/Q02 client/preview extents and narrow color/alpha/lifecycle checks pass on both GPUs; visual/native output, 4K scanout and full reference-card checks remain open |
| Mixed DPI, negative origins, drag/resize/docking/zoom | 125%→100% monitor transition, primary/secondary sequences and WebView zoom checks observed; OS 150%/200%, negative origins and quantitative alignment remain open |
| Minimized/disconnected-editor independence | Hidden-tab and separate minimized-editor intervals exceed 60 seconds with independent native output visual/submission progress; freshness, busy/disconnected cases and displayed cadence remain open |
| Alpha/color cards, copies, frame age/cadence, queues and memory budgets | Repaired NVIDIA Q01/Q02 each complete three 30+120-second release samples with full sampled call/context-timing coverage and scoped CPU/consumer-age targets met. Source-A warmup admission pressure persists. Earlier Intel stalls remain retained for their exact inputs. Fixed procedural/authored cards pass on both adapters. New short Intel Q01/Q02 traces match native roles exactly; Q02 retains 85 editor/20 output drops per 480 Presents despite full native acceptance. Their cause and independent zero loss remain unverified. Full GPU critical path, current-variant Intel series, sustained displayed cadence/physical latency, physical residency, SDK-internal copies, lifecycle/preview cards and remaining configurations stay open |
| Device recreation, sustained retirement and shutdown budgets | Repaired NVIDIA control retires 101 renderer/202 ingest scopes across 100 rebuilds with ten-second live settling and 0.531-second scoped teardown. Handles stay within ten; private growth 47.926 MiB fails the 32-MiB target. Earlier smaller-stack passes retain their exact fingerprints. Full Q05, physical retirement, complete-process/WebView shutdown and genuine removal/reset remain open |
| Production host/backend/binding decision | OPEN: D3D11 plus the sys binding is exercised provisionally; ordinary underlay must pass or trigger DirectComposition investigation |

Continue the Q01/Q02 repeated series, sustained resource/latency measurements and
remaining native visual/input configurations in this harness. Do not start M02/M03 production
architecture around an unqualified presentation path. The M00 workload and budget contract is
unchanged; short smoke runs and single warmed samples do not become complete Q01/Q02 qualification.
