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

The 1,016-lane sample does not justify a production worker pool by itself. The 100k-lane pilot
does justify measuring worker crossover, but it does not establish a real-time capacity claim.
Before changing production execution, compare one, two, four, and eight workers with ordered
effects, stateful and sparse-dirty equivalence, generation replacement, cancellation, CPU,
retained memory, and end-to-end tick evidence. If useful improvement does not survive those
checks, retain the serial path and record parallel compute as deferred.
