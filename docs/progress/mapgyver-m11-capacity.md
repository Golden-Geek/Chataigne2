# M11 concurrent 4K capacity

Updated: 2026-11-04. **M11/2 Accepted.** Adapter-aware admission, eight native 4K producers,
Reference reuse, bounded ownership, overload/recovery and the original retirement bound pass
on both adapters. Golden's teardown correction reduces NVIDIA retirement from 2629 to 62 ms.

Golden owns DXGI budget observation, admission, native resources and completion barriers.
MapGyver's public `preview::config` owns product plan/picture/count bounds; the desktop host
queries the exact selected adapter before creating a renderer. Qualification consumes these
same public app/Golden APIs. Nothing moves into the engine tick or app shell startup.

The desktop now uses Golden's existing `admission_limit(u64::MAX)` rule: half the measured
process-local DXGI LOCAL budget. Missing/zero budgets fail explicitly and retry on the host
worker. Dedicated/shared-system capacity is never substituted for a missing budget. The same
admitted value governs logical plan footprint and whole-runtime resource reservations. There
is no fixed 512 MiB product ceiling. Renderer recreation re-queries after complete retirement
of the old owner; this is a startup admission snapshot, not dynamic per-frame OS-budget control.
Sixteen bounded Screen targets permit eight independent sources plus Reference consumers and
the additional host-preview binding. Pass/dimension/input/retirement-count limits remain intact.

## Reproduction and corrected product behavior

The previously checked native app renders one and two 4096×4096 Solid Color channels, then
rejects the third: **225,326,600 B requested**, **438,653,592 B reserved**, **536,870,912 B limit**.
This reproduces the reported fixed-cap failure class; the original feedback's exact charges
describe its different viewport/graph. The corrected actual app renders all three with no
unavailable/message overlay and no runtime error. Its measured Intel admission limit is
**9,216,951,214 B**, with **505,762,456 B** steady tracked charge. No budget is disabled.

The native fixture creates eight distinct authored 4K Solid Color media, two real automatic
Reference media and ten independently received Spout outputs. It uses production ChannelRenderer,
public product configuration, explicit hardware LUIDs and real independent SDK readbacks.
These are direct channel-output routes; this does not claim a new physical Screen/DPI matrix.
Full 4096×4096 RGBA pixels are checked before/after live authored color replacement. Both
References reuse producer targets; output conversion/SDK buffers remain real separate consumers.

The initial admission checkpoint recorded the following results before the teardown correction;
its failing NVIDIA result is retained, not rewritten as a pass.

| Initial observation | Intel Graphics | NVIDIA RTX 5080 Laptop |
| --- | --- | --- |
| Measured admission ceiling | 9,216,951,214 B | 7,987,527,680 B |
| Eight independent producers / Reference outputs | 8 / 2 | 8 / 2 |
| Complete 4K output images verified | 20 plus 8 pressure/recovery images | 20 |
| Ten-second steady submissions | 172 | 600 |
| Steady tracked reservation | 2,147,484,048 B | 2,147,484,048 B |
| Replacement peak tracked reservation | 3,355,443,600 B | 3,355,443,600 B |
| Actual plan texture objects / texel payload | 18 / 1,207,959,552 B | Same |
| Actual SDK output texture payload | 671,088,640 B | Same |
| Steady total known owned texel payload | 1,879,048,192 B | Same |
| Sampled DXGI LOCAL process usage | 1,897,287,680 B, stable | 1,887,375,360 B, stable |
| Native allocations on replacement | No new textures; 18 reused | Same |
| Complete ownership retirement | 261.677 ms, passes | **2629.201 ms, fails ≤2000 ms** |

The integrated GPU safely exercises the full requested eight-output workload under its measured
budget. Its 17.2 submitted fps is recorded honestly; this is capacity evidence, not acceptance of
any retained performance gate. NVIDIA produces 60 submitted fps in its recorded steady window.
Neither figure is display-present/scanout FPS. Actual plan-owner counts and texel payloads remain
constant across ten one-second samples on each adapter; process/DXGI observations are retained.
This short series does not close M03's original private-growth or full timing qualification.

The accounting is explicit: 8 producer targets + 10 conversion targets = 18×64 MiB = 1152 MiB;
10 SDK outputs add 640 MiB. Runtime baseline reserves the 256 MiB input pool even with no active
external inputs, the 640 MiB SDK bound and 400 B constants. One full generation adds 1152 MiB;
two overlapping full reservations produce 3200 MiB + 400 B peak. Native storage is reused across
those generations; reservations are conservative upper bounds, not claims of duplicate physical
VRAM. Driver allocation size/residency remains unknown. The earlier
[native-owner inventory](evidence/mapgyver-m03-native-allocations-2026-11-01.json) establishes
the existing measurement/release mechanism without accepting its open M03 gate.

## Overload, recovery and retained failure evidence

The software test admits eight 4K sources through product configuration, rejects a larger
replacement while the old generation is retained, preserves its charge and then recovers.
The integrated native test deliberately uses Golden's stricter **2 GiB workload ceiling** to
exercise real rejection without filling the user's GPU. Four healthy outputs remain valid when
the eight-source request fails (**1,207,959,552 B requested + 1,476,395,408 B retained**).
Returning to four succeeds; complete pixels pass before/after recovery, reservations peak below
2 GiB, and final ownership retires in **121.581 ms**. This lab ceiling is separate from the
production adapter-aware policy. NVIDIA's pressure case was unrun at this checkpoint because
its preceding full-capacity retirement failed. The corrected case below now passes it.

