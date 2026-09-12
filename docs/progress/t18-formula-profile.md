# T18 formula/lane profiling

## Current evidence

On Windows x64 at committed source `8c3e5548`, four serial optimized-test runs of
`multiplex_sample_active_runtime_stays_realtime` used the real
`apps/chataigne/test-samples/test_multiplex.noisette` project. Each run measured 240 dirty ticks,
eight processors, 127 lanes per processor, 1,920 processor evaluations, and 243,840 evaluated
lanes. The sample has output command batching and no debug-value capture. Default app features
were `asio,jack,realtime`; `GC_SKIP_UI_BUILD=1` avoided unrelated UI asset rebuilding.

| Run | Tick average | Tick p95 | Tick p99 | Input preparation total | Processor evaluation total | Evaluation share |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 4,890 µs | 7,107 µs | 7,758 µs | 280.9 ms | 397.1 ms | 33.8% |
| 2 | 4,883 µs | 6,962 µs | 7,607 µs | 283.6 ms | 394.9 ms | 33.7% |
| 3 | 4,716 µs | 6,774 µs | 7,483 µs | 273.3 ms | 377.7 ms | 33.4% |
| 4 | 5,103 µs | 8,665 µs | 10,087 µs | 300.3 ms | 401.3 ms | 32.8% |

Runs 1–3 passed with no 10 ms deadline misses. Run 4 had three misses (maximum 10,737 µs)
and failed the real-time assertion. The deadline result is therefore sensitive to run-to-run
conditions on this host; a single passing run is not sufficient qualification. Input preparation times only
`processor_runtime_inputs`; processor evaluation times
`ProcessorRuntime::evaluate_processor_with_context_provider_and_runtime_delta_capture` or its
full-capture counterpart. The latter includes lane enumeration, frame construction, compiled
graph evaluation, and runtime-output assembly, but excludes downstream output dispatch. Timers
and counters exist only in the app's unit-test build, so production hot paths are unchanged.

This evaluation phase is an *upper bound* on work eligible for parallel pure-graph kernels. Even
if the entire phase scaled perfectly to eight workers with zero dispatch cost, the measured
one-thousand-lane sample could improve by at most about 1.4× by Amdahl's law. The later
opt-in probe below narrowed the compiled-kernel share. Input preparation alone took about 23–25%
of measured tick time and is not covered by a parallel graph kernel.

Reproduce one run with:

```powershell
$env:GC_SKIP_UI_BUILD='1'
./tools/asio.ps1 -- cargo test --locked -p Chataigne2 --bin Chataigne2 --target-dir target/t16-app-default multiplex_sample_active_runtime_stays_realtime -- --nocapture --test-threads=1
```

## Decision boundary

At `dc3ff6d1`, an opt-in `kernel-profiling` feature timed the app-owned calls to
`evaluate_compiled_graph` and `evaluate_compiled_graph_fresh_reusing` after constructing each
lane's property/context frame and acquiring its memory or scratch. Each of four targeted runs
evaluated the same 243,840 compiled lanes. These runs execute one test at a time but omit
`--test-threads=1`, because that argument enables the uninstrumented 5 ms development-budget
assertions; the per-lane timing probe itself perturbs the end-to-end time. The functional
assertions still ran and passed. The app's full 513-test suite and strict Clippy passed with the
feature enabled.

| Run | Compiled-graph total | Kernel share of tick | Kernel share of evaluation phase | Tick average | 10 ms misses |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 320.0 ms | 24.6% | 74.5% | 5,409 µs | 0 |
| 2 | 318.6 ms | 24.6% | 74.7% | 5,406 µs | 2 |
| 3 | 299.8 ms | 25.0% | 73.6% | 4,986 µs | 0 |
| 4 | 303.6 ms | 24.7% | 73.5% | 5,124 µs | 1 |

The probe is thread-local and excludes processor input preparation, lane enumeration, property
resolution, and output assembly. It does include function-call overhead inside the timed calls.
It is opt-in and absent from ordinary artifacts. These profiled tick/deadline figures are not
real-time qualification and cannot be directly compared with the earlier uninstrumented source
revision. Even an ideal eight-worker execution of *all* measured compiled-graph work with zero
overhead could improve this sample by at most about 1.28×; actual benefit would be lower.

Reproduce the kernel profile with:

```powershell
$env:GC_SKIP_UI_BUILD='1'
./tools/asio.ps1 -- cargo test --locked -p Chataigne2 --bin Chataigne2 --features kernel-profiling --target-dir target/t16-app-default multiplex_sample_active_runtime_stays_realtime -- --nocapture
```

