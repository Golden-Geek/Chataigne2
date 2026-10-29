# Production renderer qualification runner

The app-owned runner in `apps/mapgyver/tests/native_qualification` wires the fixed Q01/Q02
scene through Golden's desktop host and production `VideoRuntime`. It owns processes and
scenario policy. Golden Video owns optional native timing, OS observations and the generic
GPU card/marker fixtures. The [qualification contract](mapgyver-qualification-contract.md)
remains the authority for workloads and budgets; implementing this runner closes no gate.

## Capture boundaries

One renderer process launches two independent source processes and one independent consumer
using unique Spout names on one explicitly selected hardware adapter. Source A and both main
outputs are 1920×1080 for Q01 or 3840×2160 for Q02; source B remains 1280×720. The production
renderer composites the sources side by side into a native window and matching Spout sender.
Its editor has held 640×360 and 480×270 physical registrations. The host converts these sizes
to CSS units using authenticated native metrics and verifies the resolved geometry. This is
a registration check, not physical display, scanout or DPI/input qualification.

The host retains each HWND through actual runtime retirement. Sources and the consumer keep
native resources through actual completion or confirmed removal. A parent watchdog may fail
and terminate its own stuck process tree; forced termination never counts as orderly retirement.
Existing nonempty capture directories are rejected to preserve evidence.

The fixtures draw deterministic color/alpha/checker/grid cards with visible source/frame IDs
on the GPU. An opaque marker carries the full source ID, frame number, producer QPC and XOR
checksum. The consumer appends both decoded markers to a bounded GPU log and performs one
final readback after measurement. There is no recurring CPU pixel transport. Offline analysis
must also match decoded stamps to the independent producer records; a checksum alone does
not establish source identity or freshness. This marker is not a full-image color oracle.

## Observations and limits

`D3d11Config::diagnostics` defaults to disabled. Enabling it preallocates a bounded concurrent
metadata queue and two timestamp/disjoint query slots per context. The reporting consumer holds
no metadata mutex against source/render producers. Full-queue drops, clock failures,
invalid/disjoint/skipped queries and uncollected rows remain explicit. Queue storage never grows
under pressure, and final accounting is checked after producers retire. Draining and JSON/OS recording happen on an outer
host worker, not the engine or UI thread. Timing changes no rendering admission policy.

Opt-in native scheduling captures its own raw QPC origin/frequency, then computes monotonic
coordinates and exact inverse slot boundaries with checked integer floor/ceil rounding. It
does not guess an offset between independent `Instant` and QPC origins. Disabled diagnostics
keep the ordinary SystemClock. Initialization failure also retains SystemClock and records an
unavailable mapping; an arbitrary replacement clock invalidates the old mapping. Show-time
discontinuity within the same monotonic authority preserves that origin.

Each consumed cadence opportunity records Submitted/Busy/NoDemand/Rejected, skipped ordinals
and an exact device/token where submitted. A final coverage fence prevents the analyzer from
extrapolating a stopped trace past observation. Software API deadlines remain separate from
GPU/display completion: API begin after a deadline can establish a definite submission-miss
lower bound, while API end alone can include waiting or preemption. Early API acceptance cannot
pass the output deadline or scanout criterion. The delivery analysis below adds exact window
software-display points and Spout ready-time upper bounds, with explicit unknown outcomes.

CPU rows contain inclusive submission wall intervals, exact SDK/Present API brackets and
optional OS-accounted thread CPU deltas. API wall time can include CPU work and waiting;
subtracting it does not measure active CPU submission. Thread accounting also has OS-defined
granularity. Context-local GPU timestamp spans are reported separately. They do not establish
the full multi-context critical path, and their percentiles must not be added together.

Retained source metadata identifies the source-worker publication actually copied into each
renderer texture, including reuse. Versioned Spout metadata joins each accepted output to its
exact renderer token/source-use images and independently decoded GPU words. Every accepted
image belongs to the cohort; repeated captures cannot replace missing outputs. CPU producer
stamp to completed independent capture is distinct from display age or scanout. Incomplete
cohorts do not pass normative frame-age coverage. GPU dependency-path and native-window
presentation qualification remain unavailable.

Actual CPU plan lifecycle records come from the production runtime: admitted latest
publication, first worker receipt, atomic installation and terminal supersession/rejection/
cancellation. The bounded recorder is allocated before worker startup, independently of native
resources. Installation records cover recreation of the same authored plan on a new device;
first receipt is not duplicated on a preparation retry. Exact plan/resource joins measure CPU
handoff, first-use latency and retained plan age at accepted output APIs. They never substitute
for GPU queue time or physical presentation. The current analyzer rejects older schemas; their
original packets/analyses remain historical evidence.

Schema 6 adds each actual window Present's endpoint incarnation and IDXGISwapChain/1/2 aliases,
captured once during cold creation. Optional `--presentmon` uses the pinned portable PresentMon
2.6.0 with the new renderer's exact PID, an owned UUID session and a bounded collection tail.
The retained native probe supplies only its public read-only `trace-status` command; no legacy
renderer, host or private source module is imported. Its historical session namespace is an
observation-tool contract, not M01 acceptance work. Both external executable hashes are recorded.
No privilege elevation, installed service or display/driver/overlay configuration is changed.

The pinned collector's [v1 CSV implementation](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/PresentMon/CsvOutput.cpp)
writes QPCTime from PresentStartTime. The [console contract](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/README-ConsoleApplication.md)
defines the display-delay observation. Offline correlation requires that exact PID/address/QPC
fall within one actual native API call; nearest-time guesses and ambiguous aliases are rejected.
Rational display coordinates retain exact decimal numerator/denominator strings before deriving
millisecond distributions. Every accepted native frame stays in its cohort, including missing,
dropped and unavailable display/source/plan joins. The trace retains raw CSV/stderr and the live
loss query separately; the latter does not establish final collector-shutdown loss statistics.
Pending software display events do not measure native buffer occupancy or a GPU critical path.

Fresh process-local DXGI LOCAL/NON_LOCAL observations retain failures and zero budgets.
Admission uses the smaller of the workload ceiling and half the measured nonzero LOCAL budget;
shared-system capacity is never substituted. Logical texture charges, DXGI usage, private commit,
working set and handle count remain separate. Fixture processes are reported separately from
the renderer. WebView child process inventory and complete driver/physical presentation evidence
remain separate qualification requirements.

## Reproduction

Build the locked release x64 runner from the repository root:

```text
cargo build --locked --release -p MapGyver --features native-qualification --bin native_qualification
```

The qualification-only feature enables the pinned Tauri `unstable` public native-window
builder to create a separate window without another WebView. Ordinary MapGyver builds do not
enable that feature. The host/bootstrap is shared with production and is not copied into the app.

A short diagnostic control, with a new empty capture directory:

```text
target/release/native_qualification.exe --role renderer --scenario Q01 --adapter 0 --directory target/qualification-q01-control --warmup 1 --seconds 3 --independence 1
```

Q02 selects the larger fixed scene. `--independence 60` adds separate 60-second minimized and
disconnected editor intervals. These phases retain both main output demands. Short controls,
debug builds and incomplete/lost observations cannot qualify Q01/Q02. The contract still
requires 30-second warmup and three independent 120-second release samples per scenario.
The runner refuses external `GC_UI_BIND`/`GC_UI_FRONTEND_URL` overrides.

Each capture retains `manifest.json`, renderer/source/consumer JSONL, renderer and parent
summaries, and each child's stdout/stderr. Exact phase QPC coordinates, adapter LUID, release
mode, declared charges, observed resource counters, telemetry health and actual cleanup are
recorded. Consumer pixels are read only after the measurement window. Preserve failed packets
alongside successful controls; a source or consumer API acceptance is not delivered-frame proof.

The Python wrapper owns a Windows job for the newly launched renderer tree and fingerprints the
executable, selected execution sources, assets and lockfiles before/after capture. Job assignment
is explicitly non-atomic. It never terminates by executable name. Run a short owned control with:

```text
python -B tools/mapgyver/production_qualification.py run --directory target/qualification-q01-series-control --scenario Q01 --adapter 0 --diagnostic --samples 1 --warmup 1 --seconds 3 --isolation-seconds 1
```

Use `analyze <capture-directory> --diagnostic` for offline analysis. Three independent release
samples use the default 30/120-second durations and `--samples 3`; renderer criteria retain their
M03/gate-9 owner. Physical DPI/input and full-app/Matrix Q05 criteria retain M05/M09 owners.
Renderer handle/growth/drain requirements remain M03 and are not waived by that distinction.
Unknown criteria are explicit, and short samples cannot become qualification by relabeling them.
Current captures use artifact schema 7, generated from the Rust qualification contract. The
schema-1 through schema-6 archives below retain their own analyses and hashes; the current analyzer rejects
older versions rather than inventing resource-generation or copied-image identities.

