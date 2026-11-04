# M11 concurrent 4K capacity

Updated: 2026-11-04. **M11/2 Active: capacity is implemented and functionally verified;
NVIDIA retirement exceeds the retained two-second bound. No gate accepted.**

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

| Observation | Intel Graphics | NVIDIA RTX 5080 Laptop |
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

## Overload, recovery and exact remaining requirement

The software test admits eight 4K sources through product configuration, rejects a larger
replacement while the old generation is retained, preserves its charge and then recovers.
The integrated native test deliberately uses Golden's stricter **2 GiB workload ceiling** to
exercise real rejection without filling the user's GPU. Four healthy outputs remain valid when
the eight-source request fails (**1,207,959,552 B requested + 1,476,395,408 B retained**).
Returning to four succeeds; complete pixels pass before/after recovery, reservations peak below
2 GiB, and final ownership retires in **121.581 ms**. This lab ceiling is separate from the
production adapter-aware policy. NVIDIA's pressure case stays **unrun**, because its preceding
full-capacity retirement failed; it is still required after that correction.

The full native batch first exposed NVIDIA retirement above two seconds. One focused repeat
preserved the exact eight-output/two-Reference workload and sampled existing owner counters.
Closing returns in 0.007 ms and frontend drop in 0.018 ms; plan owners are first observed empty
at 2596.111 ms, and output owners/retirement complete at 2629.201 ms. All 18 plan objects are
released, and final plan/source/output inventory is empty. This is a **late completion**,
not an ownership leak or permission/tool limitation. The observations locate the cost before
or during GPU/plan release; they do not distinguish the final GPU drain from native plan-object
destruction. Do not label SDK endpoint teardown the root cause on this evidence.

The two native investigation runs are retained; no unchanged repeat or expanded profiler is
authorized by renaming this batch. Next deliverable: a justified Golden ownership/teardown
correction, then the original NVIDIA capacity/retirement and pressure/recovery acceptance.
The investigation retains the 02:45 CET batch cutoff and cumulative effort from 02:20 CET.
Flush is asynchronous and cannot replace the completion barrier; Microsoft's
[Flush](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-flush)
and [ClearState](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-clearstate)
contracts inform the implementation review. Existing submission already performs End/Flush.
Do not relax the two-second bound, release early, or add unrelated features to show progress.

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

**Counts stay 48/67 gates (72%), 6/12 phases (50%), M11 1/10 (10%).** M11/2 alone remains Active.