### Stateful 100k-lane partition pilot

At `1af6e8f1`, two ignored, manually invoked app tests load the persisted
`test_multiplex.noisette` sample (SHA-256
`5EBD05F9390462B5D666C6B54E833BF30F97D6AD2B391146C288861202F09390`). A test-only
manager hook captures one real dirty-tick input snapshot from all eight processors. Each test
shares the sample's compiled three-node stateful `Action` Formula across new processors, uses
one synthetic `scale_lane` axis, clears the copied processor condition to isolate Formula work,
and forces dense evaluation. These are direct processor evaluations, not complete engine ticks;
they do not include context-provider construction, output dispatch, UI publication, or transport.

Each invocation builds 100,000 lanes, runs one warmup and three measured ticks, verifies exactly
300,000 measured compiled calls and 100,000 retained lane memories, and reports process RSS.
Four serial invocations per shape passed with zero runtime diagnostics. The median is the middle
of just three warmed ticks per invocation, so this is a partition/phase pilot, not a p95/p99
latency qualification.

| Shape | Run | Warmed tick median | Three-tick range | Kernel total / three ticks | RSS after evaluation |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1,000×100 | 1 | 154.9 ms | 149.6–159.9 ms | 359 ms | 183 MB |
| 1,000×100 | 2 | 148.4 ms | 144.4–152.0 ms | 344 ms | 183 MB |
| 1,000×100 | 3 | 145.3 ms | 145.1–145.4 ms | 337 ms | 182 MB |
| 1,000×100 | 4 | 147.1 ms | 146.3–152.1 ms | 342 ms | 184 MB |
| 10,000×10 | 1 | 155.6 ms | 155.4–156.9 ms | 337 ms | 222 MB |
| 10,000×10 | 2 | 163.7 ms | 156.8–174.1 ms | 359 ms | 222 MB |
| 10,000×10 | 3 | 157.3 ms | 156.7–158.9 ms | 341 ms | 222 MB |
| 10,000×10 | 4 | 159.4 ms | 159.1–159.6 ms | 346 ms | 223 MB |

The higher processor-count partition allocates about 22–24 MB more before evaluation and
retains about 39–40 MB more afterward. Kernel calls account for roughly 72–77% of these direct
evaluation timings, versus about 25% of full tick time in the 1,016-lane sample. None of the
100k-lane serial passes approaches a 10 ms tick budget; even ideal eight-way acceleration of
the measured kernel alone would not reach it. The test holds no device or engine callback on a
worker thread, and no production parallel path has been added.

Reproduce each shape with `--features kernel-profiling` and the test name
`multiplex_formula_scale_1000_by_100` or `multiplex_formula_scale_10000_by_10`, adding
`-- --ignored --nocapture` to the app test command above.

### Test-only worker comparison

At `2e345617`, a separate ignored app test reuses the same captured Formula/input fixture and
builds fresh 100k-lane stateful processors for each of 1, 2, 4, and 8 scoped workers. It stages
worker results in processor-chunk order and checks every tick's context sequence, complete
ordered intent sequence, and diagnostics against the serial reference. After four ticks, the
entire retained lane-memory pool (values, states, initialization, revisions, and dirty bookkeeping)
must also equal the serial reference. All eight source-pinned invocations (four per shape) passed.
Each worker count has one warmup and three timed ticks; the table gives the range of the four
per-invocation medians, not a p95 or full-product latency statistic.

| Shape | Workers | Warmed direct-evaluation median range | Process CPU-ms per timed tick, observed range |
| --- | ---: | ---: | ---: |
| 1,000×100 | 1 | 153–158 ms | 125–172 ms |
| 1,000×100 | 2 | 94–101 ms | 172–204 ms |
| 1,000×100 | 4 | 58–68 ms | 187–266 ms |
| 1,000×100 | 8 | 36–45 ms | 250–343 ms |
| 10,000×10 | 1 | 165–170 ms | 156–188 ms |
| 10,000×10 | 2 | 103–110 ms | 171–234 ms |
| 10,000×10 | 4 | 67–74 ms | 219–266 ms |
| 10,000×10 | 8 | 40–46 ms | 265–391 ms |