## Diagnostic checkpoint — 2026-10-29

[The evidence packet](evidence/mapgyver-m03-qualification-runner-2026-10-29.json) summarizes five
captures; its SHA-256 identifies the [complete raw archive](evidence/mapgyver-m03-qualification-controls-2026-10-29.zip).
Extract the archive and analyze each `sample-001` directory offline. The initial direct capture
has no launcher/input fingerprints and fails before warmup; its limitation is preserved.

The corrected Intel release controls use 1 s warmup, 3 s sample and two 1 s interruption phases:

| Observation | Q01 | Q02 |
| --- | --- | --- |
| Actual output client/backing | 1920×1080 | 3840×2160 |
| Main window accepted / expected attempts | 180 / 180 | 75 / 180 |
| Spout accepted / expected attempts | 180 / 180 | 139 / 180 |
| Renderer-context GPU p95 / p99 | 12.051 / 14.296 ms | 25.268 / 30.077 ms |
| Renderer telemetry contention drops | 286 | 17 |
| Actual runtime retirement | 0.101 s | 0.139 s |
| Parent/fixtures forced termination | None | None |

Both corrected controls complete the functional scenario and preview geometry, with canonical
producer/consumer marker joins and one final readback. Neither qualifies: durations are short,
lost rows invalidate complete coverage, Q02 cadence and GPU observations miss targets, and the
remaining normative measurements are unavailable. The GPU values above describe one context;
they are not a summed/calibrated end-to-end critical path. M03 remains 7/9.

Earlier packets retain three harness failures and their corrections: transient `about:blank`
navigation, DPI-driven client resizing after a monitor move, and checking queued window destruction
before the OS processed it. The corrected owner settles actual size before runtime construction
and waits for actual HWND disappearance after retirement. No display or overlay configuration
was modified. Computer-use screenshot approval timed out, so no visual pass is inferred.

Validation passes 164 regular Rust tests, nine explicit native GPU tests, 185 Python tests,
22 UI unit tests, Svelte check/lint and actual bundled browser lifecycle/recovery checks. Strict
native all-target Clippy, locked release x64 build and both required Rust formatters pass.
This checkpoint's telemetry-loss and software-deadline work is covered below. Complete
GPU-path/age coverage, renderer resource-return series and native
device removal/recovery remain open; physical and full-app qualification keep their M05/M09 owners.

## Measurement integrity and equal-extent conversion — 2026-10-29

The [schema-2 packet](evidence/mapgyver-m03-measurement-integrity-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-measurement-controls-2026-10-29.zip) retain the new short
release controls. The reference workload, adapter and 1/3/1-second diagnostic durations remain
unchanged. No performance gate is awarded from these controls.

| Observation | Q01 | Q02 |
| --- | --- | --- |
| Full/clock loss in each of four process traces | 0 / 0 | 0 / 0 |
| Exactly mapped sample slots / missing slots | 180 / 0 | 180 / 0 |
| Main window / Spout API acceptance | 177 / 180; 180 / 180 | 172 / 180; 180 / 180 |
| Renderer valid GPU spans / source-use joins | 180 / 360 | 180 / 360 |
| Renderer-context GPU p95 / p99 | 16.906 / 26.750 ms | 19.640 / 21.969 ms |
| Captured source-age p95, A / B | 38.161 / 37.433 ms | 298.980 / 215.542 ms |

The bounded queue removes the old mutex-contention loss; written/drained/raw counts agree in
all four processes, with zero final queued rows. Both samples have one stable exact clock
mapping and a conservative fence after sample end. Every expected opportunity and output row
joins; Busy outputs remain visible. Independent raw recomputation agrees with the analyzer.

Equal-size output conversions now use the existing single-texel-load path, retaining explicit
color/alpha encoding. Scaled conversions stay linear. The earlier Q02 control accepted 75 main
window and 139 Spout submissions; this control accepts 172 and 180. These short, unpinned
observations are not a controlled causal comparison or sustained qualification. Q01 also has
window misses and GPU-time outliers; no favorable average replaces either distribution.

Q02 has only 59 independent-consumer capture observations per channel and 25/43 unique source
frames; its high captured age remains visible. Q01 has 1,188 observations but 173/172 unique
frames. Repeats and sparse captures cannot establish a complete output-frame age cohort.
API acceptance and mapped software opportunities do not prove displayed cadence. Full GPU
critical path, normative deadline/age, plan/queue age, growth/drain and genuine device removal
remain open. M03 stays **7/9**, overall **3/10 (30%)**.

Validation: 204 regular Rust tests, nine explicit native GPU regressions and 198 Python tests
pass. Strict default/native all-target Clippy, Chataigne all-target compilation, locked release
x64 build and both required Rust formatters pass. The native pixel oracles retain color/alpha
and filtering coverage. No display, driver or overlay setting changed; all owned processes exit
without force and HWNDs remain alive through actual retirement.

## Output admission before conversion — 2026-10-29

Golden Video now checks a native output's availability and zero-timeout frame-latency signal
before submitting its conversion draw. An exclusive admitted-output borrow spans conversion
through delivery, preventing endpoint replacement or another readiness wait between them.
Busy, hidden and recovering outputs skip conversion and backbuffer copying; shared render
passes and healthy outputs continue. A late `DXGI_ERROR_WAS_STILL_DRAWING` still retains the
acquired permit, and completion tracking still covers the copy preceding that Present attempt.
Swapchain buffering, maximum latency, source polling, workload rates and budgets are unchanged.

Schema 2 already represents skipped work: rejected admission has `conversion = null` and
`consumer_api = null`, with `delivery` bracketing the admission check. Admitted conversion and
post-conversion delivery retain separate intervals. Successful preflight time is included in
`FrameCpu.wall` but has no separate attribution; these intervals must not be treated as a complete
breakdown of submission wall time. API acceptance remains distinct from displayed delivery.

An owned auto-reset kernel-event test verifies busy/hidden rejection, readiness retention after
a late Present Busy, resumption and exactly one permit consumption after acceptance. It does
not issue GPU work; native trace checks separately verify conversion is skipped. Independent
source review found no material issues in admission, retirement or trace semantics.

Source freshness is a separate next checkpoint. The locked SDK can retain an earlier new-frame
flag after failed access and can copy repeated contents when counting is unavailable. A correct
fix needs exact per-call copied status and explicit counter availability at the public binding,
without registry writes or equality-gating unknown foreign senders. This checkpoint does not
change the SDK, source polling or source publication semantics. M03 remains **7/9**, overall
**3/10 (30%)**; recovery and qualification gates remain open.

The [evidence packet](evidence/mapgyver-m03-output-admission-2026-10-29.json) identifies the
[complete raw archive](evidence/mapgyver-m03-output-admission-controls-2026-10-29.zip), including
both analyses and pre/post input fingerprints. The same Intel adapter 0, fixed scenes and
1/3/1-second diagnostic durations are retained; no display, driver or overlay setting changed.

| Observation | Q01 | Q02 |
| --- | --- | --- |
| Sample main / Spout / editor API acceptance | 180 / 180 / 180 | 180 / 180 / 119 |
| Sample opportunities per output | 180 | 180 |
| Busy/unavailable conversions skipped, whole capture | 68 | 223 |
| Minimized editor rows without conversion/API | 60 / 60 | 47 / 47 |
| Renderer-context GPU p95 / p99 | 7.533 / 10.285 ms | 18.470 / 21.025 ms |
| Captured source-age p95, A / B | 35.679 / 34.025 ms | 216.710 / 146.563 ms |
| Consumer observations per channel / unique A / unique B | 771 / 177 / 175 | 66 / 38 / 56 |
| Actual runtime retirement | 0.106 s | 0.126 s |

Independent raw recomputation confirms every busy/unavailable row has no conversion or API
interval. All 1,093/878 accepted rows retain ordered, non-overlapping conversion and delivery
brackets. Both controls have zero telemetry loss across all four roles, 180 valid sample GPU
spans, complete source-use joins, stable input fingerprints and clean unforced owned cleanup.
No late API Busy occurred in these controls; that permit-retention branch is exercised by the
kernel-event regression, not newly established by native captures.

Editor independence has **not** passed. During minimization, main/Spout acceptance is 57/60 of
60 expected opportunities in Q01 and 27/47 of 60 in Q02; Q02's disconnected phase accepts
30/56 of 60. Both main outputs advance, but missed opportunities, busy windows and Q02 age
outliers remain. The one-second interruptions also fall short of the prescribed 60 seconds.
These unpinned short runs establish the skipped-work mechanism, not a causal timing improvement
or qualification. Full GPU-path/deadline/age, plan/queue, growth/drain and genuine device-removal
acceptance remain open. No additional performance or recovery gate closes.

