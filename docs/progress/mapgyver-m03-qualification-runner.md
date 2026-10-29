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

`D3d11Config::diagnostics` defaults to disabled. Enabling it preallocates a fixed metadata ring
and two timestamp/disjoint query slots per context. Producers never wait for the diagnostics
lock or query availability. Full/contention drops, clock failures, invalid/disjoint/skipped
queries and uncollected rows remain explicit. Draining and JSON/OS recording happen on an outer
host worker, not the engine or UI thread. Timing changes no rendering admission policy.

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
The next bounded batch is telemetry loss and exact deadline/queue anchors, followed by measured
renderer corrections. Complete GPU-path/age coverage, renderer resource-return series and native
device removal/recovery remain open; physical and full-app qualification keep their M05/M09 owners.
