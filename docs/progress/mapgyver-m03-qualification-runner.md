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
pass the normative output deadline or scanout criterion.

CPU rows contain inclusive submission wall intervals, exact SDK/Present API brackets and
optional OS-accounted thread CPU deltas. API wall time can include CPU work and waiting;
subtracting it does not measure active CPU submission. Thread accounting also has OS-defined
granularity. Context-local GPU timestamp spans are reported separately. They do not establish
the full multi-context critical path, and their percentiles must not be added together.

Retained source metadata identifies the source-worker publication actually copied into each
renderer texture, including reuse. The independent consumer reports producer-to-observed-copy
completion age as an upper bound, not display age or scanout. It does not yet identify an exact
renderer output token. Repeated or skipped consumer captures also do not establish a complete
output-frame age cohort. Both normative frame-age and GPU dependency-path qualification remain
unavailable; the observed capture-age distribution stays separate.

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
Current captures use artifact schema 2. The original schema-1 archive below retains its own
analysis and hashes; the current analyzer rejects schema 1 rather than reinterpreting old rows.

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