Validation: 172 regular Rust tests, nine explicit native GPU regressions and 31 production
analyzer/deadline tests pass. Strict default/native all-target Clippy, locked release x64 build,
both required Rust formatters and whitespace checks pass. The native finite pixel tests retain
color/alpha/filtering coverage; app source/history/reconnect and endpoint-recovery regressions pass.

## Source copy and freshness boundary — 2026-10-29

The pinned public Spout binding now distinguishes copied, unchanged, busy, updated, disconnected
and failed calls. A retained legacy `IsFrameNew` flag no longer authorizes publication after an
access timeout. Golden publishes only an actual successful copy, preserving the previous valid
publication when access is busy and withdrawing it when the endpoint/session changes.

The [local SDK patch](../../vendor/spout2-sys-0.1.1/GOLDEN_PATCH.md) enables counting per sender
instance, without writing the global frame-count registry setting. Known controlled peers opt
into `SpoutFrameCount::VersionedV1`; ordinary app-configured external sources remain explicitly
unversioned. Counter existence or an old positive value cannot establish that a foreign sender
versions its images. Unversioned successful copies may repeat producer contents and carry no
known producer version. Runtime publication sequence and producer counter remain separate.

A CPU preflight checks the current named sender's handle/layout before using a synchronized,
zero-timeout counter hint. Equality with the last successfully copied known version skips source
and independent-consumer GPU batches before query/copy/flush admission. Missing, invalid or busy
counter observations proceed to detailed receive; endpoint replacement resets the comparison.
Polling intervals, fixed 60 Hz workloads, queue bounds and qualification budgets are unchanged.

The vendored package's original hashes, licenses and normalized source baseline are retained.
SDK edits invalidate the native static archive even if the shim is unchanged. This repairs the
copy/freshness mechanism; it does not establish the complete GPU critical path, displayed
cadence/output-age cohort or genuine device-removal/retirement bounds. M03 remains **7/9**.

The [evidence packet](evidence/mapgyver-m03-source-freshness-2026-10-29.json) and
[complete raw archive](evidence/mapgyver-m03-source-freshness-controls-2026-10-29.zip) retain the
same short Intel release controls. All 244 execution fingerprints match before/after capture
and the current checkout, including the native sources, headers and provenance manifests.

| Sample observation | Q01 | Q02 |
| --- | --- | --- |
| Main / Spout / editor API acceptance | 180 / 180 / 180 | 180 / 180 / 180 |
| Submitted producer frames A / B | 180 / 180 | 169 / 173 |
| Source-worker GPU batches A / B | 180 / 180 | 169 / 173 |
| Prior output-admission control's source batches A / B | 1,156 / 1,231 | 306 / 482 |
| Independent-consumer GPU batches | 180 | 157 |
| Renderer-context GPU p95 / p99 | 3.809 / 4.378 ms | 9.471 / 13.112 ms |
| Captured source-age p95 A / B | 22.988 / 9.803 ms | 66.250 / 67.934 ms |
| Valid captures per channel / unique A / unique B | 180 / 166 / 175 | 157 / 99 / 112 |

The static-source native regression establishes that unchanged versioned polls submit no GPU
batch, that unknown peers keep copying with a frozen positive counter, and that actual access
timeouts after a copy preserve the previous publication. It also catches a repaired demand
lifecycle bug: a brief 0→1 demand pulse or a suppressed publication could clear the mailbox
without clearing copied-counter history. The worker now re-arms only missing publications with
known copied history, after prior source completion, allowing the static source to publish again.
Same-name smaller replacements reset comparison and pool lifetimes. Counted output names are
validated at the shared 239-byte boundary; unversioned input names retain their 255-byte limit.

Whole-capture copy/publication counts are 373/373 for each Q01 source, and 339/338 and 357/357
for Q02. A successful copy need not publish if demand/quiescence suppresses it. The `unchanged`
poll counters include bridge-capacity skips, which also have their own `bridge_busy` count;
neither counter counts unique producer frames. SDK receive Busy is zero in these controls;
forced named-mutex Busy is covered by the native regression. Keyed-mutex changes are compiled
and source-reviewed, without a new keyed hardware claim.

Every sample opportunity and all 540 output API-end observations per scenario map to their
software slots. All context GPU rows are valid, source-use joins are complete, and all four
roles have exact raw/written/drained counts with zero telemetry loss. Cleanup stops both
runtimes with zero resource bytes/in-flight work, clears endpoints and exits all owned processes
without force. Actual retirement takes 0.100/0.117 s. These work-count observations establish
the correction; unpinned controls do not establish a causal timing improvement.

Qualification remains open. Minimized main/Spout acceptance is 45/46 (Q01) and 28/44 (Q02) of
60 expected opportunities, with observed source-age p95 reaching 279.459/308.800 ms. Q02's
disconnected phase accepts 50/60 main/Spout submissions. Q02 producer shortfalls, sparse output
coverage and GPU outliers remain; its consumer GPU p95/p99 is 21.485/43.043 ms and producer A
has a 254.351 ms maximum context span. Separate context spans are never summed. One-second
interruptions and a single three-second sample cannot replace the prescribed series or close
full GPU-path, displayed deadline/age, plan/queue, growth/drain or recovery acceptance.

Validation passes 177 regular Rust tests, ten explicit native GPU regressions and 32 production
Python tests. Strict default/native all-target Clippy, the existing desktop probe's all-target
check, the SDK's all-feature Windows build check, the locked release x64 build and both required
Rust formatters pass. No display, driver, overlay, queue depth, polling or reference budget changed.

## Native resource reuse and preparation attribution — 2026-10-29

Ownership belongs to Golden Video's device-local backend. A weak index shares compatible
textures/views across prepared, active and retiring generations. Mutable storage is keyed by
role, logical ID and descriptor; writes and copies remain ordered on the same immediate context.
Each generation starts with fresh source-availability/sequence hints. Immutable images share by
exact prepared payload and descriptor, including across logical input IDs. Each uploaded texture
pins its payload lease, so a recycled address or equal app asset ID cannot alias different content.
Expired weak entries are pruned at preparation. The index retains no otherwise unused allocation
or asset; existing conservative per-generation admission remains unchanged.

Schema 3 gives each preparation attempt and renderer frame the same resource-generation,
project/revision, device and reservation identity. Offline analysis joins preparation wall time
to first submission and counts created/reused storage. Missing or mismatched joins cannot pass
coverage. This measures native preparation and first-use waiting, not plan publication/receipt,
per-output queue age or a GPU critical path. Earlier schema-1/2 packets keep their own analyses.

The preceding source-freshness traces show responsive renderer scheduling during interruption
stalls: Q01 has 14 busy slots while two submissions await completion; Q02 has 16, including a
290.715 ms renderer-context GPU span. Their frame submission walls are generally below 3 ms.
Plan changes also allocate complete texture sets. That redundant work motivates reuse, but
these observations do not establish allocation as the cause of the long GPU stall.

The native regression keeps an old generation outstanding while preparing compatible storage,
checks shared identity and pixels, reallocates only changed descriptors/payloads, deduplicates
one payload across two input IDs, and proves completion-based asset release without cache
retention. The Picture app regression now expects retained uploads across channel-plan changes.
Two older source tests now establish their synchronization preconditions: bridge pressure waits
for actual read-lease acquisition, and unchanged preflight runs after the competing unversioned
receiver has fenced its last copy and gone idle. Actual contested receive/Busy checks remain.

Validation passes 177 regular Rust tests, eleven explicit native GPU regressions and 36 production
Python tests. Strict default/native all-target Clippy and both required Rust formatters pass.

The [schema-3 packet](evidence/mapgyver-m03-resource-reuse-2026-10-29.json) and
[complete raw archive](evidence/mapgyver-m03-resource-reuse-controls-2026-10-29.zip) retain one
short Intel release Q01/Q02 pair. Each creates seven textures initially and reuses all seven in
each of seven later preparations: seven creations and 49 reuses, with no failed attempts.
Independent raw recomputation joins all 388/385 renderer submissions to exact preparation
identities; all 247 execution fingerprints match before/after capture and the current checkout.
Two/three prepared generations never submit, remaining explicit rather than receiving an
invented first-use time. The archive contains 36 files (815,395 bytes), SHA-256
`4e64a94776f35dc2d4b11d25b354d72d275fab604d6529fbe182fb0a0326fdda`.

