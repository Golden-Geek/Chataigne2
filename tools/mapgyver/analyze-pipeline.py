"""Join exact bridge dependencies; never compare timestamps from different D3D11 clocks."""

import argparse
import json
import math
from pathlib import Path
import runpy

CAPACITY = 216_001
GPU_COLUMNS = ["frame", "submitted_qpc", "observed_completed_qpc", "start_ticks", "end_ticks", "frequency", "collected"]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def distribution(values):
    if not values:
        return None
    ordered = sorted(values)
    count = len(ordered)
    return dict(samples=count, mean_ms=sum(ordered) / count, median_ms=ordered[(count + 1) // 2 - 1],
                p95_ms=ordered[(count * 95 + 99) // 100 - 1], p99_ms=ordered[(count * 99 + 99) // 100 - 1],
                maximum_ms=ordered[-1])


def verify_trace(timing, admission):
    trace = timing.get("gpu_trace")
    require(trace is not None and trace["columns"] == GPU_COLUMNS, "raw context GPU trace missing or changed")
    rows = trace["rows"]
    require(len(rows) <= CAPACITY, "unbounded GPU trace")
    total = admission["submitted"]
    require(all(isinstance(trace[k], int) and trace[k] >= 0 for k in ("skipped", "invalid", "pending", "truncated")),
            "invalid trace counters")
    require(len(rows) + sum(trace[k] for k in ("skipped", "invalid", "pending", "truncated")) == total,
            "raw GPU batch coverage mismatch")
    result = {}
    sampled = []
    for row in rows:
        require(len(row) == len(GPU_COLUMNS), "GPU row width")
        frame, submitted, observed, start, end, frequency, collected = row
        require(all(type(value) is int and value >= 0 for value in row[:6]) and type(collected) is bool,
                "GPU row types")
        require(frame not in result and submitted > 0 and observed >= submitted and end >= start and frequency > 0,
                "GPU identity, clock or completion invalid")
        ms = (end - start) * 1000.0 / frequency
        result[frame] = dict(submitted_qpc=submitted, observed_completed_qpc=observed, ms=ms, collected=collected)
        if collected:
            sampled.append(ms)
    if trace["truncated"] == 0:
        actual, reported = distribution(sampled), timing["gpu_execution"]
        require((actual is None) == (reported is None), "raw GPU distribution missing")
        if actual:
            require(all(math.isclose(actual[k], reported[k], rel_tol=1e-12, abs_tol=1e-12) for k in actual),
                    "raw GPU distribution mismatch")
        require(timing["truncated_samples"] == 0 and len(sampled) + sum(timing[k] for k in (
            "skipped_gpu_queries", "invalid_gpu_queries", "pending_gpu_queries")) == admission["sampled"]["admitted"],
            "sampled GPU coverage mismatch")
    return result


def analyze_generation(generation):
    workers = generation["source_workers"]
    require(len(workers) == 2, "source pair missing")
    render = verify_trace(generation["timing"], generation["gpu_admission"])
    sources = [verify_trace(worker["timing"], worker["gpu_admission"]) for worker in workers]
    for worker, source in zip(workers, sources, strict=True):
        require(all(frame < worker["receive_calls"] for frame in source), "source batch outside receive lifetime")
    trace = generation["dependency_trace"]
    require(trace["columns"] == ["renderer_frame", "source_a_batch", "source_b_batch"], "dependency columns changed")
    require(trace["qpc_frequency"] > 0 and trace["truncated"] >= 0 and len(trace["rows"]) <= CAPACITY,
            "dependency clock or bound")
    require(len(trace["rows"]) + trace["truncated"] == generation["gpu_admission"]["sampled"]["admitted"],
            "dependency coverage mismatch")
    sums, bounds = [], []
    missing_render, missing_source, absent = 0, [0, 0], [0, 0]
    repeated, prior, previous = [0, 0], [None, None], -1
    last_seen = [None, None]
    warmup_dependencies = [0, 0]
    for row in trace["rows"]:
        require(len(row) == 3 and type(row[0]) is int and row[0] > previous, "dependency frame order")
        frame, *batches = row
        previous = frame
        r = render.get(frame)
        if r is None:
            missing_render += 1
        else:
            require(r["collected"], "sampled dependency references warmup renderer")
        spans = []
        complete = r is not None
        for channel, batch in enumerate(batches):
            if batch is None:
                absent[channel] += 1
                prior[channel] = None
                continue
            require(type(batch) is int and 0 <= batch < workers[channel]["receive_calls"], "unknown source batch")
            require(last_seen[channel] is None or batch >= last_seen[channel], "source publication regressed")
            last_seen[channel] = batch
            repeated[channel] += int(prior[channel] == batch)
            prior[channel] = batch
            s = sources[channel].get(batch)
            if s is None:
                missing_source[channel] += 1
                complete = False
            else:
                warmup_dependencies[channel] += int(not s["collected"])
                if r is not None:
                    require(s["submitted_qpc"] <= r["observed_completed_qpc"], "future source publication")
                spans.append(s)
        if complete:
            # A conservative serial charge of correlated batch spans, not summed marginal percentiles.
            sums.append(r["ms"] + sum(s["ms"] for s in spans))
            earliest = min([r["submitted_qpc"], *(s["submitted_qpc"] for s in spans)])
            bounds.append((r["observed_completed_qpc"] - earliest) * 1000.0 / trace["qpc_frequency"])
    total = generation["gpu_admission"]["sampled"]["admitted"]
    complete = len(bounds) == total and trace["truncated"] == 0
    upper = distribution(bounds)
    sufficient = complete and upper is not None and upper["p95_ms"] <= 12 and upper["p99_ms"] <= 16.667
    return dict(sampled_renderer_batches=total, matched_batches=len(bounds), complete_dependency_coverage=complete,
                missing_renderer_spans=missing_render, missing_source_spans=missing_source,
                absent_source_references=absent, repeated_source_references=repeated,
                warmup_source_references=warmup_dependencies,
                correlated_serial_gpu_span_sum=distribution(sums), cpu_enclosed_pipeline_upper_bound=upper,
                upper_bound_sufficient_for_gpu_target=bool(sufficient),
                scope="app ingest to renderer completion; CPU envelope includes polling, queueing and retained-frame age",
                interpretation="GPU span sums exclude gaps and are not the pipeline critical path. An upper bound above target is inconclusive, not proof of GPU-budget failure.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    args = parser.parse_args()
    artifact = json.loads(args.artifact.read_text(encoding="utf-8"))
    runner = runpy.run_path(str(Path(__file__).with_name("run-qualification.py")))
    packets = artifact.get("packets", [artifact])
    evidence = [runner["decode_evidence"](packet) for packet in packets]
    require(all(e["schema_version"] >= 13 and e["inputs_unchanged_after_run"]
                and e["run"]["exit_code"] == 0 for e in evidence), "incomplete or stale native evidence")
    print(json.dumps([[analyze_generation(g) for g in e["run"]["report"]["generations"]]
                      for e in evidence], indent=2))


if __name__ == "__main__":
    main()