The full native batch first exposed NVIDIA retirement above two seconds. One focused repeat
preserved the exact eight-output/two-Reference workload and sampled existing owner counters.
Closing returns in 0.007 ms and frontend drop in 0.018 ms; plan owners are first observed empty
at 2596.111 ms, and output owners/retirement complete at 2629.201 ms. All 18 plan objects are
released, and final plan/source/output inventory is empty. This is a **late completion**,
not an ownership leak or permission/tool limitation. The observations locate the cost before
or during GPU/plan release; they do not distinguish the final GPU drain from native plan-object
destruction. Do not label SDK endpoint teardown the root cause on this evidence.

The two native investigation runs are retained. The subsequent implementation validation uses
the original workload/assertions after the specific correction below; no unchanged probe,
new profiler or budget change was added. Cumulative retirement effort starts at 02:20 CET.
The capacity milestone was delivered at 02:37; correction began at 02:40. The original 02:45
cutoff arrived during verification, which was finished without adding investigation/scope.
Flush is asynchronous and cannot replace the completion barrier; Microsoft's
[Flush](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-flush)
and [ClearState](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-clearstate)
contracts inform the implementation review. Existing submission already performs End/Flush.
The two-second bound and complete ownership retirement remain mandatory.

## M11/2 acceptance — quiescence releases context bindings

Golden's native backend now unbinds the immediate context's views once when quiescing inputs
and outputs. This occurs before the final GPU drain and bulk plan destruction, rather than
leaving indirect context references until backend destruction. A worker-local flag makes this
idempotent across normal shutdown and the Core drop safeguard. No further frame submissions
follow quiescence. Core retains every resource through its existing matching GPU completion
or confirmed device loss; ClearState/Flush never acts as a completion certificate. Device reset
constructs a fresh unquiesced native owner. No engine/UI polling or new diagnostic API is added.

The exact original cases pass on the corrected implementation:

| Current acceptance | Intel Graphics | NVIDIA RTX 5080 Laptop |
| --- | --- | --- |
| Eight producers + two Reference outputs; complete before/after images | 20 full 4K images pass | 20 full 4K images pass |
| Ten-second steady submissions | 189 | 600 |
| Native storage/reuse and tracked overlap | Original counts/bounds pass | Original counts/bounds pass |
| Capacity retirement, ≤2000 ms | **260.242 ms** | **61.721 ms** |
| Plan owners first observed empty | 102.931 ms | 41.344 ms |
| Native 2 GiB pressure/recovery; healthy full images | 8 pass | 8 pass |
| Pressure/recovery retirement, ≤2000 ms | **93.013 ms** | **49.990 ms** |
| Final owner inventory | All plan/source/output owners empty | All plan/source/output owners empty |

Both original scripts keep their 180-second watchdog, eight-second per-operation deadlines,
two-second retirement assertions and 192 MiB receiving/staging/CPU-readback cap. Across the
corrected adapter runs, **56 complete 4096×4096 images** pass. Steady native payload and overlap
remain 1792 MiB and 3200 MiB + 400 B respectively; replacement still reuses all 18 plan textures.
Both native overload cases reject before exceeding the declared lab ceiling, preserve healthy
output pixels, recover after returning to the admitted demand and fully retire every owner.

Golden's **188 software tests pass** (24 explicit hardware tests ignored), followed by terminal
app-fixture compilation and strict all-target Golden/MapGyver Clippy. The 399 app software tests
remain applicable to unchanged app/null-backend behavior. The app executable relinked during
fixture preflight then passes the actual desktop three-4K-channel smoke and normal shutdown.
Both Cargo formatters and existing unchanged UI checks pass. No release/deployment is claimed.

[Acceptance receipt](evidence/mapgyver-m11-capacity-acceptance-2026-11-04.json) pins corrected
binaries and both full adapter artifacts. The earlier receipt remains the admission milestone's
partial/failing evidence. **M11/2 is Accepted; M11 is 2/10 (20%), accepted gates are 49/67 (73%),
fully accepted phases remain 6/12 (50%).** Retained M03 timing/private-growth and other original
qualification gates stay open; this workload does not waive or close them. Next: M11/3 GPU choice.

## Validation and evidence limits

**399 software tests pass**, 2 ignored and the previously verified native pre-resume test
intentionally filtered. Strict all-target native/process-feature Clippy and the fresh native
app build pass. Five reader/30 preview-session tests and Svelte 0/0 remain applicable to unchanged
UI code. Both Cargo formatters and scoped fixture formatting/syntax checks pass.
Initial compile failures were stale moved-config test imports and incorrect probe API/payload
usage; these were corrected before any dependent hardware check. The probe now shares a finite
three-item/192 MiB budget across receiving, staging and CPU pixels. The watchdog distinguishes
real timeout from assertion unwind. Failed native timing assertions remain failures.
The final app smoke retains explicit manager-tab activation; removing that input caused a
fixture-only selection timeout. Restoring it passes all three 4K stages again.

[The receipt](evidence/mapgyver-m11-capacity-2026-11-04.json) pins product/fixture binaries,
artifacts and limits. The first Intel artifact predates fixture-only retirement instrumentation;
its binary hash was not recorded before rebuilding that fixture. Its raw artifact hash and
unchanged product code are retained; it is not claimed as a run of the newer fixture binary.
Native app builds include concurrent unstaged label/theme edits, excluded from this milestone.

The admission checkpoint counted 48/67 gates; the acceptance above adds exactly M11/2 with no
denominator change. Current counts are **49/67 gates (73%), 6/12 phases (50%), M11 2/10 (20%)**.