| Observation | Q01 | Q02 |
| --- | --- | --- |
| Initial native preparation | 6.587 ms | 40.588 ms |
| Maximum later native preparation | 0.0109 ms | 0.0106 ms |
| Maximum prepared-to-first-submit wait | 12.818 ms | 22.914 ms |
| Sample main / Spout / editor API acceptance | 180 / 180 / 180 | 155 / 180 / 124 |
| Sample producer A / B API acceptance | 180 / 180 | 171 / 174 |
| Renderer-context GPU p95 / p99 | 4.833 / 5.998 ms | 19.751 / 21.857 ms |
| Captured source-age p95 A / B | 33.612 / 52.810 ms | 73.588 / 84.199 ms |
| Minimized main / Spout acceptance, of 60 | 60 / 60 | 56 / 59 |
| Disconnected main / Spout acceptance, expected 60 | 61 / 61 | 60 / 61 |
| Actual resource retirement | 0.099 s | 0.115 s |

Q01's short interruption observations meet cadence/age checks; Q02's do not. Q02 minimized
source-age p95 reaches 193.446 ms despite zero new texture allocation and preparation below
0.011 ms; reuse has not resolved qualification. Q01 sample source B also exceeds 50 ms at p95.
These unpinned controls cannot establish causality for timing differences from older captures.
All sample GPU rows are valid, markers match canonical producer words, all four roles have
lossless telemetry, and owned cleanup is unforced with zero final resource bytes/in-flight work.
Conservative logical peaks remain 180,633,664/578,764,864 bytes; the new shared allocations do
not lower admission reservations. Full GPU/output age, publication/receipt/queue age, prescribed
cadence, growth/handles and native removal/stuck retirement remain open. M03 stays **7/9**.

## Exact Spout output cohorts and source-retention age — 2026-10-29

Golden Video now carries diagnostic-only copied-image identity through the native source
publication and accepted Spout output. Qualification producers record it with their canonical
GPU marker words; the independent consumer retains it beside each actual capture/decode record.
The SDK's existing public getters supply metadata without changing the SDK or submitting GPU
work. Unknown/unversioned sources claim no exact version. These ephemeral values retain/open
no resource and never enter project files or the control protocol.

The native regression exposed an x64 representation distinction: Spout's sender-info record
stores `uint32_t shareHandle`, writes it through `HandleToLong` and reads it through
`LongToHandle`. Sender/receiver pointers can therefore be zero/sign extended. Diagnostic
identity normalizes only those representations to the SDK's 32-bit field; arbitrary 64-bit NT
handles are rejected. Actual native handles and resource-access paths remain unchanged.

Schema-4 offline analysis defines a cohort of every sample-accepted Spout image. It joins an
image to the renderer batch and that batch's exact retained source versions, then independently
checks both captured GPU markers against the corresponding producer words. The first completed
valid capture supplies one source-age observation per output image. Captures after the sample
window still belong to their original output cohort; repeated captures cannot replace missing
images. Recycled/duplicate handle-counter identities remain ambiguous. Missing identities,
clocks or producer/source joins do not become zero age or complete coverage. Both editor
interruption intervals use the same exact-cohort requirement.

The fixed Q01/Q02 source pair also has receive/publication-to-accepted-output-API age
distributions for each output. Sender API begin/end brackets bound image-copy admission to
independent capture completion. These are CPU QPC observations and bounds, not calibrated GPU
queue time, physical presentation age or a cross-context GPU critical path. Native-window
presentation and plan publication/receipt remain separate measurements. M03 remains **7/9**.

The [evidence packet](evidence/mapgyver-m03-output-cohort-2026-10-29.json) and
[complete raw archive](evidence/mapgyver-m03-output-cohort-controls-2026-10-29.zip) retain one
short release Intel Q01/Q02 pair, with unchanged scenes, queue depth, polling and budgets.
Independent raw reconstruction checks output versions, exact source-use labels, producer stamps
and all twelve captured words. It reproduces every sample/interruption match and mismatch.

| Sample observation | Q01 | Q02 |
| --- | --- | --- |
| Main / Spout / editor API acceptance | 180 / 180 / 180 | 147 / 180 / 135 |
| Accepted Spout versions with a capture | 180 / 180 | 144 / 180 |
| Both source cards match exact renderer labels | 39 / 180 | 6 / 180 |
| Captures with producer/renderer lineage disagreement | 141 | 138 |
| Accepted Spout images without a capture | 0 | 36 |
| Producer A / B API acceptance | 180 / 180 | 156 / 172 |
| Source-publication-to-Spout-API p95 A / B | 18.435 / 7.599 ms | 22.881 / 13.928 ms |
| Renderer-context GPU p95 / p99 | 3.701 / 4.339 ms | 19.788 / 23.902 ms |
| Actual resource retirement | 0.096 s | 0.109 s |

**Dynamic lineage fails despite valid producer marker words.** All 180/144 observed sample
cards decode to legitimate producer records; many differ from the exact source images labeled
by the matched renderer batch. For example, Q01 output batch 127 labels producer frames
231/223, while the capture with that output version contains 229/222. In whole-capture offline
comparison, 168 Q01 mismatches match the preceding renderer prediction. This identifies a
correlation failure; it does not determine whether input copy, output copy or another GPU
handoff first separates the labels from their pixels. No further platform or driver experiment
was run. The next implementation needs a direct native pixel/version regression at that boundary.

Q01's minimized interval has 60 exact matches among 60 accepted Spout images; its disconnected
interval has eight among 53. Q02 has three among 55 and two among 60. Q02 disconnected captures
also include twelve missing-source joins. Main/Spout API counts during minimization are 60/60
and 52/55; disconnected counts are 50/53 and 58/60, against 60 opportunities per phase. Overall
independence remains unqualified, including the required 60-second durations. Full-cohort frame
age cannot pass using the favorable 39/six valid sample subsets. Earlier capture-age distributions
remain producer-to-consumer observations without proven renderer-output lineage.

Both samples have complete CPU source-publication/receive-to-output-API observations for every
accepted call on all three outputs. These metadata ages cannot certify that the labeled pixels
arrived. Sender-API-to-completed-capture bounds likewise describe only successfully joined
subsets. GPU critical-path and presentation/queue age remain unavailable; contexts are not summed.

All 248 execution fingerprints match before/after capture and current source. All four roles
have lossless telemetry and all sample context GPU rows are valid. Cleanup is actual and
unforced, with zero final resource bytes/in-flight work and all owned processes reaped.
The archive contains 36 files (882,024 bytes), SHA-256
`a2aaeec4308da3cd18401cf2e4a7fd8be613bef2b78ad7cb3240368fd07d613f`.
Validation passes 179 regular Rust tests, eleven explicit native GPU tests and 49 production
Python tests, strict default/native all-target Clippy, locked release x64 build and both required
Rust formatters. The native tests establish their finite static/pixel/recovery mechanisms; the
new dynamic lineage failures remain open under gate 9. M03 stays **7/9**, overall **3/10 (30%)**.

## GPU keyed Spout image handoff — 2026-10-29

The [SDK ownership extension](../../vendor/spout2-sys-0.1.1/GOLDEN_PATCH.md) belongs in the pinned
Spout fork; Golden Video selects it for its owned outputs. App shells and the control protocol
own no synchronization policy. The [evidence packet](evidence/mapgyver-m03-keyed-handoff-2026-10-29.json)
and [complete raw archive](evidence/mapgyver-m03-keyed-handoff-controls-2026-10-29.zip) preserve
the failed isolation runs, passing full native matrix and one short release Q01/Q02 pair.
The earlier schema-4 failure packet above remains unchanged.

The new single-source 4K native regression removes windows/WebView and separately checks fenced
SDK copy, concurrent SDK copy, SourcePool/keyed bridge copy and full production VideoRuntime
conversion/Spout output. Direct/bridge cases pass while the original full runtime has 22
marker/version disagreements. Renderer-worker GPU taps then decode the retained input and
converted output before SDK send: both stages have zero mismatches, but one outgoing capture
has preceding pixels. This locates the observed disagreement after conversion at shared-image
handoff. It is not evidence of a broken input bridge, shader or CPU marker decoder.

Owned outputs now opt into the SDK's existing GPU keyed shared-texture allocation before
initialization, including resize. Failure does not downgrade ownership. The existing legacy
32-bit share-handle contract is unchanged; peers need D3D11 keyed sharing support. Detailed
sends acquire CPU metadata then GPU image ownership with zero timeout. A held GPU owner returns
Busy without a shared copy/counter advance; after release the counter advances exactly once.
Counter peek acquires only CPU metadata, so an unchanged hint neither submits GPU work nor
acquires another device's GPU image. The original Boolean SDK send API retains its timeout;
ordinary unversioned/legacy incoming senders remain supported with their own synchronization
contract. No registry, driver, device, display or overlay setting changes are involved.

Renderer completion fences still cover all work submitted before a rejected send. A late Spout
Busy follows conversion and can retain conversion/API intervals; it is separate from the
existing window preflight rejection before conversion. Resource pools, admission reservations,
source polling, queue depth, scenes and budgets are unchanged.