The observed eight-worker median improvement is about 3.4–4.3× against this test's own serial
path. The process CPU readings are coarse accumulated CPU-millisecond differences; summed
per-thread kernel wall time is **not** CPU usage. Scoped workers are created for each tick, so
their spawn/join and ordered merge are inside the timing. The test retains a cloned serial
lane-memory reference while running later worker counts, so later RSS values (roughly 381–388 MB
for 1,000×100 and 445–446 MB for 10,000×10 at eight workers) are not independent worker-memory
comparisons. Separate-process serial RSS is in the pilot table above. No worker count reaches a
10 ms direct-evaluation budget at 100k lanes.

At `258695d5`, two more ignored worker tests rotated each processor's stable context keys after
tick two. One source-pinned invocation of each partition passed: every 1/2/4/8-worker tick still
matched the serial context sequence, ordered effects, diagnostics, and complete final lane
memory. The reorder actually changed the emitted context order and remained stable on tick four.
The warmed direct-evaluation medians were 152/105/74/40 ms (1,000×100) and 164/102/61/37 ms
(10,000×10) for 1/2/4/8 workers. These are single-invocation observations, not new latency
percentiles. All lanes use the same captured Formula inputs, so this does **not** yet prove that
lane-distinct state values follow their identities across a reorder.

At `2ae31055`, a separate focused processor regression binds a Boolean Formula property to two
context keys. The keys initialize with different values and distinct retained state, then their
order reverses; neither key replays its trigger edge. This proves key-bound state identity for
that small processor case, while the 100k-lane persisted `Action` Formula comparison still uses
identical captured inputs in every lane.

Reproduce with the same app command and `multiplex_formula_workers_1000_by_100`,
`multiplex_formula_workers_10000_by_10`, or their `multiplex_formula_workers_reordered_...`
variants, plus `-- --ignored --nocapture`. The full feature-enabled app suite passes 513 active
tests with eight scale/worker/unchanged-input tests ignored by default when run without an
explicit serial test-thread flag; the 150 Alchemist tests and strict app Clippy in both feature
modes also pass.
An explicit `--test-threads=1` enables strict 5/6 ms real-time assertions in the app tests. Two
full-suite runs with this instrumented feature missed one or both average budgets (5.8–6.0 ms),
while the two affected tests passed in isolation at 4.8 and 5.0 ms. The opt-in kernel timing
perturbs tick cost, and these mixed outcomes are not release real-time qualification.

### Unchanged-input requested evaluation

At `37b6f3b7`, two more ignored app tests call the normal
`evaluate_processor_with_context_provider_and_runtime_delta_capture` path with the same captured
real-Formula input snapshot on all four ticks. Every test deliberately schedules all 100,000
lanes even though the inputs do not change after initialization. The first tick emits 200,000
intents; each of the next three emits none. All 100,000 keyed state memories remain retained,
and the compiled-graph entrypoint is still called 300,000 times across the warmed ticks.

Four source-pinned invocations per shape passed with no diagnostics. The median of three warmed
direct-evaluation ticks ranged 94.9–96.2 ms for 1,000×100 and 104.3–106.9 ms for 10,000×10.
Summed per-thread graph-call wall time ranged 189–197 ms over three warmed ticks. Process CPU
differences were roughly 93–125 ms per tick at the host's coarse millisecond resolution. The
two tests share a process, so their observed 166–167 MB and 204 MB RSS values are not independent
partition-memory measurements.

This is an intentionally requested-evaluation cost, not a truly idle product tick: the manager
normally avoids calling clean processors. It also is not a sparse-dirty crossover or a measure
of how many authored graph nodes executed inside each graph call. It does show that output
suppression alone does not make a scheduled 100k-lane pass approach 10 ms. Reproduce with the
feature-enabled app command, the `multiplex_formula_unchanged_` filter, and
`-- --ignored --nocapture --test-threads=1`.

The source-fingerprinted [T19 direct Formula qualification report](audit-remediation-status.md#t19-direct-formula-qualification-report)
now runs all eight manual scale cases together and rejects missing partitions or worker modes.
Its PASS status is scoped to direct processor evaluation, not complete product capacity.

The 1,016-lane full-tick sample still does not justify a production worker pool by itself.
The 100k-lane test shows a useful isolated compute speedup, but not a real-time capacity claim or
a safe production commit boundary. Sparse-dirty crossover, state-machine transitions, conflicting
effect targets, triggering, lane-distinct state in the 100k-lane product Formula, generation
replacement, cancellation without partial state commit, persistent-worker costs, and full
end-to-end tick evidence remain open. Only a solution satisfying those boundaries should replace
or augment the serial production path.
