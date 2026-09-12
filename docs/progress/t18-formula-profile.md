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
one-thousand-lane sample could improve by at most about 1.4× by Amdahl's law. The actual pure
kernel share is lower and has not yet been isolated. Input preparation alone took about 23–25%
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

No parallel production evaluator is justified by this sample alone. Before changing execution,
measure the pure compiled-graph portion and end-to-end ticks on both 1k×100 and 10k×10 real
formula/lane partitions, including stateful and sparse-dirty variants. Compare one, two, four,
and eight workers with deterministic effect order, generation replacement, cancellation, CPU,
retained memory, and a serial crossover. If the improvement does not survive those checks,
retain the serial path and record parallel compute as deferred.