The final full native matrix has **518 available-image comparisons and zero disagreements**:
24 fenced, 120 concurrent SDK, 120 source bridge, 127 untapped full runtime, 125 tapped runtime
and two held-GPU/resume captures. The tapped runtime explicitly excludes one startup capture
without a source. Its input, conversion and outgoing mismatch counts are all zero. Tap stages
are null when not measured. Test-only GPU logs are read once after actual retirement; normal
rendering adds no recurring CPU pixel readback. The untapped runtime case supplements the
instrumented case because taps can perturb timing. The legacy named-sender fixture continues
to exercise static foreign inputs, CPU access contention and freshness behavior.

| Short sample observation | Q01 | Q02 |
| --- | --- | --- |
| Main / Spout / editor API acceptance, expected 180 | 180 / 180 / 180 | 138 / 176 / 100 |
| Exact source-word captures / accepted Spout images | 180 / 180 | 148 / 176 |
| Observed pixel-lineage mismatches | 0 | 0 |
| Accepted images without an exact capture | 0 | 28 |
| Producer A / B API acceptance | 176 / 177 | 176 / 175 |
| Producer A / B Busy attempts | 4 / 4 | 1 / 3 |
| Source-publication-to-Spout-API p95 A / B | 11.352 / 7.441 ms | 16.129 / 6.129 ms |
| Captured source-age p95 A / B | 16.482 / 12.618 ms | 58.565 / 48.107 ms |
| Renderer-context GPU p95 / p99 | 3.839 / 4.438 ms | 19.478 / 23.794 ms |
| Actual resource retirement | 0.095 s | 0.120 s |

Q01's one-second minimized and disconnected phases each capture all 60 accepted Spout images
with exact pixels. Q02's minimized phase captures 46/58 exactly (main acceptance 51); its
disconnected phase captures 36/61 exactly (main acceptance 58), with 16 additional captures
having no exact source join. The latter do not become pixel matches or zero-age observations.
All observed sample images match their exact source-use labels, but Q02's missing images and
source-A age prevent full-cohort qualification. Producer admission also remains insufficient.
GPU keyed sharing repairs the observed coherence failure; these controls do not establish a
timing improvement or pass the unchanged steady-state/interruption budgets.

Independent raw reconstruction joins every output/version/source-use/producer/capture and
checks all twelve GPU words plus first-capture ages in all six cohorts. All 255 execution
fingerprints per capture match before/after and current source. All roles have lossless
telemetry, sample GPU rows are valid, seven textures are created once and reused 49 times, and
cleanup is actual/unforced with zero final resource bytes/in-flight work. The archive contains
42 files (900,156 bytes), SHA-256
`939a8e36658cf2fd0fcce87e45d73227fe328665b8ae20be387a285f6e6614f8`.
It includes the reconstruction/packaging script, raw test stdout/stderr and baseline tool
captures. The first untapped failure also misclassified watchdog-channel disconnection as an
expiry; that harness error was corrected. The tapped baseline has panic stderr interleaved
inside JSON; its raw text is preserved and the packaging script removes only that injected
span to read the report. Neither artifact is silently rewritten as a passing result.

Validation passes 179 regular Rust tests, twelve explicit native GPU tests and 49 production
Python tests, strict default/native all-target Clippy, SDK all-feature/all-target check, legacy
desktop probe all-target check, locked release x64 build and both Rust formatters. The SDK's
37-file baseline and nine patched files match their provenance manifest. No M03 gate closes:
plan publication/receipt, GPU critical path, window/queue/presentation age, complete capture
coverage, prescribed cadence/growth/handle series and genuine removal/stuck retirement remain
required. **M03 remains 7/9; overall remains 3/10 (30%)**.

## CPU plan publication, receipt and first use — 2026-10-29

The production runtime now owns an optional metadata-only plan observer at
`runtime/observation.rs`. Admitted mailbox publication and first receipt are observed under the
existing short control lock; the bounded native recorder is allocated before render-worker
startup, so initial publication is not lost to initialization. First receipt is recorded once
if CPU preparation is retried on a new device. Reliable request state retains the exact authored
identity through terminal supersession/rejection/cancellation, without additional outcome credits
or payload ownership. Failed admission emits no accepted publication.

Core records each actual atomic installation before disposal of an unused previous generation.
Device recreation emits a distinct resource/device installation of the same authored plan,
without fabricating a fresh publication or resetting its age. The diagnostic observer never
submits GPU work, opens resources or invokes UI callbacks. Disabled diagnostics add no recorder
or per-frame lifecycle event. Native frame/resource identities and accepted output API brackets
supply first-use and retained plan-age joins offline.

Schema 5 and `production_plan_age.py` distinguish CPU publication-to-receipt, receipt-to-native
preparation, installation-to-first-submit, publication-to-first-submit and per-output accepted
API age. Unread superseded inputs have no receipt/first-use sample. Missing clocks or exact
joins stay incomplete; duplicates, conflicting terminals, changed resource/device identities
and reversed time order are rejected. A retained plan naturally becomes older between edits;
its age is not edit latency. No latency target is invented. The lifecycle criterion measures
coverage; combined GPU/presentation queue qualification remains unavailable.

The [schema-5 packet](evidence/mapgyver-m03-plan-age-2026-10-29.json) and
[complete raw archive](evidence/mapgyver-m03-plan-age-controls-2026-10-29.zip) retain one short
release Intel Q01/Q02 pair with the existing fixed scenes and budgets. The current analyzer
rejects schemas 1–4; the earlier failed packets and their analyses remain unchanged.

| Whole-capture CPU lifecycle observation | Q01 | Q02 |
| --- | --- | --- |
| Accepted publications / first receipts | 9 / 8 | 9 / 8 |
| Installed generations / superseded inputs | 6 / 3 | 6 / 3 |
| Superseded without receipt or first use | 1 | 1 |
| Native frame rows with exact lifecycle joins | 389 / 389 | 385 / 385 |
| Publication-to-receipt median / maximum | 0.016 / 76.504 ms | 0.016 / 80.916 ms |
| Publication-to-first-submit median / maximum | 11.271 / 82.708 ms | 4.392 / 97.093 ms |

Both captures have complete lifecycle/counter/frame/output joins. Maxima include startup;
with only eight receipts/six first uses, these are finite observations, not prescribed
steady-state edit-latency qualification. Retained-plan-to-Spout-API p95 is approximately
4.084/4.098 seconds during the sample because that same plan remains installed; it does not
mean an edit waited four seconds to appear.

| Short sample observation | Q01 | Q02 |
| --- | --- | --- |
| Main / Spout / editor API acceptance, expected 180 | 180 / 180 / 180 | 158 / 174 / 108 |
| Exact source-word captures / accepted Spout images | 180 / 180 | 153 / 174 |
| Observed pixel-lineage disagreements | 0 | 0 |
| Accepted images without an exact capture | 0 | 21 |
| Producer A / B API acceptance | 175 / 179 | 174 / 180 |
| Source-age p95 A / B, observed captures | 11.842 / 11.239 ms | 56.572 / 46.691 ms |
| Renderer-context GPU p95 / p99 | 4.306 / 4.567 ms | 19.402 / 22.218 ms |
| Actual retirement | 0.094 s | 0.114 s |

Q01's short minimized/disconnected phases capture 60/60 and 61/61 accepted Spout images exactly.
Q02 captures 31/56 and 41/59 exactly; those incomplete cohorts include eleven/three captures
without exact source joins. Its main output accepts 48 calls in each phase. Complete output
coverage, producer admission, Q02 cadence and source age remain insufficient; missing images
cannot be discarded from qualification. These unpinned controls do not establish a causal
performance difference from earlier captures.

Independent raw reconstruction reproduces every lifecycle stage, exact resource/frame join,
handoff/first-use duration and sample per-output API age. All 259 execution fingerprints per
capture match before/after and current files, all roles have lossless telemetry, sample GPU
rows are valid, and cleanup is actual/unforced with zero final resource bytes/in-flight work.
The archive contains 42 files (902,377 bytes), SHA-256
`f624a0705413137dae5a7574a48c66661bf98de1c19e7536b42cecba9b93f30e`.
It includes the independent reconstruction script and raw full-matrix/release-build logs.

Validation passes 182 regular Rust tests, twelve native GPU tests and 60 production Python
tests, strict default/native all-target Clippy, Rust-generated fixture-contract checks, locked
release x64 build and both required formatters. Tests cover receipt-once retry, terminal races,
unread supersession, typed rejection, missing/duplicate/foreign/contradictory observations and
resource recreation without resetting plan age. Full GPU critical path, window presentation/
queue/deadline age, prescribed cadence/growth/handles and genuine native removal/stuck retirement
remain open. **M03 remains 7/9; overall remains 3/10 (30%)**.

## Exact window Presents and software display age — 2026-10-29

Production window outputs now record their actual endpoint incarnation and three DXGI interface
aliases with every native Present API interval. Addresses are captured during cold creation
only when diagnostics are enabled; ordinary rendering adds no interface queries, pixel reads,
GPU waits or presentation policy. The values are non-owning diagnostic identities, absent from
projects/control protocols. Existing completion/endpoint retirement remains unchanged.

Optional collection uses the pinned PresentMon 2.6.0 binary and a unique PID-filtered session.
It starts only after the owned renderer is assigned to its watchdog job. A read-only query
runs after native retirement while the collector drains a fixed tail. Timeout cleanup targets
only this invocation's UUID session and retained collector handle. There is no administrator
restart, installed service change, user-session stop or driver/display/overlay configuration.
The query uses the retained probe's public `trace-status` CLI and historical owned namespace;
no legacy native render workload runs. Both external executable hashes join input identity.

Reproduction, with the existing pinned tools built/available:

```text
python -B tools/mapgyver/production_qualification.py run --directory target/qualification-q01-window-present --scenario Q01 --adapter 0 --diagnostic --samples 1 --warmup 5 --seconds 3 --isolation-seconds 1 --presentmon
```

The [schema-6 packet](evidence/mapgyver-m03-window-present-2026-10-29.json) and
[complete archive](evidence/mapgyver-m03-window-present-controls-2026-10-29.zip) retain one
short release Q01/Q02 pair after the decoder fix, plus the failed initial finalization.
Five-second warmup is explicit for these controls; prescribed qualification remains three
30-second warmups/120-second samples with the required interruption durations and unchanged
budgets. Older schemas retain their own analyses and are rejected by the current analyzer.

All **2,069 rows** (1,194/875) match the native PID, one of the three interface aliases and
exactly one actual API interval. Recycled addresses use the new call/incarnation; duplicates,
overlapping aliases and foreign PIDs are rejected. Unmatched rows remain explicit. Each
sample-accepted window image stays in its cohort, including dropped/missing/display/source/
plan failures. Exact producer/source-use and plan/resource joins extend source/publication/
plan age to the software display observation. Decimal rational QPC coordinates remain lossless
as numerator/denominator strings. Physical scanout, optical latency and independent window
pixel correctness remain outside these metadata observations.

| Short sample observation | Q01 | Q02 |
| --- | --- | --- |
| Main / Spout / editor API acceptance, expected 180 | 180 / 180 / 180 | 137 / 179 / 91 |
| Exact main window display rows / accepted | 180 / 180 | 137 / 137 |
| Exact editor window display rows / accepted | 180 / 180 | 91 / 91 |
| Main source-to-software-display p95 A / B | 32.185 / 28.376 ms | 45.796 / 55.334 ms |
| Editor source-to-software-display p95 A / B | 32.214 / 28.998 ms | 52.630 / 63.386 ms |
| Main Present-to-software-display p95 | 15.417 ms | 27.799 ms |
| Editor Present-to-software-display p95 | 15.200 ms | 33.818 ms |
| Exact Spout captures / accepted | 180 / 180 | 161 / 179 |
| Spout captured-source p95 A / B | 24.083 / 24.375 ms | 57.049 / 57.056 ms |
| Producer A / B API acceptance | 177 / 170 | 180 / 174 |
| Renderer-context GPU p95 / p99 | 5.998 / 6.256 ms | 19.804 / 23.269 ms |
| Actual resource retirement | 0.095 s | 0.124 s |

Both window cohorts have no missing/dropped/display/source/plan observations in these samples.
Q01's complete three-output observed source-age distributions fit 50 ms, while its producer
admission and the prescribed series remain unqualified. Q02 main source B and both editor
sources exceed 50 ms, output cadence fails and eighteen accepted Spout images have no exact
capture. Overall Q02 age stays unavailable rather than selecting a favorable subset.
Unpinned controls with an optional collector do not establish a causal timing improvement.

Known-frame pending-software-display peaks are one/two in Q01/Q02. Pre-window unknown/drop
consumption prevents complete backlog coverage. More fundamentally, frames awaiting a software
display event do not measure app/native buffer occupancy or calibrated GPU queue time. These
counts do not pass the published-but-not-consumed resource budget. Native/output deadlines,
GPU critical path and native buffer/queue qualification retain separate work.

All 262 execution fingerprints and both external tool hashes per capture match before/after
and current inputs. All four native roles have lossless telemetry; live ETW queries succeed
after the native API interval and report zero lost events/log/realtime buffers. Final collector
warnings report no loss. Live counters do not cover final shutdown, and absence of warnings
does not independently certify zero analysis loss. Cleanup is actual/unforced with zero final
resource bytes/in-flight work. Independent reconstruction matches every raw CSV/native API
join, rational display coordinate and source/publication age distribution.

The first native/collector run completed but packet finalization failed because redirected
PresentMon stdout is UTF-16. Its original files and explicit failure are preserved; admission
fingerprints/live-query data were not persisted by that failed runner and remain unavailable.
The corrected strict BOM decoder retains original byte streams, encoding and hashes. The
runner now writes pre-launch input identity and native completion independently of ancillary
trace finalization. One fresh Q01/Q02 pair verifies the fix; no failed artifact is overwritten.

The archive contains 68 files (2,324,759 bytes), SHA-256
`d12a5f49a23e708e8c3396a9d6e5c8445d37789425729fbc6aa1eefcbaf63de4`.
Validation passes 182 regular Rust tests, twelve native GPU tests and 73 Python tests, strict
default/native all-target Clippy, generated fixture contracts, release x64 build and both Rust
formatters. Full cadence/repetition, GPU/queue/deadline, growth/handle, genuine removal/stuck
retirement and M05 physical observations remain open. **M03 stays 7/9; overall stays 3/10 (30%)**.

## Direct input bridge receive — 2026-10-29

`native/inputs/worker.rs` receives directly into its already write-locked free bridge slot.
The SDK checks/copies the foreign sender into that caller-owned destination; the private
capture image and subsequent copy disappear. A successful receive retains the existing flush,
keyed handoff and publication sequence. Busy, unchanged, inactive, mismatch and error paths do
not publish a slot. Source-device completion still precedes reuse, and renderer reader leases
still prevent overwrite. Resize/recovery drains old owners before replacing the three-slot pool.

The public checked `InputLimits::source_storage_bytes` derives four image charges from the
same bridge-slot constant used by allocation and import caches. Backend validation, source-pool
admission and app/test fixtures consume it. Overflow is tested both for minimal storage and
for a descriptor whose single image fits while the whole source reservation overflows.
No producer pacing/retry, source identity, slot count, frame budget or overall ceiling changes.
The derived per-source reservation decreases because the allocation was removed. SourceCpu's
`receive_api` contains the direct copy; `bridge` measures flush/handoff without implying a
second image copy. Raw DTO fields and artifact schema 7 are unchanged.

The [packet](evidence/mapgyver-m03-direct-bridge-2026-10-29.json) and
[new raw archive](evidence/mapgyver-m03-direct-bridge-controls-2026-10-29.zip) reference the
immutable [direct-window baseline](evidence/mapgyver-m03-direct-window-controls-2026-10-29.zip).
Controls retain the same Intel route, five-second warmup/sample, one-second interruptions,
60 Hz pacing, two inputs, queue limits and fixed scenes. There is no PresentMon collector.

| Short native observation | Prior Q02 | Direct-bridge Q02 | Direct-bridge Q01 |
| --- | ---: | ---: | ---: |
| Main / Spout / editor acceptance, expected 300 each | 300/299/271 | 300/300/296 | 300/300/300 |
| Exact accepted Spout captures | 299/299 | 300/300 | 300/300 |
| Renderer context GPU p95 / p99 (ms) | 16.9713 / 18.3391 | 15.5987 / 16.0877 | 3.2120 / 3.7696 |
| Renderer context median (ms) | 13.2066 | 13.4602 | 2.8399 |
| Producer A / B accepted opportunities | 295/292 | 294/293 | 288/296 |
| Spout source-age p95 A / B (ms) | 32.7012 / 37.9231 | 27.1990 / 33.5152 | 17.3411 / 22.8673 |
| Logical reserved peak (bytes) | 505,036,864 | 575,078,464 | 144,691,264 |

Each new GPU cohort contains all 300 CPU-start-selected frames with ordered stage partitions.
The baseline Q02 cohort retains its documented 299-frame boundary case; all cadence/deadline
denominators stay 300. Q01's source A acceptance worsens 299 → 288, and source B age p95 rises
10.7016 → 22.8673 ms. These mixed observations and Q02's higher median/total reservation peak
are retained, without a broad causal performance claim from short sequential controls.

The fixed Q02 baseline reservation decreases by 36,864,000 bytes, from 291,225,664 to 254,361,664.
The recorded total peak instead rises: it equals the new baseline plus three full generation
charges, compared with two in the prior control. Sparse snapshots do not identify the precise
transient roles at that peak. This is logical reservation accounting, not GPU residency or a
leak diagnosis, and the unchanged 1 GiB bound is respected. Repetition/growth, calibrated queues,
native buffer occupancy, full GPU critical path and genuine removal/stuck retirement remain open.

Validation: 149 library plus four public integration tests, all 15 Golden native tests and two
app-native channel/Picture tests, 92 Python tests, strict clippy, final release build and both
Rust formatters pass. The native matrix covers bridge pixels, actual busy access, demand pulses,
resize/reconnect, capacity/retirement, source withdrawal and exact GPU marker lineage. Final
logs follow repairs to one stale admission multiplier and an app-fixture import; no failed
qualification history is overwritten. **M03 stays 7/9; overall stays 3/10.**

## Direct window backbuffer rendering — 2026-10-29

The measured output interval motivates a change at Golden Video's native output boundary.
`pipeline::RenderTarget` borrows a prepared view and its storage/effective shader encoding.
After output admission, windows supply their cold-prepared SRGB backbuffer view; the shader
emits opaque linear RGB over black and the view encodes it once. D3D11 explicitly permits
[SRGB views on UNORM swapchain backbuffers](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/converting-data-color-space).
Spout retains its raw UNORM target and shader encoding, preserving straight and encoded
premultiplied output semantics and its existing SDK storage format.

Window generations allocate no conversion texture or conversion-to-backbuffer copy. The
native baseline still charges two fixed backbuffers, and every source/pass/Spout allocation
remains charged. View creation stays cold; the owned latency handle precedes fallible view
creation. Context ownership is checked before drawing. An admitted endpoint retains its view
through last-use completion and failure/recreation; busy/hidden outputs cannot draw early.
Native pixel tests verify all 16 pixels for six changing colors across actual presentations,
including black, low transfer values and partial coverage, within 2/255. A separate native
test verifies exact logical charges, one actual prepared pass texture and foreign-context
rejection. Existing readiness, recovery and marker-lineage tests also pass.

The [new packet](evidence/mapgyver-m03-direct-window-2026-10-29.json) and
[control archive](evidence/mapgyver-m03-direct-window-controls-2026-10-29.zip) preserve updated
Q01/Q02 controls on the same Intel LUID `00000000:00016c2a`, and reference the immutable
[prior stage archive](evidence/mapgyver-m03-gpu-stages-controls-2026-10-29.zip) for comparison.
All controls use five-second warmup/sample and one-second interruptions; scenes, rational
cadence, queue depth, polling and budgets are unchanged. Artifact schema remains 7 and no
analyzer/protocol change is required. No PresentMon collector is enabled.

| Short native observation | Prior Q02 | Direct Q02 | Direct Q01 |
| --- | ---: | ---: | ---: |
| Main / Spout / editor accepted, expected 300 each | 276/294/165 | 300/299/271 | 300/300/300 |
| Exact completed Spout images / accepted | 272/294 | 299/299 | 300/300 |
| Invalid capture comparisons | 0 | 0 | 0 |
| Renderer context GPU p95 / p99 (ms) | 21.7279 / 24.0701 | 16.9713 / 18.3391 | 3.8944 / 5.3956 |
| Output interval p95 / median (ms) | 15.2477 / 9.5033 | 13.1157 / 9.5441 | 2.8049 / 1.8198 |
| Logical reserved peak (bytes) | 578,764,864 | 505,036,864 | 156,672,064 |
| Initial generation texture allocations | 7 | 5 | 5 |

Q02's CPU-start-selected GPU cohort contains 299 complete spans: one frame begins before the
sample boundary while its output is accepted within the sample. The output/deadline cohorts
retain all 300 opportunities; no missing slot is discarded or cadence denominator reduced.
The independent audit retains that boundary frame and compares exact raw GPU stage joins.
Captured Spout source-age p95 is 32.7012/37.9231 ms for Q02 and 18.1924/10.7016 ms for Q01.
Window display age is not recaptured, so this does not establish three-output age acceptance.
Q01 tails are mixed: its whole-context p99 rises 4.2640 → 5.3956 ms and output p95 rises
2.3158 → 2.8049 ms, despite its lower output median. These observations remain in the evidence.

The logical Q02 reduction is 73,728,000 bytes (70.3125 MiB), including conservative overlapping
generation reservations; it is not a measured physical-residency reduction. Sequential short
controls support the reported observations, not a broad causal/performance qualification.
The unchanged output median, remaining Q02 editor/Spout/producer cadence and GPU cost retain
their failures. GPU spans still include waits and command-feed gaps, without calibrated queue
time or a cross-context critical path. Physical output/DPI and full-app acceptance retain
M05/M09. No prior failure is erased.

Validation on the final source: 148 ordinary library tests plus four public integration tests,
all 15 explicit native tests, 92 production Python tests, strict clippy, final release runner
build and both Rust formatters. Capture execution fingerprints stay stable. The audit checks
the prior archive, every new raw byte/hash, resource charge reduction and GPU partitions.
**M03 remains 7/9; overall remains 3/10.**

## Renderer GPU stage partitions — 2026-10-29

`native/timing/gpu.rs` preallocates two intermediate timestamp queries in each of the two
renderer slots. `native/backend.rs` marks the end of input copies and the end of render
passes. The existing enclosing begin/end pair bounds output conversion/delivery as the
third partition. All four points share the same disjoint observation and frequency, following
the [D3D11 timestamp reliability contract](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ns-d3d11-d3d11_query_data_timestamp_disjoint).
No nested disjoint scope, extra event, flush, allocation, read or wait is added to submission.
Source/fixture contexts retain their original query counts, and disabled diagnostics allocate
no queries. Existing real completion tokens govern collection and reuse.

Typed marker state rejects duplicated or out-of-order boundaries. Incomplete scopes report
no partition; a reused slot cannot carry prior markers. Artifact schema 7 adds the optional
raw `renderer_inputs_end` and `renderer_render_end` fields to the Rust GPU row; the generated
capture metadata and Python consumer change together. `production_gpu_stages.py` joins the
full device/batch/context identity, retains every expected frame, validates ordered integer
boundaries inside the enclosing span and computes distributions for each partition. Missing/
invalid spans remain unavailable. Historical schema-6 archives retain their own analyses.

The [evidence packet](evidence/mapgyver-m03-gpu-stages-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-gpu-stages-controls-2026-10-29.zip) preserve two new
five-second warmup/sample release controls with one-second interruption intervals, on Intel
LUID `00000000:00016c2a`. The independent audit reconstructs every partition from raw GPU
rows, rejects clock/token mismatches, checks exact integer telescoping and agrees with all
600 frame records and their distributions. Capture fingerprints stay stable throughout.

| Same-context observation (ms, p95 unless named) | Q01 | Q02 |
| --- | ---: | ---: |
| Covered / expected sample GPU partitions | 300/300 | 300/300 |
| Input-copy interval | 1.0759 | 7.2783 |
| Render-pass interval | 0.6640 | 7.4998 |
| Output interval | 2.3158 | 15.2477 |
| Output median | 2.1481 | 9.5033 |
| Whole context interval | 3.9429 | 21.7279 |
| Frames with output as largest interval | 299/300 | 271/300 |
| Main / Spout / editor accepted opportunities | 300/300/300 | 276/294/165 |
| Exact completed Spout captures / accepted | 300/300 | 272/294 |

Each frame's three raw tick differences sum exactly to its enclosing difference. Distribution
percentiles cannot be summed. Elapsed GPU intervals include scheduler/dependency waits and
command-feed gaps: this identifies output conversion/delivery as the dominant interval in
this Q02 control, without isolating active GPU work or a specific API within it. The next
performance change should target that output path. Contexts are not aligned or summed into
a full critical path; native buffer occupancy, calibrated queue delay and software/physical
display observations are not provided by these markers. PresentMon was not enabled here.

Validation: 148 regular library tests plus four public integration tests, all 13 explicit
native tests, the two timing hardware tests repeated after the clippy style correction,
92 production Python tests, strict clippy, final release runner build and both Rust formatters.
Raw validation logs retain the initial clippy failure and its successful repair. The full
native matrix precedes only that equivalent conditional-style repair; the focused final
timing check and native controls exercise the final source. Q02 missing captures, cadence,
GPU/queue, growth and genuine removal/stuck retirement remain open. No gate closes, no
denominator changes, and no workload or budget is relaxed. **M03 remains 7/9; overall 3/10.**

## Hardware sRGB source decoding — 2026-10-29

Golden Video's `native/texture.rs` now prepares typeless source-local storage and a decoded
SRGB view for straight/opaque sRGB sources. `native/pipeline.rs` selects that view and marks
its sampled values linear, so the existing per-texel premultiplication/interpolation does not
decode twice. Encoded premultiplied data retains its raw view and shader unpremultiplication
before decoding. `native/backend/resources.rs` selects source allocation during preparation;
output conversion storage and shared Spout formats stay UNORM. Extra views have no extra
texture payload, and creation is outside per-frame submission.

The view/copy contracts follow Microsoft's
[typed resource views](https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm#ResourceViews)
and [compatible-format GPU copies](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copyresource).
Only source-local storage changes; raw marker views still observe encoded bits. Color tests
compare finite outputs to independent scalar transfer/coverage oracles within 2/255. Both
RGBA/BGRA layouts, opaque ignored alpha, transparent colored storage, encoded premultiplication,
nearest/linear filters and unchanged linear transfer are exercised.

The [packet](evidence/mapgyver-m03-srgb-sampling-2026-10-29.json) links raw native captures,
test logs and exact source/binary fingerprints in the
[control archive](evidence/mapgyver-m03-srgb-sampling-controls-2026-10-29.zip).
The same Intel LUID `00000000:00016c2a` runs one pre-change Q02 baseline and updated Q02/Q01
controls, each with five-second warmup/sample and one-second interruption intervals. There
is no display/driver/overlay change or PresentMon collector in this batch.

| Short native observation | Q02 baseline | Q02 decoded view | Q01 decoded view |
| --- | ---: | ---: | ---: |
| Renderer context GPU p95 / p99 (ms) | 19.9860 / 22.4258 | 20.3868 / 21.5081 | 3.8439 / 4.5881 |
| Main accepted / 300 opportunities | 249 | 292 | 300 |
| Spout accepted / 300 opportunities | 294 | 295 | 300 |
| Editor accepted / 300 opportunities | 156 | 174 | 300 |
| Exact completed Spout captures / accepted | 267/294 | 289/295 | 300/300 |
| Invalid capture comparisons | 0 | 0 | 0 |
| Logical reserved peak (bytes) | 578,764,864 | 578,764,864 | recorded in packet |

The isolated sampler cost test uses the production pipeline, identical encoded source texels
and one device, alternating path order. Four warmup pairs precede 12 measured pairs; each
timestamp/disjoint span covers four draws from 1920×1080 to 3840×2160. It observes median
2.0702 ms per manual-decode draw and 1.3827 ms per hardware-decode draw, a 33% reduction for
this operation. Valid completed timing records and zero diagnostic drops are asserted; no
performance ratio is a flaky test assertion. This is an isolated operation cost, not a whole
frame or cross-context critical path. The end-to-end Q02 GPU p95 does not improve, and the
sequential short controls do not prove the cause of their cadence/capture differences.

Reproduce the bounded cost experiment:

```powershell
cargo test -p golden_video --features qualification --lib hardware_srgb_source_decode_cost_control -- --ignored --test-threads=1 --nocapture
```

Validation: 148 ordinary library tests, four public integration tests, the 11-test native
regression matrix and the new isolated native cost test pass. Strict Golden Video clippy,
the release `native_qualification` build and both Rust formatters complete. The native
capture execution hashes remain stable within every run; only cost-test source was added
after the captures, without changing runtime code or the executable. Original input records
are retained. The prior 85 Python test baseline is unchanged; no new analyzer is introduced.
Q02 cadence, incomplete capture and age/queue/GPU/recovery qualification stay open; missing
images are not omitted. No displayed cadence, optical scanout or deadline pass is inferred
from these API observations. **M03 remains 7/9; overall remains 3/10.**

## Anchored delivery deadline bounds — 2026-10-29

Deadline measurement belongs in the app-owned qualification tools. `production_deadlines.py`
provides the checked absolute slot/cadence/device cohort, explicit skip/unconsumed ranges and
final coverage fence. `production_presentmon.py` exposes exact API/display observations over
the whole capture; `production_cohort.py` retains the first valid completed GPU-capture upper
bound for an exact Spout image. `production_delivery_deadlines.py` joins those witnesses to
each expected slot/output and classifies OnTime, Miss or Unknown. Native schema 6 already
contains every required input; no native protocol, runtime, shader or capture policy changes.

Deadline membership uses the anchored next-slot boundary in `[sample begin, sample end)`.
The first deadline can belong to a native API call before sample begin. Restricting the join
to API calls inside the sample would incorrectly lose that frame. Full-capture witnesses are
joined by exact device/batch/output/endpoint/image identities; repeats and foreign same-serial
records cannot replace missing frames. Every expected opportunity remains in the denominator.

The classification distinguishes direct observations from bounds:

- A reported window software display at/before its deadline proves that software endpoint on
  time. A later event or reported dropped frame establishes an observed software-display miss.
- An exact completed independent Spout capture observed by the deadline proves the published
  image was GPU-ready by that time. A later completion poll provides only an upper bound;
  readiness could have occurred earlier, so it remains Unknown.
- A publication API beginning after the deadline proves it could not publish that frame in
  time. Known unsubmitted/nonaccepted opportunities supply separate failure bounds. A missing
  schedule/output/witness stays Unknown. Contradictory nonaccepted/display observations do not
  become a passing frame.
- CPU API return can be preempted after successful delivery. A late return never overrides an
  earlier valid display/readiness witness. Partial/untrusted telemetry cannot certify a pass.

Each output reports lower/upper miss fractions, accounting for all Unknowns in the upper
bound, plus its exact classification records. A failure is proven when the known miss lower
bound exceeds 1/1000. A pass requires complete classification/telemetry within that budget;
other cases remain unavailable. Integer/rational comparison preserves the inclusive boundary.
These observations do not certify physical scanout, optical latency, native buffer occupancy
or a calibrated GPU critical path.

The [analysis packet](evidence/mapgyver-m03-delivery-deadlines-2026-10-29.json) and
[derived archive](evidence/mapgyver-m03-delivery-deadlines-analysis-2026-10-29.zip) use the
unchanged [existing raw archive](evidence/mapgyver-m03-window-present-controls-2026-10-29.zip),
SHA-256 `d12a5f49a23e708e8c3396a9d6e5c8445d37789425729fbc6aa1eefcbaf63de4`.
All 68 original member bytes and original analyses remain intact. Only four captured analysis
source files differ; the new analyzer/helper hashes are recorded separately. The other 258
captured execution hashes and native executable still match. No new native/collector run,
device/driver/display/overlay change or workload/budget adjustment occurred.

| Anchored delivery classification, expected 180 per output | Q01 on time / miss / unknown | Q02 on time / miss / unknown |
| --- | --- | --- |
| Main window software display | 180 / 0 / 0 | 0 / 180 / 0 |
| Spout GPU-ready bounds | 180 / 0 / 0 | 3 / 1 / 176 |
| Editor window software display | 180 / 0 / 0 | 0 / 180 / 0 |

Q01 proves its observed three-output deadlines in this short control. Q02 main combines 137
late displays and 43 nonaccepted frames; editor combines 91 late displays and 89 nonaccepted
frames. Main/editor observed lateness p95 is 11.355/18.397 ms beyond their slot deadlines.
Q02 Spout has 158 late completion upper bounds and eighteen missing exact captures, all
Unknown. Its one known nonaccepted frame is already 0.556%, exceeding 0.1%. The total miss
fraction is bounded by 0.556%–98.333%; it is not asserted to be the upper value. No late GPU
miss count is inferred by subtracting a polling delay or guessing a ready timestamp.

Independent raw reconstruction recomputes nested-ceil slot boundaries, native API/CSV
matches, rational display time and exact producer/source-use/capture words. It agrees for all
1,080 slot/output classifications, including the pre-window call. Source-age/cadence and all
earlier capture/qualification failures remain preserved. Three-second diagnostic evidence
does not qualify the required repeated 120-second samples or 60-second interruptions.

The five-member derived archive is 190,528 bytes, SHA-256
`930548d805d6ae35e92d255ccb079095af5a3b1cbac83220da44af081e8b9824`.
It includes full new analyses, independent reconstruction source/results and the test log.
Validation passes 85 production Python tests and both Rust formatters. Tests cover exact
budget inclusivity, polling/preemption bounds, anchored boundary membership, rejected and
missing frames, trust/clock faults, endpoint/device identity and duplicate witnesses. Native
implementation is unchanged; the prior 182 regular/twelve native test baseline is retained,
without a new native-test or performance claim. GPU/queue, growth/handle, genuine removal/stuck
retirement and full prescribed qualification remain open. **M03 stays 7/9; overall stays 30%**.
